use super::EditorWorkbench;
use crate::editor::compiler::CompilationResult;
pub use visual_novel_engine::authoring::CompilationCache;

impl EditorWorkbench {
    pub fn compile_current_graph(&mut self) -> CompilationResult {
        self.node_graph.commit_drag_positions();
        self.record_pending_editor_operation();
        let project_root = self.project_root.clone();
        self.compilation_cache
            .get_or_compile(self.node_graph.authoring_graph(), project_root.as_deref())
    }

    pub fn compilation_cache_stats(&self) -> (usize, usize) {
        self.compilation_cache.stats()
    }
}
