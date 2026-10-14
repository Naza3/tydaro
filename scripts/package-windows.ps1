param(
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$OutputDirectory = 'dist'
)

$ErrorActionPreference = 'Stop'
$metadataJson = cargo metadata --locked --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Unable to read Cargo package metadata.' }
$metadata = $metadataJson | ConvertFrom-Json
$package = $metadata.packages | Where-Object { $_.name -eq 'tydaro' } | Select-Object -First 1
if (-not $package) { throw 'The tydaro Cargo package was not found.' }

$executable = Join-Path $metadata.target_directory "$Target/release/tydaro.exe"
if (-not (Test-Path $executable)) {
    throw "Release executable missing: $executable. Run cargo build --locked --release --target $Target --bin tydaro first."
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$bundleName = "tydaro-$($package.version)-windows-x86_64"
$stage = Join-Path ([System.IO.Path]::GetTempPath()) "tydaro-package-$([Guid]::NewGuid())"
New-Item -ItemType Directory -Path $stage | Out-Null
try {
    Copy-Item $executable (Join-Path $stage 'tydaro.exe')
    Copy-Item (Join-Path (Split-Path $package.manifest_path) 'LICENSE') (Join-Path $stage 'LICENSE')
    $commit = git rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify the build commit.' }
    $rustVersion = rustc --version
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify the Rust compiler.' }
    @{
        application = 'Tydaro'
        version = $package.version
        commit = $commit.Trim()
        target = $Target
        compiler = $rustVersion.Trim()
        built_at_utc = (Get-Date).ToUniversalTime().ToString('o')
        data_source = 'Local simulated market data; no brokerage connection'
    } | ConvertTo-Json | Set-Content (Join-Path $stage 'build-info.json') -Encoding utf8
    @'
Tydaro desktop preview

Extract this archive, then start tydaro.exe on a 64-bit Windows 10/11 computer.
The application uses local simulated data. No account or API credentials are needed.
This preview is not signed and does not send orders to a brokerage.
If Windows reports a missing VCRUNTIME/MSVCP DLL, install Microsoft's current
Visual C++ Redistributable for Visual Studio 2015-2022 (x64).

Project and feedback: https://github.com/Naza3/tydaro
'@ | Set-Content (Join-Path $stage 'README.txt') -Encoding utf8
    $archive = Join-Path $OutputDirectory "$bundleName.zip"
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -Force
    $checksum = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$checksum  $bundleName.zip" | Set-Content "$archive.sha256" -Encoding ascii
    Write-Host "Packaged $archive"
    Write-Host "SHA-256: $checksum"
} finally {
    Remove-Item -Recurse -Force $stage
}
