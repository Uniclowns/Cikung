param(
    [ValidateSet('Debug', 'Release')][string]$Configuration = 'Release',
    [switch]$Test
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    $localCargo = Join-Path $projectRoot '.tools\cargo\bin\cargo.exe'
    $cargoArgs = @()
    $targetName = $null
    if (Test-Path -LiteralPath $localCargo) {
        $env:CARGO_HOME = Join-Path $projectRoot '.tools\cargo'
        $env:RUSTUP_HOME = Join-Path $projectRoot '.tools\rustup'
        $cargoPath = $localCargo
        $targetName = 'x86_64-pc-windows-gnu'
        $gccPath = Join-Path $env:RUSTUP_HOME 'toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained\x86_64-w64-mingw32-gcc.exe'
        if (-not (Test-Path -LiteralPath $gccPath)) { throw 'The local portable GNU toolchain is missing. Install Rust normally or restore .tools.' }
        $env:PATH = "$(Split-Path -Parent $gccPath);$(Join-Path $env:RUSTUP_HOME 'toolchains\stable-x86_64-pc-windows-gnu\bin');$(Split-Path -Parent $localCargo);$env:PATH"
        $env:CIKUNG_LLVM_LINKER = Join-Path $env:RUSTUP_HOME 'toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\rust-lld.exe'
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $env:CIKUNG_LLVM_LINKER
        $nativeLibraries = Join-Path $projectRoot '.tools\native-libs'
        New-Item -ItemType Directory -Path $nativeLibraries -Force | Out-Null
        & $env:CIKUNG_LLVM_LINKER -flavor link /lib /machine:x64 "/def:$(Join-Path $projectRoot 'scripts\shlwapi.def')" "/out:$(Join-Path $nativeLibraries 'libshlwapi.a')"
        if ($LASTEXITCODE -ne 0) { throw 'Could not prepare the Windows shell import library.' }
        $env:LIBRARY_PATH = $nativeLibraries
        $dlltoolPath = Join-Path $projectRoot '.tools\portable-dlltool.exe'
        $dlltoolSource = Join-Path $projectRoot 'scripts\portable-dlltool.rs'
        if (-not (Test-Path -LiteralPath $dlltoolPath) -or (Get-Item -LiteralPath $dlltoolSource).LastWriteTime -gt (Get-Item -LiteralPath $dlltoolPath).LastWriteTime) {
            & (Join-Path $env:CARGO_HOME 'bin\rustc.exe') +stable-x86_64-pc-windows-gnu $dlltoolSource --crate-name portable_dlltool -C "linker=$gccPath" -o $dlltoolPath
            if ($LASTEXITCODE -ne 0) { throw 'Could not prepare the portable import-library helper.' }
        }
        $env:CARGO_ENCODED_RUSTFLAGS = @('-C',"dlltool=$dlltoolPath",'-C','linker-flavor=ld.lld','-L',"native=$nativeLibraries") -join [char]0x1f
        $cargoArgs += '+stable-x86_64-pc-windows-gnu'
    } else {
        $cargoPath = (Get-Command cargo -ErrorAction Stop).Source
    }
    if ($Test) { $cargoArgs += 'test' } else { $cargoArgs += 'build' }
    if ($targetName) { $cargoArgs += @('--target', $targetName) }
    if ($Configuration -eq 'Release') { $cargoArgs += '--release' }
    $cargoArgs += @('--locked', '-j', '4')
    & $cargoPath @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Rust build failed ($LASTEXITCODE)." }
    if (-not $Test) {
        $profileFolder = $Configuration.ToLowerInvariant()
        $buildFolder = Join-Path $projectRoot 'target'
        if ($targetName) { $buildFolder = Join-Path $buildFolder $targetName }
        $exePath = Join-Path $buildFolder "$profileFolder\cikung.exe"
        $outputFolder = Join-Path $projectRoot 'dist\Cikung'
        New-Item -ItemType Directory -Path $outputFolder -Force | Out-Null
        Copy-Item -LiteralPath $exePath -Destination (Join-Path $outputFolder 'Cikung.exe') -Force
        & (Join-Path $PSScriptRoot 'set-app-icon.ps1') -Executable (Join-Path $outputFolder 'Cikung.exe')
        # Setup is the same exe: "Setup" in its file name makes it install itself (src/setup.rs).
        Copy-Item -LiteralPath (Join-Path $outputFolder 'Cikung.exe') -Destination (Join-Path $projectRoot 'dist\Cikung-Setup.exe') -Force
        if ($targetName) {
            $licenseFolder = Join-Path $outputFolder 'licenses'
            New-Item -ItemType Directory -Path $licenseFolder -Force | Out-Null
            $rustDocs = Join-Path $env:RUSTUP_HOME 'toolchains\stable-x86_64-pc-windows-gnu\share\doc\rust'
            Copy-Item -LiteralPath (Join-Path $rustDocs 'COPYRIGHT.html') -Destination (Join-Path $licenseFolder 'Rust-runtime-COPYRIGHT.html') -Force
            Copy-Item -Path (Join-Path $rustDocs 'licenses\*.txt') -Destination $licenseFolder -Force
        }
        Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $outputFolder -Force
        Copy-Item -LiteralPath (Join-Path $projectRoot 'THIRD_PARTY_NOTICES.md') -Destination $outputFolder -Force
        Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $outputFolder -Force
        $setupScripts=Join-Path $outputFolder 'scripts'
        $setupDocs=Join-Path $outputFolder 'docs'
        New-Item -ItemType Directory -Path $setupScripts,$setupDocs -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $projectRoot 'scripts\cikung-hook.ps1') -Destination $setupScripts -Force
        foreach($script in @('launch-agent.ps1','cikung-codex-hook.cjs')) {
            Copy-Item -LiteralPath (Join-Path $projectRoot "scripts\$script") -Destination $setupScripts -Force
        }
        $adapterFolder=Join-Path $outputFolder 'adapters'
        New-Item -ItemType Directory -Path $adapterFolder -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $projectRoot 'adapters\cikung-claude.cjs'),(Join-Path $projectRoot 'adapters\cikung-omp.ts') -Destination $adapterFolder -Force
        foreach($guide in @('agent-companions.md','claude-integration.md','omp-integration.md','codex-integration.md','agent-phone-link.md','cloud-integrations.md','productivity.md','desktop-shell.md','vlc.md','feature-coverage.md','asset-provenance.md','hover-polish.md','coucou-live-reference.md','music-hover-settings.md','desktop-apps.md','settings-hub.md','bluetooth-game.md','nusa-reference.md','nusa-rea-summary.json','whatsapp-notifications.md','completion-sounds.md','coucou-clock-motion.md','macos.md')) {
            Copy-Item -LiteralPath (Join-Path $projectRoot "docs\$guide") -Destination $setupDocs -Force
        }
        & (Join-Path $projectRoot 'scripts\package-notices.ps1') -ProjectRoot $projectRoot -OutputFolder $outputFolder
        Compress-Archive -Path (Join-Path $outputFolder '*') -DestinationPath (Join-Path $projectRoot 'dist\Cikung-Windows-x64.zip') -Force
        Write-Output "Ready: $outputFolder\Cikung.exe and $(Join-Path $projectRoot 'dist\Cikung-Setup.exe')"
    }
} finally { Pop-Location }
