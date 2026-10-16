param(
    [string]$Executable,
    [string]$OutputDirectory = 'artifacts/windows-smoke',
    [ValidateRange(1, 600)]
    [int]$TimeoutSeconds = 360
)

$ErrorActionPreference = 'Stop'
if (-not $Executable) {
    $metadataJson = cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Unable to read Cargo package metadata.' }
    $metadata = $metadataJson | ConvertFrom-Json
    $Executable = Join-Path $metadata.target_directory 'x86_64-pc-windows-msvc/release/tydaro.exe'
}
$Executable = (Resolve-Path -LiteralPath $Executable).Path
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$outputPath = (Resolve-Path -LiteralPath $OutputDirectory).Path
$previousDataDirectory = $env:TYDARO_DATA_DIR
$env:TYDARO_DATA_DIR = Join-Path $outputPath 'data'

# Windows supports native layout and event regressions here, but the pinned
# GPUI headless backend cannot produce pixel screenshots. Keep logs as evidence.
$scenarios = @(
    @{ Name = 'architecture'; Flag = '--architecture-smoke-test'; Output = 'workspace' },
    @{ Name = 'gallery'; Flag = '--gallery-smoke-test'; Output = 'gallery.png' }
)
try {
    foreach ($scenario in $scenarios) {
        $stdout = Join-Path $outputPath "$($scenario.Name).stdout.log"
        $stderr = Join-Path $outputPath "$($scenario.Name).stderr.log"
        $destination = Join-Path $outputPath $scenario.Output
        Write-Host "Running Windows $($scenario.Name) interaction regression..."
        $process = Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru `
            -ArgumentList @($scenario.Flag, "`"$destination`"") `
            -RedirectStandardOutput $stdout -RedirectStandardError $stderr
        try {
            # Keep the native handle alive so PowerShell can read the exit code
            # after the child exits (Start-Process alone can leave it null).
            $null = $process.Handle
            if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
                Stop-Process -Id $process.Id -Force
                throw "$($scenario.Name) regression exceeded $TimeoutSeconds seconds. Logs: $outputPath"
            }
            if ($process.ExitCode -ne 0) {
                Get-Content -LiteralPath $stdout -Tail 60
                Get-Content -LiteralPath $stderr -Tail 60
                throw "$($scenario.Name) regression failed with exit code $($process.ExitCode)."
            }
            Write-Host "PASS: $($scenario.Name) (native layout/events; no pixel rendering)."
        } finally {
            $process.Dispose()
        }
    }
} finally {
    $env:TYDARO_DATA_DIR = $previousDataDirectory
}
