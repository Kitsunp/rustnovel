#!/usr/bin/env python3
"""
Story Graph Analysis Example

Demonstrates how to generate and analyze a story graph from a script,
including detecting unreachable nodes and exporting to DOT format.
"""

import visual_novel_engine as vn


# Sample script with branching and an unreachable node
SCRIPT_JSON = """{
    "script_schema_version": "1.0",
    "events": [
        {
            "type": "dialogue",
            "speaker": "Narrator",
            "text": "Welcome to the adventure!"
        },
        {
            "type": "choice",
            "prompt": "Which path do you take?",
            "options": [
                { "text": "Take the forest path", "target": "forest" },
                { "text": "Take the mountain path", "target": "mountain" }
            ]
        },
        {
            "type": "dialogue",
            "speaker": "Narrator",
            "text": "You enter the dark forest..."
        },
        {
            "type": "jump",
            "target": "ending"
        },
        {
            "type": "dialogue",
            "speaker": "Narrator",
            "text": "You climb the snowy mountain..."
        },
        {
            "type": "jump",
            "target": "ending"
        },
        {
            "type": "dialogue",
            "speaker": "Secret",
            "text": "This is a hidden path that cannot be reached!"
        },
        {
            "type": "dialogue",
            "speaker": "Narrator",
            "text": "Your adventure ends here."
        }
    ],
    "labels": {
        "start": 0,
        "forest": 2,
        "mountain": 4,
        "secret": 6,
        "ending": 7
    }
}"""


def main():
    print("=== Story Graph Analysis Example ===\n")

    # Generate the story graph
    graph = vn.StoryGraph.from_json(SCRIPT_JSON)
    print(f"Generated: {graph}")

    # Get statistics
    stats = graph.stats()
    print("\n--- Graph Statistics ---")
    print(f"  Total nodes:      {stats.total_nodes}")
    print(f"  Reachable nodes:  {stats.reachable_nodes}")
    print(f"  Unreachable nodes: {stats.unreachable_nodes}")
    print(f"  Dialogues:        {stats.dialogue_count}")
    print(f"  Choices:          {stats.choice_count}")
    print(f"  Branch points:    {stats.branch_count}")
    print(f"  Edges:            {stats.edge_count}")

    # Check for unreachable nodes (dead code detection)
    unreachable = graph.unreachable_nodes()
    if unreachable:
        print(f"\n⚠️  Unreachable nodes detected: {unreachable}")
        for node in graph.nodes():
            if node.id in unreachable:
                print(f"  Node {node.id}: {node.node_type} - {node.details}")
    else:
        print("\n✓ All nodes are reachable!")

    # List all nodes
    print("\n--- Nodes ---")
    for node in graph.nodes():
        status = "✓" if node.reachable else "✗"
        labels = f" [{', '.join(node.labels)}]" if node.labels else ""
        print(f"  {status} Node {node.id}{labels}: {node.node_type}")

    # List all edges
    print("\n--- Edges ---")
    for edge in graph.edges():
        label = f' "{edge.label}"' if edge.label else ""
        print(f"  {edge.from_id} -> {edge.to_id}: {edge.edge_type}{label}")

    # Find nodes by label
    print("\n--- Label Lookup ---")
    for label in ["start", "forest", "secret", "nonexistent"]:
        node_id = graph.find_by_label(label)
        if node_id is not None:
            print(f"  '{label}' -> Node {node_id}")
        else:
            print(f"  '{label}' -> Not found")

    # Export to DOT format for Graphviz visualization
    print("\n--- DOT Export ---")
    dot_content = graph.to_dot()
    print(dot_content)

    # Save DOT file
    with open("story_graph.dot", "w") as f:
        f.write(dot_content)
    print("\nSaved to 'story_graph.dot'")
    print("Generate PNG with: dot -Tpng story_graph.dot -o story_graph.png")


if __name__ == "__main__":
    main()
