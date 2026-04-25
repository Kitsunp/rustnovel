#!/usr/bin/env python3
"""
Timeline Animation Example

Demonstrates how to use the Timeline, Track, and Keyframe classes
for creating deterministic animations in Visual Novel Engine.
"""

import visual_novel_engine as vn


def main():
    print("=== Timeline Animation Example ===\n")

    # Create a timeline with 60 ticks per second
    timeline = vn.Timeline(ticks_per_second=60)
    print(f"Created timeline: {timeline}")

    # Create a track for entity 1's X position
    track_x = vn.Track(entity_id=1, property="position_x")

    # Add keyframes with different easing
    track_x.add_keyframe(vn.Keyframe(time=0, value=0, easing="linear"))
    track_x.add_keyframe(
        vn.Keyframe(time=60, value=100, easing="ease_out")
    )  # At 1 second
    track_x.add_keyframe(
        vn.Keyframe(time=120, value=50, easing="ease_in_out")
    )  # At 2 seconds

    print(f"Created track: {track_x}")
    print(f"Keyframes: {track_x.keyframes()}")

    # Create a track for opacity (fade in)
    track_opacity = vn.Track(entity_id=1, property="opacity")
    track_opacity.add_keyframe(vn.Keyframe(0, 0))  # Start transparent
    track_opacity.add_keyframe(vn.Keyframe(30, 1000, "ease_in"))  # Fade in over 0.5s

    # Add tracks to timeline
    timeline.add_track(track_x)
    timeline.add_track(track_opacity)

    print(f"\nTimeline has {len(timeline)} tracks")
    print(f"Duration: {timeline.duration} ticks ({timeline.duration / 60:.2f} seconds)")

    # Evaluate at different times
    print("\n--- Evaluating at different times ---")
    for time in [0, 15, 30, 60, 90, 120]:
        values = timeline.evaluate_at(time)
        print(f"t={time:3} ticks: {values}")

    # Demonstrate deterministic scrubbing
    print("\n--- Scrubbing Determinism Test ---")

    # Method 1: Advance step by step
    timeline.seek(0)
    for _ in range(45):
        timeline.advance(1)
    value_stepped = timeline.evaluate()

    # Method 2: Jump directly
    value_jumped = timeline.evaluate_at(45)

    print(f"Value at t=45 (stepped): {value_stepped}")
    print(f"Value at t=45 (jumped):  {value_jumped}")
    print(f"Deterministic: {value_stepped == value_jumped}")


if __name__ == "__main__":
    main()
