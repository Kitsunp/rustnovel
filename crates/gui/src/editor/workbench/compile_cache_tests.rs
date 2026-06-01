use std::path::Path;

use crate::editor::{NodeGraph, StoryNode};

use super::*;

fn create_file_symlink(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = link;
        let _ = target;
        false
    }
}

#[test]
fn referenced_asset_hash_does_not_follow_relative_symlink_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    let outside = temp.path().join("outside.png");
    std::fs::create_dir_all(project_root.join("assets/backgrounds")).expect("asset dir");
    std::fs::write(&outside, b"outside-one").expect("outside one");
    let link = project_root.join("assets/backgrounds/escape.png");
    if !create_file_symlink(&link, &outside) {
        eprintln!("file symlink creation not supported on this platform");
        return;
    }

    let mut graph = NodeGraph::new();
    graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("assets/backgrounds/escape.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        eframe::egui::pos2(0.0, 0.0),
    );

    let before = hash_referenced_asset_state(&graph, &project_root);
    std::fs::write(&outside, b"outside-two").expect("outside two");
    let after = hash_referenced_asset_state(&graph, &project_root);

    assert_eq!(
        before, after,
        "relative symlink escapes should hash as blocked traversal, not outside file contents"
    );
}
