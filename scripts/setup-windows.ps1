#Requires -Version 5.1
<#
.SYNOPSIS
    Sets up the Windows sender for the Virtual Display Extender project.
.DESCRIPTION
    Checks prerequisites (Rust, GStreamer), builds the win-sender crate,
    and optionally installs the IddCx Virtual Display Driver.
#>

param(
    [switch]$InstallVirtualDisplay
)

$ErrorActionPreference = "Stop"

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "    OK: $msg" -ForegroundColor Green }
function Write-Fail($msg) { Write-Host "    FAIL: $msg" -ForegroundColor Red }

# --- Check Rust -----------------------------------------------------------
Write-Step "Checking Rust toolchain"
$rustc = Get-Command rustc -ErrorAction SilentlyContinue
if (-not $rustc) {
    Write-Fail "rustc not found. Install from https://rustup.rs/"
    exit 1
}
$rustVersion = & rustc --version
Write-Ok $rustVersion

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Write-Fail "cargo not found. Install from https://rustup.rs/"
    exit 1
}

# --- Check GStreamer -------------------------------------------------------
Write-Step "Checking GStreamer"
$gstInspect = Get-Command gst-inspect-1.0 -ErrorAction SilentlyContinue
if (-not $gstInspect) {
    Write-Fail "GStreamer not found."
    Write-Host @"
    Install GStreamer from https://gstreamer.freedesktop.org/download/
    - Download the MSVC 64-bit installer (runtime AND development).
    - Ensure 'Complete' installation is selected to include all plugins.
    - Add GStreamer bin directory to PATH:
        C:\gstreamer\1.0\msvc_x86_64\bin
    - Set environment variables:
        GSTREAMER_1_0_ROOT_MSVC_X86_64=C:\gstreamer\1.0\msvc_x86_64
        PKG_CONFIG_PATH=C:\gstreamer\1.0\msvc_x86_64\lib\pkgconfig
"@ -ForegroundColor Yellow
    exit 1
}

$gstVersion = & gst-inspect-1.0 --version | Select-Object -First 1
Write-Ok $gstVersion

# Check required plugins
$requiredPlugins = @("d3d11screencapturesrc", "mfh264enc", "rtph264pay", "udpsink")
$missingPlugins = @()
foreach ($plugin in $requiredPlugins) {
    $result = & gst-inspect-1.0 $plugin 2>&1
    if ($LASTEXITCODE -ne 0) {
        $missingPlugins += $plugin
    }
}

if ($missingPlugins.Count -gt 0) {
    Write-Host "    Warning: Missing GStreamer elements: $($missingPlugins -join ', ')" -ForegroundColor Yellow
    Write-Host "    The sender has fallback encoders, but d3d11screencapturesrc is required." -ForegroundColor Yellow
} else {
    Write-Ok "All required GStreamer plugins found"
}

# --- Check pkg-config (needed for gstreamer-rs build) ----------------------
Write-Step "Checking pkg-config"
$pkgConfig = Get-Command pkg-config -ErrorAction SilentlyContinue
if (-not $pkgConfig) {
    Write-Host "    Warning: pkg-config not found. The gstreamer crate needs it." -ForegroundColor Yellow
    Write-Host "    Install via: choco install pkgconfiglite" -ForegroundColor Yellow
    Write-Host "    Or set GSTREAMER_1_0_ROOT_MSVC_X86_64 env var." -ForegroundColor Yellow
} else {
    Write-Ok "pkg-config found"
}

# --- Build win-sender ------------------------------------------------------
Write-Step "Building win-sender"
$senderDir = Join-Path $PSScriptRoot "..\win-sender"
Push-Location $senderDir
try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Fail "cargo build failed"
        exit 1
    }
    Write-Ok "Build successful: target\release\win-sender.exe"
} finally {
    Pop-Location
}

# --- Optional: Virtual Display Driver --------------------------------------
if ($InstallVirtualDisplay) {
    Write-Step "Virtual Display Driver"
    $driverDir = "$env:ProgramData\Virtual Display Driver"
    if (Test-Path $driverDir) {
        Write-Ok "Driver config directory already exists: $driverDir"
    } else {
        Write-Host "    The IddCx Virtual Display Driver must be installed manually." -ForegroundColor Yellow
        Write-Host "    Download from: https://github.com/itsmikethetech/Virtual-Display-Driver" -ForegroundColor Yellow
        Write-Host "    Follow the installation instructions in the README." -ForegroundColor Yellow
    }
}

# --- Done ------------------------------------------------------------------
Write-Step "Setup complete!"
Write-Host @"

    Usage:
        cd win-sender
        cargo run --release -- --host RECEIVER_IP --monitor 0

    Flags:
        --list-monitors       Show available monitors
        --virtual-display     Create and capture a virtual monitor (requires IddCx driver)
        --test-stream         Stream to localhost for 5 seconds (verification test)
        --help                Show all options
"@
