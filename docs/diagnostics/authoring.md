# Authoring Diagnostics

This page is the stable docs target for authoring, dry-run and runtime-validation diagnostics.
Each heading maps directly to a `LintCode::label()` value and is used by exported reports,
CLI JSON, GUI imports and Python bindings.

## val-start-missing

The graph must contain one connected Start node so all clients agree on the entry point.

## val-start-multiple

Only one Start node can be authoritative. Extra Start nodes make execution ambiguous.

## val-unreachable

Nodes that cannot be reached from Start are draft-only unless reconnected before export.

## val-potential-loop

Reachable cycles are allowed as warnings, but they must have intentional exits or QA limits.

## val-dead-end

Narrative nodes should connect to a next event or to a clean terminal route.

## val-choice-empty

A Choice must present at least one route to the player.

## val-choice-unlinked

Every Choice option must resolve to a node or label target.

## val-choice-port-oob

Choice connections must use ports that exist in the current option list.

## val-audio-missing

Audio play actions require a valid project-relative asset.

## val-audio-empty

Empty audio fields should be cleared or replaced by a valid asset.

## val-asset-not-found

Asset references are resolved relative to the project root and must exist for strict validation.

## val-scene-bg-empty

Background fields should be either unset or a valid image asset.

## val-asset-unsafe-path

Assets must use safe relative paths and cannot escape the project root.

## val-audio-channel-invalid

Audio channels must match the runtime contract, such as bgm, sfx or voice.

## val-audio-action-invalid

Audio actions must map to supported runtime operations.

## val-audio-volume-invalid

Volumes must be finite and within the accepted normalized range.

## val-audio-fade-invalid

Audio fade durations must be finite, non-negative and practical for the backend.

## val-scale-invalid

Character scale must be finite and visible inside the composer stage.

## val-transition-duration

Transition duration must be finite and non-negative.

## val-transition-kind-invalid

Transition kind must be supported by the renderer contract.

## val-character-name-empty

Visual characters need stable names so poses, bindings and speaker metadata stay correlated.

## val-speaker-empty

Dialogue needs a speaker or narrator identity for traceability and localization.

## val-jump-empty

Jump targets cannot be empty in strict export.

## val-jump-target-missing

Broken textual targets are preserved as broken targets until the author reconnects them.

## val-state-key-empty

Conditions and state mutations need non-empty keys.

## val-layout-position-invalid

Node layout coordinates must be finite and within editor-safe bounds.

## val-choice-placeholder

Generated placeholder options must be replaced by final player-facing text.

## val-contract-export-unsupported

Unsupported authoring semantics must be migrated to native events or documented as capabilities.

## val-generic-unchecked

Generic events require review because the editor cannot fully interpret their semantics.

## cmp-script-error

Compilation errors mean the runtime script cannot be produced safely.

## cmp-runtime-init

Runtime initialization errors mean compiled output failed engine/security checks.

## dry-unreachable

Compiled flow can contain unreachable IPs that need route review.

## dry-step-limit

Dry-run step limits indicate a possibly infinite or very long route.

## dry-runtime-error

Runtime errors during dry-run must be reproduced and fixed before export.

## dry-parity-mismatch

Preview/runtime signature mismatches indicate divergent semantics between editor and engine.

## dry-extcall-simulated

ExtCall is simulated in headless validation for safety; reports are partial for that capability.

## dry-finished

Dry-run finished marks a clean terminal route, not complete coverage of every branch.
