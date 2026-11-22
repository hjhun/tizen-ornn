$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$owner = [Diagnostics.Process]::GetProcessById([int]$env:ORNN_TRAY_PARENT)
$null = $owner.Handle
$icon = New-Object Windows.Forms.NotifyIcon
$menu = New-Object Windows.Forms.ContextMenuStrip
function Add-Action($title, $action) {
    $item = $menu.Items.Add($title)
    $item.Tag = $action
    $item.add_Click({ param($sender, $eventArgs) [Console]::WriteLine($sender.Tag) })
}
try {
    $name = "Tizen Ornn - $env:ORNN_TRAY_IMAGE (running)"
    $icon.Text = $name.Substring(0, [Math]::Min(63, $name.Length))
    $icon.Icon = [Drawing.Icon]::ExtractAssociatedIcon($env:ORNN_TRAY_GUI)
    $header = $menu.Items.Add($name.Replace('&', '&&'))
    $header.Enabled = $false
    Add-Action 'Open terminal' 'terminal'
    Add-Action 'Settings' 'settings'
    $null = $menu.Items.Add('-')
    Add-Action 'Shut down guest' 'shutdown'
    $icon.ContextMenuStrip = $menu
    $icon.add_DoubleClick({ [Console]::WriteLine('terminal') })
    $timer = New-Object Windows.Forms.Timer
    $timer.Interval = 500
    $timer.add_Tick({ if ($owner.HasExited) { [Windows.Forms.Application]::ExitThread() } })
    $timer.Start()
    $icon.Visible = $true
    [Windows.Forms.Application]::Run()
} finally {
    $icon.Visible = $false
    $icon.Dispose()
    $menu.Dispose()
    $owner.Dispose()
}
