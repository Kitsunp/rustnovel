use super::EditorWorkbench;
use crate::editor::compiler::CompilationResult;
pub use visual_novel_engine::authoring::CompilationCache;

impl EditorWorkbench {
    pub fn compile_current_graph(&mut self) -> CompilationResult {
        let drag_error = self.node_graph.try_commit_drag_positions().err();
        if drag_error.is_none() {
            self.record_pending_editor_operation();
        }
        let project_root = self.project_root.clone();
        let mut result = self
            .compilation_cache
            .get_or_compile(self.node_graph.authoring_graph(), project_root.as_deref());
        if let Some(error) = drag_error {
            let message = format!("Could not commit dragged nodes: {error}");
            result.engine_result = Err(message.clone());
            result.issues.push(crate::editor::LintIssue::error(
                None,
                crate::editor::ValidationPhase::Graph,
                crate::editor::LintCode::CompileError,
                message,
            ));
        }
        result
    }

    pub fn compilation_cache_stats(&self) -> (usize, usize) {
        self.compilation_cache.stats()
    }
}
