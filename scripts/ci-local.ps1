param(
    [ValidateSet(
        "lint",
        "test",
        "matrix-smoke",
        "reproducible-smoke",
        "sbom-policy",
        "fuzz-smoke",
        "python-tests",
        "all"
    )]
    [string] $Job = "lint",
    [string] $Python = "python",
    [switch] $SkipToolInstall
)

$ErrorActionPreference = "Stop"

function Invoke-CiStep {
    param(
        [string] $Name,
        [scriptblock] $Command
    )

    Write-Host ""
    Write-Host "==> $Name" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne $null -and $LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

function Test-CommandAvailable {
    param([string] $Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Test-IsWindowsHost {
    if (Get-Variable -Name IsWindows -Scope Global -ErrorAction SilentlyContinue) {
        return $Global:IsWindows
    }
    return [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT
}

function Test-PythonModuleAvailable {
    param(
        [string] $PythonExe,
        [string] $Module
    )

    & $PythonExe -c "import importlib.util, sys; sys.exit(0 if importlib.util.find_spec('$Module') else 1)" *> $null
    return $LASTEXITCODE -eq 0
}

function Invoke-LintJob {
    Invoke-CiStep "rustc version" { rustc --version }
    Invoke-CiStep "cargo version" { cargo --version }
    Invoke-CiStep "clippy version" { cargo clippy --version }
    Invoke-CiStep "ruff format --check ." { ruff format --check . }
    Invoke-CiStep "ruff check ." { ruff check . }
    Invoke-CiStep "cargo check --workspace --all-targets --locked" {
        cargo check --workspace --all-targets --locked
    }
    Invoke-CiStep "cargo fmt --check" { cargo fmt --check }
    Invoke-CiStep "cargo clippy --workspace --all-targets --locked -- -D warnings" {
        cargo clippy --workspace --all-targets --locked -- -D warnings
    }
    if (-not (Test-CommandAvailable "cargo-audit")) {
        if ($SkipToolInstall) {
            throw "cargo-audit is not installed; rerun without -SkipToolInstall"
        }
        Invoke-CiStep "cargo install cargo-audit --locked" {
            cargo install cargo-audit --locked
        }
    }
    $auditDb = Join-Path (Get-Location) "target/audit-db"
    Invoke-CiStep "cargo audit -D warnings" {
        cargo audit --db $auditDb -D warnings --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2026-0097
    }
}

function Invoke-TestJob {
    Invoke-CiStep "cargo test --workspace --all-targets --locked --verbose" {
        cargo test --workspace --all-targets --locked --verbose
    }
    Invoke-CiStep "cargo build -p vnengine_py --profile python --features extension-module --locked --verbose" {
        cargo build -p vnengine_py --profile python --features extension-module --locked --verbose
    }
    Invoke-CiStep "cargo bench core_benches smoke" {
        cargo bench -p visual_novel_engine --bench core_benches --locked -- --warm-up-time 0.1 --measurement-time 0.1 --sample-size 10
    }
}

function Invoke-MatrixSmokeJob {
    Invoke-CiStep "cargo check --workspace --all-targets --locked" {
        cargo check --workspace --all-targets --locked
    }
    Invoke-CiStep "cargo test -p visual_novel_engine --locked --verbose" {
        cargo test -p visual_novel_engine --locked --verbose
    }
    Invoke-CiStep "cargo test -p vnengine_runtime --locked --verbose" {
        cargo test -p vnengine_runtime --locked --verbose
    }
}

function Invoke-ReproducibleSmokeJob {
    $env:SOURCE_DATE_EPOCH = "1704067200"
    $env:CARGO_PROFILE_RELEASE_DEBUG = "0"
    $reproDir = Join-Path (Get-Location) "target/repro"
    New-Item -ItemType Directory -Force -Path $reproDir | Out-Null

    Invoke-CiStep "cargo clean -p visual_novel_engine" {
        cargo clean -p visual_novel_engine
    }
    Invoke-CiStep "first release build" {
        cargo build -p visual_novel_engine --release
    }
    $first = Get-ChildItem "target/release/deps" -Filter "visual_novel_engine-*.rlib" |
        Select-Object -First 1
    if (-not $first) {
        $first = Get-ChildItem "target/release/deps" -Filter "libvisual_novel_engine-*.rlib" |
            Select-Object -First 1
    }
    if (-not $first) {
        throw "No first visual_novel_engine rlib found"
    }
    $firstCopy = Join-Path $reproDir "vn_first.rlib"
    Copy-Item -LiteralPath $first.FullName -Destination $firstCopy -Force

    Invoke-CiStep "second clean" { cargo clean -p visual_novel_engine }
    Invoke-CiStep "second release build" {
        cargo build -p visual_novel_engine --release
    }
    $second = Get-ChildItem "target/release/deps" -Filter "visual_novel_engine-*.rlib" |
        Select-Object -First 1
    if (-not $second) {
        $second = Get-ChildItem "target/release/deps" -Filter "libvisual_novel_engine-*.rlib" |
            Select-Object -First 1
    }
    if (-not $second) {
        throw "No second visual_novel_engine rlib found"
    }
    $secondCopy = Join-Path $reproDir "vn_second.rlib"
    Copy-Item -LiteralPath $second.FullName -Destination $secondCopy -Force

    $firstHash = Get-FileHash -Algorithm SHA256 -LiteralPath $firstCopy
    $secondHash = Get-FileHash -Algorithm SHA256 -LiteralPath $secondCopy
    Write-Host $firstHash.Hash
    Write-Host $secondHash.Hash
    if ($firstHash.Hash -ne $secondHash.Hash) {
        throw "Release artifact hashes differ"
    }
}

function Invoke-SbomPolicyJob {
    if (-not (Test-CommandAvailable "cargo-cyclonedx")) {
        if ($SkipToolInstall) {
            throw "cargo-cyclonedx is not installed; rerun without -SkipToolInstall"
        }
        Invoke-CiStep "cargo install cargo-cyclonedx --locked" {
            cargo install cargo-cyclonedx --locked
        }
    }
    Invoke-CiStep "cargo cyclonedx" {
        cargo cyclonedx --format json --all --override-filename sbom.cdx
    }
    Invoke-CiStep "validate SBOM content" {
        $files = @(
            Get-ChildItem -Path "crates", "tools" -Recurse -Filter "sbom.cdx.json" |
                Sort-Object FullName
        )
        if (-not $files) {
            throw "SBOM files not found"
        }
        $total = 0
        foreach ($file in $files) {
            $payload = Get-Content -Raw -LiteralPath $file.FullName | ConvertFrom-Json
            if ($payload.bomFormat -ne "CycloneDX") {
                throw "$($file.FullName): bomFormat is not CycloneDX"
            }
            if (-not $payload.components -or $payload.components.Count -eq 0) {
                throw "$($file.FullName): SBOM does not contain components"
            }
            $total += $payload.components.Count
        }
        Write-Host "SBOM files: $($files.Count)"
        Write-Host "SBOM total components: $total"
    }
}

function Invoke-FuzzSmokeJob {
    Invoke-CiStep "cargo test fuzz smoke" {
        cargo test -p visual_novel_engine --features arbitrary --test fuzz_tests --locked --verbose
    }
}

function Invoke-PythonTestsJob {
    $venv = Join-Path (Get-Location) ".venv"
    Write-Host ""
    Write-Host "==> python -m venv .venv" -ForegroundColor Cyan
    & $Python -m venv $venv
    $useSystemPython = $false
    if ($LASTEXITCODE -ne $null -and $LASTEXITCODE -ne 0) {
        if ($env:CI -eq "true") {
            throw "python -m venv .venv failed with exit code $LASTEXITCODE"
        }
        Write-Warning "python -m venv .venv failed locally; falling back to the configured Python interpreter."
        $useSystemPython = $true
    }
    $pythonExe = if ($useSystemPython) {
        $Python
    } else {
        if (Test-IsWindowsHost) {
            Join-Path $venv "Scripts/python.exe"
        } else {
            Join-Path $venv "bin/python"
        }
    }
    Invoke-CiStep "install maturin" {
        if ($useSystemPython) {
            if (-not (Test-PythonModuleAvailable $pythonExe "maturin")) {
                & $pythonExe -m pip install --user maturin
            }
        } else {
            & $pythonExe -m pip install --upgrade pip
            & $pythonExe -m pip install maturin
        }
    }
    Invoke-CiStep "maturin develop" {
        & $pythonExe -m maturin develop --manifest-path crates/py/Cargo.toml --features extension-module
    }
    Invoke-CiStep "python unittest" {
        $env:PYTHONPATH = "python"
        & $pythonExe -m unittest discover -s tests/python -p "test_*.py" -v
    }
}

$jobs = if ($Job -eq "all") {
    @(
        "lint",
        "test",
        "matrix-smoke",
        "reproducible-smoke",
        "sbom-policy",
        "fuzz-smoke",
        "python-tests"
    )
} else {
    @($Job)
}

foreach ($selected in $jobs) {
    switch ($selected) {
        "lint" { Invoke-LintJob }
        "test" { Invoke-TestJob }
        "matrix-smoke" { Invoke-MatrixSmokeJob }
        "reproducible-smoke" { Invoke-ReproducibleSmokeJob }
        "sbom-policy" { Invoke-SbomPolicyJob }
        "fuzz-smoke" { Invoke-FuzzSmokeJob }
        "python-tests" { Invoke-PythonTestsJob }
    }
}
