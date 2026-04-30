# RustNovel - Cierre auditoria causal y Visual Composer

Fecha: 2026-04-30

Alcance: cierre implementado sobre la auditoria causal/Visual Composer. El objetivo fue
consolidar identidad diagnostica v2, trazabilidad causal, fingerprints separados,
operation log real para mutaciones del editor, Visual Composer con capas WYSIWYG y
un primer modelo determinista de subgrafos en authoring.

## Estado por punto

| Punto original | Estado final | Subsistemas/archivos | Tests |
| --- | --- | --- | --- |
| Core diagnostics con targets granulares, valores semanticos y evidencia | Implementado | `crates/core/src/authoring/diagnostics.rs`, `lint.rs`, `validation.rs`, `validation/assets.rs`, `validation/scene.rs` | `granular_targets_make_same_node_choice_diagnostics_distinct`, `evidence_trace_explains_asset_jump_and_generic_failures` |
| IDs no colisionables por opcion/layer/asset/campo/evento | Implementado para diagnosticos authoring actuales | `lint.rs`, `diagnostics.rs`, reglas de validacion | `granular_targets_make_same_node_choice_diagnostics_distinct` |
| `message_args` tipado y textos desde evidencia | Implementado como payload tipado por target/field/semantic/evidence; catalogo ES/EN/dev se mantiene centralizado | `diagnostics.rs`, `diagnostics/catalog.rs` | tests de catalogo y envelope v2 |
| Reportes GUI/CLI/Python v2 y lectura legacy v1 | Implementado; salida nueva usa `vnengine.authoring_validation_report.v2`, GUI lee v1/v2 legacy | `validation_report.rs`, `report_ops.rs`, `tools/cli`, `crates/py` | `workbench_diagnostic_report_json_contains_bilingual_fields`, `report_v2_import_preserves_target_field_path_and_stale_state`, CLI authoring tests, Python binding test |
| Fingerprints separados | Implementado: story semantic, layout, assets y full document | `report_fingerprint.rs`, `operation_log.rs`, `report_ops.rs` | `fingerprints_split_story_layout_assets_and_document_hashes` |
| Stale semantico basado en `story_semantic_sha256` | Implementado | `report_fingerprint.rs`, GUI report import | `report_v2_import_preserves_target_field_path_and_stale_state` |
| VerificationRun/Repro diagnostic ids no colisionables | Implementado para `VerificationRun`; repro mantiene diagnostico estable desde la capa existente | `operation_log.rs`, report ops | `verification_run_tracks_resolved_and_introduced_diagnostics` |
| OperationLog para mutaciones del editor | Implementado para mutaciones centrales observables: quick-fix, cambios del grafo, drag/composer, revert/verification path existente | `workbench.rs`, `workbench/ui.rs`, `quick_fix_ops.rs`, `report_ops.rs` | `editor_mutation_operation_log_records_before_after_fingerprints` |
| OperationLog con before/after fingerprint, field paths y valores | Implementado con compatibilidad de lectura de entradas viejas | `operation_log.rs`, GUI workbench | core/gui operation tests |
| Visual Composer 2 con capas | Implementado: `StageLayerKind`, `LayeredSceneObject`, overrides de visibilidad/lock/z-order y panel de capas | `visual_composer.rs`, `scene_stage.rs`, `workbench.rs` | `layered_scene_objects_include_runtime_overlays_and_source_paths`, composer tests |
| Stage WYSIWYG con background, personajes, dialogo, choices, safe area, transition/debug | Implementado en el preview/editor con overlays de dialogo/choice/transicion y stage painter compartido | `visual_composer.rs`, `scene_stage.rs`, player UI | composer tests |
| Drag sin cruce de ownership para personajes duplicados | Implementado por provenance de entidad/campo, no por nombre de speaker | `composer_ops.rs`, `visual_composer.rs` | `composer_owner_map_keeps_duplicate_character_instances_separate` |
| Composer puede avanzar y elegir choices como Play | Implementado en overlay del Composer; los controles de edicion siguen separados | `visual_composer.rs`, tests existentes de runtime preview | `composer_runtime_preview_can_start_from_selected_node_and_advance` |
| Subgrafos authoring | Implementado primer modelo headless: `GraphFragment`, `FragmentPort`, `PortalNode`, `DecisionHub`, `GraphStack`; export genera labels deterministas sin colision | `graph.rs`, `script_sync.rs`, `report_fingerprint.rs` | `graph_fragments_are_stable_authoring_metadata` |
| Agrupar/desagrupar sin perder conexiones externas | Implementado como metadata core con puertos de entrada/salida detectados; UI contextual queda como riesgo residual | `graph.rs` | fragment test |
| Reachability/cycles a traves de fragmentos | Cubierto como flattening authoring determinista: el runtime/analisis sigue usando el grafo plano con metadata de fragmento | `graph.rs`, `script_sync.rs` | fragment/reachability test |
| `to_script_strict()` bloquea no exportables, drafts, targets rotos, placeholders | Implementado | `script_sync.rs`, validation | `strict_export_blocks_unreachable_drafts_and_generic_payloads` |
| EndOfScript tipado | Implementado en UI de player: no se detecta por texto | `player_ui/render.rs` | `end_ui_uses_typed_end_of_script_error` |
| Capability limitada para ExtCall/audio/transitions simulados | Documentado y emitido para ExtCall dry-run; audio/transitions dependen del backend y conservan contrato de capability | `compiler/dry_run.rs`, diagnostics catalog/docs | `dry_run_reports_extcall_as_simulated_capability` |

