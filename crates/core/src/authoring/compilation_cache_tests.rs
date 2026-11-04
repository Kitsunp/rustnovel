use std::path::Path;

use crate::authoring::{AuthoringPosition, NodeGraph, StoryNode};

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
        AuthoringPosition::new(0.0, 0.0),
    );

    let before = hash_referenced_asset_state(&graph, &project_root);
    std::fs::write(&outside, b"outside-two").expect("outside two");
    let after = hash_referenced_asset_state(&graph, &project_root);

    assert_eq!(
        before, after,
        "relative symlink escapes should hash as blocked traversal, not outside file contents"
    );
}

#[test]
fn cache_does_not_reuse_none_scale_for_nonfinite_scale() {
    let mut graph = NodeGraph::new();
    let id = graph.add_node(
        StoryNode::CharacterPlacement {
            name: "Ava".into(),
            x: 0,
            y: 0,
            scale: None,
        },
        AuthoringPosition::new(0.0, 0.0),
    );
    let mut cache = CompilationCache::default();
    let first = cache.get_or_compile(&graph, None);
    assert!(!first
        .issues
        .iter()
        .any(|issue| issue.code == crate::authoring::LintCode::InvalidCharacterScale));
    if let Some(StoryNode::CharacterPlacement { scale, .. }) = graph.get_node_mut(id) {
        *scale = Some(f32::NAN);
    } else {
        panic!("expected character node");
    }
    let second = cache.get_or_compile(&graph, None);
    assert!(second
        .issues
        .iter()
        .any(|issue| issue.code == crate::authoring::LintCode::InvalidCharacterScale));
}

#[test]
fn cache_reuses_layout_moves_until_position_validity_changes() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".into(),
            text: "Hello".into(),
        },
        AuthoringPosition::new(0.0, 90.0),
    );
    graph.connect(start, dialogue);
    let mut cache = CompilationCache::default();
    for (x, invalid, stats) in [
        (0.0, false, (0, 1)),
        (240.0, false, (1, 1)),
        (2_000_000.0, true, (1, 2)),
        (3_000_000.0, true, (2, 2)),
        (400.0, false, (2, 3)),
    ] {
        graph.set_node_pos(dialogue, AuthoringPosition::new(x, 90.0));
        let result = cache.get_or_compile(&graph, None);
        assert_eq!(
            result
                .issues
                .iter()
                .any(|issue| { issue.code == crate::authoring::LintCode::InvalidLayoutPosition }),
            invalid
        );
        assert_eq!(cache.stats(), stats);
    }
}

#[test]
fn unsafe_assets_do_not_affect_cache_but_valid_asset_content_does() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir_all(root.join("subdir")).unwrap();
    let outside = temp.path().join("outside.png");
    let inside = root.join("inside.png");
    std::fs::write(&outside, b"first").unwrap();
    std::fs::write(&inside, b"first").unwrap();
    for path in [
        outside.to_string_lossy().into_owned(),
        "subdir/../inside.png".into(),
    ] {
        let mut graph = NodeGraph::new();
        graph.add_node(
            StoryNode::Scene {
                profile: None,
                background: Some(path),
                music: None,
                characters: Vec::new(),
            },
            AuthoringPosition::new(0.0, 0.0),
        );
        let mut cache = CompilationCache::default();
        let initial = cache.get_or_compile(&graph, Some(&root));
        assert!(initial
            .issues
            .iter()
            .any(|issue| issue.code == crate::authoring::LintCode::UnsafeAssetPath));
        std::fs::write(&outside, b"other").unwrap();
        std::fs::write(&inside, b"other").unwrap();
        let after = cache.get_or_compile(&graph, Some(&root));
        assert!(after
            .issues
            .iter()
            .any(|issue| issue.code == crate::authoring::LintCode::UnsafeAssetPath));
        assert_eq!(cache.stats(), (1, 1));
        std::fs::write(&outside, b"first").unwrap();
        std::fs::write(&inside, b"first").unwrap();
    }
    let mut graph = NodeGraph::new();
    graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("inside.png".into()),
            music: None,
            characters: Vec::new(),
        },
        AuthoringPosition::new(0.0, 0.0),
    );
    let mut cache = CompilationCache::default();
    cache.get_or_compile(&graph, Some(&root));
    std::fs::write(&inside, b"other").unwrap();
    cache.get_or_compile(&graph, Some(&root));
    assert_eq!(cache.stats(), (0, 2));

    let empty = NodeGraph::new();
    let restored_root = temp.path().join("restored");
    let unavailable = cache.get_or_compile(&empty, Some(&restored_root));
    assert!(unavailable
        .issues
        .iter()
        .any(|issue| issue.message.contains("Project root unavailable")));
    std::fs::create_dir(&restored_root).unwrap();
    let restored = cache.get_or_compile(&empty, Some(&restored_root));
    assert!(!restored
        .issues
        .iter()
        .any(|issue| issue.message.contains("Project root unavailable")));
}
