param([Parameter(Mandatory=$true)][string]$ReadyFile)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
$form=[Windows.Forms.Form]::new()
$form.Text='Cikung Desktop Fixture'
$form.Size=[Drawing.Size]::new(390,220)
$form.StartPosition='Manual'
$form.Location=[Drawing.Point]::new(40,650)
$form.ShowInTaskbar=$false
$conversation=[Windows.Forms.Panel]::new()
$conversation.AccessibleName='Conversation'
$conversation.Location=[Drawing.Point]::new(12,12)
$conversation.Size=[Drawing.Size]::new(350,50)
$reply=[Windows.Forms.Label]::new()
$reply.Text='Fixture conversation ready'
$reply.AutoSize=$true
$conversation.Controls.Add($reply)
$composer=[Windows.Forms.TextBox]::new()
$composer.AccessibleName='Work with ChatGPT'
$composer.Location=[Drawing.Point]::new(12,75)
$composer.Size=[Drawing.Size]::new(350,26)
$send=[Windows.Forms.Button]::new()
$send.Text='Send message'
$send.AccessibleName='Send message'
$send.Location=[Drawing.Point]::new(12,112)
$send.Size=[Drawing.Size]::new(150,28)
$stop=[Windows.Forms.Button]::new()
$stop.Text='Stop generating'
$stop.AccessibleName='Stop generating'
$stop.Location=[Drawing.Point]::new(170,112)
$stop.Size=[Drawing.Size]::new(150,28)
$stop.Visible=$false
$completeFile=[IO.Path]::ChangeExtension($ReadyFile,'complete')
$timer=[Windows.Forms.Timer]::new()
$timer.Interval=50
$timer.Add_Tick({
    if([IO.File]::Exists($completeFile)){
        $timer.Stop()
        $reply.Text='Fixture reply: '+$script:pendingMessage
        $stop.Visible=$false
        $send.Enabled=$true
    }
})
$send.Add_Click({
    $script:pendingMessage=$composer.Text
    $composer.Text=''
    $reply.Text='Fixture working'
    $send.Enabled=$false
    $stop.Visible=$true
    $timer.Start()
})
$form.Controls.AddRange(@($conversation,$composer,$send,$stop))
$form.Add_Shown({[IO.File]::WriteAllText($ReadyFile,$form.Handle.ToInt64().ToString())})
[Windows.Forms.Application]::Run($form)
