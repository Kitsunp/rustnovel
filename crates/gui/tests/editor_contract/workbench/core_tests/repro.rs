use super::*;

#[test]
fn workbench_can_import_repro_case_json() {
    let config = VnConfig::default();
    let mut source = EditorWorkbench::new(config.clone());
    let start = source
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = source.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 120.0),
    );
    source.node_graph.connect(start, dialogue);

    let case = source
        .build_repro_case_from_current_graph()
        .expect("repro case should be generated");
    let payload = case.to_json().expect("repro case should serialize");

    let mut target = EditorWorkbench::new(config);
    target.composer_layer_overrides.insert("old-layer".to_string(), crate::editor::visual_composer::LayerOverride { visible: false, locked: true });
    target.composer_background_fit_overrides.insert("0".to_string(), crate::editor::BackgroundFit::Contain);
    target.composer_entity_owners.insert(99, 99);
    target.composer_image_failures.insert("old.png".to_string(), "old failure".to_string());
    target.operation_log.push(visual_novel_engine::authoring::OperationLogEntry::new_typed(
        visual_novel_engine::authoring::OperationKind::FieldEdited,
        visual_novel_engine::authoring::OperationStatus::Applied,
        "old document operation",
    ));
    target.selected_entity = Some(99);
    target.pending_graph_focus = Some(99);
    target.show_fix_confirm = true;
    target.imported_report_untrusted = true;
    target.queue_editor_operation("stale", "old pending edit", None);
    target
        .apply_repro_case_json(&payload)
        .expect("repro case should import");

    assert!(target.loaded_repro_case.is_some());
    assert!(target.current_script.is_some());
    assert!(target.composer_layer_overrides.is_empty());
    assert!(target.composer_background_fit_overrides.is_empty());
    assert!(target.operation_log.is_empty());
    assert!(target.composer_entity_owners.is_empty());
    assert!(target.composer_image_failures.is_empty());
    assert!(target.selected_entity.is_none());
    assert!(target.pending_graph_focus.is_none());
    assert!(!target.show_fix_confirm);
    assert!(!target.imported_report_untrusted);
    target.record_pending_editor_operation();
    assert_ne!(target.operation_log.last().unwrap().operation_kind, "stale");
    assert!(target
        .node_graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Dialogue { .. })));
}

#[test]
fn workbench_runs_loaded_repro_case_and_records_report() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let script = visual_novel_engine::runtime::ScriptRaw::new(
        vec![visual_novel_engine::runtime::EventRaw::Dialogue(
            visual_novel_engine::runtime::DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "Hola".to_string(),
            },
        )],
        std::collections::BTreeMap::from([("start".to_string(), 0usize)]),
    );
    let mut case = visual_novel_engine::ReproCase::new("editor_repro", script);
    case.oracle
        .monitors
        .push(visual_novel_engine::ReproMonitor::EventKindAtStep {
            monitor_id: "dialogue_step_0".to_string(),
            step: 0,
            expected: "dialogue".to_string(),
        });
    workbench.loaded_repro_case = Some(case);

    workbench.run_loaded_repro_case();
    let report = workbench
        .last_repro_report
        .as_ref()
        .expect("repro report should be available");
    assert!(report.oracle_triggered);
    assert!(report
        .matched_monitors
        .iter()
        .any(|id| id == "dialogue_step_0"));
}
