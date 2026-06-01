use super::*;

pub(super) fn compile_event_raw(
    ctx: &mut CompileCtx<'_>,
    event: &EventRaw,
) -> VnResult<EventCompiled> {
    Ok(match event {
        EventRaw::Dialogue(dialogue) => EventCompiled::Dialogue(DialogueCompiled {
            speaker: ctx.intern(&dialogue.speaker),
            text: ctx.intern(&dialogue.text),
        }),
        EventRaw::Choice(choice) => {
            let options = choice
                .options
                .iter()
                .map(|option| {
                    let target_ip = ctx.resolve_target("choice", &option.target)?;
                    Ok(ChoiceOptionCompiled {
                        text: ctx.intern(&option.text),
                        target_ip,
                    })
                })
                .collect::<VnResult<Vec<_>>>()?;
            EventCompiled::Choice(ChoiceCompiled {
                prompt: ctx.intern(&choice.prompt),
                options,
            })
        }
        EventRaw::Scene(scene) => EventCompiled::Scene(SceneUpdateCompiled {
            background: scene.background.as_deref().map(|value| ctx.intern(value)),
            music: scene.music.as_deref().map(|value| ctx.intern(value)),
            characters: scene
                .characters
                .iter()
                .map(|character| CharacterPlacementCompiled {
                    name: ctx.intern(&character.name),
                    expression: character
                        .expression
                        .as_deref()
                        .map(|value| ctx.intern(value)),
                    position: character.position.as_deref().map(|value| ctx.intern(value)),
                    x: character.x,
                    y: character.y,
                    scale: character.scale,
                })
                .collect(),
        }),
        EventRaw::Jump { target } => EventCompiled::Jump {
            target_ip: ctx.resolve_target("jump", target)?,
        },
        EventRaw::SetFlag { key, value } => EventCompiled::SetFlag {
            flag_id: ctx.flag_id(key)?,
            value: *value,
        },
        EventRaw::SetVar { key, value } => EventCompiled::SetVar {
            var_id: ctx.var_id(key)?,
            value: *value,
        },
        EventRaw::JumpIf { cond, target } => EventCompiled::JumpIf {
            cond: ctx.compile_cond(cond)?,
            target_ip: ctx.resolve_target("jump_if", target)?,
        },
        EventRaw::Patch(patch) => EventCompiled::Patch(ScenePatchCompiled {
            background: patch.background.as_deref().map(|value| ctx.intern(value)),
            music: patch.music.as_deref().map(|value| ctx.intern(value)),
            add: patch
                .add
                .iter()
                .map(|character| CharacterPlacementCompiled {
                    name: ctx.intern(&character.name),
                    expression: character
                        .expression
                        .as_deref()
                        .map(|value| ctx.intern(value)),
                    position: character.position.as_deref().map(|value| ctx.intern(value)),
                    x: character.x,
                    y: character.y,
                    scale: character.scale,
                })
                .collect(),
            update: patch
                .update
                .iter()
                .map(|character| CharacterPatchCompiled {
                    name: ctx.intern(&character.name),
                    expression: character
                        .expression
                        .as_deref()
                        .map(|value| ctx.intern(value)),
                    position: character.position.as_deref().map(|value| ctx.intern(value)),
                    x: character.x,
                    y: character.y,
                    scale: character.scale,
                })
                .collect(),
            remove: patch.remove.iter().map(|name| ctx.intern(name)).collect(),
        }),
        EventRaw::ExtCall { command, args } => EventCompiled::ExtCall {
            command: command.clone(),
            args: args.clone(),
        },
        EventRaw::AudioAction(action) => {
            let channel = compile_audio_channel(&action.channel)?;
            let action_kind = compile_audio_action(&action.action)?;
            if action_kind == 0
                && action
                    .asset
                    .as_deref()
                    .is_none_or(|asset| asset.trim().is_empty())
            {
                return Err(VnError::InvalidScript(
                    "audio play action requires a non-empty asset".to_string(),
                ));
            }
            EventCompiled::AudioAction(AudioActionCompiled {
                channel,
                action: action_kind,
                asset: action.asset.as_deref().map(|asset| ctx.intern(asset)),
                volume: action.volume,
                fade_duration_ms: action.fade_duration_ms,
                loop_playback: action.loop_playback,
            })
        }
        EventRaw::Transition(transition) => EventCompiled::Transition(SceneTransitionCompiled {
            kind: compile_transition_kind(&transition.kind)?,
            duration_ms: transition.duration_ms,
            color: transition.color.as_deref().map(|color| ctx.intern(color)),
        }),
        EventRaw::SetCharacterPosition(pos) => {
            EventCompiled::SetCharacterPosition(SetCharacterPositionCompiled {
                name: ctx.intern(&pos.name),
                x: pos.x,
                y: pos.y,
                scale: pos.scale,
            })
        }
    })
}

