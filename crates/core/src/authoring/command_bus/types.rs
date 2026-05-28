use serde::Serialize;

use super::super::{
    AuthoringPosition, GraphConnection, GraphFragment, GraphStack, LintIssue, OperationLogEntry,
    StoryNode, VerificationRun,
};

#[derive(Clone, Debug)]
pub enum AuthoringCommand {
    CreateNode {
        node_id: u32,
        node: StoryNode,
        position: AuthoringPosition,
    },
    RemoveNode {
        node_id: u32,
    },
    Connect {
        from: u32,
        from_port: usize,
        to: u32,
    },
    Disconnect {
        from: u32,
        from_port: usize,
    },
    ConnectNewChoiceOption {
        choice_id: u32,
        to: u32,
        text: String,
    },
    SetChoiceOptionTarget {
        node_id: u32,
        option_index: usize,
        target_node_id: Option<u32>,
    },
    ConnectOrBranch {
        from: u32,
        from_port: usize,
        to: u32,
        branch_position: AuthoringPosition,
    },
    EditNode {
        node_id: u32,
        replacement: StoryNode,
    },
    EditDialogue {
        node_id: u32,
        speaker: String,
        text: String,
    },
    EditChoicePrompt {
        node_id: u32,
        prompt: String,
    },
    EditChoiceOptionText {
        node_id: u32,
        option_index: usize,
        text: String,
    },
    ReorderChoiceOption {
        node_id: u32,
        from_index: usize,
        to_index: usize,
    },
    RemoveChoiceOption {
        node_id: u32,
        option_index: usize,
    },
    CreateFragment {
        fragment_id: String,
        title: String,
        node_ids: Vec<u32>,
    },
    RemoveFragment {
        fragment_id: String,
    },
    EnterFragment {
        fragment_id: String,
    },
    LeaveFragment,
    RefreshFragmentPorts {
        fragment_id: String,
    },
    ApplyQuickFix {
        issue: Box<LintIssue>,
        fix_id: String,
    },
    ImportAsset {
        path: String,
    },
    MoveLayer {
        object_id: String,
        x: i32,
        y: i32,
        scale: Option<f32>,
    },
    RevertLast,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum AuthoringDelta {
    NodeCreated {
        node_id: u32,
        node: StoryNode,
        position: AuthoringPosition,
    },
    NodeRemoved {
        node_id: u32,
        node: StoryNode,
        position: AuthoringPosition,
        connections: Vec<GraphConnection>,
    },
    Connected {
        connection: GraphConnection,
        replaced: Vec<GraphConnection>,
    },
    Disconnected {
        removed: Vec<GraphConnection>,
    },
    ChoiceOptionConnected {
        choice_id: u32,
        option_index: usize,
        before_node: StoryNode,
        connection: GraphConnection,
    },
    ChoiceOptionTargetSet {
        node_id: u32,
        option_index: usize,
        before: Vec<GraphConnection>,
        after: Option<GraphConnection>,
    },
    BranchConnected {
        from: u32,
        from_port: usize,
        to: u32,
    },
    NodeEdited {
        node_id: u32,
        before: StoryNode,
        after: StoryNode,
    },
    FragmentCreated {
        fragment: GraphFragment,
    },
    FragmentRemoved {
        fragment: GraphFragment,
        before_stack: GraphStack,
    },
    FragmentEntered {
        fragment_id: String,
        before_stack: GraphStack,
        after_stack: GraphStack,
    },
    FragmentLeft {
        before_stack: GraphStack,
        after_stack: GraphStack,
    },
    FragmentPortsRefreshed {
        fragment_id: String,
        before: GraphFragment,
        after: GraphFragment,
    },
    ChoiceOptionReordered {
        node_id: u32,
        from_index: usize,
        to_index: usize,
        before_node: StoryNode,
        before_connections: Vec<GraphConnection>,
        after_connections: Vec<GraphConnection>,
    },
    ChoiceOptionRemoved {
        node_id: u32,
        option_index: usize,
        before_node: StoryNode,
        before_connections: Vec<GraphConnection>,
        after_connections: Vec<GraphConnection>,
    },
    QuickFixApplied {
        diagnostic_id: String,
        fix_id: String,
        before_sha256: String,
        after_sha256: String,
    },
    AssetImported {
        path: String,
    },
    LayerMoved {
        object_id: String,
        before: (Option<i32>, Option<i32>, Option<f32>),
        after: (Option<i32>, Option<i32>, Option<f32>),
    },
    Reverted {
        reverted: Box<AuthoringDelta>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthoringCommandOutcome {
    pub delta: AuthoringDelta,
    pub operation: OperationLogEntry,
    pub verification: VerificationRun,
}
