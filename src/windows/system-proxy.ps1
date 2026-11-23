$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$policy = Get-ItemProperty 'HKLM:\Software\Policies\Microsoft\Windows\CurrentVersion\Internet Settings' -ErrorAction SilentlyContinue
$hive = if ($policy.ProxySettingsPerUser -eq 0) { 'HKLM:' } else { 'HKCU:' }
$p = Get-ItemProperty "$hive\Software\Microsoft\Windows\CurrentVersion\Internet Settings" -ErrorAction SilentlyContinue
# WinHTTP also has a machine-wide static configuration (netsh winhttp).
if (!$p.ProxyEnable -or !$p.ProxyServer) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class OrnnWinHttp {
    [StructLayout(LayoutKind.Sequential)]
    public struct ProxyInfo { public uint AccessType; public IntPtr Proxy; public IntPtr Bypass; }
    [DllImport("winhttp.dll")]
    public static extern bool WinHttpGetDefaultProxyConfiguration(out ProxyInfo info);
    [DllImport("kernel32.dll")]
    public static extern IntPtr GlobalFree(IntPtr value);
}
'@
    $info = New-Object OrnnWinHttp+ProxyInfo
    if ([OrnnWinHttp]::WinHttpGetDefaultProxyConfiguration([ref]$info)) {
        try {
            if ($info.AccessType -eq 3) {
                $p = [pscustomobject]@{
                    ProxyEnable = 1
                    ProxyServer = [Runtime.InteropServices.Marshal]::PtrToStringUni($info.Proxy)
                    ProxyOverride = [Runtime.InteropServices.Marshal]::PtrToStringUni($info.Bypass)
                }
            }
        } finally {
            if ($info.Proxy -ne [IntPtr]::Zero) { [void][OrnnWinHttp]::GlobalFree($info.Proxy) }
            if ($info.Bypass -ne [IntPtr]::Zero) { [void][OrnnWinHttp]::GlobalFree($info.Bypass) }
        }
    }
}
$result = @{}
if ($p.ProxyEnable -and $p.ProxyServer) {
    if ($p.ProxyServer.Contains('=')) {
        foreach ($item in $p.ProxyServer.Split(';')) {
            $pair = $item.Trim() -split '=',2
            if ($pair.Count -ne 2) {continue}
            switch ($pair[0]) {
                'http'  { $result.http_proxy = $pair[1] }
                'https' { $result.https_proxy = $pair[1] }
                'socks' { $result.all_proxy = if ($pair[1].Contains('://')) { $pair[1] } else { "socks5h://$($pair[1])" } }
            }
        }
    } else {
        $result.http_proxy = $result.https_proxy = $p.ProxyServer.Trim()
    }
    if ($p.ProxyOverride) {
        $result.no_proxy = (($p.ProxyOverride -split ';' | ForEach-Object { $_.Trim() -replace '^\*\.', '.' } | Where-Object { $_ -and $_ -ne '<local>' }) -join ',')
    }
}
ConvertTo-Json -InputObject $result -Compress
