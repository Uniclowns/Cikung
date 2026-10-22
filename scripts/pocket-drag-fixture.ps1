param([string]$ReadyFile,[string]$File)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
$form=[Windows.Forms.Form]::new()
$form.Text='Cikung pocket drag fixture'
$form.StartPosition='Manual'
$form.Location=[Drawing.Point]::new(40,600)
$form.ClientSize=[Drawing.Size]::new(300,100)
$form.FormBorderStyle='FixedToolWindow'
$form.TopMost=$true
$label=[Windows.Forms.Button]::new()
$label.Dock='Fill'
$label.Text='Drag this test file to Cikung'
$label.TextAlign='MiddleCenter'
$label.Add_MouseDown({
    $data=[Windows.Forms.DataObject]::new()
    $data.SetData([Windows.Forms.DataFormats]::FileDrop,[string[]]@($File))
    [void]$label.DoDragDrop($data,[Windows.Forms.DragDropEffects]::Copy)
})
$form.Controls.Add($label)
$form.Add_Shown({[IO.File]::WriteAllText($ReadyFile,$label.Handle.ToInt64().ToString())})
[Windows.Forms.Application]::Run($form)

