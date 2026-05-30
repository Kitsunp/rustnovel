#!/bin/bash
# Script para ejecutar todos los tests del proyecto

set -e

TEST_TMPDIR="${PWD}/target/tmp"
mkdir -p "${TEST_TMPDIR}"
export TMPDIR="${TEST_TMPDIR}"
export TMP="${TEST_TMPDIR}"
export TEMP="${TEST_TMPDIR}"

echo "=== Verificando formato del código ==="
cargo fmt -- --check || {
    echo "Error: El código no está formateado correctamente."
    echo "Ejecuta 'cargo fmt' para arreglarlo."
    exit 1
}

echo ""
echo "=== Ejecutando Clippy ==="
cargo clippy --workspace --all-targets -- -D warnings

echo ""
echo "=== Ejecutando auditoría de dependencias ==="
if ! command -v cargo-audit &> /dev/null; then
    cargo install cargo-audit --locked
fi
cargo audit -D warnings --ignore RUSTSEC-2024-0436

echo ""
echo "=== Ejecutando tests de Rust ==="
cargo test --workspace --all-targets --locked --verbose

echo ""
echo "=== Ejecutando tests de Rust con feature Python (embed) ==="

# python-embed habilita PyO3 auto-initialize para tests Rust que embeben CPython.

PYTHON_LIBDIR=$(python -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR"))') || {
    echo "Error: No se pudo obtener LIBDIR de Python"
    exit 1
}
export LD_LIBRARY_PATH="${PYTHON_LIBDIR}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

cargo test -p visual_novel_engine --features python-embed --verbose

echo ""
echo "=== Ejecutando fuzz smoke ==="
cargo test -p visual_novel_engine --features arbitrary --test fuzz_tests --verbose

echo ""
echo "=== Construyendo extensión de Python ==="
# python habilita pyo3/extension-module para construir el módulo Python vía maturin.
PY_TEST_VENV="${PWD}/target/py-test-venv"
if [ ! -d "${PY_TEST_VENV}" ]; then
    python -m venv "${PY_TEST_VENV}"
fi

source "${PY_TEST_VENV}/bin/activate"

python -m pip install --upgrade pip
python -m pip install maturin pytest

mkdir -p target/py-wheels
maturin build --manifest-path crates/py/Cargo.toml --features extension-module --out target/py-wheels
python -m pip install --force-reinstall target/py-wheels/visual_novel_engine-*.whl

echo ""
echo "=== Ejecutando tests de Python ==="
export PYTHONPATH="${PWD}/python${PYTHONPATH:+:$PYTHONPATH}"
python -m pytest tests/python/ -v --tb=short

echo ""
echo "✅ Todos los tests pasaron exitosamente!"
