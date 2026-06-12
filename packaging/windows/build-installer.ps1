# Build the Windows two-binary artefact end-to-end:
#   cargo build (meeting-server.exe)  ->  cmake build + install (GUI + helper)
#   ->  windeployqt (Qt6 DLLs / plugins / QML runtime)
#   ->  Inno Setup -> MeetingAssistant-Setup-<ver>.exe
#
# No code-signing cert in v1 (fast-follow). Unsigned -> one-click SmartScreen
# "More info -> Run anyway"; see SMARTSCREEN.md.
#
# Usage:  pwsh packaging/windows/build-installer.ps1 [-Debug]
#
# Qt discovery: $env:CMAKE_PREFIX_PATH -> $env:QT_DIR -> ~\Qt\<ver>\msvc* .
[CmdletBinding()]
param([switch]$Debug)
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Repo      = (Resolve-Path "$ScriptDir\..\..").Path
$RustDir   = Join-Path $Repo "rust"
$QtAppDir  = Join-Path $Repo "qt-app"
$BuildDir  = Join-Path $QtAppDir "build-windows-pkg"
$Dist      = Join-Path $Repo "dist\windows"
$Stage     = Join-Path $Dist "stage"
$Profile   = if ($Debug) { "debug" } else { "release" }

function Find-QtPrefix {
    if ($env:CMAKE_PREFIX_PATH) { return $env:CMAKE_PREFIX_PATH }
    if ($env:QT_DIR)            { return $env:QT_DIR }
    $qtRoot = Join-Path $env:USERPROFILE "Qt"
    if (Test-Path $qtRoot) {
        $ver = Get-ChildItem $qtRoot -Directory |
            Where-Object Name -match '^6\.' |
            Sort-Object Name | Select-Object -Last 1
        if ($ver) {
            $kit = Get-ChildItem $ver.FullName -Directory |
                Where-Object Name -match 'msvc' |
                Sort-Object Name | Select-Object -Last 1
            if ($kit) { return $kit.FullName }
        }
    }
    throw "Qt 6 not found. Set CMAKE_PREFIX_PATH to the Qt msvc kit."
}

$QtPrefix    = Find-QtPrefix
$WinDeployQt = Join-Path $QtPrefix "bin\windeployqt.exe"
if (-not (Test-Path $WinDeployQt)) { throw "windeployqt not at $WinDeployQt" }
Write-Host "-> Qt prefix:   $QtPrefix"
Write-Host "-> windeployqt: $WinDeployQt"

# 1. Rust sidecar (same revision as the GUI; version-pinned together).
Write-Host "-> cargo build meeting-server ($Profile)..."
$relFlag = if ($Debug) { @() } else { @("--release") }
cargo build @relFlag --manifest-path "$RustDir\Cargo.toml" --bin meeting-server
if ($LASTEXITCODE) { throw "cargo build failed" }
$Sidecar = Join-Path $RustDir "target\$Profile\meeting-server.exe"
if (-not (Test-Path $Sidecar)) { throw "sidecar not built: $Sidecar" }

# 2. Configure + build + install (CMake Windows branch: GUI + helper).
Write-Host "-> cmake configure + build + install..."
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
$gen = if (Get-Command ninja -ErrorAction SilentlyContinue) { @("-G","Ninja") } else { @() }
cmake -S $QtAppDir -B $BuildDir @gen `
    -DCMAKE_BUILD_TYPE=Release `
    -DCMAKE_PREFIX_PATH="$QtPrefix" `
    -DMEETING_SERVER_BIN="$Sidecar" `
    -DCMAKE_INSTALL_PREFIX="$Stage"
if ($LASTEXITCODE) { throw "cmake configure failed" }
cmake --build $BuildDir --parallel --config Release
if ($LASTEXITCODE) { throw "cmake build failed" }
cmake --install $BuildDir
if ($LASTEXITCODE) { throw "cmake install failed" }

$Gui = Join-Path $Stage "meeting-assistant-qt.exe"
if (-not (Test-Path $Gui)) { throw "GUI exe not installed: $Gui" }
if (-not (Test-Path (Join-Path $Stage "meeting-server.exe"))) {
    throw "helper not installed beside GUI (CMake rule broken)"
}

# 3. windeployqt — Qt6 DLLs / plugins / QML beside the GUI. It does NOT touch
#    meeting-server.exe (the CMake install rule already placed it).
Write-Host "-> windeployqt..."
& $WinDeployQt --release --qmldir "$QtAppDir\qml" --compiler-runtime `
    --no-translations "$Gui"
if ($LASTEXITCODE) { throw "windeployqt failed" }

# Fail-fast: the in-card audio player needs the QtMultimedia ffmpeg backend.
# windeployqt bundles it from the QtMultimedia QML import, but assert it landed
# (a missing plugin ships a dead player). Inno Setup packs Stage\* recursively,
# so once it's here it reaches the installer.
$mmPlugin = Get-ChildItem -Path $Stage -Recurse -Filter "ffmpegmediaplugin.dll" `
    -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $mmPlugin) { throw "QtMultimedia ffmpeg plugin not bundled by windeployqt" }
Write-Host "-> QtMultimedia backend bundled: $($mmPlugin.FullName)"

# 4. Inno Setup -> installer.
$Iscc = Get-Command iscc.exe -ErrorAction SilentlyContinue
if (-not $Iscc) {
    $cand = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    if (Test-Path $cand) { $Iscc = $cand } else { throw "Inno Setup (ISCC.exe) not found" }
} else { $Iscc = $Iscc.Source }

$Version = (Get-Item $Gui).VersionInfo.ProductVersion
if (-not $Version) { $Version = "0.1.0" }
Write-Host "-> Inno Setup (v$Version)..."
& $Iscc "/DMyAppVersion=$Version" "/DStageDir=$Stage" `
    "/O$Dist" "$ScriptDir\installer.iss"
if ($LASTEXITCODE) { throw "Inno Setup failed" }

Write-Host "OK installer: $Dist\MeetingAssistant-Setup-$Version.exe"
Write-Host "   Unsigned -> SmartScreen one-click bypass (SMARTSCREEN.md)."
Write-Host "   Qt6 dynamically linked + written offer shipped under licenses\"
Write-Host "   (LGPLv3-compliant)."
