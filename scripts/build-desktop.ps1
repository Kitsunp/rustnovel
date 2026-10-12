param()

$ErrorActionPreference = 'Stop'
$desktopRepo = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $desktopRepo
try {
    # Package-scoped builds avoid unifying the optional Python binding features.
    cargo build --package visual_novel_gui --bins --profile desktop --locked
    if ($LASTEXITCODE -ne 0) { throw "Desktop build failed (exit $LASTEXITCODE)" }

    $desktopTarget = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
    $desktopOutput = Join-Path $desktopTarget 'desktop'
    $desktopSuffix = if ($IsWindows -or $env:OS -eq 'Windows_NT') { '.exe' } else { '' }
    foreach ($desktopName in @('vn_editor', 'vn_player')) {
        $desktopPath = Join-Path $desktopOutput ($desktopName + $desktopSuffix)
        if (-not (Test-Path -LiteralPath $desktopPath -PathType Leaf)) {
            throw "Expected executable missing: $desktopPath"
        }
        $desktopFile = Get-Item -LiteralPath $desktopPath
        Write-Output ('{0}: {1:N2} MiB' -f $desktopFile.FullName, ($desktopFile.Length / 1MB))
        # Exercise the loader and argument parsing without opening an app window.
        if ($desktopSuffix -eq '.exe') {
            $desktopProbe = Start-Process -FilePath $desktopFile.FullName -ArgumentList '--help' -WindowStyle Hidden -PassThru
            if (-not $desktopProbe.WaitForExit(10000)) {
                Stop-Process -Id $desktopProbe.Id
                throw "$desktopName startup check timed out"
            }
            if ($desktopProbe.ExitCode -ne 0) {
                throw "$desktopName startup failed (exit $($desktopProbe.ExitCode))"
            }
        } else {
            & $desktopFile.FullName --help
            if ($LASTEXITCODE -ne 0) { throw "$desktopName startup failed (exit $LASTEXITCODE)" }
        }
    }
} finally {
    Pop-Location
}
