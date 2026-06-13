# Canonical dev workflow on Windows — the PowerShell sibling of run-qt.sh.
#
# Builds the Rust `meeting-server` sidecar + the Qt GUI, copies the sidecar
# next to the GUI binary (so SidecarManager finds it via applicationDirPath),
# then runs the GUI.
#
# Usage:  powershell -ExecutionPolicy Bypass -File run-qt.ps1 [options]
#   -Debug        Debug build of the Rust sidecar (default: release)
#   -SkipRust     Don't rebuild the Rust sidecar
#   -SkipBuild    Don't reconfigure/rebuild the Qt app
#   -NoRun        Build only; do not launch the GUI
#   -Cuda         Build the sidecar with the CUDA Whisper backend (needs CUDA Toolkit)
#   -Vulkan       Build the sidecar with the Vulkan Whisper backend (needs Vulkan SDK)
#
# Dev-only:
#   $env:FIRST_RUN = "1"; .\run-qt.ps1
#       Launch with fresh isolated temporary data/config dirs so the app behaves
#       like a clean first install without touching real meetings/settings.
#
# Toolchain: unlike Linux (apt puts tools on PATH globally), the MSVC compiler
# here lives behind an activation shim. If cl.exe is not already on PATH, this
# script sources $env:MA_BUILD_ENV (e.g. E:\build-env.cmd) to activate the
# portable MSVC + Rust + CMake/Ninja + Qt toolchain — the equivalent of opening
# a Developer Command Prompt. Run from such a prompt and it's skipped.
#
# Qt discovery (first hit wins): $env:CMAKE_PREFIX_PATH -> $env:QT_DIR
#                                -> %USERPROFILE%\Qt\<ver>\msvc* -> qmake on PATH

# NOTE: no [CmdletBinding()] — it auto-adds a -Debug common parameter that
# collides with our own -Debug switch.
param(
    [switch]$Debug,
    [switch]$SkipRust,
    [switch]$SkipBuild,
    [switch]$NoRun,
    [switch]$Cuda,
    [switch]$Vulkan
)
$ErrorActionPreference = "Stop"

$ScriptDir = $PSScriptRoot
$RustDir   = Join-Path $ScriptDir "rust"
$QtAppDir  = Join-Path $ScriptDir "qt-app"
$BuildDir  = Join-Path $QtAppDir "build"
$Profile   = if ($Debug) { "debug" } else { "release" }

# ── Activate the build toolchain if MSVC isn't already on PATH ────────────────
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    $envCmd = $env:MA_BUILD_ENV
    if ($envCmd -and (Test-Path $envCmd)) {
        Write-Host "-> Activating build env: $envCmd"
        cmd /c "call `"$envCmd`" && set" | ForEach-Object {
            if ($_ -match '^([^=]+)=(.*)$') {
                Set-Item -Path "env:$($matches[1])" -Value $matches[2]
            }
        }
    } else {
        throw "MSVC (cl.exe) is not on PATH. Open a Developer Command Prompt, or point `$env:MA_BUILD_ENV at a build-env.cmd that activates the toolchain."
    }
}

# ── Locate Qt 6 ──────────────────────────────────────────────────────────────
function Find-QtPrefix {
    if ($env:CMAKE_PREFIX_PATH) { return $env:CMAKE_PREFIX_PATH }
    if ($env:QT_DIR)            { return $env:QT_DIR }
    $qtRoot = Join-Path $env:USERPROFILE "Qt"
    if (Test-Path $qtRoot) {
        $ver = Get-ChildItem $qtRoot -Directory | Where-Object Name -match '^6\.' |
               Sort-Object Name | Select-Object -Last 1
        if ($ver) {
            $kit = Get-ChildItem $ver.FullName -Directory | Where-Object Name -match 'msvc' |
                   Sort-Object Name | Select-Object -Last 1
            if ($kit) { return $kit.FullName }
        }
    }
    $qmake = Get-Command qmake.exe -ErrorAction SilentlyContinue
    if ($qmake) { return (Split-Path (Split-Path $qmake.Source)) }
    return $null
}

