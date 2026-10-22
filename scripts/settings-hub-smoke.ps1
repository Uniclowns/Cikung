param([string]$Executable='D:\Nuch\dist\Cikung\Cikung.exe',[string]$Report='D:\Nuch\docs\settings-hub-smoke.json')
$ErrorActionPreference='Stop'
$Executable=(Resolve-Path -LiteralPath $Executable).Path
if(@(Get-Process Cikung -ErrorAction SilentlyContinue | Where-Object Path -EQ $Executable).Count){throw 'Close Cikung before this owned-process check.'}
$source=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'hover-smoke.ps1'))
$body=[regex]::Match($source,"(?s)Add-Type -TypeDefinition @'\r?\n(.*?)\r?\n'@").Groups[1].Value.Replace('NotchProbe','HubProbe').Replace('FindApp(uint pid)','FindApp(uint pid,string title)').Replace('text.ToString()=="Cikung"','text.ToString()==title')
Add-Type -TypeDefinition $body
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;
public static class HubWindow {
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr w,IntPtr after,int x,int y,int width,int height,uint flags);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr w,IntPtr dc,uint flags);
}
'@
$previousDpi=[HubProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$cursor=[HubProbe+Point]::new()
[void][HubProbe]::GetCursorPos([ref]$cursor)
$owned=$null
$root=[IntPtr]::Zero
function Wait-Window([string]$title) {
 $deadline=[datetime]::UtcNow.AddSeconds(15)
 do { $window=[HubProbe]::FindApp($script:owned.Id,$title);if($window -ne [IntPtr]::Zero){return $window};Start-Sleep -Milliseconds 100 } while([datetime]::UtcNow -lt $deadline)
 throw "Missing owned window: $title"
}
function Open-Page([string]$page) {
 $second=Start-Process -FilePath $Executable -ArgumentList @('--page',$page) -WindowStyle Hidden -PassThru
 if(-not $second.WaitForExit(5000)){throw 'Secondary launch did not exit'}
 Start-Sleep -Milliseconds 700
}
function Capture([IntPtr]$window,[string]$name) {
 $r=[HubProbe+Rect]::new();[void][HubProbe]::GetWindowRect($window,[ref]$r)
 $bitmap=[Drawing.Bitmap]::new($r.Right-$r.Left,$r.Bottom-$r.Top)
 $graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
 try {if(-not [HubWindow]::PrintWindow($window,$dc,2)){throw 'Own-window capture failed'}} finally {$graphics.ReleaseHdc($dc);$graphics.Dispose()}
 try {$bitmap.Save((Join-Path (Split-Path -Parent $Report) $name),[Drawing.Imaging.ImageFormat]::Png)} finally {$bitmap.Dispose()}
}
try {
 [void][HubProbe]::SetCursorPos(100,1000)
 $owned=Start-Process -FilePath $Executable -ArgumentList @('--page','settings/agents') -WindowStyle Hidden -PassThru
 $root=Wait-Window 'Cikung';$popup=Wait-Window 'Cikung Settings'
 Start-Sleep -Milliseconds 900
 $hostRect=[HubProbe+Rect]::new();[void][HubProbe]::GetWindowRect($root,[ref]$hostRect)
 $scale=($hostRect.Right-$hostRect.Left)/580.0
 foreach($page in @('settings/notifications','settings/game','settings/water','settings/tasks','settings/chat','settings/music','settings/cats','settings/shortcuts','settings/agents')) {
  Open-Page $page
  if([HubProbe]::FindApp($owned.Id,'Cikung Settings') -ne $popup){throw 'Hub navigation replaced the popup'}
  if([HubProbe]::FindApp($owned.Id,'Cikung') -ne $root){throw 'Hub navigation replaced the notch'}
 }
 Open-Page 'settings/notifications'
 Capture $popup 'cikung-whatsapp-settings-live.png'
 # Test the actual native minimum, without sending prompts or changing settings.
 [void][HubWindow]::SetWindowPos($popup,[IntPtr]::Zero,0,0,[int](760*$scale),[int](540*$scale),0x16)
 Start-Sleep -Milliseconds 900
 Capture $popup 'cikung-whatsapp-minimum.png'
 $smallRect=[HubProbe+Rect]::new();[void][HubProbe]::GetWindowRect($popup,[ref]$smallRect)
 [void][HubProbe]::PostMessage($popup,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
 Start-Sleep -Milliseconds 800
 if($owned.HasExited -or [HubProbe]::FindApp($owned.Id,'Cikung') -ne $root){throw 'Closing the hub closed the notch'}
 if([HubProbe]::FindApp($owned.Id,'Cikung Settings') -ne [IntPtr]::Zero){throw 'Hub did not close'}
 Open-Page 'cats';[void](Wait-Window 'Cikung Settings')
 $running=@(Get-Process Cikung -ErrorAction SilentlyContinue | Where-Object Path -EQ $Executable)
 if($running.Count -ne 1 -or $running[0].Id -ne $owned.Id){throw 'Hub started another app process'}
 $region=[HubProbe]::CreateRectRgn(0,0,1,1);$shape=[HubProbe+Rect]::new()
 try {[void][HubProbe]::GetWindowRgn($root,$region);[void][HubProbe]::GetRgnBox($region,[ref]$shape)} finally {[void][HubProbe]::DeleteObject($region)}
 if(($shape.Bottom-$shape.Top) -gt 42*$scale){throw 'Hub navigation expanded the notch'}
 [pscustomobject]@{one_process=$true;popup_reused_for_hub_routes=$true;root_window_unchanged=$true;notch_stays_collapsed=$true;popup_closes_independently=$true;cats_reopens_popup=$true;minimum_window_pixels=@(($smallRect.Right-$smallRect.Left),($smallRect.Bottom-$smallRect.Top));no_prompts_or_playback_actions=$true} | ConvertTo-Json | Set-Content -LiteralPath $Report -Encoding utf8
 Get-Content -LiteralPath $Report
} finally {
 if($owned -and -not $owned.HasExited){[void][HubProbe]::PostMessage($root,0x10,[IntPtr]::Zero,[IntPtr]::Zero);if(-not $owned.WaitForExit(8000)){throw 'Owned app did not close normally'}}
 [void][HubProbe]::SetCursorPos($cursor.X,$cursor.Y)
 [void][HubProbe]::SetThreadDpiAwarenessContext($previousDpi)
}
