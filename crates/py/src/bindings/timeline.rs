//! Python bindings for the timeline system.
//!
//! These bindings expose the timeline, track, and keyframe types for
//! use in Python-based visual editors and tools.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use visual_novel_engine::{Easing, EntityId, Keyframe, PropertyType, Timeline, Track};

/// Converts a string to an Easing enum.
fn parse_easing(s: &str) -> Result<Easing, String> {
    match s.to_lowercase().as_str() {
        "linear" => Ok(Easing::Linear),
        "ease_in" | "easein" => Ok(Easing::EaseIn),
        "ease_out" | "easeout" => Ok(Easing::EaseOut),
        "ease_in_out" | "easeinout" => Ok(Easing::EaseInOut),
        "step" => Ok(Easing::Step),
        _ => Err(format!(
            "unknown easing '{s}'; expected one of: linear, ease_in, ease_out, ease_in_out, step"
        )),
    }
}

/// Converts an Easing enum to a string.
fn easing_to_string(e: Easing) -> &'static str {
    match e {
        Easing::Linear => "linear",
        Easing::EaseIn => "ease_in",
        Easing::EaseOut => "ease_out",
        Easing::EaseInOut => "ease_in_out",
        Easing::Step => "step",
    }
}

/// Converts a string to a PropertyType enum.
fn parse_property(s: &str) -> Result<PropertyType, String> {
    match s.to_lowercase().as_str() {
        "position_x" | "positionx" | "x" => Ok(PropertyType::PositionX),
        "position_y" | "positiony" | "y" => Ok(PropertyType::PositionY),
        "z_order" | "zorder" | "z" => Ok(PropertyType::ZOrder),
        "scale" => Ok(PropertyType::Scale),
        "opacity" | "alpha" => Ok(PropertyType::Opacity),
        "rotation" => Ok(PropertyType::Rotation),
        _ => Err(format!(
            "unknown property '{s}'; expected one of: position_x, position_y, z_order, scale, opacity, rotation"
        )),
    }
}

/// Python wrapper for a Keyframe.
#[pyclass(name = "Keyframe")]
#[derive(Clone)]
pub struct PyKeyframe {
    #[pyo3(get, set)]
    pub time: u32,
    #[pyo3(get, set)]
    pub value: i32,
    easing: Easing,
}

#[pymethods]
impl PyKeyframe {
    #[new]
    #[pyo3(signature = (time, value, easing=None))]
    fn new(time: u32, value: i32, easing: Option<String>) -> PyResult<Self> {
        Ok(Self {
            time,
            value,
            easing: match easing {
                Some(s) => parse_easing(&s).map_err(PyValueError::new_err)?,
                None => Easing::Linear,
            },
        })
    }

    /// Gets the easing function as a string.
    #[getter]
    fn easing(&self) -> &'static str {
        easing_to_string(self.easing)
    }

    /// Sets the easing function from a string.
    #[setter]
    fn set_easing(&mut self, easing: String) -> PyResult<()> {
        self.easing = parse_easing(&easing).map_err(PyValueError::new_err)?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "Keyframe(time={}, value={}, easing='{}')",
            self.time,
            self.value,
            easing_to_string(self.easing)
        )
    }
}

impl From<&Keyframe> for PyKeyframe {
    fn from(kf: &Keyframe) -> Self {
        Self {
            time: kf.time,
            value: kf.value,
            easing: kf.easing,
        }
    }
}

impl From<&PyKeyframe> for Keyframe {
    fn from(kf: &PyKeyframe) -> Self {
        Self::new(kf.time, kf.value, kf.easing)
    }
}

/// Python wrapper for a Track.
#[pyclass(name = "Track")]
pub struct PyTrack {
    inner: Track,
}

#[pymethods]
impl PyTrack {
    #[new]
    fn new(entity_id: u32, property: String) -> PyResult<Self> {
        Ok(Self {
            inner: Track::new(
                EntityId::new(entity_id),
                parse_property(&property).map_err(PyValueError::new_err)?,
            ),
        })
    }

