# Forced Bugs Report: Script Parser, Importer & StoryGraph

**Audit Target**: `crates/core/src/renpy_import`, `crates/core/src/script`, `crates/core/src/graph`  
**Test Suite**: `crates/core/tests/force_parser_bugs.rs` (16 adversarial tests, all executed and passing)  
**Execution Command**:
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";
cargo test -p visual_novel_engine --test force_parser_bugs -- --test-threads=1
```

---

## Executive Summary

During adversarial testing of the script parsing pipeline, Ren'Py importer, JSON schema validator, and StoryGraph flow analysis, **9 distinct defects, contract violations, and failure modes** were identified and forced.

Key highlights:
1. **Critical Off-by-One Boundary Defect (`>` vs `>=`)**: Label indices pointing to `events.len()` pass both compilation and security policy validation, causing dangling edges in `StoryGraph` and execution crashes in `Engine`.
2. **StoryGraph Cycle Detection Phantom Node Bug**: Cycle detection constructs its node set from edge endpoints rather than the graph's node list, returning phantom nodes that do not exist in the graph.
3. **Silent Duplicate Label Overwrite**: Re-declaring an existing label silently erases the original entry with zero warnings or errors, turning predecessor events into unreachable dead code.
4. **Ren'Py Menu Empty Option Out-Of-Bounds Bug**: Trailing empty menu options assign target labels to `events.len()`, directly producing dangling graph edges.
5. **Runtime Infinite Hang on Missing Jump Targets**: Missing jump targets in non-strict mode produce synthetic `ExtCall` events that do not advance the instruction pointer, causing caller loops to hang indefinitely.
6. **Unicode Whitespace Asymmetry**: Fullwidth spaces (`\u{3000}`) are counted as 0-indent by `count_indent` but stripped by `trim_start()`, breaking block structure.

---

## Detailed Bug Findings

### Bug 1: Label and Target Index Range Checks Use `>` Instead of `>=` Allowing Dangling Edges and Out-of-Bounds Jumps

- **Files & Line Numbers**:
  - `crates/core/src/script/raw.rs:143`
  - `crates/core/src/security.rs:29`
  - `crates/core/src/security.rs:250`, `258`, `264`
- **Exact Behavior**:
  Events are 0-indexed with valid range `0..self.events.len()`. When a script defines a non-`start` label pointing to index `self.events.len()`, compilation succeeds:
  ```rust
  if *index > self.events.len() { // Should be >=
      return Err(VnError::InvalidScript(format!("label '{label}' points outside events")));
  }
  ```
  In `security.rs`:
  ```rust
  EventCompiled::Jump { target_ip } if *target_ip as usize > script.events.len() => ...
  ```
  When `target_ip == script.events.len()`, this passes validation.
  Consequently:
  - `StoryGraph::from_script` creates a `GraphEdge` where `to = events.len()`. No node with that ID exists in `graph.nodes`. Calling `graph.get_node(target_ip)` returns `None`.
  - In `Engine`, jumping to this target sets `self.state.position = events.len()`, causing the subsequent step to fail with `VnError::EndOfScript`.
- **Root Cause**:
  Off-by-one boundary comparison. `self.events.len()` is an out-of-bounds index for an array of length `self.events.len()`.
- **Minimal Fix**:
  Replace `>` with `>=` in `raw.rs:143` and `security.rs:29, 250, 258, 264`.

---

### Bug 2: `StoryGraph` Flow Analysis Cycle Detection Emits Phantom Nodes

- **File & Line Number**:
  - `crates/core/src/graph/analysis.rs:77-83`
- **Exact Behavior**:
  In `analyze_flow_graph(&nodes, &edges, &start_nodes)`:
  If `edges` contains a target node `X` that is NOT in `nodes` (e.g. `edges = [(0, 1), (1, 99), (99, 1)]` where `nodes = [0, 1]`), `analysis.reachable_cycle_nodes` returns `[99]`.
  Node `99` is reported as a reachable cycle node despite not existing in the graph's node set!
- **Root Cause**:
  `detect_reachable_cycle_nodes` reconstructs its `node_set` strictly from `adjacency` map keys and values:
  ```rust
  let mut node_set = BTreeSet::new();
  for (from, targets) in adjacency {
      node_set.insert(*from);
      node_set.extend(targets.iter().copied());
  }
  node_set.extend(start_nodes.iter().copied());
  ```
  It ignores the graph's actual `nodes` slice passed to `analyze_flow_graph`.
- **Minimal Fix**:
  Pass `node_set` (filtered to valid nodes) from `analyze_flow_graph` into `detect_reachable_cycle_nodes`:
  ```rust
  let reachable_cycle_nodes = detect_reachable_cycle_nodes(&node_set, &adjacency, start_nodes);
  ```

---

### Bug 3: Ren'Py Menu Empty Option Bodies Assign Out-of-Bounds Target

- **File & Line Number**:
  - `crates/core/src/renpy_import/flow.rs:133-147`
- **Exact Behavior**:
  When a Ren'Py script contains a menu option with an empty block at the end of the script:
  ```python
  label start:
      menu:
          "Option 1":
  ```
  `parse_menu` inserts `self.labels.insert(target, self.events.len())`. No events are parsed for the empty block.
  The target label is left pointing to `1` when `events.len()` is `1`.
  `compile()` compiles the choice option pointing to `target_ip = 1`.
  `StoryGraph` produces a choice edge to node `1` which does not exist in `nodes`.
- **Root Cause**:
  `parse_menu` pre-registers the option label at `self.events.len()` without checking whether any executable event was actually generated in the option body.
- **Minimal Fix**:
  If `self.events.len() == before_len` after parsing the option block, append a synthetic jump, return, or fallback event to ensure the label points to an executable event within bounds.

---

### Bug 4: Silent Duplicate Label Overwrite Corrupts Story Flow Without Warning

- **File & Line Number**:
  - `crates/core/src/renpy_import/types.rs:309-311`
- **Exact Behavior**:
  In Ren'Py, duplicate labels are syntax errors. In `ImportState`:
  ```rust
  pub fn add_label(&mut self, name: &str) {
      self.labels.insert(name.to_string(), self.events.len());
  }
  ```
  When the same label is declared twice, the second definition silently overwrites the first one. No `ImportIssue` is generated in `ImportReport`, and the original label's section becomes orphaned dead code.
- **Root Cause**:
  `add_label` does not check `self.labels.contains_key(name)`.
- **Minimal Fix**:
  ```rust
  pub fn add_label(&mut self, name: &str) {
      if self.labels.contains_key(name) {
          self.push_issue("warning", "duplicate_label", format!("Duplicate label '{name}' overwrites earlier declaration"), None, None);
      }
      self.labels.insert(name.to_string(), self.events.len());
  }
  ```

---

### Bug 5: Importer Degraded Missing Jump Targets Cause Infinite Execution Hang

- **Files & Line Numbers**:
  - `crates/core/src/renpy_import/postprocess.rs:28-40`
  - `crates/core/src/engine/runtime.rs:180`
- **Exact Behavior**:
  A jump to an unresolvable label (`jump non_existent`) is remapped in non-strict mode to a synthetic `ExtCall` event (`"vn.import.renpy.ext_v2"`).
  In `Engine::step()`:
  ```rust
  EventCompiled::ExtCall { .. } => Ok(()), // Position is NOT advanced
  ```
  When the host steps through the engine, position remains at `event_ip`. Subsequent calls to `step()` continuously re-execute the identical `ExtCall` without terminating or advancing, creating an infinite loop.
- **Root Cause**:
  `ExtCall` is designed for host-handled RPCs requiring `complete_external_call()`. Using it as an error fallback without host handling causes engine lockup.
- **Minimal Fix**:
  In strict mode this is rejected. In non-strict mode, provide an explicit fallback behavior or document that missing-target synthetic calls must be completed via `complete_external_call()`.

---

### Bug 6: Unicode Whitespace in Indentation Miscalculated as Level 0

- **Files & Line Numbers**:
  - `crates/core/src/renpy_import/parser.rs:380-389`
  - `crates/core/src/renpy_import/parser.rs:279, 287`
- **Exact Behavior**:
  `count_indent` only counts ASCII `' '` and `'\t'`:
  ```rust
  fn count_indent(line: &str) -> usize {
      let mut count = 0usize;
      for ch in line.chars() {
          match ch {
              ' ' => count += 1,
              '\t' => count += 4,
              _ => break,
          }
      }
      count
  }
  ```
  If a line begins with fullwidth spaces (e.g. `\u{3000}` Ideographic Space) or non-breaking spaces (`\u{00A0}`), `count_indent` immediately breaks and returns `0`.
  However, `line.trim_start()` in `preprocess_lines` strips Unicode whitespace.
  The line is treated as an unindented top-level line (indent 0), causing block parser functions (`parse_block`, `parse_menu`, `parse_if_chain`) to exit prematurely and discard the block.
- **Root Cause**:
  Mismatch between ASCII-only `count_indent` and Unicode-aware `trim_start()`.
- **Minimal Fix**:
  Use `line.trim_start_matches([' ', '\t'])` instead of `trim_start()`, and push an `unexpected_character` issue if leading non-ASCII whitespace is detected.

---

### Bug 7: Importer Accepts Oversized Label Names Exceeding Engine `SecurityPolicy`

- **Files & Line Numbers**:
  - `crates/core/src/renpy_import/mod.rs:71-86`
  - `crates/core/src/resource.rs:20` (`max_label_length = 64`)
  - `crates/core/src/security.rs:26-28`
- **Exact Behavior**:
  `import_renpy_project` successfully imports scripts containing label names > 64 bytes (e.g. 200 characters) and writes `main.json`.
  When `Engine::new()` or `SecurityPolicy::validate_raw()` is called on the imported script, it fails immediately with:
  `ResourceLimit("label '...' too long")`.
  The importer outputs a project that the engine refuses to run.
- **Root Cause**:
  `import_renpy_project` performs no resource limit validation prior to emitting the project manifest and script.
- **Minimal Fix**:
  Invoke `SecurityPolicy::default().validate_raw(&script, ResourceLimiter::default())` during postprocessing, truncating or reporting errors for violating labels.

---

### Bug 8: Inequality Boolean Condition (`!= true` / `!= false`) Unhandled in Expression Parser

- **File & Line Number**:
  - `crates/core/src/renpy_import/syntax.rs:171-176`
- **Exact Behavior**:
  Valid Python/Ren'Py expressions such as `if my_flag != true:` fail to parse and are converted into `unsupported_if_condition` ext_calls.
- **Root Cause**:
  `parse_cond_expr` only matches boolean literals against `CmpOp::Eq`:
  ```rust
  if rhs.eq_ignore_ascii_case("true") && matches!(op, CmpOp::Eq) {
      return Some(CondRaw::Flag { key, is_set: true });
  }
  if rhs.eq_ignore_ascii_case("false") && matches!(op, CmpOp::Eq) {
      return Some(CondRaw::Flag { key, is_set: false });
  }
  ```
  When `op` is `CmpOp::Ne`, it drops through to `rhs.parse::<i32>()`, which fails on `"true"`, returning `None`.
- **Minimal Fix**:
  Add handling for `CmpOp::Ne`:
  ```rust
  if rhs.eq_ignore_ascii_case("true") && matches!(op, CmpOp::Ne) {
      return Some(CondRaw::Flag { key, is_set: false });
  }
  if rhs.eq_ignore_ascii_case("false") && matches!(op, CmpOp::Ne) {
      return Some(CondRaw::Flag { key, is_set: true });
  }
  ```

---

### Bug 9: `parse_label_decl` Allows Empty and Whitespace-Containing Label Declarations

- **File & Line Number**:
  - `crates/core/src/renpy_import/parser.rs:334-343`
- **Exact Behavior**:
  - `label :` is parsed as an empty label name `""`.
  - `label has spaces inside:` is parsed as `"has spaces inside"`.
- **Root Cause**:
  `parse_label_decl` extracts substring without validating identifier syntax:
  ```rust
  fn parse_label_decl(text: &str) -> Option<String> {
      if !text.starts_with("label ") || !text.ends_with(':') {
          return None;
      }
      Some(text["label ".len()..text.len().saturating_sub(1)].trim().to_string())
  }
  ```
- **Minimal Fix**:
  Validate that the extracted name is a valid identifier (e.g. `!name.is_empty() && !name.contains(char::is_whitespace)`).

---

## Verification Test Suite Summary

File: [`force_parser_bugs.rs`](file:///D:/Rust/audit/rustnovel/crates/core/tests/force_parser_bugs.rs)

| Test Name | Target Area | Status |
|---|---|---|
| `test_hostile_renpy_inconsistent_indentation` | Area 1: Inconsistent tabs/spaces | **PASSED** |
| `test_hostile_renpy_unicode_whitespace_indentation_bug` | Area 1: Unicode `\u{3000}` whitespace bug | **PASSED** |
| `test_hostile_renpy_unclosed_quotes` | Area 1: Unclosed quotes & escapes | **PASSED** |
| `test_hostile_renpy_python_blocks` | Area 1: Inline `python:` blocks | **PASSED** |
| `test_hostile_renpy_duplicate_labels_silent_corruption` | Area 1: Duplicate labels silent overwrite | **PASSED** |
| `test_hostile_renpy_malformed_and_empty_label_decl` | Area 1: Empty and spaced label declarations | **PASSED** |
| `test_renpy_menu_empty_option_points_out_of_bounds` | Area 1: Menu empty option target out-of-bounds | **PASSED** |
| `test_renpy_ne_boolean_condition_unsupported_bug` | Area 1: `!= true` unhandled condition | **PASSED** |
| `test_labels_special_chars_and_emojis` | Area 2: Emoji labels (`🦀🎉`) | **PASSED** |
| `test_labels_unicode_long_string_contract_violation` | Area 2: 200-char label exceeding policy limit | **PASSED** |
| `test_jump_to_nonexistent_label_fallback_behavior` | Area 3: Missing jump targets & infinite loop | **PASSED** |
| `test_json_schema_manipulations` | Area 4: Floats, invalid strings, nulls, empty arrays | **PASSED** |
| `test_labels_index_equals_events_len_dangling_edge_bug` | Area 4: Off-by-one `>` bug in compile & security | **PASSED** |
| `test_story_graph_strongly_connected_cycles` | Area 5: Strongly connected components (2, 3-cycles, cliques) | **PASSED** |
| `test_story_graph_cycle_detection_phantom_node_bug` | Area 5: Phantom node cycle detection defect | **PASSED** |
| `test_story_graph_disconnected_hundreds_components` | Area 5: 500 disconnected components, 200 pairs, 100 cycles | **PASSED** |
