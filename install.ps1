# Rust is the default installation; Python bindings are optional.
param([switch]$WithPython)

$ErrorActionPreference = 'Stop'
Push-Location -LiteralPath $PSScriptRoot
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'Rust is required. Install it from https://rustup.rs/'
    }
    & ./scripts/build-desktop.ps1
    if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed' }

    if ($WithPython) {
        if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
            throw 'Python is required only when -WithPython is requested.'
        }
        python -m pip install maturin
        if ($LASTEXITCODE -ne 0) { throw 'Could not install maturin' }
        # Keep this build separate from stale wheels or wheels for other interpreters.
        $wheelOutput = Join-Path $PSScriptRoot ("target/wheels/install-" + [guid]::NewGuid().ToString('N'))
        python -m maturin build --manifest-path crates/py/Cargo.toml --profile python --features extension-module --locked --out $wheelOutput
        if ($LASTEXITCODE -ne 0) { throw 'Python wheel build failed' }
        $bindingWheels = @(Get-ChildItem -LiteralPath $wheelOutput -Filter '*.whl')
        if ($bindingWheels.Count -ne 1) { throw 'Expected exactly one Python wheel from this build' }
        $bindingWheel = $bindingWheels[0]
        python -m pip install $bindingWheel.FullName --force-reinstall
        if ($LASTEXITCODE -ne 0) { throw 'Python binding installation failed' }
        python -E -c "import sys; sys.path.pop(0); import visual_novel_engine; assert callable(visual_novel_engine.Engine); print('Python bindings loaded:', visual_novel_engine.__file__)"
        if ($LASTEXITCODE -ne 0) { throw 'Installed Python bindings could not be loaded' }
    }
    Write-Output 'Rust desktop build complete. Open target/desktop/vn_editor.exe.'
} finally {
    Pop-Location
}