pub(super) fn execute_event_compiled(
    ctx: &mut ExecutionCtx<'_>,
    event: &EventCompiled,
) -> VnResult<()> {
    match event {
        EventCompiled::Jump { target_ip } => ctx.jump_to_ip(*target_ip),
        EventCompiled::SetFlag { flag_id, value } => {
            ctx.state.set_flag(*flag_id, *value);
            ctx.advance_position()
        }
        EventCompiled::Scene(scene) => {
            let before_music = ctx.state.visual.music.clone();
            ctx.state.visual.apply_scene(scene);
            append_music_delta(before_music, &ctx.state.visual.music, ctx.audio_commands);
            ctx.advance_position()
        }
        EventCompiled::Choice(_) => Ok(()),
        EventCompiled::Dialogue(dialogue) => {
            let current_ip = ctx.current_ip();
            ctx.state.record_dialogue(dialogue);
            ctx.read_dialogue_ips.insert(current_ip);
            ctx.advance_position()
        }
        EventCompiled::SetVar { var_id, value } => {
            ctx.state.set_var(*var_id, *value);
            ctx.advance_position()
        }
        EventCompiled::JumpIf { cond, target_ip } => {
            if ctx.evaluate_cond(cond) {
                ctx.jump_to_ip(*target_ip)
            } else {
                ctx.advance_position()
            }
        }
        EventCompiled::Patch(patch) => {
            let before_music = ctx.state.visual.music.clone();
            ctx.state.visual.apply_patch(patch);
            append_music_delta(before_music, &ctx.state.visual.music, ctx.audio_commands);
            ctx.advance_position()
        }
        EventCompiled::ExtCall { .. } => Ok(()),
        EventCompiled::AudioAction(action) => {
            if let Some(command) = audio_command_from_action(action) {
                ctx.audio_commands.push(command);
            }
            ctx.advance_position()
        }
        EventCompiled::SetCharacterPosition(pos) => {
            ctx.state.visual.set_character_position(pos)?;
            ctx.advance_position()
        }
        EventCompiled::Transition(transition) => {
            *ctx.pending_transition = Some(transition.clone());
            ctx.advance_position()
        }
    }
}

pub(super) fn preview_event_compiled(
    ctx: &mut PreviewCtx<'_>,
    event: &EventCompiled,
) -> Option<String> {
    match event {
        EventCompiled::Scene(scene) => ctx.visual.apply_scene(scene),
        EventCompiled::Patch(patch) => ctx.visual.apply_patch(patch),
        EventCompiled::SetCharacterPosition(position) => {
            match ctx.visual.set_character_position(position) {
                Ok(()) => {}
                Err(err) => return Some(format!("scene frame visual update failed: {err}")),
            }
        }
        _ => {}
    }
    None
}

pub(super) fn append_scene_frame_for_event(ctx: &mut SceneFrameCtx<'_>, event: &EventCompiled) {
    match event {
        EventCompiled::Dialogue(dialogue) => {
            ctx.commands.push(RenderCommand::Panel {
                style: "dialogue_box".to_string(),
                rect: dialogue_panel_rect(),
            });
            if !dialogue.speaker.is_empty() {
                ctx.commands.push(RenderCommand::Text {
                    text: dialogue.speaker.to_string(),
                    style: "dialogue.speaker".to_string(),
                    rect: LayoutRect {
                        x: 96.0,
                        y: 512.0,
                        width: 1088.0,
                        height: 32.0,
                    },
                });
            }
            ctx.commands.push(RenderCommand::Text {
                text: dialogue.text.to_string(),
                style: "dialogue.text".to_string(),
                rect: LayoutRect {
                    x: 96.0,
                    y: 552.0,
                    width: 1088.0,
                    height: 96.0,
                },
            });
            ctx.commands.push(RenderCommand::Button {
                id: "continue".to_string(),
                label: "Continue".to_string(),
                style: "button.primary".to_string(),
                rect: LayoutRect {
                    x: 1040.0,
                    y: 656.0,
                    width: 144.0,
                    height: 40.0,
                },
            });
            ctx.interactions.push(InteractionSpec {
                id: "continue".to_string(),
                label: "Continue".to_string(),
                action: "advance".to_string(),
            });
        }
        EventCompiled::Choice(choice) => {
            ctx.commands.push(RenderCommand::Panel {
                style: "choice_list".to_string(),
                rect: LayoutRect {
                    x: 336.0,
                    y: 160.0,
                    width: 608.0,
                    height: (96.0 + choice.options.len() as f32 * 56.0).min(480.0),
                },
            });
            ctx.commands.push(RenderCommand::Text {
                text: choice.prompt.to_string(),
                style: "choice.prompt".to_string(),
                rect: LayoutRect {
                    x: 368.0,
                    y: 192.0,
                    width: 544.0,
                    height: 48.0,
                },
            });
            for (index, option) in choice.options.iter().enumerate() {
                let id = format!("choice:{index}");
                let y = 256.0 + index as f32 * 56.0;
                ctx.commands.push(RenderCommand::Button {
                    id: id.clone(),
                    label: option.text.to_string(),
                    style: "button.choice".to_string(),
                    rect: LayoutRect {
                        x: 384.0,
                        y,
                        width: 512.0,
                        height: 44.0,
                    },
                });
                ctx.interactions.push(InteractionSpec {
                    id,
                    label: option.text.to_string(),
                    action: format!("choose:{index}"),
                });
            }
        }
        EventCompiled::ExtCall { command, .. } => {
            ctx.commands.push(RenderCommand::Panel {
                style: "system_overlay".to_string(),
                rect: dialogue_panel_rect(),
            });
            ctx.commands.push(RenderCommand::Text {
                text: format!("External command: {command}"),
                style: "system.text".to_string(),
                rect: LayoutRect {
                    x: 96.0,
                    y: 552.0,
                    width: 1088.0,
                    height: 96.0,
                },
            });
            ctx.interactions.push(InteractionSpec {
                id: "resume".to_string(),
                label: "Resume".to_string(),
                action: "resume".to_string(),
            });
        }
        _ => {}
    }
}

