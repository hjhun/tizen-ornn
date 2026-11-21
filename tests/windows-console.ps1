# Test a serial round trip through the Windows CLI, QEMU and the fixture kernel.
param([Parameter(Mandatory)] [string] $Cli, [Parameter(Mandatory)] [string] $Image)
$ErrorActionPreference = 'Stop'
$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = $Cli
$info.Arguments = 'run --headless --image-dir "' + $Image + '"'
$info.UseShellExecute = $false
$info.RedirectStandardInput = $true
$info.RedirectStandardOutput = $true
$proc = [System.Diagnostics.Process]::Start($info)
$timer = [Diagnostics.Stopwatch]::StartNew()
$answered = $false
$tail = [System.Collections.Generic.Queue[string]]::new()
try {
    while ($timer.Elapsed.TotalSeconds -lt 150) {
        $read = $proc.StandardOutput.ReadLineAsync()
        if (-not $read.Wait([Math]::Max(1, 150000 - [int]$timer.ElapsedMilliseconds))) { break }
        $line = $read.Result
        if ($null -eq $line) { break }
        $tail.Enqueue($line)
        if ($tail.Count -gt 40) { [void]$tail.Dequeue() }
        if ($line.Trim() -eq 'ORNN_SERIAL_READY') {
            $proc.StandardInput.WriteLine('ornn-console-check')
            $proc.StandardInput.Flush()
        }
        if ($line.Trim() -eq 'ORNN_SERIAL_OK') { $answered = $true; break }
    }
    if (-not $answered -or -not $proc.WaitForExit(20000) -or $proc.ExitCode -ne 0) {
        Write-Host ($tail -join [Environment]::NewLine)
        throw "Guest console failed: answered=$answered exited=$($proc.HasExited) code=$($proc.ExitCode)"
    }
} finally {
    if (-not $proc.HasExited) { $proc.Kill($true) }
    $proc.Dispose()
}
