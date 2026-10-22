<#
.SYNOPSIS
Publish agent activity or adapt Claude Code hooks to Cikung. Never runs tool commands.
.DESCRIPTION
Read the endpoint and token from Cikung's Agent setup panel. Credentials are session-only.
Without -ClaudeCode this sends a generic state event. -ClaudeCode reads the hook JSON from stdin.
PermissionRequest waits up to 110 seconds and denies on timeout or bridge failure.
#>
param(
    [switch]$ClaudeCode,
    [string]$Endpoint = $env:CIKUNG_ENDPOINT,
    [string]$Token = $env:CIKUNG_TOKEN,
    [string]$SessionId = 'manual',
    [string]$Agent = 'Local agent',
    [string]$Project = '',
    [ValidateSet('idle','working','thinking','waiting','approval','complete','error')][string]$State = 'working',
    [string]$Detail = '',
    [string]$FileDiff = '',
    [ValidateRange(5,110)][int]$TimeoutSeconds = 110
)
$ErrorActionPreference = 'Stop'
$permissionRequest = $false
$permissionResult = 'deny'
$failureReason = 'Cikung did not receive your approval. The request was denied.'
function Send-CikungEvent($payload) {
    $body = $payload | ConvertTo-Json -Depth 12 -Compress
    Invoke-RestMethod -Uri "$Endpoint/v1/agents/events" -Method Post -Headers @{Authorization="Bearer $Token"} -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes($body)) -TimeoutSec 5 | Out-Null
}
try {
    # Parse the hook event before validating connection settings, so permission requests
    # always return an explicit deny if a missing credential prevents connection.
    if ($ClaudeCode) {
        $inputJson = [Console]::In.ReadToEnd()
        if ($inputJson.Length -gt 131072) { throw 'Hook input exceeds 128 KB.' }
        $hook = $inputJson | ConvertFrom-Json
        $permissionRequest = $hook.hook_event_name -eq 'PermissionRequest'
        $Agent = 'Claude Code'
        $SessionId = [string]$hook.session_id
        $Project = [string]$hook.cwd
        switch ([string]$hook.hook_event_name) {
            'SessionStart' { $State='working'; $Detail='Session started' }
            'UserPromptSubmit' { $State='thinking'; $Detail='Working on your request' }
            'PreToolUse' { $State='working'; $Detail="Using $($hook.tool_name)" }
            'PostToolUse' { $State='working'; $Detail="Finished $($hook.tool_name)" }
            'PermissionRequest' {
                $State='approval'
                # This is a review preview; the agent, not Cikung, owns execution.
                $preview = $hook.tool_input | ConvertTo-Json -Depth 8 -Compress
                if ($preview.Length -gt 3000) { $preview = $preview.Substring(0,3000) + '…' }
                $Detail="Allow $($hook.tool_name)?`n$preview"
            }
            'Notification' { $State='waiting'; $Detail=[string]$hook.message }
            'Stop' { $State='complete'; $Detail='Agent finished' }
            'SessionEnd' { $State='idle'; $Detail='Session ended' }
            default { $State='working'; $Detail=[string]$hook.hook_event_name }
        }
    }
    if ([string]::IsNullOrWhiteSpace($Endpoint) -or [string]::IsNullOrWhiteSpace($Token)) { throw 'Set CIKUNG_ENDPOINT and CIKUNG_TOKEN from the Cikung Agent setup panel.' }
    $uri = [uri]$Endpoint
    if ($uri.Scheme -ne 'http' -or $uri.Host -ne '127.0.0.1' -or $uri.AbsolutePath -ne '/') { throw 'CIKUNG_ENDPOINT must be the Cikung http://127.0.0.1:port address.' }
    if ($SessionId -notmatch '^[A-Za-z0-9_.:-]{1,128}$') { throw 'SessionId must be 1–128 letters, numbers, dots, colons, underscores, or dashes.' }
    if ($Project.Length -gt 256) { $Project=$Project.Substring(0,256) }
    if ($Detail.Length -gt 3900) { $Detail=$Detail.Substring(0,3900) }
    $payload = @{session_id=$SessionId;agent=$Agent;project=$Project;state=$State;detail=$Detail}
    if ($FileDiff) { if ($FileDiff.Length -gt 12000) { $FileDiff=$FileDiff.Substring(0,12000) }; $payload.file_diff=$FileDiff }
    if ($State -eq 'approval') { $requestId=[guid]::NewGuid().ToString('N'); $payload.request_id=$requestId }
    Send-CikungEvent $payload
    if ($State -eq 'approval') {
        $deadline=[datetime]::UtcNow.AddSeconds($TimeoutSeconds)
        $decisionReceived=$false
        while ([datetime]::UtcNow -lt $deadline) {
            $response=Invoke-RestMethod -Uri "$Endpoint/v1/agents/decisions/$requestId" -Headers @{Authorization="Bearer $Token"} -TimeoutSec 5
            if ($response.decision -eq 'allow') { $permissionResult='allow'; $decisionReceived=$true; break }
            if ($response.decision -eq 'deny') { $decisionReceived=$true; break }
            Start-Sleep -Milliseconds 700
        }
        if (-not $decisionReceived) {
            # Withdraw the expired UI request, preventing an approval after this caller stopped waiting.
            Send-CikungEvent @{session_id=$SessionId;agent=$Agent;project=$Project;state='waiting';detail='Permission wait expired; denied.'}
        }
        if (-not $permissionRequest) { @{request_id=$requestId;decision=$permissionResult} | ConvertTo-Json -Compress }
    }
} catch {
    # No non-JSON text is written to stdout in Claude Code mode.
    [Console]::Error.WriteLine("Cikung: $($_.Exception.Message)")
    if (-not $ClaudeCode) { exit 1 }
} finally {
    if ($permissionRequest) {
        $decision=@{behavior=$permissionResult}
        if ($permissionResult -eq 'deny') { $decision.message=$failureReason }
        @{hookSpecificOutput=@{hookEventName='PermissionRequest';decision=$decision}} | ConvertTo-Json -Depth 8 -Compress
    }
}
