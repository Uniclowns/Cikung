param([string]$Executable='D:\Nuch\dist\Cikung\Cikung.exe',[string]$Report='D:\Nuch\docs\music-popup-smoke.json')
$ErrorActionPreference='Stop'
$Executable=(Resolve-Path -LiteralPath $Executable).Path
if(@(Get-Process Cikung -ErrorAction SilentlyContinue | Where-Object Path -EQ $Executable).Count){throw 'Close Cikung before this owned-process check.'}
$source=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'hover-smoke.ps1'))
$body=[regex]::Match($source,"(?s)Add-Type -TypeDefinition @'\r?\n(.*?)\r?\n'@").Groups[1].Value.Replace('NotchProbe','MusicPopupProbe').Replace('FindApp(uint pid)','FindApp(uint pid,string title)').Replace('text.ToString()=="Cikung"','text.ToString()==title')
Add-Type -TypeDefinition $body
$oldDpi=[MusicPopupProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$cursor=[MusicPopupProbe+Point]::new()
[void][MusicPopupProbe]::GetCursorPos([ref]$cursor)
$owned=$null
$root=[IntPtr]::Zero
function Shape {
    $r=[MusicPopupProbe+Rect]::new()
    $region=[MusicPopupProbe]::CreateRectRgn(0,0,1,1)
    try { if([MusicPopupProbe]::GetWindowRgn($script:root,$region) -eq 0){return $null}; [void][MusicPopupProbe]::GetRgnBox($region,[ref]$r);return $r }
    finally {[void][MusicPopupProbe]::DeleteObject($region)}
}
function Wait-Window([string]$title) {
    $limit=[datetime]::UtcNow.AddSeconds(15)
    do { $w=[MusicPopupProbe]::FindApp($script:owned.Id,$title); if($w -ne [IntPtr]::Zero){return $w};Start-Sleep -Milliseconds 100 } while([datetime]::UtcNow -lt $limit)
    throw "Missing window: $title"
}
function Open-Page([string]$page) {
    $second=Start-Process -FilePath $Executable -ArgumentList @('--page',$page) -WindowStyle Hidden -PassThru
    if(-not $second.WaitForExit(5000)){throw 'Secondary launch did not exit'}
}
try {
    [void][MusicPopupProbe]::SetCursorPos(100,700)
    $owned=Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru
    $root=Wait-Window 'Cikung'
    $limit=[datetime]::UtcNow.AddSeconds(15)
    do { $collapsed=Shape; if($collapsed){break};Start-Sleep -Milliseconds 100 } while([datetime]::UtcNow -lt $limit)
    if(-not $collapsed){throw 'The notch shape was not ready'}
    $hostRect=[MusicPopupProbe+Rect]::new()
    [void][MusicPopupProbe]::GetWindowRect($root,[ref]$hostRect)
    $scale=($hostRect.Right-$hostRect.Left)/580.0
    [void][MusicPopupProbe]::SetCursorPos([int](($hostRect.Left+$hostRect.Right)/2),$hostRect.Top+15)
    Start-Sleep -Milliseconds 1300
    $music=Shape
    $expectedHeight=[int][math]::Round(270*$scale)
    if([math]::Abs(($music.Bottom-$music.Top)-$expectedHeight) -gt 3){throw "Hover did not open Music. This check requires currently playing system media."}
    [void][MusicPopupProbe]::PostMessage($root,0x100,[IntPtr]27,[IntPtr]::Zero)
    [void][MusicPopupProbe]::PostMessage($root,0x101,[IntPtr]27,[IntPtr]::Zero)
    [void][MusicPopupProbe]::SetCursorPos(100,700)
    Start-Sleep -Milliseconds 600
    Open-Page 'settings'
    $popup=Wait-Window 'Cikung Settings'
    $popupRect=[MusicPopupProbe+Rect]::new()
    [void][MusicPopupProbe]::GetWindowRect($popup,[ref]$popupRect)
    Start-Sleep -Milliseconds 250
    $whileSettings=Shape
    if(($whileSettings.Bottom-$whileSettings.Top) -gt 52){throw 'Settings expanded the notch instead of opening independently'}
    if(($popupRect.Right-$popupRect.Left) -lt 680*$scale){throw 'Settings is constrained to the notch size'}
    Open-Page 'settings'
    Start-Sleep -Milliseconds 400
    if([MusicPopupProbe]::FindApp($owned.Id,'Cikung Settings') -ne $popup){throw 'Settings did not reuse its native window'}
    [void][MusicPopupProbe]::PostMessage($popup,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    Start-Sleep -Milliseconds 800
    if($owned.HasExited -or [MusicPopupProbe]::FindApp($owned.Id,'Cikung') -ne $root){throw 'Closing settings closed or replaced the notch'}
    if([MusicPopupProbe]::FindApp($owned.Id,'Cikung Settings') -ne [IntPtr]::Zero){throw 'Settings did not close'}
    Open-Page 'cats'
    $cats=Wait-Window 'Cikung Settings'
    $sameProcess=@(Get-Process Cikung -ErrorAction SilentlyContinue | Where-Object Path -EQ $Executable)
    if($sameProcess.Count -ne 1 -or $sameProcess[0].Id -ne $owned.Id){throw 'Settings started another process'}
    [void][MusicPopupProbe]::PostMessage($cats,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    [pscustomobject]@{live_music_hover=$true;music_height_pixels=$music.Bottom-$music.Top;expected_music_height_pixels=$expectedHeight;settings_is_native_popup=$true;settings_reused=$true;settings_closes_independently=$true;cats_reopens_popup=$true;one_process=$true;root_window_unchanged=$true;playback_buttons_not_pressed=$true} | ConvertTo-Json | Set-Content -LiteralPath $Report -Encoding utf8
    Get-Content -LiteralPath $Report
} finally {
    if($owned -and -not $owned.HasExited){[void][MusicPopupProbe]::PostMessage($root,0x10,[IntPtr]::Zero,[IntPtr]::Zero);if(-not $owned.WaitForExit(5000)){throw 'Owned app did not close normally'}}
    [void][MusicPopupProbe]::SetCursorPos($cursor.X,$cursor.Y)
    [void][MusicPopupProbe]::SetThreadDpiAwarenessContext($oldDpi)
}