    /// Adds a keyframe to the track.
    fn add_keyframe(&mut self, kf: &PyKeyframe) -> PyResult<()> {
        self.inner
            .add_keyframe(kf.into())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    /// Removes a keyframe at the given time.
    fn remove_keyframe(&mut self, time: u32) -> bool {
        self.inner.remove_keyframe(time)
    }

    /// Evaluates the track at the given time.
    fn evaluate(&self, time: u32) -> Option<i32> {
        self.inner.evaluate(time)
    }

    /// Returns the number of keyframes.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Returns True if the track has no keyframes.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns all keyframes as a list.
    fn keyframes(&self) -> Vec<PyKeyframe> {
        self.inner.keyframes().map(PyKeyframe::from).collect()
    }

    /// Returns the target entity ID.
    #[getter]
    fn entity_id(&self) -> u32 {
        self.inner.target.raw()
    }

    /// Returns the property name.
    #[getter]
    fn property(&self) -> String {
        format!("{:?}", self.inner.property)
    }

    fn __repr__(&self) -> String {
        format!(
            "Track(entity={}, property='{:?}', keyframes={})",
            self.inner.target.raw(),
            self.inner.property,
            self.inner.len()
        )
    }
}

/// Python wrapper for a Timeline.
#[pyclass(name = "Timeline")]
pub struct PyTimeline {
    inner: Timeline,
}

#[pymethods]
impl PyTimeline {
    #[new]
    #[pyo3(signature = (ticks_per_second=None))]
    fn new(ticks_per_second: Option<u32>) -> Self {
        Self {
            inner: Timeline::new(ticks_per_second.unwrap_or(60)),
        }
    }

    /// Adds a track to the timeline.
    fn add_track(&mut self, track: &PyTrack) -> PyResult<()> {
        self.inner
            .add_track(track.inner.clone())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    /// Returns the number of tracks.
    fn __len__(&self) -> usize {
        self.inner.track_count()
    }

    /// Seeks to a specific time (for scrubbing).
    fn seek(&mut self, time: u32) {
        self.inner.seek(time)
    }

    /// Advances the timeline by delta ticks.
    fn advance(&mut self, delta_ticks: u32) {
        self.inner.advance(delta_ticks)
    }

    /// Returns the current time in ticks.
    #[getter]
    fn current_time(&self) -> u32 {
        self.inner.current_time()
    }

    /// Returns the total duration of the timeline.
    #[getter]
    fn duration(&self) -> u32 {
        self.inner.duration()
    }

    /// Ticks per second.
    #[getter]
    fn ticks_per_second(&self) -> u32 {
        self.inner.ticks_per_second
    }

    /// Evaluates all tracks at the current time.
    /// Returns a list of (entity_id, property_name, value) tuples.
    fn evaluate(&self) -> Vec<(u32, String, i32)> {
        self.inner
            .evaluate()
            .into_iter()
            .map(|(id, prop, val)| (id.raw(), format!("{:?}", prop), val))
            .collect()
    }

    /// Evaluates all tracks at a specific time.
    fn evaluate_at(&self, time: u32) -> Vec<(u32, String, i32)> {
        self.inner
            .evaluate_at(time)
            .into_iter()
            .map(|(id, prop, val)| (id.raw(), format!("{:?}", prop), val))
            .collect()
    }

    /// Clears all tracks.
    fn clear(&mut self) {
        self.inner.clear()
    }

    fn __repr__(&self) -> String {
        format!(
            "Timeline(tracks={}, time={}, duration={})",
            self.inner.track_count(),
            self.inner.current_time(),
            self.inner.duration()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_easing_rejects_unknown_tokens() {
        let err = parse_easing("ease_sideways")
            .expect_err("unknown easing tokens must be rejected instead of becoming linear");

        assert!(err.to_string().contains("unknown easing"));
    }

    #[test]
    fn parse_easing_accepts_documented_aliases() {
        assert_eq!(parse_easing("easein").unwrap(), Easing::EaseIn);
        assert_eq!(parse_easing("ease_in_out").unwrap(), Easing::EaseInOut);
    }

    #[test]
    fn parse_property_rejects_unknown_tokens() {
        let err = parse_property("blur_radius")
            .expect_err("unknown property tokens must be rejected instead of becoming position_x");

        assert!(err.to_string().contains("unknown property"));
    }

    #[test]
    fn parse_property_accepts_documented_aliases() {
        assert_eq!(parse_property("x").unwrap(), PropertyType::PositionX);
        assert_eq!(parse_property("alpha").unwrap(), PropertyType::Opacity);
    }
}
