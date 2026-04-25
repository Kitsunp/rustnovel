#![allow(unused_assignments)]
//! Timeline and animation system for the Visual Novel Engine.
//!
//! This module implements deterministic timeline-based animations following the
//! implementation plan. It supports keyframes, easing functions, and scrubbing.
//!
//! # Contracts
//! - **Invariant**: Keyframes in a track are always sorted by time ($t_i < t_{i+1}$).
//! - **Invariant**: Interpolation is deterministic and bit-exact across architectures.
//!
//! # Fixed-Point Arithmetic
//! To ensure determinism, we use Q16.16 fixed-point for interpolation factors.
//! This means 16 bits for the integer part and 16 bits for the fractional part.

use serde::{Deserialize, Serialize};

use crate::entity::EntityId;

#[path = "timeline/types.rs"]
mod types;
pub use types::{Easing, Fixed, Keyframe, PropertyType, PropertyValue};

// =============================================================================
// Track
// =============================================================================

/// An animation track for a single property of an entity.
///
/// # Invariant
/// Keyframes are always sorted by time and have strictly increasing times.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    /// The entity this track animates.
    pub target: EntityId,
    /// Which property is being animated.
    pub property: PropertyType,
    /// The keyframes in this track (sorted by time).
    keyframes: Vec<Keyframe>,
}

/// Maximum keyframes per track (Criterio C: Presupuesto).
pub const MAX_KEYFRAMES_PER_TRACK: usize = 256;

impl Track {
    /// Creates a new empty track.
    pub fn new(target: EntityId, property: PropertyType) -> Self {
        Self {
            target,
            property,
            keyframes: Vec::new(),
        }
    }

    /// Returns the number of keyframes.
    pub fn len(&self) -> usize {
        self.keyframes.len()
    }

    /// Returns true if the track has no keyframes.
    pub fn is_empty(&self) -> bool {
        self.keyframes.is_empty()
    }

    /// Adds a keyframe, maintaining sorted order.
    ///
    /// # Errors
    /// Returns `Err` if a keyframe at the same time already exists (violates $t_i < t_{i+1}$).
    pub fn add_keyframe(&mut self, kf: Keyframe) -> Result<(), TimelineError> {
        if self.keyframes.len() >= MAX_KEYFRAMES_PER_TRACK {
            return Err(TimelineError::KeyframeLimitExceeded);
        }

        // Check for duplicate time
        if self.keyframes.iter().any(|k| k.time == kf.time) {
            return Err(TimelineError::DuplicateKeyframeTime { time: kf.time });
        }

        // Insert in sorted position
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
        Ok(())
    }

    /// Removes a keyframe at the given time.
    pub fn remove_keyframe(&mut self, time: u32) -> bool {
        if let Some(pos) = self.keyframes.iter().position(|k| k.time == time) {
            self.keyframes.remove(pos);
            true
        } else {
            false
        }
    }

    /// Evaluates the track at a given time (in ticks).
    ///
    /// Returns `None` if the track has no keyframes.
    pub fn evaluate(&self, time: u32) -> Option<i32> {
        if self.keyframes.is_empty() {
            return None;
        }

        // Before first keyframe
        if time <= self.keyframes[0].time {
            return Some(self.keyframes[0].value);
        }

        // After last keyframe
        let last = self.keyframes.last().unwrap();
        if time >= last.time {
            return Some(last.value);
        }

        // Find the segment we're in
        for i in 0..self.keyframes.len() - 1 {
            let k0 = &self.keyframes[i];
            let k1 = &self.keyframes[i + 1];
            if time >= k0.time && time < k1.time {
                return Some(self.interpolate(k0, k1, time));
            }
        }

        Some(last.value)
    }

    /// Interpolates between two keyframes.
    fn interpolate(&self, k0: &Keyframe, k1: &Keyframe, time: u32) -> i32 {
        let duration = k1.time - k0.time;
        if duration == 0 {
            return k1.value;
        }

        // Calculate normalized time t in [0, 1] using fixed-point
        let elapsed = time - k0.time;
        let t_raw = ((elapsed as i64) << Fixed::FRAC_BITS) / duration as i64;
        let t = Fixed::from_raw(t_raw as i32);

        // Apply easing (uses the easing of the END keyframe)
        let eased_t = k1.easing.apply(t);

        // Lerp the values
        let v0 = Fixed::from_int(k0.value);
        let v1 = Fixed::from_int(k1.value);
        Fixed::lerp(v0, v1, eased_t).to_int()
    }

    /// Returns an iterator over keyframes.
    pub fn keyframes(&self) -> impl Iterator<Item = &Keyframe> {
        self.keyframes.iter()
    }

    /// Returns the start time (first keyframe).
    pub fn start_time(&self) -> Option<u32> {
        self.keyframes.first().map(|k| k.time)
    }

