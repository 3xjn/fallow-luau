# Install fallow-luau from GitHub Releases (or cargo as fallback).
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.ps1 | iex
#   & ([scriptblock]::Create((irm ...))) -Version v0.1.0
[CmdletBinding()]
param(
    [string]$Version = $env:FALLOW_LUAU_VERSION,
    [string]$InstallDir = $(if ($env:FALLOW_LUAU_INSTALL_DIR) { $env:FALLOW_LUAU_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "fallow-luau\bin" }),
    [string]$Repo = $(if ($env:FALLOW_LUAU_REPO) { $env:FALLOW_LUAU_REPO } else { "3xjn/fallow-luau" })
)

$ErrorActionPreference = "Stop"
$Triple = "x86_64-pc-windows-msvc"
$Headers = @{ "User-Agent" = "fallow-luau-install"; "Accept" = "application/vnd.github+json" }

function Install-FromCargo {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "No GitHub Release found and cargo is not on PATH. Install Rust from https://rustup.rs or wait for a release."
    }
    Write-Host "fallow-luau: no release asset; installing via cargo --git…"
    & cargo install --git "https://github.com/$Repo" --locked fallow-luau
    & fallow-luau --version
}

try {
    if ($Version) {
        if ($Version -notmatch '^v') { $Version = "v$Version" }
        $releaseUrl = "https://api.github.com/repos/$Repo/releases/tags/$Version"
    } else {
        $releaseUrl = "https://api.github.com/repos/$Repo/releases/latest"
    }

    Write-Host "fallow-luau: fetching release metadata…"
    try {
        $release = Invoke-RestMethod -Uri $releaseUrl -Headers $Headers
    } catch {
        Install-FromCargo
        return
    }

    $tag = $release.tag_name
    $ver = $tag.TrimStart('v')
    $assetName = "fallow-luau-$ver-$Triple.zip"
    $asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
    if (-not $asset) {
        $assetName = "fallow-luau-$Triple.zip"
        $asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
    }
    if (-not $asset) {
        throw "No Windows asset ($Triple) in release $tag. See https://github.com/$Repo/releases/tag/$tag"
    }

    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("fallow-luau-" + [guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        $zip = Join-Path $tmp "pkg.zip"
        Write-Host "fallow-luau: downloading $($asset.browser_download_url)"
        Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zip -Headers @{ "User-Agent" = "fallow-luau-install" }
        Expand-Archive -LiteralPath $zip -DestinationPath $tmp -Force

        $exe = Get-ChildItem -Path $tmp -Recurse -Filter "fallow-luau.exe" | Select-Object -First 1
        if (-not $exe) { throw "Archive did not contain fallow-luau.exe" }

        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        $dest = Join-Path $InstallDir "fallow-luau.exe"
        Copy-Item -Force $exe.FullName $dest

        $pathEntry = $InstallDir
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if (-not $userPath) { $userPath = "" }
        $parts = $userPath -split ';' | Where-Object { $_ -and $_.Trim() -ne "" }
        if ($parts -notcontains $pathEntry) {
            [Environment]::SetEnvironmentVariable("Path", (($parts + $pathEntry) -join ';'), "User")
            $env:Path = "$env:Path;$pathEntry"
            Write-Host "fallow-luau: added $pathEntry to your user PATH (open a new terminal if needed)"
        }

        Write-Host "fallow-luau: installed $dest ($tag)"
        & $dest --version
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
} catch {
    Write-Error $_
    exit 1
}
