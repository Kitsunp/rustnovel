//! Entity-based scene state for the Visual Novel Engine.
//!
//! This module implements a "Simple Entity List" architecture as specified in the
//! implementation plan. It provides extensibility without full ECS complexity.
//!
//! # Contracts
//! - **Postcondition**: Entity IDs are unique within a SceneState.
//! - **Invariant**: Entities are always processed in deterministic order (by z_order, then id).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::event::SharedStr;

// -----------------------------------------------------------------------------
// EntityId
// -----------------------------------------------------------------------------

/// Unique identifier for an entity within a scene.
///
/// In the authoring layer, this may map to a UUID. In the execution layer,
/// this is a compact u32 for performance and determinism.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct EntityId(u32);

impl EntityId {
    /// Creates a new EntityId from a raw u32.
    #[inline]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({})", self.0)
    }
}

// -----------------------------------------------------------------------------
// Transform
// -----------------------------------------------------------------------------

/// 2D transform for an entity.
///
/// Uses integer coordinates for deterministic positioning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transform {
    /// X position in logical pixels.
    pub x: i32,
    /// Y position in logical pixels.
    pub y: i32,
    /// Z-order for rendering. Higher values are drawn on top.
    pub z_order: i32,
    /// Scale in fixed-point (1000 = 1.0x).
    pub scale: u32,
    /// Opacity in fixed-point (0-1000, where 1000 = fully opaque).
    pub opacity: u32,
}

impl Transform {
    /// Creates a new transform at the origin with default scale and opacity.
    pub const fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            z_order: 0,
            scale: 1000,
            opacity: 1000,
        }
    }

    /// Creates a transform at the given position.
    pub const fn at(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            z_order: 0,
            scale: 1000,
            opacity: 1000,
        }
    }
}

// -----------------------------------------------------------------------------
// EntityKind
// -----------------------------------------------------------------------------

/// The kind of content an entity represents.
///
/// This enum follows the "Simple Entity List" pattern: each variant contains
/// its own data directly, avoiding complex trait objects or dynamic dispatch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityKind {
    /// A background or foreground image.
    Image(ImageData),
    /// A text label or dialogue box.
    Text(TextData),
    /// A character sprite with expressions.
    Character(CharacterData),
    /// A video clip descriptor for runtime playback surfaces.
    ///
    /// Rendering and decode are backend responsibilities; the entity stores
    /// deterministic scene intent (path + looping) for preview/runtime parity.
    Video(VideoData),
    /// An audio source attached to this entity.
    Audio(AudioData),
}

/// Data for an image entity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageData {
    /// Path or asset ID of the image.
    pub path: SharedStr,
    /// Optional tint color (RGBA as u32).
    pub tint: Option<u32>,
}

/// Data for a text entity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextData {
    /// The text content.
    pub content: String,
    /// Font size in logical pixels.
    pub font_size: u32,
    /// Color (RGBA as u32).
    pub color: u32,
}

/// Data for a character sprite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterData {
    /// Character's identifier/name.
    pub name: SharedStr,
    /// Current expression/pose.
    pub expression: Option<SharedStr>,
}

/// Data for a video entity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoData {
    /// Path or asset ID of the video.
    pub path: SharedStr,
    /// Whether the video should loop.
    pub looping: bool,
}

/// Data for an audio source entity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioData {
    /// Path or asset ID of the audio.
    pub path: SharedStr,
    /// Volume (0-1000, fixed-point).
    pub volume: u32,
    /// Whether the audio should loop.
    pub looping: bool,
}

// -----------------------------------------------------------------------------
// Entity
// -----------------------------------------------------------------------------

/// A single entity in the scene.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// Unique identifier.
    pub id: EntityId,
    /// Transform (position, z-order, scale, opacity).
    pub transform: Transform,
    /// The kind of entity and its associated data.
    pub kind: EntityKind,
}

impl Entity {
    /// Creates a new entity with the given id and kind.
    pub fn new(id: EntityId, kind: EntityKind) -> Self {
        Self {
            id,
            transform: Transform::new(),
            kind,
        }
    }

    /// Creates a new entity with a transform.
    pub fn with_transform(id: EntityId, transform: Transform, kind: EntityKind) -> Self {
        Self {
            id,
            transform,
            kind,
        }
    }
}

// -----------------------------------------------------------------------------
// SceneState
// -----------------------------------------------------------------------------

