# Needs Settings → General → "Tuck away during fullscreen apps" turned on; by default the notch stays in front.
param([string]$Executable='D:\Nuch\dist\Cikung\Cikung.exe',[string]$Report='D:\Nuch\docs\fullscreen-smoke.json')
$ErrorActionPreference='Stop'
$Executable=(Resolve-Path -LiteralPath $Executable).Path
if(@(Get-Process Cikung -ErrorAction SilentlyContinue | Where-Object Path -EQ $Executable).Count){throw 'Close Cikung before this owned-process test.'}
$source=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'hover-smoke.ps1'))
$body=[regex]::Match($source,"(?s)Add-Type -TypeDefinition @'\r?\n(.*?)\r?\n'@").Groups[1].Value.Replace('NotchProbe','FullscreenProbe')
Add-Type -TypeDefinition $body
Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;
public static class FullscreenFixture {
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr w);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr w,IntPtr after,int x,int y,int width,int height,uint flags);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr w,out uint pid);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
}
'@
$previousDpi=[FullscreenProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$cursor=[FullscreenProbe+Point]::new();[void][FullscreenProbe]::GetCursorPos([ref]$cursor)
$owned=$null;$fixture=$null;$root=[IntPtr]::Zero;$fixtureWindow=[IntPtr]::Zero
$ready=Join-Path (Split-Path -Parent $PSScriptRoot) ('.tools\fullscreen-'+[guid]::NewGuid().ToString()+'.txt')
function Shape {
 $region=[FullscreenProbe]::CreateRectRgn(0,0,1,1);$bounds=[FullscreenProbe+Rect]::new()
 try {if([FullscreenProbe]::GetWindowRgn($script:root,$region) -eq 0){return $null};[void][FullscreenProbe]::GetRgnBox($region,[ref]$bounds);return $bounds} finally {[void][FullscreenProbe]::DeleteObject($region)}
}
function Wait-Shape([bool]$empty) {
 $deadline=[datetime]::UtcNow.AddSeconds(12)
 do {$shape=Shape;if($shape -and (($shape.Right-$shape.Left -eq 0) -eq $empty)){return $shape};Start-Sleep -Milliseconds 80} while([datetime]::UtcNow -lt $deadline)
 throw "Notch did not reach expected hidden state: $empty"
}
try {
 [void][FullscreenProbe]::SetCursorPos(100,1000)
 $owned=Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru
 $deadline=[datetime]::UtcNow.AddSeconds(15)
 do {$script:root=[FullscreenProbe]::FindApp($owned.Id);Start-Sleep -Milliseconds 100} while($root -eq [IntPtr]::Zero -and [datetime]::UtcNow -lt $deadline)
 if($root -eq [IntPtr]::Zero){throw 'Owned notch was not created'}
 [void](Wait-Shape $false)
 $hostRect=[FullscreenProbe+Rect]::new();[void][FullscreenProbe]::GetWindowRect($root,[ref]$hostRect)
 $fixture=Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -ArgumentList @('-NoProfile','-STA','-File',(Join-Path $PSScriptRoot 'fullscreen-fixture.ps1'),'-ReadyFile',$ready) -WindowStyle Hidden -PassThru
 $deadline=[datetime]::UtcNow.AddSeconds(15)
 while(-not (Test-Path -LiteralPath $ready) -and [datetime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 100}
 if(-not (Test-Path -LiteralPath $ready)){throw 'Owned fullscreen fixture was not ready'}
 $fixtureWindow=[IntPtr]([long]([IO.File]::ReadAllText($ready)))
 [void][FullscreenFixture]::SetForegroundWindow($fixtureWindow)
 Start-Sleep -Milliseconds 200
 if([FullscreenFixture]::GetForegroundWindow() -ne $fixtureWindow){
  # Windows can deny activation from background automation. Click only the verified owned fixture.
  [void][FullscreenFixture]::SetWindowPos($fixtureWindow,[IntPtr](-1),0,0,0,0,0x13)
  $click=[FullscreenProbe+Point]::new();$click.X=60;$click.Y=160
  [void][FullscreenProbe]::SetCursorPos($click.X,$click.Y)
  $hit=[FullscreenProbe]::WindowFromPoint($click);$hitPid=[uint32]0
  [void][FullscreenFixture]::GetWindowThreadProcessId($hit,[ref]$hitPid)
  if($hitPid -ne $fixture.Id){throw 'Owned fixture does not cover the test click; no input was sent'}
  [FullscreenFixture]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
  [FullscreenFixture]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 200
 }
 if([FullscreenFixture]::GetForegroundWindow() -ne $fixtureWindow){throw 'Windows did not foreground the owned fixture; fullscreen detection is untested'}
 [void](Wait-Shape $true)
 $point=[FullscreenProbe+Point]::new();$point.X=[int](($hostRect.Left+$hostRect.Right)/2);$point.Y=15
 if([FullscreenProbe]::WindowFromPoint($point) -eq $root){throw 'Hidden notch still catches input'}
 $second=Start-Process -FilePath $Executable -ArgumentList @('--page','game') -WindowStyle Hidden -PassThru
 if(-not $second.WaitForExit(5000)){throw 'Secondary request did not exit'}
 $opened=Wait-Shape $false
 Start-Sleep -Milliseconds 600
 $opened=Shape
 if($opened.Bottom-$opened.Top -lt 150){throw 'Explicit open collapsed immediately in fullscreen'}
 [void](Wait-Shape $true)
 [void][FullscreenFixture]::SetWindowPos($fixtureWindow,[IntPtr]::Zero,100,180,600,400,0x14)
 $restored=Wait-Shape $false
 $after=[FullscreenProbe+Rect]::new();[void][FullscreenProbe]::GetWindowRect($root,[ref]$after)
 if($after.Left -ne $hostRect.Left -or $after.Top -ne $hostRect.Top){throw 'Fullscreen changed the notch anchor'}
 [pscustomobject]@{fullscreen_hides_notch=$true;hidden_region_is_click_through=$true;explicit_page_request_opens_notch=$true;notch_tucks_again_after_grace=$true;restores_after_fullscreen=$true;anchor_unchanged=$true;owned_fixture_only=$true;real_game_or_media_controls_used=$false} | ConvertTo-Json | Set-Content -LiteralPath $Report -Encoding utf8
 Get-Content -LiteralPath $Report
} finally {
 if($fixture -and -not $fixture.HasExited -and $fixtureWindow -ne [IntPtr]::Zero){[void][FullscreenProbe]::PostMessage($fixtureWindow,0x10,[IntPtr]::Zero,[IntPtr]::Zero);[void]$fixture.WaitForExit(5000)}
 if($owned -and -not $owned.HasExited){[void][FullscreenProbe]::PostMessage($root,0x10,[IntPtr]::Zero,[IntPtr]::Zero);if(-not $owned.WaitForExit(8000)){throw 'Owned Cikung did not close normally'}}
 [void][FullscreenProbe]::SetCursorPos($cursor.X,$cursor.Y)
 [void][FullscreenProbe]::SetThreadDpiAwarenessContext($previousDpi)
}
