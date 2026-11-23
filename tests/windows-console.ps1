# Drive the serial console of a fixture or a freshly provisioned development VM.
param([Parameter(Mandatory)] [string] $Cli, [Parameter(Mandatory)] [string] $Image,
      [switch] $Development)
$ErrorActionPreference = 'Stop'
# Use a private settings directory for both the CLI and its guest.
$state = New-Item -ItemType Directory (Join-Path $env:TEMP ([guid]::NewGuid()))
$env:TIZEN_ORNN_PORTABLE_DIR = $state.FullName
$share = New-Item -ItemType Directory "$state/shared folder"
Set-Content "$share/host.txt" 'from-host' -Encoding ascii -NoNewline
& $Cli share-dir $share.FullName
if ($LASTEXITCODE) { throw 'Failed to save the shared folder' }
$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = $Cli
$info.ArgumentList.Add('run')
$info.ArgumentList.Add('--headless')
$info.ArgumentList.Add('--image-dir')
$info.ArgumentList.Add($Image)
$info.UseShellExecute = $false
$info.RedirectStandardInput = $true
$info.RedirectStandardOutput = $true
$proc = [System.Diagnostics.Process]::Start($info)

function Wait-Console([string] $Pattern) {
    $buffer = [char[]]::new(4096)
    $tail = ''
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while ($timer.Elapsed.TotalSeconds -lt 180) {
        $read = $proc.StandardOutput.ReadAsync($buffer, 0, $buffer.Length)
        if (-not $read.Wait([Math]::Max(1, 180000 - [int]$timer.ElapsedMilliseconds))) { break }
        if ($read.Result -eq 0) { break }
        $tail += [string]::new($buffer, 0, $read.Result)
        if ($tail -match $Pattern) { return }
        if ($tail.Length -gt 12000) { $tail = $tail.Substring($tail.Length - 12000) }
    }
    throw "Missing console output matching $Pattern. Last guest output: $tail"
}
function Send-Console([string] $Command) {
    $proc.StandardInput.Write($Command + [char]13)
    $proc.StandardInput.Flush()
}
try {
    if ($Development) {
        Wait-Console 'tizen@[^\r\n]*\$ '
        Send-Console 'id; test "$(id -un)" = tizen && test "$(id -u)" != 0 && id -G | tr " " "\n" | grep -qx 0 && rpm -q git >/dev/null && printf "ORNN_%s_OK\n" ROOTGROUP'
        Wait-Console '(?m)^ORNN_ROOTGROUP_OK\r?$'
        Send-Console 'test "$(cat /mnt/host/host.txt)" = from-host && echo guest-write > /home/tizen/fixture-check && test "$(cat /mnt/host/fixture-check)" = guest-write && printf "ORNN_%s_OK\n" SHARE'
        Wait-Console '(?m)^ORNN_SHARE_OK\r?$'
        Send-Console 'su -c "sync; systemctl --force poweroff"'
    } else {
        Wait-Console 'ORNN_SERIAL_READY'
        # Close the viewer while the guest is waiting for input and reconnect.
        $proc.Kill()
        $proc.WaitForExit()
        $proc.Dispose()
        $info.ArgumentList.Clear()
        foreach ($arg in @('console', '--image-dir', $Image)) {
            $info.ArgumentList.Add($arg)
        }
        $proc = [System.Diagnostics.Process]::Start($info)
        Wait-Console 'ORNN_SERIAL_READY'
        Send-Console 'ornn-console-check'
        Wait-Console '(?m)^ORNN_SERIAL_OK\r?$'
    }
    if ((Get-Content -LiteralPath "$share/fixture-check") -ne 'guest-write') {
        throw 'Missing guest write on the Windows host'
    }
    if (-not $proc.WaitForExit(30000) -or $proc.ExitCode -ne 0) {
        throw "Guest shutdown failed: exited=$($proc.HasExited) code=$($proc.ExitCode)"
    }
} finally {
    if (-not $proc.HasExited) { $proc.Kill($true) }
    $proc.Dispose()
    Remove-Item -Recurse -Force -LiteralPath $state.FullName
}
