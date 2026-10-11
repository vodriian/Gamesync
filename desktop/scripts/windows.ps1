# Load the MSVC and Windows SDK environment for the native desktop package.
param(
    [ValidateSet('build', 'test', 'clippy', 'fmt')]
    [string]$Command = 'build',
    [switch]$Release,
    [switch]$UpdateLock
)

$ErrorActionPreference = 'Stop'
$desktopRoot = Split-Path -Parent $PSScriptRoot
$rustBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:PATH = "$rustBin;C:\Program Files\Git\cmd;$env:PATH"
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw 'Install Visual Studio Build Tools with the Desktop development with C++ workload.'
}
$installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $installation) {
    throw 'The MSVC x64 build tools are unavailable.'
}
$vcvars = Join-Path $installation 'VC\Auxiliary\Build\vcvars64.bat'
# Import only the environment produced by the installed Microsoft tool script.
$environment = & $env:ComSpec /d /c "call `"$vcvars`" >nul && set"
if ($LASTEXITCODE -ne 0) { throw 'Could not load the MSVC environment.' }
foreach ($line in $environment) {
    if ($line -match '^([^=]+)=(.*)$') {
        [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
    }
}
$env:GPUI_FXC_PATH = (Get-Command fxc.exe -ErrorAction Stop).Source
$cargoArgs = @($Command, '--manifest-path', (Join-Path $desktopRoot 'Cargo.toml'))
if ($Command -eq 'fmt') {
    $cargoArgs += '--check'
} else {
    if (-not $UpdateLock) { $cargoArgs += '--locked' }
    if ($Release) { $cargoArgs += '--release' }
    if ($Command -eq 'clippy') { $cargoArgs += @('--all-targets', '--', '-D', 'warnings') }
}
& (Join-Path $rustBin 'cargo.exe') @cargoArgs
exit $LASTEXITCODE
