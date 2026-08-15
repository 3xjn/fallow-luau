# irm https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.ps1 | iex
$ErrorActionPreference = 'Stop'
$r = '3xjn/fallow-luau'
$d = if ($env:FALLOW_LUAU_INSTALL_DIR) { $env:FALLOW_LUAU_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'fallow-luau\bin' }
try {
  $rel = Invoke-RestMethod "https://api.github.com/repos/$r/releases/latest"
} catch {
  throw "No GitHub Release yet for $r. Wait for the release workflow, then retry: https://github.com/$r/releases"
}
$v = $rel.tag_name.TrimStart('v')
$asset = $rel.assets | Where-Object name -eq "fallow-luau-$v-x86_64-pc-windows-msvc.zip" | Select-Object -First 1
if (-not $asset) { throw "Release $($rel.tag_name) has no Windows zip yet." }
$tmp = Join-Path $env:TEMP ([guid]::NewGuid())
New-Item $tmp -ItemType Directory | Out-Null
try {
  $zip = Join-Path $tmp 'p.zip'
  Invoke-WebRequest $asset.browser_download_url -OutFile $zip
  Expand-Archive $zip $tmp -Force
  New-Item $d -ItemType Directory -Force | Out-Null
  Copy-Item (Get-ChildItem $tmp -Recurse -Filter fallow-luau.exe | Select-Object -First 1).FullName (Join-Path $d 'fallow-luau.exe') -Force
  $p = [Environment]::GetEnvironmentVariable('Path', 'User')
  if ($p -notlike "*$d*") { [Environment]::SetEnvironmentVariable('Path', "$p;$d", 'User'); $env:Path += ";$d" }
  & (Join-Path $d 'fallow-luau.exe') --version
} finally { Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue }
