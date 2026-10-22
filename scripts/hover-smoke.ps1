param(
    [string]$Executable = 'D:\Nuch\target\x86_64-pc-windows-gnu\debug\cikung.exe',
    [string]$Report = 'D:\Nuch\docs\hover-smoke.json'
)
$ErrorActionPreference = 'Stop'
# Test only the Cikung process launched here; restore the cursor and close it afterward.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class NotchProbe {
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr w,out Rect r);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point p);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point p);
  [DllImport("user32.dll")] public static extern int GetWindowRgn(IntPtr w,IntPtr r);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateRectRgn(int l,int t,int r,int b);
  [DllImport("gdi32.dll")] public static extern int GetRgnBox(IntPtr r,out Rect b);
  [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr r);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr w,uint m,IntPtr a,IntPtr b);
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
  private delegate bool EnumProc(IntPtr w,IntPtr p);
  [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc f,IntPtr p);
  [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr w,out uint p);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr w,StringBuilder s,int n);
  public static IntPtr FindApp(uint pid) {
    IntPtr found=IntPtr.Zero;
    EnumWindows((w,p)=>{ uint owner; GetWindowThreadProcessId(w,out owner);
      if(owner==pid){var text=new StringBuilder(128);GetWindowText(w,text,128);
        Rect r;GetWindowRect(w,out r);if(text.ToString()=="Cikung"&&r.Right-r.Left>100){found=w;return false;}}
      return true;
    },IntPtr.Zero);return found;
  }
}
'@
$originalCursor = [NotchProbe+Point]::new()
$previousDpiContext=[NotchProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
[NotchProbe]::GetCursorPos([ref]$originalCursor) | Out-Null
$ownedApp = $null
$samples = [Collections.Generic.List[object]]::new()
$clock = [Diagnostics.Stopwatch]::StartNew()
function Read-Shape([string]$phase) {
    $bounds = [NotchProbe+Rect]::new()
    $hostBounds = [NotchProbe+Rect]::new()
    [NotchProbe]::GetWindowRect($script:window,[ref]$hostBounds) | Out-Null
    $region = [NotchProbe]::CreateRectRgn(0,0,1,1)
    try {
        if([NotchProbe]::GetWindowRgn($script:window,$region) -eq 0) { return }
        [NotchProbe]::GetRgnBox($region,[ref]$bounds) | Out-Null
    } finally { [NotchProbe]::DeleteObject($region) | Out-Null }
    $cursorNow=[NotchProbe+Point]::new()
    [NotchProbe]::GetCursorPos([ref]$cursorNow) | Out-Null
    $samples.Add([pscustomobject][ordered]@{ms=$clock.ElapsedMilliseconds;phase=$phase;cursor=@($cursorNow.X,$cursorNow.Y);
        width=$bounds.Right-$bounds.Left;height=$bounds.Bottom-$bounds.Top;
        region=@($bounds.Left,$bounds.Top,$bounds.Right,$bounds.Bottom);
        host=@($hostBounds.Left,$hostBounds.Top,$hostBounds.Right,$hostBounds.Bottom)})
}
function Sample-For([string]$phase,[int]$duration) {
    $deadline=$clock.ElapsedMilliseconds+$duration
    while($clock.ElapsedMilliseconds -lt $deadline) { Read-Shape $phase; Start-Sleep -Milliseconds 12 }
}
try {
    [NotchProbe]::SetCursorPos(100,700) | Out-Null
    $ownedApp = Start-Process -FilePath $Executable -ArgumentList '--demo' -WindowStyle Hidden -PassThru
    $deadline=[datetime]::UtcNow.AddSeconds(10)
    do { Start-Sleep -Milliseconds 80; $ownedApp.Refresh(); $script:window=[NotchProbe]::FindApp($ownedApp.Id) }
    while($script:window -eq [IntPtr]::Zero -and [datetime]::UtcNow -lt $deadline)
    if($script:window -eq [IntPtr]::Zero){ throw 'Cikung test window was not created.' }
    # A native window can exist while the embedded character textures are loading.
    # Begin pointer scenarios only after the shaped notch is actually ready.
    $readyDeadline=[datetime]::UtcNow.AddSeconds(15)
    do {
        $readyRegion=[NotchProbe]::CreateRectRgn(0,0,1,1)
        try {$shapeReady=[NotchProbe]::GetWindowRgn($script:window,$readyRegion) -ne 0}
        finally {[void][NotchProbe]::DeleteObject($readyRegion)}
        if(-not $shapeReady){Start-Sleep -Milliseconds 100}
    } while(-not $shapeReady -and [datetime]::UtcNow -lt $readyDeadline)
    if(-not $shapeReady){throw 'The shaped Cikung window was not ready.'}
    Start-Sleep -Milliseconds 1000
    Sample-For 'initial' 300
    $hostRect=[NotchProbe+Rect]::new()
    [NotchProbe]::GetWindowRect($script:window,[ref]$hostRect) | Out-Null
    $center=[int](($hostRect.Left+$hostRect.Right)/2)
    $insideY=$hostRect.Top+20
    $outsideX=$hostRect.Left+10
    $outsideY=$hostRect.Bottom+70
    [NotchProbe]::SetCursorPos($center,$insideY) | Out-Null
    Sample-For 'brief-pass' 55
    [NotchProbe]::SetCursorPos($outsideX,$outsideY) | Out-Null
    Sample-For 'brief-left' 250
    [NotchProbe]::SetCursorPos($center,$insideY) | Out-Null
    Sample-For 'open' 750
    [NotchProbe]::SetCursorPos($outsideX,$outsideY) | Out-Null
    Sample-For 'leave-grace' 280
    [NotchProbe]::SetCursorPos($center,$insideY) | Out-Null
    Sample-For 'reenter' 450
    [NotchProbe]::SetCursorPos($outsideX,$outsideY) | Out-Null
    Sample-For 'close' 1100
    $transparentPoint=[NotchProbe+Point]::new()
    $transparentPoint.X=$hostRect.Left+15; $transparentPoint.Y=$hostRect.Top+150
    $clickThrough=[NotchProbe]::WindowFromPoint($transparentPoint) -ne $script:window
    [NotchProbe]::SetCursorPos($center,$insideY) | Out-Null
    Sample-For 'open-again' 650
    [NotchProbe]::PostMessage($script:window,0x100,[IntPtr]0x1B,[IntPtr]::Zero) | Out-Null
    [NotchProbe]::PostMessage($script:window,0x101,[IntPtr]0x1B,[IntPtr]::Zero) | Out-Null
    Sample-For 'explicit-close' 800
    $initialWidth=$samples[0].width
    $hostVariants=@($samples | ForEach-Object { $_.host -join ',' } | Select-Object -Unique)
    $openingVariants=@($samples | Where-Object phase -eq 'open' | Select-Object -ExpandProperty width -Unique)
    $expandedWidth=($samples | Measure-Object -Property width -Maximum).Maximum
    $briefMax=($samples | Where-Object phase -in @('brief-pass','brief-left') | Measure-Object -Property width -Maximum).Maximum
    $reentryMin=($samples | Where-Object phase -eq 'reenter' | Measure-Object -Property width -Minimum).Minimum
    $closedWidth=($samples | Where-Object phase -eq 'close' | Select-Object -Last 1).width
    $explicitWidth=($samples | Where-Object phase -eq 'explicit-close' | Select-Object -Last 1).width
    $centered=@($samples | Where-Object { [Math]::Abs(($_.region[0]+$_.region[2])-($_.host[2]-$_.host[0])) -gt 2 }).Count -eq 0
    $result=[ordered]@{fixed_host=$hostVariants.Count -eq 1;centered=$centered;
        brief_pass_ignored=$briefMax -le $initialWidth+2;opening_distinct_widths=$openingVariants.Count;
        reentry_keeps_open=$reentryMin -ge $expandedWidth-2;auto_close=$closedWidth -le $initialWidth+2;
        explicit_close_stays_closed=$explicitWidth -le $initialWidth+2;transparent_area_click_through=$clickThrough;
        initial_width=$initialWidth;expanded_width=$expandedWidth;samples=$samples}
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $Report
    [pscustomobject]$result | Select-Object -ExcludeProperty samples | ConvertTo-Json
    if(-not $result.fixed_host -or -not $result.centered -or -not $result.brief_pass_ignored -or
        $openingVariants.Count -lt 5 -or -not $result.reentry_keeps_open -or -not $result.auto_close -or
        -not $result.explicit_close_stays_closed -or -not $clickThrough){throw 'One or more native hover checks failed; see report.'}
} finally {
    if($ownedApp -and -not $ownedApp.HasExited){
        [NotchProbe]::PostMessage($script:window,0x10,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
        if(-not $ownedApp.WaitForExit(3000)){ $ownedApp.Kill() }
    }
    [NotchProbe]::SetCursorPos($originalCursor.X,$originalCursor.Y) | Out-Null
    [NotchProbe]::SetThreadDpiAwarenessContext($previousDpiContext) | Out-Null
}
