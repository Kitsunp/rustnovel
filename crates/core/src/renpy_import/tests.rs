use super::import_renpy_project;
use super::syntax::{
    parse_cond_expr, parse_dialogue_line, parse_menu_option_decl, parse_show_decl,
};
use super::{ImportProfile, ImportRenpyOptions};
use crate::{CmpOp, CondRaw, EventRaw, ScriptRaw};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

pub(crate) fn temp_renpy_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");
    let output_root = dir.path().join("out_project");
    (dir, project_root, game_dir, output_root)
}

pub(crate) fn write_renpy_file(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write script");
}

#[path = "tests_import_core.rs"]
mod tests_import_core;
#[path = "tests_parse.rs"]
mod tests_parse;
#[path = "tests_traceability.rs"]
mod tests_traceability;
