# Script de instalación y configuración para Visual Novel Engine
# Requisitos: Rust, Python 3.8+

Write-Host "=== Iniciando configuración de Visual Novel Engine ===" -ForegroundColor Cyan

# 1. Comprobar herramientas
try {
    cargo --version | Out-Null
    Write-Host "[OK] Rust detectado" -ForegroundColor Green
} catch {
    Write-Error "Rust no está instalado. Visita https://rustup.rs/"
    exit 1
}

try {
    python --version | Out-Null
    Write-Host "[OK] Python detectado" -ForegroundColor Green
} catch {
    Write-Error "Python no está instalado."
    exit 1
}

# 2. Compilar binarios de Rust (Release)
Write-Host "`n=== Compilando Core y GUI (Rust) ===" -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Error "Fallo en compilación de Rust"; exit 1 }

# 3. Preparar entorno Python
Write-Host "`n=== Configurando entorno Python ===" -ForegroundColor Cyan

# Instalar maturin si no existe
python -m pip install maturin --quiet
Write-Host "[OK] Maturin instalado/actualizado" -ForegroundColor Green

# 4. Compilar e instalar bindings de Python
Write-Host "`n=== Construyendo Bindings de Python ===" -ForegroundColor Cyan
# Usamos maturin build e install explícitamente para mayor control
python -m maturin build --manifest-path crates/py/Cargo.toml --release --out target/wheels
if ($LASTEXITCODE -ne 0) { Write-Error "Fallo al construir wheel de Python"; exit 1 }

# Encontrar el wheel generado más reciente
$wheel = Get-ChildItem target/wheels/*.whl | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Write-Host "Instalando: $($wheel.Name)"

python -m pip install $wheel.FullName --force-reinstall
if ($LASTEXITCODE -ne 0) { Write-Error "Fallo al instalar paquete Python"; exit 1 }

Write-Host "`n=== ¡Instalación Completada! ===" -ForegroundColor Green
Write-Host "Para probar Rust GUI: cargo run -p visual_novel_gui --example gui_demo"
Write-Host "Para probar Python GUI: python examples/python/gui_demo.py"
