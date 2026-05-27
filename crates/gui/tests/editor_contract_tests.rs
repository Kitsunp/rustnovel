pub use visual_novel_gui::editor;

mod compiler_tests {
    pub use crate::editor::compiler::*;
    #[path = "../editor_contract/compiler_tests.rs"]
    mod cases;
}

mod node_editor_tests {
    pub use crate::editor::node_editor::*;
    pub use crate::editor::{NodeGraph, UndoStack};
    #[path = "../editor_contract/node_editor_tests.rs"]
    mod cases;
}

mod node_graph_tests {
    pub use crate::editor::node_graph::*;
    pub use crate::editor::node_types::*;
    pub use eframe::egui;
    #[path = "../editor_contract/node_graph_tests.rs"]
    mod cases;
}

mod node_graph_scene_profile_tests {
    pub use crate::editor::node_graph::*;
    pub use crate::editor::node_types::*;
    pub use eframe::egui;
    #[path = "../editor_contract/node_graph_scene_profile_tests.rs"]
    mod cases;
}

mod node_graph_interaction_tests {
    pub use crate::editor::node_graph::*;
    pub use crate::editor::node_types::*;
    pub use eframe::egui;
    #[path = "../editor_contract/node_graph_interaction_tests.rs"]
    mod cases;
}

mod node_graph_fragment_view_tests {
    pub use crate::editor::node_graph::*;
    pub use crate::editor::node_types::*;
    pub use eframe::egui;
    #[path = "../editor_contract/node_graph_fragment_view_tests.rs"]
    mod cases;
}

mod player_ui_tests {
    pub use crate::editor::player_ui::*;
    #[path = "../editor_contract/player_ui_tests.rs"]
    mod cases;
}

mod quick_fix_tests {
    pub use crate::editor::quick_fix::*;
    pub use crate::editor::{LintIssue, NodeGraph};
    #[path = "../editor_contract/quick_fix_tests.rs"]
    mod cases;
}

mod validator_tests {
    pub use crate::editor::validator::*;
    #[path = "../editor_contract/validator_tests.rs"]
    mod cases;
}

mod asset_browser_tests {
    pub use crate::editor::asset_browser::*;
    pub use crate::editor::image_asset_cache::*;
    #[path = "../editor_contract/asset_browser_tests.rs"]
    mod cases;
}

mod node_rendering_tests {
    pub use crate::editor::node_rendering::*;
    pub use crate::editor::{NodeGraph, StoryNode};
    #[path = "../editor_contract/node_rendering_tests.rs"]
    mod cases;
}

mod scene_stage_tests {
    pub use std::collections::HashMap;

    pub use crate::editor::image_asset_cache::*;
    pub use crate::editor::scene_stage::*;
    pub use visual_novel_engine::EntityKind;
    #[path = "../editor_contract/scene_stage_tests.rs"]
    mod cases;
}

mod visual_composer_tests {
    pub use crate::editor::visual_composer::*;
    #[path = "../editor_contract/visual_composer_tests.rs"]
    mod cases;
}

mod visual_composer_overlays_tests {
    pub use std::collections::HashMap;

    pub use crate::editor::visual_composer::overlays::*;
    pub use crate::editor::visual_composer::*;
    pub use crate::editor::{ComposerPreviewMode, StoryNode};
    pub use visual_novel_engine::runtime::EventCompiled;
    #[path = "../editor_contract/visual_composer_overlays_tests.rs"]
    mod cases;
}

mod workbench_layout_tests {
    pub use crate::editor::workbench::layout::*;
    pub use crate::editor::workbench::LayoutOverrides;
    #[path = "../editor_contract/workbench_layout_tests.rs"]
    mod cases;
}

mod workbench_tests {
    pub use crate::editor::workbench::*;
    pub use crate::editor::*;
    pub use visual_novel_gui::VnConfig;
    #[path = "../editor_contract/workbench_tests.rs"]
    mod cases;
}
