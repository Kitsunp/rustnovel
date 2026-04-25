//! Example: Story Graph Analysis
//!
//! This example demonstrates how to generate and analyze a story graph
//! from a compiled script, including detecting unreachable nodes.

use visual_novel_engine::{ScriptRaw, StoryGraph};

fn main() {
    println!("=== Story Graph Analysis Example ===\n");

    // Define a sample script with branching
    let script_json = r#"{
        "script_schema_version": "1.0",
        "events": [
            {
                "type": "dialogue",
                "speaker": "Narrator",
                "text": "Welcome to the story!"
            },
            {
                "type": "choice",
                "prompt": "What do you want to do?",
                "options": [
                    { "text": "Go left", "target": "left_path" },
                    { "text": "Go right", "target": "right_path" }
                ]
            },
            {
                "type": "dialogue",
                "speaker": "Narrator",
                "text": "You went left!"
            },
            {
                "type": "jump",
                "target": "ending"
            },
            {
                "type": "dialogue",
                "speaker": "Narrator",
                "text": "You went right!"
            },
            {
                "type": "jump",
                "target": "ending"
            },
            {
                "type": "dialogue",
                "speaker": "Narrator",
                "text": "This is a secret unreachable path!"
            },
            {
                "type": "dialogue",
                "speaker": "Narrator",
                "text": "The End!"
            }
        ],
        "labels": {
            "start": 0,
            "left_path": 2,
            "right_path": 4,
            "secret": 6,
            "ending": 7
        }
    }"#;

    // Parse and compile the script
    let raw = ScriptRaw::from_json(script_json).expect("parse script");
    let compiled = raw.compile().expect("compile script");

    // Generate the story graph
    let graph = StoryGraph::from_script(&compiled);

    // Print statistics
    let stats = graph.stats();
    println!("Graph Statistics:");
    println!("  Total nodes: {}", stats.total_nodes);
    println!("  Reachable nodes: {}", stats.reachable_nodes);
    println!("  Unreachable nodes: {}", stats.unreachable_nodes);
    println!("  Dialogues: {}", stats.dialogue_count);
    println!("  Choices: {}", stats.choice_count);
    println!("  Branch points: {}", stats.branch_count);
    println!("  Edges: {}", stats.edge_count);

    // Check for unreachable nodes
    let unreachable = graph.unreachable_nodes();
    if !unreachable.is_empty() {
        println!("\n⚠️  Unreachable nodes detected: {:?}", unreachable);
        for id in &unreachable {
            if let Some(node) = graph.get_node(*id) {
                println!("  Node {}: {:?}", id, node.node_type);
            }
        }
    } else {
        println!("\n✓ All nodes are reachable!");
    }

    // Print nodes
    println!("\n--- Nodes ---");
    for node in &graph.nodes {
        let status = if node.reachable { "✓" } else { "✗" };
        let labels = if node.labels.is_empty() {
            String::new()
        } else {
            format!(" [{}]", node.labels.join(", "))
        };
        println!(
            "  {} Node {}{}: {:?}",
            status, node.id, labels, node.node_type
        );
    }

    // Print edges
    println!("\n--- Edges ---");
    for edge in &graph.edges {
        let label = edge
            .label
            .as_ref()
            .map(|l| format!(" \"{}\"", l))
            .unwrap_or_default();
        println!(
            "  {} -> {}: {:?}{}",
            edge.from, edge.to, edge.edge_type, label
        );
    }

    // Find node by label
    println!("\n--- Label Lookup ---");
    for label in ["start", "left_path", "secret", "nonexistent"] {
        match graph.find_by_label(label) {
            Some(id) => println!("  '{}' -> Node {}", label, id),
            None => println!("  '{}' -> Not found", label),
        }
    }

    // Export to DOT format
    println!("\n--- DOT Export (Graphviz) ---");
    let dot = graph.to_dot();
    println!("{}", dot);
    println!("\nTip: Save this to 'graph.dot' and run:");
    println!("  dot -Tpng graph.dot -o graph.png");
}
