#[cfg(feature = "arbitrary")]
mod fuzz {
    use arbitrary::{Arbitrary, Unstructured};
    use visual_novel_engine::{
        Engine, EventCompiled, EventRaw, ResourceLimiter, ScriptCompiled, ScriptRaw,
        SecurityPolicy, VnError,
    };

    fn fill_deterministic(buf: &mut [u8], seed: u64) {
        let mut state = seed;
        for byte in buf.iter_mut() {
            // xorshift64*
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
            *byte = (state & 0xFF) as u8;
        }
    }

    fn drive_engine_trace(mut engine: Engine, max_steps: usize) -> Vec<String> {
        let mut trace = Vec::with_capacity(max_steps);
        for step in 0..max_steps {
            let event = match engine.current_event() {
                Ok(event) => event,
                Err(VnError::EndOfScript) => break,
                Err(_) => break,
            };
            trace.push(event.to_json_string());
            match event {
                EventCompiled::Choice(choice) => {
                    if choice.options.is_empty() {
                        break;
                    }
                    let index = step % choice.options.len();
                    if engine.choose(index).is_err() {
                        break;
                    }
                }
                EventCompiled::ExtCall { .. } => {
                    if engine.resume().is_err() {
                        break;
                    }
                }
                _ => {
                    if engine.step().is_err() {
                        break;
                    }
                }
            }
        }
        trace
    }

    fn event_kind(event: &EventRaw) -> &'static str {
        match event {
            EventRaw::Dialogue(_) => "dialogue",
            EventRaw::Choice(_) => "choice",
            EventRaw::Scene(_) => "scene",
            EventRaw::Jump { .. } => "jump",
            EventRaw::SetFlag { .. } => "set_flag",
            EventRaw::SetVar { .. } => "set_var",
            EventRaw::JumpIf { .. } => "jump_if",
            EventRaw::Patch(_) => "patch",
            EventRaw::ExtCall { .. } => "ext_call",
            EventRaw::AudioAction(_) => "audio_action",
            EventRaw::Transition(_) => "transition",
            EventRaw::SetCharacterPosition(_) => "set_character_position",
        }
    }

    #[test]
    fn fuzz_compile_raw_scripts() {
        let mut raw_data = [0u8; 1024 * 64];

        for i in 0..128u64 {
            fill_deterministic(&mut raw_data, 0xA11C_E55u64 ^ i);
            let mut u = Unstructured::new(&raw_data);

            if let Ok(script) = ScriptRaw::arbitrary(&mut u) {
                let policy = SecurityPolicy::default();
                let limits = ResourceLimiter::default();

                let _ = policy.validate(&script, limits);

                if let Ok(compiled) = script.compile() {
                    // Binary roundtrip must remain panic-free and lossless in type shape.
                    if let Ok(bytes) = compiled.to_binary() {
                        let decoded = ScriptCompiled::from_binary(&bytes)
                            .expect("compiled script should deserialize");
                        assert_eq!(decoded.events.len(), compiled.events.len());
                        assert_eq!(decoded.labels, compiled.labels);
                        assert_eq!(decoded.start_ip, compiled.start_ip);
                        assert_eq!(decoded.flag_count, compiled.flag_count);

                        let bytes_again = compiled
                            .to_binary()
                            .expect("compiled script should reserialize deterministically");
                        assert_eq!(bytes_again, bytes);

                        let trace_original = drive_engine_trace(
                            Engine::from_compiled(
                                compiled.clone(),
                                SecurityPolicy::default(),
                                ResourceLimiter::default(),
                            )
                            .expect("original compiled script should boot"),
                            256,
                        );
                        let trace_roundtrip = drive_engine_trace(
                            Engine::from_compiled(
                                decoded,
                                SecurityPolicy::default(),
                                ResourceLimiter::default(),
                            )
                            .expect("roundtrip compiled script should boot"),
                            256,
                        );
                        assert_eq!(trace_roundtrip, trace_original);
                    }

                    let _ = Engine::from_compiled(
                        compiled,
                        SecurityPolicy::default(),
                        ResourceLimiter::default(),
                    );
                }
            }
        }
    }

    #[test]
    fn fuzz_json_roundtrip_stability() {
        let mut raw_data = [0u8; 1024 * 32];

        for i in 0..64u64 {
            fill_deterministic(&mut raw_data, 0x5EED_1234u64 ^ (i << 1));
            let mut u = Unstructured::new(&raw_data);
            if let Ok(script) = ScriptRaw::arbitrary(&mut u) {
                if let Ok(json) = script.to_json() {
                    let reparsed =
                        ScriptRaw::from_json_with_limits(&json, ResourceLimiter::default())
                            .expect("json roundtrip should parse after script serialization");
                    assert_eq!(reparsed.labels, script.labels);
                    assert_eq!(reparsed.events.len(), script.events.len());
                    assert_eq!(
                        reparsed.events.iter().map(event_kind).collect::<Vec<_>>(),
                        script.events.iter().map(event_kind).collect::<Vec<_>>()
                    );

                    let json_again = reparsed
                        .to_json()
                        .expect("reparsed script should serialize deterministically");
                    let reparsed_again =
                        ScriptRaw::from_json_with_limits(&json_again, ResourceLimiter::default())
                            .expect("second parse should remain stable");
                    assert_eq!(reparsed_again.labels, reparsed.labels);
                    assert_eq!(reparsed_again.events.len(), reparsed.events.len());
                    assert_eq!(
                        reparsed_again
                            .events
                            .iter()
                            .map(event_kind)
                            .collect::<Vec<_>>(),
                        reparsed.events.iter().map(event_kind).collect::<Vec<_>>()
                    );
                    let json_third = reparsed_again
                        .to_json()
                        .expect("second parsed script should serialize");
                    assert_eq!(json_again, json_third);
                }
            }
        }
    }
}
