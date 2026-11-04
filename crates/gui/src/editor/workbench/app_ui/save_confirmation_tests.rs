use super::*;
use crate::editor::node_types::{StoryNode, ToastKind};

fn click_save_button(workbench: &mut EditorWorkbench, ctx: &egui::Context, label: &str) {
    let mut render = |events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| workbench.handle_save_confirmation(ctx),
        )
    };
    drop(render(vec![]));
    let output = render(vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::epaint::Shape::Text(text) = &shape.shape {
                (text.galley.text() == label).then_some(text.pos + text.galley.size() * 0.5)
            } else {
                None
            }
        })
        .expect("save dialog button must be rendered");
    for pressed in [true, false] {
        drop(render(vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]));
    }
}

#[test]
fn save_confirmation_preserves_drafts_and_reports_publication_failure() {
    for blocked_output in [true, false] {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("draft.vnauthoring");
        let mut workbench = EditorWorkbench::new(VnConfig::default());
        let dialogue = workbench.node_graph.add_node(
            StoryNode::Dialogue {
                speaker: "Ava".into(),
                text: "Still writing".into(),
            },
            egui::pos2(0.0, 100.0),
        );
        if blocked_output {
            std::fs::create_dir(&path).unwrap();
            let start = workbench
                .node_graph
                .add_node(StoryNode::Start, egui::Pos2::ZERO);
            let end = workbench
                .node_graph
                .add_node(StoryNode::End, egui::pos2(0.0, 200.0));
            workbench.node_graph.connect(start, dialogue);
            workbench.node_graph.connect(dialogue, end);
        }
        workbench.pending_save_path = Some(path.clone());
        workbench.prepare_save_confirmation();
        let ctx = egui::Context::default();
        click_save_button(&mut workbench, &ctx, "Confirmar Guardado");
        assert!(
            !workbench.show_save_confirm,
            "the confirmation click must be handled"
        );
        let toast = workbench
            .toast
            .as_ref()
            .expect("save outcome must be visible");
        if blocked_output {
            assert!(matches!(toast.kind, ToastKind::Error), "{}", toast.message);
            assert!(
                workbench.node_graph.is_modified(),
                "a failed save must keep unsaved work dirty"
            );
            assert!(workbench.saved_script_snapshot.is_none());
        } else {
            assert!(
                matches!(toast.kind, ToastKind::Success),
                "{}",
                toast.message
            );
            let saved = visual_novel_engine::authoring::load_authoring_entry(&path).unwrap();
            assert!(
                matches!(saved.graph.get_node(dialogue), Some(StoryNode::Dialogue { text, .. }) if text == "Still writing")
            );
            assert!(!workbench.node_graph.is_modified());
        }

        workbench.prepare_save_confirmation();
        assert!(workbench.diff_dialog.is_some());
        click_save_button(&mut workbench, &ctx, "Cancelar");
        assert!(!workbench.show_save_confirm);
        assert!(
            workbench.diff_dialog.is_none(),
            "closing the preview must release its document snapshots"
        );
    }
}
