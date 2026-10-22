param([string]$Executable='D:\Nuch\dist\Cikung\Cikung.exe',[string]$Report='D:\Nuch\docs\single-instance-smoke.json')
$ErrorActionPreference='Stop'
$Executable=(Resolve-Path -LiteralPath $Executable).Path
if(@(Get-Process Cikung -ErrorAction SilentlyContinue | Where-Object Path -EQ $Executable).Count){throw 'Close the running Cikung before this owned-process test.'}
$probeSource=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'hover-smoke.ps1'))
$probeBody=[regex]::Match($probeSource,"(?s)Add-Type -TypeDefinition @'\r?\n(.*?)\r?\n'@").Groups[1].Value.Replace('NotchProbe','SingleProbe')
Add-Type -TypeDefinition $probeBody
$owned=$null
$window=[IntPtr]::Zero
try {
    $owned=Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru
    $deadline=[datetime]::UtcNow.AddSeconds(20)
    do {Start-Sleep -Milliseconds 100; $window=[SingleProbe]::FindApp($owned.Id)} while($window -eq [IntPtr]::Zero -and [datetime]::UtcNow -lt $deadline)
    if($window -eq [IntPtr]::Zero){throw 'No Cikung window'}
    Start-Sleep -Milliseconds 1200
    $sameWindow=$true
    foreach($page in @('agents','music','focus','cats')) {
        $second=Start-Process -FilePath $Executable -ArgumentList @('--page',$page) -WindowStyle Hidden -PassThru
        if(-not $second.WaitForExit(5000)){throw 'A second launch remained running'}
        Start-Sleep -Milliseconds 650
        $running=@(Get-Process Cikung -ErrorAction SilentlyContinue | Where-Object Path -EQ $Executable)
        if($running.Count -ne 1 -or $running[0].Id -ne $owned.Id){throw 'A feature started another Cikung'}
        $sameWindow=$sameWindow -and ([SingleProbe]::FindApp($owned.Id) -eq $window)
    }
    if(-not $sameWindow){throw 'A feature replaced the notch window'}
    [pscustomobject]@{one_process=$true;same_window_for_all_features=$sameWindow;second_launch_exits=$true;feature_requests=@('agents','music','focus','cats')} | ConvertTo-Json | Set-Content -LiteralPath $Report -Encoding utf8
    Get-Content -LiteralPath $Report
} finally {
    if($owned -and -not $owned.HasExited){[void][SingleProbe]::PostMessage($window,0x10,[IntPtr]::Zero,[IntPtr]::Zero);if(-not $owned.WaitForExit(5000)){throw 'Owned Cikung did not close normally'}}
}
