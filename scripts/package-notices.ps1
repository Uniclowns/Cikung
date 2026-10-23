param([string]$ProjectRoot, [string]$OutputFolder)
$ErrorActionPreference = 'Stop'
$cargoCache = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$registryRoot = Join-Path $cargoCache 'registry\src'
$destination = Join-Path $OutputFolder 'licenses\crates'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $ProjectRoot 'Cargo.lock') -Destination (Join-Path $OutputFolder 'Cargo.lock') -Force
$lock = Get-Content -LiteralPath (Join-Path $ProjectRoot 'Cargo.lock') -Raw
$packages = [regex]::Matches($lock, '(?m)^name = "([^"]+)"\r?\nversion = "([^"]+)"')
Copy-Item -LiteralPath (Join-Path $ProjectRoot 'assets\fonts\OFL-Figtree.txt') -Destination (Join-Path $OutputFolder 'licenses\OFL-Figtree.txt') -Force
$inventory = @('Dependency license inventory. Exact dependency versions: Cargo.lock.', 'Figtree: SIL Open Font License 1.1 (licenses/OFL-Figtree.txt).', '')
foreach ($package in $packages) {
    $packageName = $package.Groups[1].Value
    $packageVersion = $package.Groups[2].Value
    $identity = "$packageName-$packageVersion"
    $source = Get-ChildItem -Path $registryRoot -Directory -ErrorAction SilentlyContinue | ForEach-Object { Join-Path $_.FullName $identity } | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $source) { continue }
    $manifest = Get-Content -LiteralPath (Join-Path $source 'Cargo.toml') -Raw
    $licenseMatch = [regex]::Match($manifest, '(?m)^license = "([^"]+)"')
    $inventory += "$identity : $($licenseMatch.Groups[1].Value)"
    $notices = Get-ChildItem -LiteralPath $source -File | Where-Object { $_.Name -match '^(licen[cs]e|copying|copyright|notice|readme)([._-]|$)' }
    if ($notices) {
        $packageFolder = Join-Path $destination $identity
        New-Item -ItemType Directory -Path $packageFolder -Force | Out-Null
        foreach ($notice in $notices) {
            Copy-Item -LiteralPath $notice.FullName -Destination $packageFolder -Force
            $copied = Get-Item -LiteralPath (Join-Path $packageFolder $notice.Name)
            if ($copied.LastWriteTime.Year -lt 1980) { $copied.LastWriteTime = [datetime]'1980-01-01' }
        }
    }
}
Set-Content -LiteralPath (Join-Path $OutputFolder 'licenses\dependency-inventory.txt') -Value $inventory