# ── Rust sidecar ─────────────────────────────────────────────────────────────
if (-not $SkipRust) {
    $relFlag     = if ($Debug)  { @() } else { @("--release") }
    $featureFlag = @()
    if     ($Cuda)   { $featureFlag = @("--features","whisper-cuda");   Write-Host "-> Building meeting-server sidecar ($Profile, whisper-cuda)..." }
    elseif ($Vulkan) { $featureFlag = @("--features","whisper-vulkan"); Write-Host "-> Building meeting-server sidecar ($Profile, whisper-vulkan)..." }
    else             { Write-Host "-> Building meeting-server sidecar ($Profile)..." }
    # Plain $relFlag/$featureFlag, NOT @-splat: PowerShell unwraps a single-element
    # array (@("--release")) to a scalar string, and splatting a scalar iterates
    # its characters — passing a lone "-" first (cargo: unexpected argument '-').
    # A native command expands a plain array/scalar as args correctly.
    cargo build $relFlag $featureFlag --manifest-path "$RustDir\Cargo.toml" --bin meeting-server
    if ($LASTEXITCODE) { throw "cargo build failed" }
    Write-Host "OK Sidecar built"
}

$Sidecar = Join-Path $RustDir "target\$Profile\meeting-server.exe"
if (-not (Test-Path $Sidecar)) {
    throw "$Sidecar not found. Run without -SkipRust."
}

# ── Qt GUI ───────────────────────────────────────────────────────────────────
$QtPrefix = Find-QtPrefix
if (-not $QtPrefix) { throw "Could not locate Qt 6. Set CMAKE_PREFIX_PATH or QT_DIR." }
Write-Host "-> Qt prefix: $QtPrefix"

if (-not $SkipBuild) {
    $gen = if (Get-Command ninja -ErrorAction SilentlyContinue) { @("-G","Ninja") } else { @() }
    Write-Host "-> Configuring qt-app..."
    cmake -S $QtAppDir -B $BuildDir $gen `
        -DCMAKE_BUILD_TYPE=Release `
        -DCMAKE_PREFIX_PATH="$QtPrefix"
    if ($LASTEXITCODE) { throw "cmake configure failed" }
    Write-Host "-> Building qt-app..."
    cmake --build $BuildDir --parallel --config Release
    if ($LASTEXITCODE) { throw "cmake build failed" }
    Write-Host "OK Qt app built"
}

# Resolve the GUI binary (Ninja: build root; VS generator: build\Release).
$GuiBin = Join-Path $BuildDir "meeting-assistant-qt.exe"
if (-not (Test-Path $GuiBin)) {
    $GuiBin = Get-ChildItem $BuildDir -Recurse -Filter "meeting-assistant-qt.exe" -ErrorAction SilentlyContinue |
              Select-Object -First 1 -ExpandProperty FullName
}
if (-not $GuiBin -or -not (Test-Path $GuiBin)) {
    throw "GUI binary not found under $BuildDir. Run without -SkipBuild."
}

# Co-locate the sidecar with the GUI — SidecarManager looks in applicationDirPath().
Copy-Item -Force $Sidecar (Join-Path (Split-Path $GuiBin) "meeting-server.exe")
Write-Host "OK Sidecar staged next to GUI: $(Split-Path $GuiBin)\meeting-server.exe"

if ($NoRun) {
    Write-Host "-> Build complete (-NoRun). GUI: $GuiBin"
    return
}

# Qt runtime: a dev run (no windeployqt) needs Qt's DLLs / plugins / QML modules.
$env:PATH = "$QtPrefix\bin;$env:PATH"
if (-not $env:QT_PLUGIN_PATH)   { $env:QT_PLUGIN_PATH   = "$QtPrefix\plugins" }
if (-not $env:QML2_IMPORT_PATH) { $env:QML2_IMPORT_PATH = "$QtPrefix\qml" }

if ($env:FIRST_RUN -eq "1") {
    if (-not $env:FIRST_RUN_DIR) {
        $env:FIRST_RUN_DIR = Join-Path $env:TEMP ("meeting-assistant-first-run-" + [guid]::NewGuid().ToString("N").Substring(0,8))
    }
    New-Item -ItemType Directory -Force -Path "$env:FIRST_RUN_DIR\data","$env:FIRST_RUN_DIR\config" | Out-Null
    $env:XDG_DATA_HOME   = "$env:FIRST_RUN_DIR\data"
    $env:XDG_CONFIG_HOME = "$env:FIRST_RUN_DIR\config"
    Write-Host "-> FIRST_RUN=1: using isolated app data under $env:FIRST_RUN_DIR"
}

Write-Host "-> Launching Meeting Assistant (Qt)..."
& $GuiBin
