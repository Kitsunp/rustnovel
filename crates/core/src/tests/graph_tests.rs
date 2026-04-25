use super::*;
use crate::event::{
    ChoiceCompiled, ChoiceOptionCompiled, DialogueCompiled, EventCompiled, SharedStr,
};

fn make_dialogue(speaker: &str, text: &str) -> EventCompiled {
    EventCompiled::Dialogue(DialogueCompiled {
        speaker: SharedStr::from(speaker),
        text: SharedStr::from(text),
    })
}

fn make_choice(prompt: &str, options: Vec<(&str, u32)>) -> EventCompiled {
    EventCompiled::Choice(ChoiceCompiled {
        prompt: SharedStr::from(prompt),
        options: options
            .into_iter()
            .map(|(text, target)| ChoiceOptionCompiled {
                text: SharedStr::from(text),
                target_ip: target,
            })
            .collect(),
    })
}

#[test]
fn test_linear_script_graph() {
    let script = ScriptCompiled {
        events: vec![
            make_dialogue("Alice", "Hello!"),
            make_dialogue("Bob", "Hi there!"),
            make_dialogue("Alice", "Nice to meet you."),
        ],
        labels: [("start".to_string(), 0)].into_iter().collect(),
        start_ip: 0,
        flag_count: 0,
    };

    let graph = StoryGraph::from_script(&script);

    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.edges.len(), 2);
    assert!(graph.nodes.iter().all(|n| n.reachable));

    let stats = graph.stats();
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.reachable_nodes, 3);
    assert_eq!(stats.unreachable_nodes, 0);
    assert_eq!(stats.dialogue_count, 3);
}

#[test]
fn test_branching_script_graph() {
    let script = ScriptCompiled {
        events: vec![
            make_dialogue("Narrator", "What do you choose?"),
            make_choice("Choose wisely", vec![("Option A", 2), ("Option B", 3)]),
            make_dialogue("Narrator", "You chose A!"),
            make_dialogue("Narrator", "You chose B!"),
        ],
        labels: [("start".to_string(), 0)].into_iter().collect(),
        start_ip: 0,
        flag_count: 0,
    };

    let graph = StoryGraph::from_script(&script);

    assert_eq!(graph.nodes.len(), 4);
    let stats = graph.stats();
    assert_eq!(stats.choice_count, 1);
    assert_eq!(stats.branch_count, 1);
    assert!(graph.nodes.iter().all(|n| n.reachable));
}

#[test]
fn test_unreachable_detection() {
    let script = ScriptCompiled {
        events: vec![
            make_dialogue("Alice", "Start"),
            EventCompiled::Jump { target_ip: 3 },
            make_dialogue("Hidden", "This is unreachable!"),
            make_dialogue("Alice", "End"),
        ],
        labels: [("start".to_string(), 0)].into_iter().collect(),
        start_ip: 0,
        flag_count: 0,
    };

    let graph = StoryGraph::from_script(&script);
    let unreachable = graph.unreachable_nodes();

    assert_eq!(unreachable.len(), 1);
    assert_eq!(unreachable[0], 2);

    let stats = graph.stats();
    assert_eq!(stats.unreachable_nodes, 1);
}

#[test]
fn flow_analysis_reports_reachability_and_reachable_cycles() {
    let nodes = [0, 1, 2, 3];
    let edges = [(0, 1), (1, 2), (2, 1)];

    let analysis = analyze_flow_graph(&nodes, &edges, &[0]);

    assert!(analysis.reachable.contains(&0));
    assert!(analysis.reachable.contains(&1));
    assert!(analysis.reachable.contains(&2));
    assert_eq!(analysis.unreachable, vec![3]);
    assert!(analysis.reachable_cycle_nodes.contains(&1));
    assert!(analysis.reachable_cycle_nodes.contains(&2));
}

#[test]
fn test_dot_export() {
    let script = ScriptCompiled {
        events: vec![
            make_dialogue("Test", "Hello"),
            EventCompiled::Jump { target_ip: 0 },
        ],
        labels: [("start".to_string(), 0)].into_iter().collect(),
        start_ip: 0,
        flag_count: 0,
    };

    let graph = StoryGraph::from_script(&script);
    let dot = graph.to_dot();

    assert!(dot.contains("digraph StoryGraph"));
    assert!(dot.contains("n0 ->"));
    assert!(dot.contains("n1 ->"));
}

#[test]
fn test_find_by_label() {
    let script = ScriptCompiled {
        events: vec![
            make_dialogue("A", "Start"),
            make_dialogue("B", "Middle"),
            make_dialogue("C", "End"),
        ],
        labels: [
            ("start".to_string(), 0),
            ("middle".to_string(), 1),
            ("end".to_string(), 2),
        ]
        .into_iter()
        .collect(),
        start_ip: 0,
        flag_count: 0,
    };

    let graph = StoryGraph::from_script(&script);

    assert_eq!(graph.find_by_label("start"), Some(0));
    assert_eq!(graph.find_by_label("middle"), Some(1));
    assert_eq!(graph.find_by_label("end"), Some(2));
    assert_eq!(graph.find_by_label("nonexistent"), None);
}

#[test]
fn test_dialogue_preview_handles_utf8_boundaries() {
    let long_utf8 = "🙂".repeat(80);
    let script = ScriptCompiled {
        events: vec![make_dialogue("Narrador", &long_utf8)],
        labels: [("start".to_string(), 0)].into_iter().collect(),
        start_ip: 0,
        flag_count: 0,
    };

    let graph = StoryGraph::from_script(&script);
    let node = graph.get_node(0).expect("node 0 must exist");
    let NodeType::Dialogue { text_preview, .. } = &node.node_type else {
        panic!("expected dialogue node");
    };

    assert!(text_preview.ends_with("..."));
    assert!(text_preview.chars().count() <= 50);
}
