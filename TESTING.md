# Guía de Testing

## CI/CD Automático

Los tests se ejecutan automáticamente en GitHub Actions.

Ver `.github/workflows/ci.yml` y `.github/workflows/tests.yml` para detalles.

## Comandos de Test

### Core (Lógica del Motor)

```bash
# Tests unitarios
cargo test -p visual_novel_engine --verbose

# Benchmarks (Criterion)
cargo bench -p visual_novel_engine --bench core_benches

# Fuzz smoke (determinista en CI)
cargo test -p visual_novel_engine --features arbitrary --test fuzz_tests --verbose
```

### GUI (Interfaz Gráfica)

```bash
# Tests unitarios de configuración
cargo test -p visual_novel_gui --verbose
```

### Python Bindings

Los tests Python deben ejecutarse contra la extension nativa local, no contra una
instalacion global de `visual_novel_engine`. El `conftest.py` falla temprano si
el modulo importado no viene del workspace o del virtualenv activo, o si faltan
APIs publicas esperadas.

```powershell
py -m venv target\py-audit-venv
target\py-audit-venv\Scripts\python -m pip install --upgrade pip
target\py-audit-venv\Scripts\python -m pip install maturin pytest
target\py-audit-venv\Scripts\maturin develop --manifest-path crates\py\Cargo.toml --features extension-module
target\py-audit-venv\Scripts\python -m pytest -q
```

### Linting y auditoría

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -D warnings
cargo audit
```

### Tests de Renderizado (Híbrido)

El motor soporta pruebas duales para su arquitectura híbrida:

1.  **Suite de Integración (CPU/Software)**:
    - Se ejecuta por defecto con `cargo test`.
    - Verifica la lógica de negocio y la integración básica del bucle principal.
    - Utiliza el fallback de software, por lo que funciona en CI sin GPU.

2.  **Suite de GPU (WGPU)**:
    - Requiere hardware gráfico compatible.
    - Actualmente cubierto por los mismos tests de integración, que intentan inicializar WGPU primero.
    - Observar logs de stderr para confirmar "Using WGPU Hardware Backend".

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
