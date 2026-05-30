# Guía de Testing

## CI/CD Automático

Los tests se ejecutan automáticamente en GitHub Actions.

Ver `.github/workflows/ci.yml` y `.github/workflows/tests.yml` para detalles.

## Comandos de Test

### Rust workspace

```bash
# Cierre local amplio: no filtra paquetes, targets ni casos.
cargo test --workspace --all-targets --locked --verbose

# Benchmarks (Criterion)
cargo bench -p visual_novel_engine --bench core_benches

# Fuzz smoke (determinista en CI)
cargo test -p visual_novel_engine --features arbitrary --test fuzz_tests --verbose
```

### Exportacion nativa Windows/Linux

Windows local es el flujo rapido de iteracion. Linux se valida en GitHub Actions.

```powershell
# Smoke local opcional de paquete Windows
.\scripts\ci-local.ps1 -Job package-windows-smoke

# Regresiones de export/report/HMAC. Son targets completos, no casos filtrados.
cargo test -p visual_novel_engine --test export_bundle_tests --locked
cargo test -p visual_novel_engine --test export_bundle_integrity_tests --locked
cargo test -p visual_novel_engine --test export_executable_bundle_tests --locked
cargo test -p vnengine_cli --test package_command_tests --locked
cargo test -p visual_novel_gui --bin vn_player --locked
```

En CI, `package-linux-smoke` construye `vn_player`, empaqueta un bundle Linux
con `vnengine package --require-executable --integrity hmac-sha256`, valida
`game` con permiso `0o755`, `package_report.json`, `bundle_file_manifest.json` y
`compat_report.json`, ejecuta `game --smoke --smoke-report
meta/runtime_smoke_report.json` bajo Xvfb con backend software, retropropaga el
`smoke_result` a `package_report.json` y `compat_report.json`, y sube el
bundle/logs como artifact. Los reportes deben coincidir en `runtime_artifact` y
`runtime_artifact_sha256`, validado contra el entry correspondiente del
`bundle_file_manifest`. El smoke valida paths de lanzamiento, parseo de
script, init de engine, manifest de assets, carga de asset, render de frame,
avance de escena y cierre con checks estructurados (`code`, `severity`,
`phase`, `target`, `trace_id`, `status`, `message`, `probable_cause`,
`suggested_action`, `consequence`, `blocking_release` y `asset`/`file` cuando
aplica). Si el smoke falla antes de completar el flujo, debe escribir
`runtime_smoke_report.json` con `status = "failed"` y retropropagar ese mismo
`smoke_result` a `package_report.json` y `compat_report.json` cuando esos
reportes existan.

El HMAC cubre el contenido exacto de `meta/bundle_file_manifest.json`. Ese
manifest lista payload verificable y excluye metadata autorreferencial
(`package_report.json`, `compat_report.json`, `bundle_file_manifest.json` y
`bundle.hmac_sha256`) para que la firma no dependa de archivos que contienen la
propia firma. `meta/runtime_smoke_report.json` tambien queda fuera del manifest
firmado porque se produce despues de ejecutar el paquete. `meta/bundle.hmac_sha256`
contiene la firma y `package_report.json`/`compat_report.json` retropropagan
`generator_os`, `expected_executable`, backend/fallback, `total_size`, `hashes`,
`bundle_file_manifest_sha256`, `bundle_hmac_sha256` y `smoke_result` para que
CLI, GUI, API y CI puedan comparar el mismo origen. Los tests de manipulacion
recalculan el HMAC sobre un manifest alterado y verifican que ya no coincide.

### Python Bindings

Los tests Python deben ejecutarse contra la extension nativa local, no contra una
instalacion global de `visual_novel_engine`. El `conftest.py` falla temprano si
el modulo importado no viene del workspace o del virtualenv activo, o si faltan
APIs publicas esperadas.

```powershell
py -m venv target\py-audit-venv
target\py-audit-venv\Scripts\python -m pip install --upgrade pip
target\py-audit-venv\Scripts\python -m pip install maturin pytest
target\py-audit-venv\Scripts\maturin build --manifest-path crates\py\Cargo.toml --features extension-module --out target\py-wheels
target\py-audit-venv\Scripts\python -m pip install --force-reinstall target\py-wheels\visual_novel_engine-0.1.0-cp38-abi3-win_amd64.whl
target\py-audit-venv\Scripts\python -m pytest tests\python\ -q
```

### Linting y auditoría

```bash
cargo fmt -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo audit
```

### Tests de Renderizado (Híbrido)

El motor soporta pruebas duales para su arquitectura híbrida:

1.  **Suite de Integración (CPU/Software)**:
    - Se ejecuta por defecto con `cargo test`.
    - Verifica la lógica de negocio y la integración básica del bucle principal.
    - En CI se fuerza con `VNENGINE_RENDER_BACKEND=software` y Xvfb, por lo que funciona sin GPU.

2.  **Suite de GPU (WGPU)**:
    - Requiere hardware gráfico compatible.
    - Es opcional. El fallback obligatorio se valida con `VNENGINE_FORCE_WGPU_FAILURE=1`.
    - Observar logs de stderr para confirmar "Using WGPU Hardware Backend" cuando se pruebe manualmente.

## Tests Manuales Recomendados

1. **GUI Básica**: Ejecutar `cargo run --example gui_demo` y verificar que la ventana abre correctamente.
2. **Guardado/Carga**: Usar el menú (`ESC`) para guardar, cerrar, reabrir y cargar la partida.
3. **Inspector**: Presionar `F12` y modificar una bandera; verificar que el cambio persiste.
4. **Historial**: Avanzar varios diálogos y abrir el historial para verificar que se registran.

## Estructura de Tests

```
tests/
├── python/
│   └── test_vnengine.py    # Tests de integración Python
crates/
├── core/
│   └── src/lib.rs          # Tests unitarios inline (#[cfg(test)])
└── gui/
    └── src/lib.rs          # Tests de configuración
```
