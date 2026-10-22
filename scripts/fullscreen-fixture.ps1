param([Parameter(Mandatory=$true)][string]$ReadyFile)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
$form=[Windows.Forms.Form]::new()
$form.Text='Cikung Fullscreen Fixture'
$form.FormBorderStyle='None'
$form.StartPosition='Manual'
$form.Bounds=[Windows.Forms.Screen]::PrimaryScreen.Bounds
$form.BackColor=[Drawing.Color]::FromArgb(24,24,28)
$label=[Windows.Forms.Label]::new()
$label.Text='Cikung fullscreen check — closes automatically'
$label.ForeColor=[Drawing.Color]::White
$label.AutoSize=$true
$label.Location=[Drawing.Point]::new(40,90)
$form.Controls.Add($label)
$form.Add_Shown({$form.Activate();[IO.File]::WriteAllText($ReadyFile,$form.Handle.ToInt64().ToString())})
[Windows.Forms.Application]::Run($form)
