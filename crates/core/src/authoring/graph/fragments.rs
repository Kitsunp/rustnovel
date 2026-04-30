use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentPort {
    pub port_id: String,
    pub label: String,
    pub node_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphFragment {
    pub fragment_id: String,
    pub title: String,
    pub node_ids: Vec<u32>,
    #[serde(default)]
    pub inputs: Vec<FragmentPort>,
    #[serde(default)]
    pub outputs: Vec<FragmentPort>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalNode {
    pub fragment_id: String,
    pub port_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionHub {
    pub hub_id: String,
    pub option_ports: Vec<FragmentPort>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphStack {
    pub active_fragment: Option<String>,
    #[serde(default)]
    pub breadcrumb: Vec<String>,
}
