param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [string]$Icon = (Join-Path $PSScriptRoot '..\assets\icons\cikung.ico')
)
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$Icon = (Resolve-Path -LiteralPath $Icon).Path
if (-not ('CikungIconResource' -as [type])) {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class CikungIconResource {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    public static extern IntPtr BeginUpdateResource(string file, bool deleteExisting);
    [DllImport("kernel32.dll", SetLastError=true)]
    public static extern bool UpdateResource(IntPtr update, IntPtr type, IntPtr name, ushort language, byte[] data, uint size);
    [DllImport("kernel32.dll", SetLastError=true)]
    public static extern bool EndUpdateResource(IntPtr update, bool discard);
    [DllImport("shell32.dll", CharSet=CharSet.Unicode)]
    public static extern uint ExtractIconEx(string file, int index, IntPtr large, IntPtr small, uint count);
}
"@
}
$bytes = [IO.File]::ReadAllBytes($Icon)
if ($bytes.Length -lt 6 -or [BitConverter]::ToUInt16($bytes,0) -ne 0 -or [BitConverter]::ToUInt16($bytes,2) -ne 1) {
    throw 'Expected a Windows ICO file.'
}
$count = [BitConverter]::ToUInt16($bytes,4)
if ($count -eq 0 -or $count -gt 32 -or $bytes.Length -lt 6+16*$count) { throw 'Invalid ICO directory.' }
$group = [byte[]]::new(6+14*$count)
[Array]::Copy($bytes,0,$group,0,6)
$images = [Collections.Generic.List[byte[]]]::new()
for ($i=0; $i -lt $count; $i++) {
    $entry=6+16*$i
    $length=[BitConverter]::ToUInt32($bytes,$entry+8)
    $offset=[BitConverter]::ToUInt32($bytes,$entry+12)
    if ($length -eq 0 -or $offset -lt 6+16*$count -or [uint64]$offset+$length -gt $bytes.LongLength) { throw 'ICO image exceeds file bounds.' }
    $image=[byte[]]::new($length)
    [Array]::Copy($bytes,$offset,$image,0,$length)
    $images.Add($image)
    [Array]::Copy($bytes,$entry,$group,6+14*$i,12)
    [Array]::Copy([BitConverter]::GetBytes([uint16]($i+1)),0,$group,6+14*$i+12,2)
}
$update=[CikungIconResource]::BeginUpdateResource($Executable,$false)
if ($update -eq [IntPtr]::Zero) { throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error()) }
try {
    for ($i=0; $i -lt $count; $i++) {
        if (-not [CikungIconResource]::UpdateResource($update,[IntPtr]3,[IntPtr]($i+1),0,$images[$i],$images[$i].Length)) {
            throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
        }
    }
    if (-not [CikungIconResource]::UpdateResource($update,[IntPtr]14,[IntPtr]1,0,$group,$group.Length)) {
        throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
    }
    if (-not [CikungIconResource]::EndUpdateResource($update,$false)) {
        throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
    }
    $update=[IntPtr]::Zero
} finally {
    if ($update -ne [IntPtr]::Zero) { [void][CikungIconResource]::EndUpdateResource($update,$true) }
}
if ([CikungIconResource]::ExtractIconEx($Executable,-1,[IntPtr]::Zero,[IntPtr]::Zero,0) -lt 1) { throw 'Windows could not read the embedded icon.' }
Write-Output "Orange cat icon embedded: $Executable"