    /// Returns the end time (last keyframe).
    pub fn end_time(&self) -> Option<u32> {
        self.keyframes.last().map(|k| k.time)
    }
}

// =============================================================================
// Timeline
// =============================================================================

/// A collection of tracks for a scene.
///
/// The timeline holds multiple animation tracks that can be evaluated together.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Timeline {
    /// All tracks in this timeline.
    tracks: Vec<Track>,
    /// Current playback position in ticks.
    current_time: u32,
    /// Ticks per second (for converting from real time).
    pub ticks_per_second: u32,
}

/// Maximum tracks per timeline (Criterio C: Presupuesto).
pub const MAX_TRACKS: usize = 512;

impl Timeline {
    /// Creates a new empty timeline.
    pub fn new(ticks_per_second: u32) -> Self {
        Self {
            tracks: Vec::new(),
            current_time: 0,
            ticks_per_second,
        }
    }

    /// Returns the number of tracks.
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Adds a track to the timeline.
    pub fn add_track(&mut self, track: Track) -> Result<(), TimelineError> {
        if self.tracks.len() >= MAX_TRACKS {
            return Err(TimelineError::TrackLimitExceeded);
        }
        self.tracks.push(track);
        Ok(())
    }

    /// Gets a track by index.
    pub fn get_track(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    /// Gets a mutable track by index.
    pub fn get_track_mut(&mut self, index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(index)
    }

    /// Finds a track for a specific entity and property.
    pub fn find_track(&self, target: EntityId, property: PropertyType) -> Option<&Track> {
        self.tracks
            .iter()
            .find(|t| t.target == target && t.property == property)
    }

    /// Finds or creates a track for a specific entity and property.
    pub fn get_or_create_track(
        &mut self,
        target: EntityId,
        property: PropertyType,
    ) -> Result<&mut Track, TimelineError> {
        // Find existing
        if let Some(pos) = self
            .tracks
            .iter()
            .position(|t| t.target == target && t.property == property)
        {
            return Ok(&mut self.tracks[pos]);
        }

        // Create new
        if self.tracks.len() >= MAX_TRACKS {
            return Err(TimelineError::TrackLimitExceeded);
        }
        self.tracks.push(Track::new(target, property));
        Ok(self.tracks.last_mut().unwrap())
    }

    /// Advances the timeline by delta ticks.
    pub fn advance(&mut self, delta_ticks: u32) {
        self.current_time = self.current_time.saturating_add(delta_ticks);
    }

    /// Sets the current time directly (for scrubbing).
    pub fn seek(&mut self, time: u32) {
        self.current_time = time;
    }

    /// Returns the current time in ticks.
    pub fn current_time(&self) -> u32 {
        self.current_time
    }

    /// Evaluates all tracks at the current time.
    /// Returns a list of (EntityId, PropertyType, value) tuples.
    pub fn evaluate(&self) -> Vec<(EntityId, PropertyType, i32)> {
        self.tracks
            .iter()
            .filter_map(|track| {
                track
                    .evaluate(self.current_time)
                    .map(|value| (track.target, track.property, value))
            })
            .collect()
    }

    /// Evaluates all tracks at a specific time.
    pub fn evaluate_at(&self, time: u32) -> Vec<(EntityId, PropertyType, i32)> {
        self.tracks
            .iter()
            .filter_map(|track| {
                track
                    .evaluate(time)
                    .map(|value| (track.target, track.property, value))
            })
            .collect()
    }

    /// Returns the total duration of the timeline (end of last keyframe).
    pub fn duration(&self) -> u32 {
        self.tracks
            .iter()
            .filter_map(|t| t.end_time())
            .max()
            .unwrap_or(0)
    }

    /// Clears all tracks.
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current_time = 0;
    }

    /// Returns an iterator over all tracks.
    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.tracks.iter()
    }
}

// =============================================================================
// Errors
// =============================================================================

use miette::Diagnostic;

/// Errors that can occur in the timeline system.
///
/// # Diagnostics
/// All errors include diagnostic codes for tooling integration.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, Diagnostic)]
#[allow(unused_assignments)]
pub enum TimelineError {
    #[error("keyframe limit exceeded (max: {MAX_KEYFRAMES_PER_TRACK})")]
    #[diagnostic(
        code(vn::timeline::keyframe_limit),
        help("Consider splitting the track or removing unused keyframes")
    )]
    KeyframeLimitExceeded,

    #[error("duplicate keyframe time: {time}")]
    #[diagnostic(
        code(vn::timeline::duplicate_time),
        help("Keyframe times must be strictly increasing (t_i < t_{{i+1}})")
    )]
    DuplicateKeyframeTime { time: u32 },

    #[error("track limit exceeded (max: {MAX_TRACKS})")]
    #[diagnostic(
        code(vn::timeline::track_limit),
        help("Consider consolidating tracks or removing unused animations")
    )]
    TrackLimitExceeded,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[path = "tests/timeline_tests.rs"]
mod tests;
