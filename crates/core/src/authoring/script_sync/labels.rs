use std::collections::BTreeMap;

use super::super::NodeGraph;

pub(super) fn append_fragment_labels(
    graph: &NodeGraph,
    node_event_indices: &BTreeMap<u32, usize>,
    labels: &mut BTreeMap<String, usize>,
) {
    for (fragment_id, fragment) in graph.fragments() {
        let fragment_token = stable_label_token(fragment_id);
        for node_id in &fragment.node_ids {
            let Some(event_idx) = node_event_indices.get(node_id).copied() else {
                continue;
            };
            insert_unique_label(
                labels,
                format!("fragment_{fragment_token}_node_{node_id}"),
                event_idx,
            );
        }
        for port in fragment.inputs.iter().chain(fragment.outputs.iter()) {
            let Some(node_id) = port.node_id else {
                continue;
            };
            let Some(event_idx) = node_event_indices.get(&node_id).copied() else {
                continue;
            };
            let port_token = stable_label_token(&port.port_id);
            insert_unique_label(
                labels,
                format!("fragment_{fragment_token}_port_{port_token}"),
                event_idx,
            );
        }
    }
}

fn insert_unique_label(labels: &mut BTreeMap<String, usize>, label: String, event_idx: usize) {
    if let std::collections::btree_map::Entry::Vacant(entry) = labels.entry(label.clone()) {
        entry.insert(event_idx);
        return;
    }
    let mut counter = 1usize;
    loop {
        let candidate = format!("{label}_{counter}");
        if let std::collections::btree_map::Entry::Vacant(entry) = labels.entry(candidate) {
            entry.insert(event_idx);
            return;
        }
        counter += 1;
    }
}

fn stable_label_token(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let token = out.trim_matches('_');
    if token.is_empty() {
        "fragment".to_string()
    } else {
        token.to_string()
    }
}
