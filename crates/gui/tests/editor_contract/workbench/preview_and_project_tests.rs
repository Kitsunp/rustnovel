use std::fs;

use super::super::*;
use crate::editor::StoryNode;
use tempfile::tempdir;

#[path = "preview_and_project_tests/composer_audio_owner.rs"]
mod composer_audio_owner;
#[path = "preview_and_project_tests/project_loading.rs"]
mod project_loading;
#[path = "preview_and_project_tests/scene_preview.rs"]
mod scene_preview;
