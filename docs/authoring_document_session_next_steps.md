# AuthoringDocumentSession y Read Model - Siguiente Refactor Core

## Objetivo

El siguiente cambio recomendado para el core es convertir el bus documental en
una sesion persistente de autoria, no solo en un objeto transaccional corto.

El estado actual ya centraliza mutaciones documentales en
`AuthoringDocumentCommandBus`, pero GUI y Python todavia tienden a construir un
documento, aplicar un comando, extraer el documento resultante y copiar estado
de vuelta.

Ese flujo es correcto para trazabilidad, pero no es ideal para escalabilidad,
memoria ni eficiencia cuando el editor aplique muchas acciones pequenas.

Direccion propuesta:

```text
GUI / CLI / Python / API
    -> AuthoringDocumentSession.apply(command)
    -> core muta el documento
    -> core actualiza caches e indices
    -> core devuelve outcome canonico
```

## Componente 1: AuthoringDocumentSession

`AuthoringDocumentSession` seria el dueno persistente del documento durante una
sesion de autoria.

Forma aproximada:

```rust
pub struct AuthoringDocumentSession {
    document: AuthoringDocument,
    undo_deltas: Vec<AuthoringDocumentDelta>,
    redo_deltas: Vec<AuthoringDocumentDelta>,
    recorded_commands: Vec<AuthoringDocumentCommand>,
    cached_fingerprint: Option<AuthoringReportFingerprint>,
    cached_validation: Option<Vec<LintIssue>>,
    dirty: AuthoringDirtyFlags,
}
```

Responsabilidades:

- Mantener vivo el `AuthoringDocument`.
- Aplicar comandos documentales y de grafo.
- Producir `AuthoringDocumentCommandOutcome`.
- Mantener undo/redo por deltas.
- Evitar snapshots completos por accion.
- Reducir clones de `NodeGraph`, operation logs y overrides.
- Decidir que fingerprints, validaciones y caches deben invalidarse.

## Objetivo De Las Salidas

El core no debe devolver solo `Ok(())`, porque eso obliga a GUI, CLI, Python o
API a adivinar que cambio.

La salida canonica debe explicar el cambio completo:

```rust
pub struct AuthoringDocumentCommandOutcome {
    pub delta: AuthoringDocumentDelta,
    pub operation: OperationLogEntry,
    pub verification: VerificationRun,
    pub before_fingerprint: AuthoringReportFingerprint,
    pub after_fingerprint: AuthoringReportFingerprint,
}
```

### Delta

Describe que cambio exactamente.

Sirve para:

- undo/redo
- replay
- sincronizacion incremental
- tests fuertes
- evitar snapshots completos

### Operation Log

Registra la accion de autoria.

Sirve para:

- historial visible en GUI
- auditoria
- CLI JSON
- reportes
- trazabilidad entre accion del usuario y cambio de documento

Ejemplo:

```text
operation_kind: layer_visibility_changed
field_path: composer.layers[node:1:character:0:Ava].visible
status: applied
```

### Verification Run

Prueba que el estado posterior fue evaluado.

Sirve para:

- saber que diagnostics siguen activos
- saber cuales se resolvieron
- saber cuales aparecieron
- mantener reportes confiables

### Fingerprints Before/After

Separan que tipo de estado cambio.

Ejemplos:

- `visible` / `locked` cambia fingerprint documental o layout.
- `visible` / `locked` no debe cambiar fingerprint semantico.
- `Graph(EditDialogue)` si cambia fingerprint semantico.
- background fit no debe cambiar script runtime.

Sirven para:

- invalidar caches correctamente
- detectar reportes stale
- decidir si hay que recompilar runtime
- evitar trabajo innecesario

### Field Paths

Indican donde ocurrio el cambio.

Ejemplos:

```text
composer.layers[{object_id}].locked
graph.nodes[3].dialogue.text
composer.background_fit[5]
```

Sirven para:

- enfocar paneles en GUI
- resaltar campos modificados
- serializar outcomes en CLI/API
- comparar paridad entre core, GUI, Python y CLI

## Dirty Flags

La sesion deberia mantener banderas de invalidacion por dominio:

```rust
pub struct AuthoringDirtyFlags {
    pub graph_dirty: bool,
    pub layout_dirty: bool,
    pub assets_dirty: bool,
    pub document_dirty: bool,
    pub validation_dirty: bool,
    pub runtime_export_dirty: bool,
}
```

Ejemplos:

