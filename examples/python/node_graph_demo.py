#!/usr/bin/env python3
"""
Node Graph Demo - Visual Novel Editor Python API

This example demonstrates how to use the Python bindings for the
visual novel editor's node graph system.

Usage:
    python node_graph_demo.py
"""

from visual_novel_engine import NodeGraph, StoryNode, py_validate_graph


def create_simple_story():
    """Creates a simple linear story."""
    print("=== Creating Simple Story ===")

    graph = NodeGraph()

    # Add nodes
    start = graph.add_node(StoryNode.start(), 0, 0)
    scene = graph.add_node(StoryNode.scene("forest_bg.png"), 0, 100)
    dialogue1 = graph.add_node(
        StoryNode.dialogue("Alice", "Welcome to the enchanted forest!"), 0, 200
    )
    dialogue2 = graph.add_node(
        StoryNode.dialogue("Bob", "It's beautiful here."), 0, 300
    )
    end = graph.add_node(StoryNode.end(), 0, 400)

    # Connect nodes
    graph.connect(start, scene)
    graph.connect(scene, dialogue1)
    graph.connect(dialogue1, dialogue2)
    graph.connect(dialogue2, end)

    print(f"Created graph: {graph}")
    print(f"  Nodes: {graph.node_count()}")
    print(f"  Connections: {graph.connection_count()}")

    return graph


def create_branching_story():
    """Creates a branching story with choices."""
    print("\n=== Creating Branching Story ===")

    graph = NodeGraph()

    # Setup
    start = graph.add_node(StoryNode.start(), 0, 0)
    intro = graph.add_node(
        StoryNode.dialogue("Narrator", "You find yourself at a crossroads."), 0, 100
    )

    # Choice
    choice = graph.add_node(
        StoryNode.choice(
            "Which path do you take?",
            ["Go left into the forest", "Go right to the mountains"],
        ),
        0,
        200,
    )

    # Left path
    left_scene = graph.add_node(StoryNode.scene("forest.png"), -150, 300)
    left_dialogue = graph.add_node(
        StoryNode.dialogue("Forest Spirit", "Welcome, traveler."), -150, 400
    )

    # Right path
    right_scene = graph.add_node(StoryNode.scene("mountains.png"), 150, 300)
    right_dialogue = graph.add_node(
        StoryNode.dialogue("Mountain Guide", "The peak awaits!"), 150, 400
    )

    # Endings
    end1 = graph.add_node(StoryNode.end(), -150, 500)
    end2 = graph.add_node(StoryNode.end(), 150, 500)

    # Connections
    graph.connect(start, intro)
    graph.connect(intro, choice)
    graph.connect(choice, left_scene)  # First option
    graph.connect(choice, right_scene)  # Second option
    graph.connect(left_scene, left_dialogue)
    graph.connect(left_dialogue, end1)
    graph.connect(right_scene, right_dialogue)
    graph.connect(right_dialogue, end2)

    print(f"Created graph: {graph}")

    return graph


def validate_story(graph):
    """Validates a story graph for issues."""
    print("\n=== Validating Story ===")

    issues = py_validate_graph(graph)

    if not issues:
        print("✓ No issues found!")
        return True

    for issue in issues:
        severity_icon = {
            "LintSeverity.Error": "❌",
            "LintSeverity.Warning": "⚠️",
            "LintSeverity.Info": "ℹ️",
        }.get(str(issue.severity), "?")

        print(f"{severity_icon} {issue.message}")
        if issue.node_id is not None:
            print(f"   at node {issue.node_id}")

    # Check for errors
    has_errors = any(str(issue.severity) == "LintSeverity.Error" for issue in issues)
    return not has_errors


def save_and_load_demo(graph):
    """Demonstrates saving and loading a graph."""
    print("\n=== Save/Load Demo ===")

    # Save to file
    filepath = "demo_story.json"
    graph.save(filepath)
    print(f"Saved to {filepath}")

    # Load from file
    loaded_graph = NodeGraph.load(filepath)
    print(f"Loaded: {loaded_graph}")

    # Verify
    assert graph.node_count() == loaded_graph.node_count()
    print("✓ Save/Load verified!")

    # Cleanup
    import os

    os.remove(filepath)


def create_invalid_story():
    """Creates a story with validation issues for demo."""
    print("\n=== Creating Invalid Story (for validation demo) ===")

    graph = NodeGraph()

    # Missing Start - this will be an error
    graph.add_node(StoryNode.dialogue("Someone", "Hello!"), 0, 0)

    # Missing End - this will be a warning

    print(f"Created graph: {graph}")
    return graph


def main():
    """Main demo function."""
    print("Node Graph Python API Demo")
    print("=" * 40)

    # Demo 1: Simple story
    simple = create_simple_story()
    validate_story(simple)

    # Demo 2: Branching story
    branching = create_branching_story()
    validate_story(branching)
    save_and_load_demo(branching)

    # Demo 3: Validation with issues
    invalid = create_invalid_story()
    validate_story(invalid)

    print("\n" + "=" * 40)
    print("Demo complete!")


if __name__ == "__main__":
    main()
