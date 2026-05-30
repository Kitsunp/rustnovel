param(
    [ValidateSet(
        "lint",
        "test",
        "matrix-smoke",
        "reproducible-smoke",
        "sbom-policy",
        "fuzz-smoke",
        "python-tests",
        "package-windows-smoke",
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

function Write-Utf8NoBom {
    param(
        [string] $Path,
        [string[]] $Lines
    )

    $content = [string]::Join([Environment]::NewLine, $Lines)
    [System.IO.File]::WriteAllText(
        $Path,
        $content,
        [System.Text.UTF8Encoding]::new($false)
    )
}

function Invoke-LintJob {
    Invoke-CiStep "rustc version" { rustc --version }
    Invoke-CiStep "cargo version" { cargo --version }
    Invoke-CiStep "clippy version" { cargo clippy --version }
    Invoke-CiStep "ruff format --check ." { ruff format --check . }
    Invoke-CiStep "ruff check ." { ruff check . }
    Invoke-CiStep "mypy" { mypy }
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
    $auditDb = Join-Path (Get-Location) "target/ci-local/audit-db"
    $auditRemote = Join-Path $auditDb ".git/config"
    if ((Test-Path $auditDb) -and (
            -not (Test-Path $auditRemote) -or
            -not (Select-String -Path $auditRemote -Pattern 'url = https://github.com/RustSec/advisory-db.git' -Quiet)
        )) {
        Remove-Item -LiteralPath $auditDb -Recurse -Force
    }
    Invoke-CiStep "cargo audit -D warnings --ignore RUSTSEC-2024-0436" {
        cargo audit --db $auditDb -D warnings --ignore RUSTSEC-2024-0436
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
    Invoke-CiStep "install Python test tools" {
        if ($useSystemPython) {
            if ((-not (Test-PythonModuleAvailable $pythonExe "maturin")) -or (-not (Test-PythonModuleAvailable $pythonExe "pytest"))) {
                & $pythonExe -m pip install --user maturin pytest
            }
        } else {
            & $pythonExe -m pip install --upgrade pip
            & $pythonExe -m pip install maturin pytest
        }
    }
    Invoke-CiStep "maturin wheel install" {
        $wheelDir = Join-Path (Get-Location) "target/py-wheels"
        New-Item -ItemType Directory -Force -Path $wheelDir | Out-Null
        & $pythonExe -m maturin build --manifest-path crates/py/Cargo.toml --features extension-module --out $wheelDir
        if ($LASTEXITCODE -ne $null -and $LASTEXITCODE -ne 0) {
            throw "maturin build failed with exit code $LASTEXITCODE"
        }
        $wheel = Get-ChildItem -LiteralPath $wheelDir -Filter "visual_novel_engine-*.whl" |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($null -eq $wheel) {
            throw "maturin build did not produce a visual_novel_engine wheel in $wheelDir"
        }
        & $pythonExe -m pip install --force-reinstall $wheel.FullName
    }
    Invoke-CiStep "python pytest" {
        $env:PYTHONPATH = "python"
        & $pythonExe -m pytest tests/python/ -v --tb=short
    }
}

function Invoke-PackageWindowsSmokeJob {
    if (-not (Test-IsWindowsHost)) {
        throw "package-windows-smoke is a Windows-local job; Linux package smoke runs in GitHub Actions"
    }

    $smokeRoot = Join-Path (Get-Location) "target/ci-local/package-windows-smoke"
    $projectDir = Join-Path $smokeRoot "project"
    $bundleDir = Join-Path $smokeRoot "bundle"
    $logDir = Join-Path $smokeRoot "logs"

    if (Test-Path $smokeRoot) {
        Remove-Item -LiteralPath $smokeRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $projectDir, $logDir | Out-Null

    $manifest = @(
        'manifest_schema_version = "1.0"',
        '',
        '[metadata]',
        'name = "Package Windows Smoke"',
        'author = "local"',
        'version = "0.1.0"',
        'description = "Native Windows package smoke fixture"',
        '',
        '[settings]',
        'resolution = [960, 540]',
        'default_language = "en"',
        'supported_languages = ["en"]',
        'entry_point = "main.json"',
        '',
        '[assets.backgrounds]',
        '[assets.characters]',
        '[assets.audio]'
    )
    Write-Utf8NoBom -Path (Join-Path $projectDir "project.vnm") -Lines $manifest
    New-Item -ItemType Directory -Force -Path (Join-Path $projectDir "assets/backgrounds") | Out-Null
    [System.IO.File]::WriteAllBytes(
        (Join-Path $projectDir "assets/backgrounds/smoke.png"),
        [System.Convert]::FromBase64String("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=")
    )

    $script = @(
        '{',
        '  "script_schema_version": "1.0",',
        '  "events": [',
        '    {',
        '      "type": "scene",',
        '      "background": "assets/backgrounds/smoke.png",',
        '      "characters": []',
        '    },',
        '    {',
        '      "type": "dialogue",',
        '      "speaker": "Narrator",',
        '      "text": "Package Windows smoke"',
        '    }',
        '  ],',
        '  "labels": {',
        '    "start": 0',
        '  }',
        '}'
    )
    Write-Utf8NoBom -Path (Join-Path $projectDir "main.json") -Lines $script

    Invoke-CiStep "build Windows runtime artifact" {
        cargo build -p visual_novel_gui --bin vn_player --locked --verbose
    }

    $runtime = Join-Path (Get-Location) "target/debug/vn_player.exe"
    if (-not (Test-Path $runtime)) {
        throw "Missing Windows runtime artifact: $runtime"
    }

    $packageLog = Join-Path $logDir "package-windows-smoke.log"
    Invoke-CiStep "vnengine package Windows smoke" {
        cargo run -p vnengine_cli --bin vnengine -- --json package $projectDir `
            --output $bundleDir `
            --target windows `
            --runtime-artifact $runtime `
            --require-executable `
            --integrity hmac-sha256 `
            --hmac-key local-smoke `
            --execute | Tee-Object -FilePath $packageLog
    }

    foreach ($relative in @(
            "game.exe",
            "meta/package_report.json",
            "meta/bundle_file_manifest.json",
            "meta/compat_report.json",
            "meta/bundle.hmac_sha256"
        )) {
        $path = Join-Path $bundleDir $relative
        if (-not (Test-Path $path)) {
            throw "Package smoke missing expected artifact: $relative"
        }
    }

    $envelope = Get-Content -Raw -LiteralPath $packageLog | ConvertFrom-Json
    $report = Get-Content -Raw -LiteralPath (Join-Path $bundleDir "meta/package_report.json") | ConvertFrom-Json
    $compat = Get-Content -Raw -LiteralPath (Join-Path $bundleDir "meta/compat_report.json") | ConvertFrom-Json
    $manifestPath = Join-Path $bundleDir "meta/bundle_file_manifest.json"
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    $signature = Get-Content -Raw -LiteralPath (Join-Path $bundleDir "meta/bundle.hmac_sha256")
    if (-not $envelope.ok -or $envelope.code -ne "ok") {
        throw "Package smoke JSON envelope did not report success"
    }
    if ($envelope.data.schema -ne "vnengine.export_bundle_report.v1") {
        throw "Package smoke JSON envelope did not return an export report"
    }
    if ($report.target_platform -ne "windows" -or $report.executable -ne "game.exe" -or $report.expected_executable -ne "game.exe") {
        throw "Unexpected package report target/executable"
    }
    $runtimeEntry = @($manifest.files | Where-Object { $_.path -eq $report.runtime_artifact }) | Select-Object -First 1
    if ($null -eq $runtimeEntry) {
        throw "Bundle manifest is missing runtime artifact entry: $($report.runtime_artifact)"
    }
    if ($report.runtime_artifact_sha256 -ne $runtimeEntry.sha256 -or $compat.runtime_artifact_sha256 -ne $runtimeEntry.sha256) {
        throw "Runtime artifact sha256 disagrees between package report, compat report and bundle manifest"
    }
    if ($envelope.data.executable -ne $report.executable -or $envelope.data.bundle_hmac_sha256 -ne $report.bundle_hmac_sha256 -or $envelope.data.bundle_file_manifest_sha256 -ne $report.bundle_file_manifest_sha256) {
        throw "CLI JSON envelope and package_report.json disagree"
    }
    if ($compat.generator_os -ne $report.generator_os) {
        throw "Package and compat report generator_os disagree"
    }
    if ($compat.expected_executable -ne $report.expected_executable) {
        throw "Package and compat report expected_executable disagree"
    }
    if ($compat.graphics_backend -ne "software" -or $report.graphics_backend -ne "software" -or -not $compat.wgpu_fallback -or -not $report.wgpu_fallback) {
        throw "Package or compat report did not record software fallback contract"
    }
    if (-not $compat.bundle_file_manifest_sha256 -or -not $report.bundle_file_manifest_sha256 -or $null -eq $compat.diagnostics) {
        throw "Package or compat report is missing manifest hash or structured diagnostics"
    }
    if ($compat.bundle_hmac_sha256 -ne $signature -or $report.bundle_hmac_sha256 -ne $signature) {
        throw "HMAC signature disagrees between report, compat report and signature file"
    }

    $manifestHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $manifestPath).Hash.ToLowerInvariant()
    if ($compat.bundle_file_manifest_sha256 -ne $manifestHash -or $report.bundle_file_manifest_sha256 -ne $manifestHash) {
        throw "Package or compat report manifest hash does not match bundle_file_manifest.json"
    }
    $totalSize = 0
    foreach ($entry in $manifest.files) {
        $reportHashEntry = @($report.hashes | Where-Object { $_.path -eq $entry.path }) | Select-Object -First 1
        $compatHashEntry = @($compat.hashes | Where-Object { $_.path -eq $entry.path }) | Select-Object -First 1
        if ($null -eq $reportHashEntry -or $null -eq $compatHashEntry) {
            throw "Package or compat hashes are missing manifest entry: $($entry.path)"
        }
        if ($reportHashEntry.sha256 -ne $entry.sha256 -or $compatHashEntry.sha256 -ne $entry.sha256 -or [int64]$reportHashEntry.size -ne [int64]$entry.size -or [int64]$compatHashEntry.size -ne [int64]$entry.size) {
            throw "Package or compat hash entry disagrees with manifest entry: $($entry.path)"
        }
        $relativePath = $entry.path -replace '/', [System.IO.Path]::DirectorySeparatorChar
        $path = Join-Path $bundleDir $relativePath
        if (-not (Test-Path $path)) {
            throw "Manifest entry is missing from bundle: $($entry.path)"
        }
        $item = Get-Item -LiteralPath $path
        if ($item.Length -ne [int64]$entry.size) {
            throw "Manifest size mismatch for $($entry.path)"
        }
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
        if ($hash -ne $entry.sha256) {
            throw "Manifest sha256 mismatch for $($entry.path)"
        }
        $totalSize += $item.Length
    }
    if ([int64]$compat.total_size -ne [int64]$totalSize -or [int64]$report.total_size -ne [int64]$totalSize) {
        throw "Package or compat total_size does not match manifest entries"
    }

    $smokeReportPath = Join-Path $bundleDir "meta/runtime_smoke_report.json"
    $gameExe = Join-Path $bundleDir "game.exe"
    Invoke-CiStep "vn_player Windows runtime smoke" {
        & $gameExe --smoke --smoke-report $smokeReportPath
    }

    $smoke = Get-Content -Raw -LiteralPath $smokeReportPath | ConvertFrom-Json
    $report = Get-Content -Raw -LiteralPath (Join-Path $bundleDir "meta/package_report.json") | ConvertFrom-Json
    $compat = Get-Content -Raw -LiteralPath (Join-Path $bundleDir "meta/compat_report.json") | ConvertFrom-Json
    if ($smoke.schema -ne "vnengine.player_runtime_smoke.v1" -or $smoke.status -ne "passed") {
        throw "Runtime smoke did not produce a passed structured report"
    }
    if ($smoke.target_platform -ne "windows" -or $smoke.backend -ne "software") {
        throw "Runtime smoke target/backend does not match package target"
    }
    if ($report.smoke_result.status -ne "passed" -or $compat.smoke_result.status -ne "passed") {
        throw "Runtime smoke result was not propagated back into package and compat reports"
    }
    if ($report.smoke_result.trace_id -ne $smoke.smoke_result.trace_id -or $compat.smoke_result.trace_id -ne $smoke.smoke_result.trace_id) {
        throw "Runtime smoke trace_id disagrees between smoke, package and compat reports"
    }
    $requiredSmokeCodes = @(
        "export.runtime_smoke.asset_load",
        "export.runtime_smoke.render_frame",
        "export.runtime_smoke.advance_scene",
        "export.runtime_smoke.close"
    )
    $smokeCodes = @($smoke.checks | ForEach-Object { $_.code })
    foreach ($code in $requiredSmokeCodes) {
        if ($smokeCodes -notcontains $code) {
            throw "Runtime smoke missing check code: $code"
        }
    }
    foreach ($check in $smoke.checks) {
        if (-not $check.severity -or -not $check.probable_cause -or -not $check.suggested_action -or -not $check.consequence -or $check.trace_id -notlike "export-smoke-*") {
            throw "Runtime smoke check is missing structured diagnostic fields: $($check.code)"
        }
        if ($check.status -eq "passed" -and $check.blocking_release) {
            throw "Passed runtime smoke check must not be release-blocking: $($check.code)"
        }
    }
    $assetLoadCheck = @($smoke.checks | Where-Object { $_.code -eq "export.runtime_smoke.asset_load" }) | Select-Object -First 1
    if (-not $assetLoadCheck.asset) {
        throw "Runtime smoke asset_load check did not record the asset path"
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
        "python-tests",
        "package-windows-smoke"
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
        "package-windows-smoke" { Invoke-PackageWindowsSmokeJob }
    }
}
