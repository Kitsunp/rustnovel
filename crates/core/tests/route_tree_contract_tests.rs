use std::collections::BTreeMap;

use visual_novel_engine::{
    build_route_tree, resolve_visual_at_ip, runtime::*, ResourceLimiter, SecurityPolicy,
    VisualResolveStrategy,
};

fn branching_script() -> ScriptRaw {
    ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/room.png".to_string()),
                ..Default::default()
            }),
            EventRaw::Choice(ChoiceRaw {
                prompt: "Go?".to_string(),
                options: vec![
                    ChoiceOptionRaw {
                        text: "Loop".to_string(),
                        target: "start".to_string(),
                    },
                    ChoiceOptionRaw {
                        text: "End".to_string(),
                        target: "end".to_string(),
                    },
                ],
            }),
            EventRaw::JumpIf {
                cond: CondRaw::Flag {
                    key: "seen".to_string(),
                    is_set: true,
                },
                target: "end".to_string(),
            },
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "Middle".to_string(),
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "End".to_string(),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0), ("end".to_string(), 4)]),
    )
}

#[test]
fn route_tree_contains_choices_cycles_conditionals_and_endings() {
    let compiled = branching_script().compile().expect("compile");
    let tree = build_route_tree(&compiled);

    assert_eq!(tree.root, 0);
    assert!(tree
        .edges
        .iter()
        .any(|edge| edge.kind == RouteEdgeKind::Choice && edge.to == Some(0)));
    assert!(tree
        .edges
        .iter()
        .any(|edge| edge.kind == RouteEdgeKind::Conditional && edge.to == Some(4)));
    assert!(tree
        .edges
        .iter()
        .any(|edge| edge.kind == RouteEdgeKind::Ending && edge.from == 4));
}

#[test]
fn engine_snapshots_preserve_read_model_and_route_progress_on_set_state() {
    let mut engine = Engine::new(
        branching_script(),
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    let (_audio, _change) = engine.step().expect("scene");
    engine.choose(1).expect("choose ending");
    let (_audio, _change) = engine.step().expect("ending dialogue");

    let saved = engine.state().clone();
    let mut restored = Engine::new(
        branching_script(),
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    restored.set_state(saved).expect("restore");

    assert_eq!(
        restored.read_model_snapshot().visited_dialogue_ips,
        engine.read_model_snapshot().visited_dialogue_ips
    );
    assert_eq!(
        restored.route_progress_snapshot().selected_choices,
        engine.route_progress_snapshot().selected_choices
    );
}

#[test]
fn resolve_visual_at_ip_replays_visual_state_for_preview() {
    let compiled = branching_script().compile().expect("compile");
    let visual = resolve_visual_at_ip(&compiled, 3, VisualResolveStrategy::LinearReplay);
    assert_eq!(visual.background.as_deref(), Some("bg/room.png"));
}
