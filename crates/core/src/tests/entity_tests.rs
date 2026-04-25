use super::*;

#[test]
fn test_spawn_and_get() {
    let mut scene = SceneState::new();
    let id = scene
        .spawn(EntityKind::Image(ImageData {
            path: "bg.png".into(),
            tint: None,
        }))
        .expect("spawn should succeed");

    assert_eq!(scene.len(), 1);
    let entity = scene.get(id).expect("entity should exist");
    assert_eq!(entity.id, id);
    match &entity.kind {
        EntityKind::Image(data) => assert_eq!(data.path.as_ref(), "bg.png"),
        _ => panic!("wrong entity kind"),
    }
}

#[test]
fn test_despawn() {
    let mut scene = SceneState::new();
    let id1 = scene
        .spawn(EntityKind::Text(TextData {
            content: "Hello".into(),
            font_size: 16,
            color: 0xFFFFFFFF,
        }))
        .unwrap();
    let id2 = scene
        .spawn(EntityKind::Text(TextData {
            content: "World".into(),
            font_size: 16,
            color: 0xFFFFFFFF,
        }))
        .unwrap();

    assert_eq!(scene.len(), 2);
    assert!(scene.despawn(id1));
    assert_eq!(scene.len(), 1);
    assert!(scene.get(id1).is_none());
    assert!(scene.get(id2).is_some());
}

#[test]
fn test_iter_sorted_determinism() {
    let mut scene = SceneState::new();

    // Spawn entities with different z_orders
    let id1 = scene
        .spawn_with_transform(
            Transform {
                z_order: 10,
                ..Transform::new()
            },
            EntityKind::Image(ImageData {
                path: "front.png".into(),
                tint: None,
            }),
        )
        .unwrap();
    let id2 = scene
        .spawn_with_transform(
            Transform {
                z_order: 0,
                ..Transform::new()
            },
            EntityKind::Image(ImageData {
                path: "back.png".into(),
                tint: None,
            }),
        )
        .unwrap();
    let id3 = scene
        .spawn_with_transform(
            Transform {
                z_order: 5,
                ..Transform::new()
            },
            EntityKind::Image(ImageData {
                path: "mid.png".into(),
                tint: None,
            }),
        )
        .unwrap();

    // Sorted order should be: id2 (z=0), id3 (z=5), id1 (z=10)
    let sorted: Vec<_> = scene.iter_sorted().map(|e| e.id).collect();
    assert_eq!(sorted, vec![id2, id3, id1]);
}

#[test]
fn test_entity_limit() {
    let mut scene = SceneState::new();
    for i in 0..MAX_ENTITIES {
        let result = scene.spawn(EntityKind::Text(TextData {
            content: format!("Entity {}", i),
            font_size: 12,
            color: 0xFFFFFFFF,
        }));
        assert!(result.is_some(), "spawn {} should succeed", i);
    }
    // Next spawn should fail
    let result = scene.spawn(EntityKind::Text(TextData {
        content: "Overflow".into(),
        font_size: 12,
        color: 0xFFFFFFFF,
    }));
    assert!(result.is_none(), "spawn beyond limit should fail");
}

#[test]
fn test_serialization_roundtrip() {
    let mut scene = SceneState::new();
    scene
        .spawn(EntityKind::Character(CharacterData {
            name: "Alice".into(),
            expression: Some("happy".into()),
        }))
        .unwrap();

    let json = serde_json::to_string(&scene).expect("serialize");
    let mut restored: SceneState = serde_json::from_str(&json).expect("deserialize");
    restored.rebuild_index();

    assert_eq!(restored.len(), 1);
    let entity = restored.iter().next().unwrap();
    match &entity.kind {
        EntityKind::Character(data) => {
            assert_eq!(data.name.as_ref(), "Alice");
            assert_eq!(data.expression.as_ref().map(|s| s.as_ref()), Some("happy"));
        }
        _ => panic!("wrong kind"),
    }
}

#[test]
fn test_transform_defaults() {
    let t = Transform::new();
    assert_eq!(t.x, 0);
    assert_eq!(t.y, 0);
    assert_eq!(t.z_order, 0);
    assert_eq!(t.scale, 1000);
    assert_eq!(t.opacity, 1000);
}