fn compile_audio_channel(channel: &str) -> VnResult<u8> {
    let normalized = channel.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "bgm" => Ok(0),
        "sfx" => Ok(1),
        "voice" => Ok(2),
        _ => Err(VnError::InvalidScript(format!(
            "invalid audio channel '{channel}' (expected bgm|sfx|voice)"
        ))),
    }
}

fn compile_audio_action(action: &str) -> VnResult<u8> {
    let normalized = action.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "play" => Ok(0),
        "stop" => Ok(1),
        "fade_out" => Ok(2),
        _ => Err(VnError::InvalidScript(format!(
            "invalid audio action '{action}' (expected play|stop|fade_out)"
        ))),
    }
}

fn compile_transition_kind(kind: &str) -> VnResult<u8> {
    let normalized = kind.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "fade" | "fade_black" => Ok(0),
        "dissolve" => Ok(1),
        "cut" => Ok(2),
        _ => Err(VnError::InvalidScript(format!(
            "invalid transition kind '{kind}' (expected fade|fade_black|dissolve|cut)"
        ))),
    }
}

const DEFAULT_AUDIO_FADE_MS: u64 = 500;

pub(super) fn append_music_delta(
    before: Option<SharedStr>,
    after: &Option<SharedStr>,
    audio_commands: &mut Vec<AudioCommand>,
) {
    if before.as_deref() == after.as_deref() {
        return;
    }
    match after {
        Some(music) => audio_commands.push(AudioCommand::PlayBgm {
            resource: AssetId::from_path(music.as_ref()),
            path: music.clone(),
            r#loop: true,
            volume: None,
            fade_in: Duration::from_millis(DEFAULT_AUDIO_FADE_MS),
        }),
        None => audio_commands.push(AudioCommand::StopBgm {
            fade_out: Duration::from_millis(DEFAULT_AUDIO_FADE_MS),
        }),
    }
}

fn audio_command_from_action(action: &AudioActionCompiled) -> Option<AudioCommand> {
    match action.action {
        0 => audio_play_command(action),
        1 | 2 => audio_stop_command(action),
        _ => None,
    }
}

fn audio_play_command(action: &AudioActionCompiled) -> Option<AudioCommand> {
    let path = action.asset.as_ref()?;
    match action.channel {
        0 => Some(AudioCommand::PlayBgm {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            r#loop: action.loop_playback.unwrap_or(true),
            volume: action.volume,
            fade_in: Duration::from_millis(
                action.fade_duration_ms.unwrap_or(DEFAULT_AUDIO_FADE_MS),
            ),
        }),
        1 => Some(AudioCommand::PlaySfx {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            volume: action.volume,
        }),
        2 => Some(AudioCommand::PlayVoice {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            volume: action.volume,
        }),
        _ => None,
    }
}

fn audio_stop_command(action: &AudioActionCompiled) -> Option<AudioCommand> {
    match action.channel {
        0 => Some(AudioCommand::StopBgm {
            fade_out: Duration::from_millis(
                action.fade_duration_ms.unwrap_or(DEFAULT_AUDIO_FADE_MS),
            ),
        }),
        1 => Some(AudioCommand::StopSfx),
        2 => Some(AudioCommand::StopVoice),
        _ => None,
    }
}

fn dialogue_panel_rect() -> LayoutRect {
    LayoutRect {
        x: 64.0,
        y: 496.0,
        width: 1152.0,
        height: 200.0,
    }
}
