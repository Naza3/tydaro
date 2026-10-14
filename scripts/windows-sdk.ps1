# Run this script before a native Windows Release build. GPUI compiles its HLSL
# shaders with the Windows SDK's fxc.exe; a Rust target alone is not sufficient.
$ErrorActionPreference = 'Stop'

$sdkBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
if (-not (Test-Path $sdkBin)) {
    throw 'Windows SDK missing. Install Visual Studio Build Tools with Desktop development with C++ and a Windows 10/11 SDK.'
}

$compiler = Get-ChildItem $sdkBin -Directory |
    Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } |
    Sort-Object { [Version]$_.Name } -Descending |
    ForEach-Object { Join-Path $_.FullName 'x64\fxc.exe' } |
    Where-Object { Test-Path $_ } |
    Select-Object -First 1

if (-not $compiler) {
    throw 'No x64 fxc.exe found in the installed Windows SDKs.'
}

$env:GPUI_FXC_PATH = $compiler
$env:PATH = "$(Split-Path $compiler);$env:PATH"
if ($env:GITHUB_ENV) {
    "GPUI_FXC_PATH=$compiler" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
    Split-Path $compiler | Out-File -FilePath $env:GITHUB_PATH -Append -Encoding utf8
}
Write-Host "Using Windows SDK shader compiler: $compiler"
