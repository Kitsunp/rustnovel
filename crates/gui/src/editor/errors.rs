//! Error types for the editor module.
//!
//! Uses thiserror for derive and miette for structured diagnostics.

/// Errors that can occur in the editor.
#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum EditorError {
    /// No script file path is set.
    #[error("No file path set. Use 'Save As' first.")]
    #[diagnostic(code(editor::no_path))]
    NoFilePath,

    /// File I/O operation failed.
    #[error("File operation failed: {0}")]
    #[diagnostic(code(editor::io_error))]
    IoError(#[from] std::io::Error),

    /// JSON serialization/deserialization failed.
    #[error("JSON error: {0}")]
    #[diagnostic(code(editor::json_error))]
    JsonError(#[from] serde_json::Error),

    /// Script compilation failed.
    #[error("Script compilation failed: {0}")]
    #[diagnostic(code(editor::compile_error))]
    CompileError(String),

    /// Engine creation failed.
    #[error("Engine error: {0}")]
    #[diagnostic(code(editor::engine_error))]
    EngineError(String),
}