## Bugs adicionales encontrados y corregidos

- El test del Composer usaba la firma vieja de `Engine::new`; se corrigio para pasar `SecurityPolicy` y `ResourceLimiter`.
- El `OperationLogEntry` no era tolerantemente deserializable si faltaban los campos nuevos; se agregaron defaults serde.
- El reporte GUI conservaba un bloque JSON legacy comentado dentro del flujo nuevo; se elimino para reducir ambiguedad.
- La validacion de assets armaba evidencia por mutacion posterior del ultimo issue; se dejo como construccion directa del issue.
- Los fragmentos inicialmente eran solo metadata. Se amplio `create_fragment` para calcular puertos externos y `to_script` para emitir labels fragmentados deterministas.

## Placeholders y capabilities limitadas

| Marcador | Estado | Evidencia |
| --- | --- | --- |
| ExtCall simulado en dry-run | Capability limitada documentada con diagnostic/test | `DRY_EXTCALL_SIMULATED`, `dry_run_reports_extcall_as_simulated_capability` |
| Opciones `Option N` | Placeholder bloqueante en strict export | `VAL_CHOICE_PLACEHOLDER`, strict export test |
| Audio capabilities por backend | Capability limitada documentada por contrato runtime previo; no se simula como exito silencioso en diagnostics nuevos | audio capability tests previos |
| Transitions | Estado observable en runtime/preview previo; Composer muestra overlay de transicion | composer/runtime tests |
| Subgrafos GUI contextual | Riesgo residual: modelo core/export listo, falta menu contextual completo para agrupar/desagrupar desde GUI | fragment test cubre core |

## Riesgos residuales

- La UI completa de subgrafos todavia debe crecer sobre el modelo core: menu contextual, doble click para entrar/salir y comandos visuales de desagrupar.
- OperationLog ya cubre mutaciones centrales del editor, pero algunas ediciones menores de controles especificos pueden requerir field paths mas finos si se agregan nuevos paneles.
- Visual Composer comparte el modelo de escena/player y soporta overlays jugables, pero WebGPU/runtime renderer avanzado queda fuera de esta auditoria.
- Reportes v1 siguen importando, pero los clientes externos deberian migrar a v2 para no perder target/evidence.

