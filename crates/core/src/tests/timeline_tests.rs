use super::*;

#[test]
fn test_fixed_basic_ops() {
    let one = Fixed::ONE;
    let two = Fixed::from_int(2);
    let half = one / two;

    assert_eq!(half.to_f32(), 0.5);
    assert_eq!((one + one).to_int(), 2);
    assert_eq!((two * half).to_int(), 1);
}

#[test]
fn test_fixed_lerp() {
    let a = Fixed::from_int(0);
    let b = Fixed::from_int(100);
    let half = Fixed::from_raw(Fixed::ONE.0 / 2);

    let result = Fixed::lerp(a, b, half);
    assert_eq!(result.to_int(), 50);

    let result_zero = Fixed::lerp(a, b, Fixed::ZERO);
    assert_eq!(result_zero.to_int(), 0);

    let result_one = Fixed::lerp(a, b, Fixed::ONE);
    assert_eq!(result_one.to_int(), 100);
}

#[test]
fn test_easing_linear() {
    let t = Fixed::from_raw(Fixed::ONE.0 / 2); // 0.5
    let result = Easing::Linear.apply(t);
    assert_eq!(result.raw(), t.raw());
}

#[test]
fn test_easing_step() {
    let half = Fixed::from_raw(Fixed::ONE.0 / 2);
    assert_eq!(Easing::Step.apply(half), Fixed::ZERO);
    assert_eq!(Easing::Step.apply(Fixed::ONE), Fixed::ONE);
}

#[test]
fn test_track_add_keyframes() {
    let mut track = Track::new(EntityId::new(1), PropertyType::PositionX);

    track
        .add_keyframe(Keyframe::new(0, 0, Easing::Linear))
        .unwrap();
    track
        .add_keyframe(Keyframe::new(100, 500, Easing::Linear))
        .unwrap();
    track
        .add_keyframe(Keyframe::new(50, 250, Easing::Linear))
        .unwrap();

    // Should be sorted
    let times: Vec<_> = track.keyframes().map(|k| k.time).collect();
    assert_eq!(times, vec![0, 50, 100]);
}

#[test]
fn test_track_duplicate_time_error() {
    let mut track = Track::new(EntityId::new(1), PropertyType::PositionX);
    track
        .add_keyframe(Keyframe::new(50, 100, Easing::Linear))
        .unwrap();

    let result = track.add_keyframe(Keyframe::new(50, 200, Easing::Linear));
    assert!(matches!(
        result,
        Err(TimelineError::DuplicateKeyframeTime { time: 50 })
    ));
}

#[test]
fn test_track_evaluate_linear() {
    let mut track = Track::new(EntityId::new(1), PropertyType::PositionX);
    track
        .add_keyframe(Keyframe::new(0, 0, Easing::Linear))
        .unwrap();
    track
        .add_keyframe(Keyframe::new(100, 100, Easing::Linear))
        .unwrap();

    // At keyframes
    assert_eq!(track.evaluate(0), Some(0));
    assert_eq!(track.evaluate(100), Some(100));

    // Midpoint
    assert_eq!(track.evaluate(50), Some(50));

    // Before and after
    assert_eq!(track.evaluate(0), Some(0));
    assert_eq!(track.evaluate(200), Some(100));
}

#[test]
fn test_track_evaluate_ease_in() {
    let mut track = Track::new(EntityId::new(1), PropertyType::Scale);
    track
        .add_keyframe(Keyframe::new(0, 0, Easing::Linear))
        .unwrap();
    track
        .add_keyframe(Keyframe::new(100, 1000, Easing::EaseIn))
        .unwrap();

    // EaseIn at midpoint should be less than linear
    let value_at_50 = track.evaluate(50).unwrap();
    assert!(
        value_at_50 < 500,
        "EaseIn at t=0.5 should be < 500, got {}",
        value_at_50
    );
}

#[test]
fn test_timeline_evaluate() {
    let entity = EntityId::new(42);
    let mut timeline = Timeline::new(60);

    let mut track_x = Track::new(entity, PropertyType::PositionX);
    track_x
        .add_keyframe(Keyframe::new(0, 0, Easing::Linear))
        .unwrap();
    track_x
        .add_keyframe(Keyframe::new(60, 100, Easing::Linear))
        .unwrap();
    timeline.add_track(track_x).unwrap();

    let mut track_y = Track::new(entity, PropertyType::PositionY);
    track_y
        .add_keyframe(Keyframe::new(0, 0, Easing::Linear))
        .unwrap();
    track_y
        .add_keyframe(Keyframe::new(60, 200, Easing::Linear))
        .unwrap();
    timeline.add_track(track_y).unwrap();

    timeline.seek(30);
    let values = timeline.evaluate();

    assert_eq!(values.len(), 2);
    assert!(values
        .iter()
        .any(|(id, prop, val)| *id == entity && *prop == PropertyType::PositionX && *val == 50));
    assert!(values
        .iter()
        .any(|(id, prop, val)| *id == entity && *prop == PropertyType::PositionY && *val == 100));
}

#[test]
fn test_scrubbing_determinism() {
    let entity = EntityId::new(1);
    let mut timeline = Timeline::new(60);

    let mut track = Track::new(entity, PropertyType::Opacity);
    track
        .add_keyframe(Keyframe::new(0, 0, Easing::Linear))
        .unwrap();
    track
        .add_keyframe(Keyframe::new(300, 1000, Easing::Linear))
        .unwrap();
    timeline.add_track(track).unwrap();

    // Evaluate by advancing step by step
    timeline.seek(0);
    for _ in 0..150 {
        timeline.advance(1);
    }
    let value_stepped = timeline.evaluate()[0].2;

    // Evaluate by jumping directly
    let value_jumped = timeline.evaluate_at(150)[0].2;

    // Should be identical (determinism test)
    assert_eq!(
        value_stepped, value_jumped,
        "Scrubbing should be deterministic"
    );
}
