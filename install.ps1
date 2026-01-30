# mcp-hwp Installer Script for Windows
# Usage: iwr -useb https://raw.githubusercontent.com/mrchypark/mcp-hwp/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "mrchypark/mcp-hwp"
$BinaryName = "mcp-hwp.exe"
$InstallDir = $env:INSTALL_DIR
if (-not $InstallDir) {
    $InstallDir = "$env:LOCALAPPDATA\Programs"
}

# Detect architecture
$Arch = if ([Environment]::Is64BitOperatingSystem) {
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
        "aarch64-pc-windows-msvc"
    } else {
        "x86_64-pc-windows-msvc"
    }
} else {
    Write-Error "32-bit Windows is not supported"
    exit 1
}

Write-Host "Detected architecture: $Arch"

# Get latest release
Write-Host "Fetching latest release..."
$LatestRelease = (Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest").tag_name

if (-not $LatestRelease) {
    Write-Error "Could not fetch latest release"
    exit 1
}

Write-Host "Latest release: $LatestRelease"

# Download URL
$DownloadUrl = "https://github.com/$Repo/releases/download/$LatestRelease/${BinaryName}-${Arch}.zip"

# Create temp directory
$TempDir = Join-Path $env:TEMP ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null

try {
    Write-Host "Downloading from: $DownloadUrl"
    $ZipPath = Join-Path $TempDir "$BinaryName.zip"
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath

    Write-Host "Extracting..."
    Expand-Archive -Path $ZipPath -DestinationPath $TempDir -Force

    # Create install directory if not exists
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Install
    $InstallPath = Join-Path $InstallDir $BinaryName
    Write-Host "Installing to $InstallPath..."
    Move-Item -Path (Join-Path $TempDir $BinaryName) -Destination $InstallPath -Force

    # Add to PATH if not already there
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host "Adding $InstallDir to PATH..."
        [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
        Write-Host "⚠️  Please restart your terminal or run 'refreshenv' to update PATH"
    }

    Write-Host "✅ Successfully installed $BinaryName!"
    Write-Host "Version: $(&$InstallPath --version)"
    Write-Host ""
    Write-Host "Usage: $BinaryName --help"
}
finally {
    # Cleanup
    Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
}
