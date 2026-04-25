//! Example: Timeline and Animation System
//!
//! This example demonstrates how to use the timeline system
//! for creating deterministic animations.

use visual_novel_engine::{Easing, EntityId, Fixed, Keyframe, PropertyType, Timeline, Track};

fn main() {
    println!("=== Timeline and Animation Example ===\n");

    // Create a timeline with 60 ticks per second
    let mut timeline = Timeline::new(60);

    // Create a track for entity 1's X position
    let entity_id = EntityId::new(1);
    let mut track_x = Track::new(entity_id, PropertyType::PositionX);

    // Add keyframes
    track_x
        .add_keyframe(Keyframe::new(0, 0, Easing::Linear))
        .expect("add keyframe");
    track_x
        .add_keyframe(Keyframe::new(60, 100, Easing::EaseOut)) // At 1 second
        .expect("add keyframe");
    track_x
        .add_keyframe(Keyframe::new(120, 50, Easing::EaseInOut)) // At 2 seconds
        .expect("add keyframe");

    timeline.add_track(track_x).expect("add track");

    // Create a track for opacity (fade in)
    let mut track_opacity = Track::new(entity_id, PropertyType::Opacity);
    track_opacity
        .add_keyframe(Keyframe::new(0, 0, Easing::Linear))
        .expect("add keyframe");
    track_opacity
        .add_keyframe(Keyframe::new(30, 1000, Easing::EaseIn)) // Fade in over 0.5s
        .expect("add keyframe");

    timeline.add_track(track_opacity).expect("add track");

    println!("Timeline created with {} tracks", timeline.track_count());
    println!(
        "Duration: {} ticks ({} seconds)\n",
        timeline.duration(),
        timeline.duration() / 60
    );

    // Evaluate at different times
    println!("--- Evaluating at different times ---");
    for time in [0, 15, 30, 60, 90, 120] {
        let values = timeline.evaluate_at(time);
        println!("t={:3} ticks:", time);
        for (id, prop, val) in values {
            println!("  Entity {:?}: {:?} = {}", id, prop, val);
        }
    }

    // Demonstrate scrubbing determinism
    println!("\n--- Scrubbing Determinism Test ---");

    // Method 1: Advance step by step
    timeline.seek(0);
    for _ in 0..45 {
        timeline.advance(1);
    }
    let value_stepped = timeline.evaluate();

    // Method 2: Jump directly
    let value_jumped = timeline.evaluate_at(45);

    println!("Value at t=45 (stepped): {:?}", value_stepped);
    println!("Value at t=45 (jumped):  {:?}", value_jumped);
    println!(
        "Deterministic: {}",
        value_stepped
            .iter()
            .zip(value_jumped.iter())
            .all(|(a, b)| a == b)
    );

    // Fixed-point math demonstration
    println!("\n--- Fixed-Point Math (Q16.16) ---");
    let half = Fixed::from_f32(0.5);
    let quarter = Fixed::from_f32(0.25);
    let result = half + quarter;
    println!("0.5 + 0.25 = {} (raw: {})", result.to_f32(), result.raw());

    let a = Fixed::from_int(0);
    let b = Fixed::from_int(100);
    let t = Fixed::from_f32(0.3);
    let lerp = Fixed::lerp(a, b, t);
    println!("lerp(0, 100, 0.3) = {} (exact: 30)", lerp.to_int());
}
