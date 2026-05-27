use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{tempdir, TempDir};
use visual_novel_engine::{
    import_renpy_project,
    runtime::{CmpOp, CondRaw, EventRaw, ScriptRaw},
    ImportFallbackPolicy, ImportProfile, ImportRenpyOptions,
};

mod event {
    pub use visual_novel_engine::runtime::{
        AudioActionRaw, CharacterPlacementRaw, CmpOp, CondRaw, DialogueRaw, ScenePatchRaw,
        SceneTransitionRaw, SceneUpdateRaw,
    };
}

#[path = "../src/renpy_import/syntax.rs"]
#[allow(dead_code)]
mod syntax;

use syntax::{
    parse_assignment_decl, parse_cond_expr, parse_dialogue_line, parse_menu_caption_line,
    parse_menu_option_decl, parse_show_decl, AssignmentValue,
};

fn temp_renpy_fixture() -> (TempDir, PathBuf, PathBuf, PathBuf) {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");
    let output_root = dir.path().join("out_project");
    (dir, project_root, game_dir, output_root)
}

fn write_renpy_file(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write script");
}

#[path = "renpy_import_contract/asset_resolution.rs"]
mod asset_resolution;
#[path = "renpy_import_contract/import_core.rs"]
mod import_core;
#[path = "renpy_import_contract/parse.rs"]
mod parse;
#[path = "renpy_import_contract/profile_and_security.rs"]
mod profile_and_security;
#[path = "renpy_import_contract/traceability.rs"]
mod traceability;
