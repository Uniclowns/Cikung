param([Parameter(Mandatory=$true)][string]$Profile)
$ErrorActionPreference='Stop'
# This is fixed launcher code. Paths and argv are data, never an evaluated command string.
function ConvertTo-NativeArgument([AllowEmptyString()][string]$Value) {
    # Windows CommandLineToArgvW / CRT rules. ProcessStartInfo.ArgumentList is
    # unavailable in Windows PowerShell 5.1, and its call operator strips quotes.
    $builder = New-Object System.Text.StringBuilder
    [void]$builder.Append('"')
    $backslashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq '\') { $backslashes++; continue }
        if ($character -eq '"') {
            [void]$builder.Append(('\' * ($backslashes * 2 + 1)))
            [void]$builder.Append('"')
        } else {
            if ($backslashes) { [void]$builder.Append(('\' * $backslashes)) }
            [void]$builder.Append($character)
        }
        $backslashes = 0
    }
    if ($backslashes) { [void]$builder.Append(('\' * ($backslashes * 2))) }
    [void]$builder.Append('"')
    $builder.ToString()
}
try {
    $launch = Get-Content -LiteralPath $Profile -Raw -Encoding UTF8 | ConvertFrom-Json
    Set-Location -LiteralPath $launch.project
    $host.UI.RawUI.WindowTitle = "Cikung - $($launch.agent)"
    Write-Host "Connected terminal: $($launch.agent)" -ForegroundColor DarkYellow
    Write-Host 'Keep Cikung open to receive activity and permission requests.'
    if($launch.agent -eq 'Codex'){Write-Host 'In Codex, review and trust the Cikung definitions in /hooks.' -ForegroundColor Yellow}
    $agentArguments=@($launch.arguments | ForEach-Object {[string]$_})
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = [string]$launch.executable
    $start.WorkingDirectory = [string]$launch.project
    $start.UseShellExecute = $false
    $start.Arguments = (($agentArguments | ForEach-Object { ConvertTo-NativeArgument $_ }) -join ' ')
    $process = [System.Diagnostics.Process]::Start($start)
    try { $process.WaitForExit() } finally { $process.Dispose() }
} catch {Write-Host "Cikung could not start this session: $($_.Exception.Message)" -ForegroundColor Red}
finally {
    # -NoExit intentionally leaves a usable terminal after the agent finishes.
    # Do not leave a live Cikung bridge credential in that shell environment.
    Remove-Item Env:CIKUNG_TOKEN -ErrorAction SilentlyContinue
    Remove-Item Env:CIKUNG_ENDPOINT -ErrorAction SilentlyContinue
    Remove-Item Env:CIKUNG_PROJECT -ErrorAction SilentlyContinue
    Remove-Item Env:CIKUNG_OMP_APPROVALS -ErrorAction SilentlyContinue
}
