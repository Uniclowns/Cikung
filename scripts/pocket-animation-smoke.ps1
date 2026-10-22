param([string]$Executable='D:\Nuch\target\x86_64-pc-windows-gnu\debug\cikung.exe',[string]$Report='D:\Nuch\docs\pocket-animation-smoke.json')
$ErrorActionPreference='Stop'
$source=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'hover-smoke.ps1'))
$body=[regex]::Match($source,"(?s)Add-Type -TypeDefinition @'\r?\n(.*?)\r?\n'@").Groups[1].Value.Replace('NotchProbe','PocketProbe')
Add-Type -TypeDefinition $body
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;
public static class PocketInput {
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr w);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr w,int cmd);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr w,out uint pid);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr w);
 [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr w);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr w,IntPtr after,int x,int y,int width,int height,uint flags);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr w,IntPtr dc,uint flags);
}
'@
$oldDpi=[PocketProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$cursor=[PocketProbe+Point]::new();[void][PocketProbe]::GetCursorPos([ref]$cursor)
$pressed=$false;$owned=$null;$fixture=$null;$root=[IntPtr]::Zero;$fixtureHandle=[IntPtr]::Zero
$temp=Join-Path ([IO.Path]::GetTempPath()) ('cikung-pocket-'+[guid]::NewGuid().ToString('N'))
[void](New-Item -ItemType Directory -Path $temp)
$file=Join-Path $temp 'pocket-test.txt';[IO.File]::WriteAllText($file,'Cikung original file fixture')
$ready=Join-Path $temp 'ready.txt'
$samples=[Collections.Generic.List[object]]::new();$clock=[Diagnostics.Stopwatch]::StartNew()
function Sample([string]$phase,[int]$duration) {
 $deadline=$clock.ElapsedMilliseconds+$duration
 while($clock.ElapsedMilliseconds -lt $deadline) {
  $region=[PocketProbe]::CreateRectRgn(0,0,1,1);$r=[PocketProbe+Rect]::new()
  try {if([PocketProbe]::GetWindowRgn($script:root,$region) -ne 0){[void][PocketProbe]::GetRgnBox($region,[ref]$r);$samples.Add([pscustomobject]@{phase=$phase;ms=$clock.ElapsedMilliseconds;width=$r.Right-$r.Left;height=$r.Bottom-$r.Top})}}
  finally {[void][PocketProbe]::DeleteObject($region)}
  Start-Sleep -Milliseconds 10
 }
}
function Capture([string]$name) {
 $r=[PocketProbe+Rect]::new();[void][PocketProbe]::GetWindowRect($script:root,[ref]$r)
 $bitmap=[Drawing.Bitmap]::new($r.Right-$r.Left,$r.Bottom-$r.Top)
 $g=[Drawing.Graphics]::FromImage($bitmap);$dc=$g.GetHdc()
 try {[void][PocketInput]::PrintWindow($script:root,$dc,2)} finally {$g.ReleaseHdc($dc);$g.Dispose()}
 try {$bitmap.Save((Join-Path (Split-Path -Parent $Report) $name),[Drawing.Imaging.ImageFormat]::Png)} finally {$bitmap.Dispose()}
}
function Start-Drag {
 [void][PocketInput]::ShowWindow([PocketInput]::GetParent($script:fixtureHandle),9)
 [void][PocketInput]::SetForegroundWindow([PocketInput]::GetParent($script:fixtureHandle))
 [void][PocketInput]::SetWindowPos([PocketInput]::GetParent($script:fixtureHandle),[IntPtr](-1),30,550,380,160,0x50)
 Start-Sleep -Milliseconds 120
 $r=[PocketProbe+Rect]::new();[void][PocketProbe]::GetWindowRect($script:fixtureHandle,[ref]$r)
 $p=[PocketProbe+Point]::new();$p.X=[int](($r.Left+$r.Right)/2);$p.Y=[int](($r.Top+$r.Bottom)/2)
 [void][PocketProbe]::SetCursorPos($p.X,$p.Y)
 $actual=[PocketProbe]::WindowFromPoint($p);$actualPid=[uint32]0
 [void][PocketInput]::GetWindowThreadProcessId($actual,[ref]$actualPid)
 if($actualPid -ne $script:fixture.Id){throw "Owned drag source is not under cursor: wanted=$script:fixtureHandle actual=$actual pid=$actualPid fixture=$($script:fixture.Id) visible=$([PocketInput]::IsWindowVisible($script:fixtureHandle)) parentVisible=$([PocketInput]::IsWindowVisible([PocketInput]::GetParent($script:fixtureHandle))) point=$($p.X),$($p.Y) rect=$($r.Left),$($r.Top),$($r.Right),$($r.Bottom)"}
 [PocketInput]::mouse_event(2,0,0,0,[UIntPtr]::Zero);$script:pressed=$true
 Start-Sleep -Milliseconds 100
 [void][PocketProbe]::SetCursorPos($script:center,$script:top+15)
}
try {
 [void][PocketProbe]::SetCursorPos(100,800)
 $owned=Start-Process -FilePath $Executable -ArgumentList @('--demo','--page','music') -WindowStyle Hidden -PassThru
 $deadline=[datetime]::UtcNow.AddSeconds(15)
 do {$root=[PocketProbe]::FindApp($owned.Id);if($root -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100} while([datetime]::UtcNow -lt $deadline)
 if($root -eq [IntPtr]::Zero){throw 'Owned Cikung did not open'}
 Start-Sleep -Milliseconds 1500
 $hostRect=[PocketProbe+Rect]::new();[void][PocketProbe]::GetWindowRect($root,[ref]$hostRect)
 $scale=($hostRect.Right-$hostRect.Left)/580.0;$center=[int](($hostRect.Left+$hostRect.Right)/2);$top=$hostRect.Top
 $fixture=Start-Process powershell.exe -ArgumentList @('-NoProfile','-STA','-File',('"'+(Join-Path $PSScriptRoot 'pocket-drag-fixture.ps1')+'"'),'-ReadyFile',('"'+$ready+'"'),'-File',('"'+$file+'"')) -WindowStyle Hidden -PassThru
 $deadline=[datetime]::UtcNow.AddSeconds(10)
 while(-not (Test-Path -LiteralPath $ready) -and [datetime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 100}
 if(-not (Test-Path -LiteralPath $ready)){throw 'Owned drag fixture did not open'}
 $fixtureHandle=[IntPtr]([long][IO.File]::ReadAllText($ready))
 Sample 'music' 100
 Start-Drag;Sample 'drag-enter' 650;Capture 'pocket-drag-hover.png'
 [void][PocketProbe]::SetCursorPos(100,800);Sample 'cancel' 650
 if($script:pressed){[PocketInput]::mouse_event(4,0,0,0,[UIntPtr]::Zero);$script:pressed=$false}
 Start-Drag;Sample 'second-enter' 400
 if($script:pressed){[PocketInput]::mouse_event(4,0,0,0,[UIntPtr]::Zero);$script:pressed=$false};Sample 'drop' 700;Capture 'pocket-dropped.png'
 $initial=($samples|Where-Object phase -EQ 'music'|Select-Object -Last 1).height
 $hover=($samples|Where-Object phase -EQ 'drag-enter'|Select-Object -Last 1).height
 $cancel=($samples|Where-Object phase -EQ 'cancel'|Select-Object -Last 1).height
 $drop=($samples|Where-Object phase -EQ 'drop'|Select-Object -Last 1).height
 $variants=@($samples|Where-Object phase -EQ 'drag-enter'|Select-Object -ExpandProperty height -Unique).Count
 $result=[ordered]@{hover_opens_pocket=[math]::Abs($hover-380*$scale)-lt 3;cancel_restores_music=[math]::Abs($cancel-$initial)-lt 3;drop_keeps_pocket=[math]::Abs($drop-$hover)-lt 3;original_untouched=[IO.File]::ReadAllText($file)-eq 'Cikung original file fixture';height_intermediate_steps=$variants;samples=$samples}
 $result|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $Report -Encoding utf8
 [pscustomobject]$result|Select-Object -ExcludeProperty samples|ConvertTo-Json
 if(-not $result.hover_opens_pocket -or -not $result.cancel_restores_music -or -not $result.drop_keeps_pocket -or -not $result.original_untouched -or $variants -lt 5){throw 'Pocket animation checks failed'}
} finally {
 if($script:pressed){[PocketInput]::mouse_event(4,0,0,0,[UIntPtr]::Zero);$script:pressed=$false}
 if($owned -and -not $owned.HasExited){[void][PocketProbe]::PostMessage($root,0x10,[IntPtr]::Zero,[IntPtr]::Zero);[void]$owned.WaitForExit(5000)}
 if($fixture -and -not $fixture.HasExited){[void][PocketProbe]::PostMessage($fixtureHandle,0x10,[IntPtr]::Zero,[IntPtr]::Zero);if(-not $fixture.WaitForExit(3000)){$fixture.Kill()}}
 [void][PocketProbe]::SetCursorPos($cursor.X,$cursor.Y)
 [void][PocketProbe]::SetThreadDpiAwarenessContext($oldDpi)
 Remove-Item -LiteralPath $file,$ready -Force -ErrorAction SilentlyContinue
 Remove-Item -LiteralPath $temp -Force -ErrorAction SilentlyContinue
}
