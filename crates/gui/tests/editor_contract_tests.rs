pub use visual_novel_gui::editor;

macro_rules! editor_contract_cases {
    ($file:literal) => {
        mod cases {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/editor_contract/",
                $file
            ));
        }
    };
}

mod compiler_tests {
    pub use crate::editor::compiler::*;
    editor_contract_cases!("compiler_tests.rs");
}

mod node_editor_tests {
    pub use crate::editor::node_editor::*;
    pub use crate::editor::{NodeGraph, UndoStack};
    editor_contract_cases!("node_editor_tests.rs");
}

mod node_graph_tests {
    pub use crate::editor::node_graph::*;
    pub use crate::editor::node_types::*;
    pub use eframe::egui;
    editor_contract_cases!("node_graph_tests.rs");
}

mod node_graph_scene_profile_tests {
    pub use crate::editor::node_graph::*;
    pub use crate::editor::node_types::*;
    pub use eframe::egui;
    editor_contract_cases!("node_graph_scene_profile_tests.rs");
}

mod node_graph_interaction_tests {
    pub use crate::editor::node_graph::*;
    pub use crate::editor::node_types::*;
    pub use eframe::egui;
    editor_contract_cases!("node_graph_interaction_tests.rs");
}

mod node_graph_fragment_view_tests {
    pub use crate::editor::node_graph::*;
    pub use crate::editor::node_types::*;
    pub use eframe::egui;
    editor_contract_cases!("node_graph_fragment_view_tests.rs");
}

mod player_ui_tests {
    pub use crate::editor::player_ui::*;
    editor_contract_cases!("player_ui_tests.rs");
}

mod quick_fix_tests {
    pub use crate::editor::quick_fix::*;
    pub use crate::editor::{LintIssue, NodeGraph};
    editor_contract_cases!("quick_fix_tests.rs");
}

mod validator_tests {
    pub use crate::editor::validator::*;
    editor_contract_cases!("validator_tests.rs");
}

mod asset_browser_tests {
    pub use crate::editor::asset_browser::*;
    pub use crate::editor::image_asset_cache::*;
    editor_contract_cases!("asset_browser_tests.rs");
}

mod asset_candidates_tests {
    pub use crate::editor::asset_candidates::*;
    editor_contract_cases!("asset_candidates_tests.rs");
}

mod atomic_io_tests {
    pub use crate::editor::atomic_io::*;
    editor_contract_cases!("atomic_io_tests.rs");
}

mod node_rendering_tests {
    pub use crate::editor::node_rendering::*;
    pub use crate::editor::{NodeGraph, StoryNode};
    editor_contract_cases!("node_rendering_tests.rs");
}

mod scene_stage_tests {
    pub use std::collections::HashMap;

    pub use crate::editor::image_asset_cache::*;
    pub use crate::editor::scene_stage::*;
    pub use visual_novel_engine::EntityKind;
    editor_contract_cases!("scene_stage_tests.rs");
}

mod visual_composer_tests {
    pub use crate::editor::visual_composer::*;
    editor_contract_cases!("visual_composer_tests.rs");
}

mod visual_composer_overlays_tests {
    pub use std::collections::HashMap;

    pub use crate::editor::visual_composer::overlays::*;
    pub use crate::editor::visual_composer::*;
    pub use crate::editor::{ComposerPreviewMode, StoryNode};
    pub use visual_novel_engine::runtime::EventCompiled;
    editor_contract_cases!("visual_composer_overlays_tests.rs");
}

mod workbench_layout_tests {
    pub use crate::editor::workbench::layout::*;
    pub use crate::editor::workbench::LayoutOverrides;
    editor_contract_cases!("workbench_layout_tests.rs");
}

mod workbench_tests {
    pub use crate::editor::workbench::*;
    pub use crate::editor::*;
    pub use visual_novel_gui::VnConfig;
    editor_contract_cases!("workbench_tests.rs");
}
