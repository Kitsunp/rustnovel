use std::fs;
use std::process::Command;

use tempfile::TempDir;

#[test]
fn repo_report_lists_files_skipped_due_to_read_errors() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("repo");
    fs::create_dir_all(&root).expect("repo dir");
    fs::write(root.join("main.rs"), "fn main() {}\n").expect("source file");
    fs::write(root.join("broken.rs"), [0xff, 0xfe, 0xfd]).expect("invalid utf8 file");
    let report_path = root.join("line_report.md");

    let output = Command::new(env!("CARGO_BIN_EXE_repo_report"))
        .current_dir(&root)
        .arg("--output")
        .arg(&report_path)
        .output()
        .expect("run repo_report");

    assert!(
        output.status.success(),
        "repo_report should succeed while reporting skipped files: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fs::read_to_string(report_path).expect("report");
    assert!(
        report.contains("## Archivos omitidos"),
        "report should expose skipped files instead of silently dropping them:\n{report}"
    );
    assert!(
        report.contains("`broken.rs`"),
        "report should identify the skipped file:\n{report}"
    );
}