/// The state of all entities in the current scene.
///
/// # Determinism Contract
/// - Entities are always iterated in a deterministic order: by `z_order` ascending,
///   then by `id` ascending.
/// - The `spawn` method assigns monotonically increasing IDs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SceneState {
    /// All entities in the scene.
    entities: Vec<Entity>,
    /// Index for fast lookup by ID.
    #[serde(skip)]
    index: HashMap<EntityId, usize>,
    /// Counter for generating new entity IDs.
    #[serde(skip, default = "default_next_id")]
    next_id: u32,
}

fn default_next_id() -> u32 {
    0
}

/// Maximum number of entities allowed (Criterio C: Presupuesto de Recursos).
pub const MAX_ENTITIES: usize = 1024;

impl SceneState {
    /// Creates a new empty scene state.
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            index: HashMap::new(),
            next_id: 0,
        }
    }

    /// Returns the number of entities.
    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Returns true if there are no entities.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Spawns a new entity with the given kind.
    ///
    /// # Errors
    /// Returns `None` if the entity limit has been reached.
    ///
    /// # Contract
    /// - **Precondition**: `self.len() < MAX_ENTITIES`
    /// - **Postcondition**: The returned `EntityId` is unique within this SceneState.
    pub fn spawn(&mut self, kind: EntityKind) -> Option<EntityId> {
        if self.entities.len() >= MAX_ENTITIES {
            return None;
        }
        let id = EntityId::new(self.next_id);
        self.next_id += 1;
        let entity = Entity::new(id, kind);
        let idx = self.entities.len();
        self.entities.push(entity);
        self.index.insert(id, idx);
        Some(id)
    }

    /// Spawns a new entity with a specific transform.
    pub fn spawn_with_transform(
        &mut self,
        transform: Transform,
        kind: EntityKind,
    ) -> Option<EntityId> {
        if self.entities.len() >= MAX_ENTITIES {
            return None;
        }
        let id = EntityId::new(self.next_id);
        self.next_id += 1;
        let entity = Entity::with_transform(id, transform, kind);
        let idx = self.entities.len();
        self.entities.push(entity);
        self.index.insert(id, idx);
        Some(id)
    }

    /// Despawns an entity by ID.
    ///
    /// # Contract
    /// - **Postcondition**: The entity with `id` no longer exists in the scene.
    /// - **Note**: Uses swap_remove for O(1) removal, so iteration order may change.
    ///   For deterministic iteration, always use `iter_sorted()`.
    pub fn despawn(&mut self, id: EntityId) -> bool {
        if let Some(idx) = self.index.remove(&id) {
            self.entities.swap_remove(idx);
            // Update the index for the swapped entity (if any)
            if idx < self.entities.len() {
                let swapped_id = self.entities[idx].id;
                self.index.insert(swapped_id, idx);
            }
            true
        } else {
            false
        }
    }

    /// Gets a reference to an entity by ID.
    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.index.get(&id).map(|&idx| &self.entities[idx])
    }

    /// Gets a mutable reference to an entity by ID.
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.index
            .get(&id)
            .copied()
            .map(|idx| &mut self.entities[idx])
    }

    /// Iterates over all entities in deterministic order.
    ///
    /// **Order**: Sorted by (z_order, id) ascending.
    /// This ensures bit-exact reproduction regardless of internal Vec order.
    pub fn iter_sorted(&self) -> impl Iterator<Item = &Entity> {
        let mut sorted: Vec<_> = self.entities.iter().collect();
        sorted.sort_by(|a, b| {
            a.transform
                .z_order
                .cmp(&b.transform.z_order)
                .then_with(|| a.id.cmp(&b.id))
        });
        sorted.into_iter()
    }

    /// Iterates over all entities in insertion order (non-deterministic after despawn).
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.entities.iter()
    }

    /// Iterates mutably over all entities.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entity> {
        self.entities.iter_mut()
    }

    /// Clears all entities from the scene.
    pub fn clear(&mut self) {
        self.entities.clear();
        self.index.clear();
    }

    /// Rebuilds the internal index after deserialization.
    pub fn rebuild_index(&mut self) {
        self.index.clear();
        for (idx, entity) in self.entities.iter().enumerate() {
            self.index.insert(entity.id, idx);
        }
        // Restore next_id to max + 1
        self.next_id = self
            .entities
            .iter()
            .map(|e| e.id.raw())
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
#[path = "tests/entity_tests.rs"]
mod tests;