```text
SetLayerVisible:
  layout_dirty = true
  document_dirty = true
  graph_dirty = false
  runtime_export_dirty = false

SetBackgroundFitOverride:
  layout_dirty = true
  document_dirty = true
  graph_dirty = false
  runtime_export_dirty = false

Graph(EditDialogue):
  graph_dirty = true
  document_dirty = true
  validation_dirty = true
  runtime_export_dirty = true
```

## Componente 2: AuthoringReadModel / Projection Cache

El segundo componente recomendado es un modelo de lectura incremental:

```rust
pub struct AuthoringReadModel {
    node_index: NodeIndex,
    asset_ref_index: AssetRefIndex,
    composer_layer_index: ComposerLayerIndex,
    diagnostics_index: DiagnosticsIndex,
    route_index: RouteIndex,
}
```

Su objetivo es cachear vistas derivadas del documento para que GUI, CLI, Python
y API no recalculen lo mismo por separado.

Preguntas que deberia responder:

```text
dame las capas del composer
dame los objetos visibles
dame diagnostics agrupados
dame asset refs
dame nodos por texto
dame rutas alcanzables
dame preview data para este nodo
dame estado stale del reporte
```

## Integracion Entre Session y Read Model

Flujo propuesto:

```text
session.apply(command)
    -> valida no-op
    -> muta AuthoringDocument
    -> produce delta
    -> actualiza AuthoringReadModel desde delta
    -> actualiza dirty flags
    -> recalcula solo fingerprints necesarios
    -> genera operation log
    -> genera verification run
    -> devuelve outcome
```

Ejemplo:

```text
SetLayerVisible
    -> no recalcula rutas narrativas
    -> no recompila runtime
    -> no revalida todo el grafo
    -> actualiza composer layer projection
    -> recalcula fingerprint documental/layout
```

Ejemplo:

```text
Graph(EditDialogue)
    -> actualiza grafo
    -> invalida semantic fingerprint
    -> invalida script runtime
    -> invalida diagnostics
    -> actualiza search index
```

## Beneficios

### Memoria

- Menos clones completos de `NodeGraph`.
- Menos documentos temporales.
- Menos JSON intermedio.
- Undo/redo por delta, no por snapshot completo.

### CPU

- Fingerprints parciales.
- Validation cache.
- Indices incrementales.
- Preview/layout invalidation mas precisa.

### Arquitectura

- GUI deja de ser dueno de estado derivado complejo.
- CLI puede serializar outcomes reales del core.
- Python conserva una API simple pero usa estado core real.
- API HTTP o headless puede aplicar comandos y devolver outcomes canonicos.

## Integracion Por Consumidor

### GUI

```text
workbench mantiene AuthoringDocumentSession
    -> aplica comandos
    -> lee composer layers desde read model
    -> actualiza UI segun outcome
```

### Python

```text
NodeGraph wrapper mantiene AuthoringDocumentSession
    -> set_layer_visible(...)
    -> session.apply(...)
    -> operation_log() lee logs producidos por core
```

### CLI

```text
vnengine authoring apply-command --json
    -> parsea AuthoringDocumentCommand
    -> session.apply(command)
    -> serializa AuthoringDocumentCommandOutcome real
```

### API

```text
POST /authoring/document/commands
    -> aplica comando
    -> devuelve outcome
```

## Orden Recomendado

1. Crear `AuthoringDocumentSession` como wrapper persistente del bus actual.
2. Mover undo/redo, recorded commands y caches a la sesion.
3. Agregar `AuthoringDirtyFlags`.
4. Hacer fingerprints parciales segun delta.
5. Crear `AuthoringReadModel` basico con composer layers y asset refs.
6. Migrar GUI para mantener una sesion core viva.
7. Migrar Python para mantener una sesion core viva.
8. Agregar comandos CLI documentales que serialicen el outcome real del core.
9. Ampliar read model con diagnostics, rutas, search index y preview cache.

## Criterio De Exito

El refactor es exitoso si:

- GUI, CLI, Python y API aplican comandos contra la misma sesion core.
- Comandos documentales pequenos no clonan ni recalculan todo el documento.
- Fingerprints se recalculan por dominio afectado.
- Validation se reutiliza cuando el delta no puede afectar diagnostics.
- Read model se actualiza por delta.
- Los outcomes siguen siendo canonicos y comparables entre consumidores.
- Undo/redo sigue funcionando sin snapshots completos por accion.
