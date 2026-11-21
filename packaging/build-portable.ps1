# Inputs are the matching Windows CLI, GUI and builder/ plus an extracted x64 QEMU distribution.
param([Parameter(Mandatory)][string]$Payload,
      [Parameter(Mandatory)][string]$QemuDir,
      [Parameter(Mandatory)][string]$Output)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$devices = & "$QemuDir/qemu-system-x86_64.exe" -device help
if ($LASTEXITCODE -or !($devices -match 'virtio-9p-pci')) {
    throw 'The Windows QEMU distribution must include virtio-9p-pci (QCW UCRT64).'
}
$net = & "$QemuDir/qemu-system-x86_64.exe" -netdev help
if ($LASTEXITCODE -or !($net -match '^user$')) { throw 'QEMU user networking is required.' }
$stage = New-Item -ItemType Directory (Join-Path $env:TEMP ([guid]::NewGuid()))
try {
    $files = New-Item -ItemType Directory "$stage/files"
    Copy-Item (Join-Path $PSScriptRoot "../LICENSE") $files
    Copy-Item (Join-Path $PSScriptRoot "art/fonts/OFL.txt") "$files/Barlow-OFL.txt"
    $builder = New-Item -ItemType Directory "$files/builder"
    $qemu = New-Item -ItemType Directory "$files/qemu"
    $share = New-Item -ItemType Directory "$qemu/share"
    Copy-Item "$Payload/tizen-ornn.exe","$Payload/tizen-ornn-gui.exe" $files
    Copy-Item "$Payload/builder/bzImage","$Payload/builder/tizen-ornn_amd64.deb" $builder
    Copy-Item "$QemuDir/*.dll","$QemuDir/qemu-system-x86_64.exe","$QemuDir/qemu-img.exe","$QemuDir/COPYING*","$QemuDir/README.rst","$QemuDir/VERSION" $qemu
    Copy-Item "$QemuDir/share/*.bin","$QemuDir/share/*.rom","$QemuDir/share/keymaps" $share -Recurse
    $archive = "$stage/payload.zip"
    [IO.Compression.ZipFile]::CreateFromDirectory($files.FullName, $archive)
    $compiler = "$env:WINDIR/Microsoft.NET/Framework64/v4.0.30319/csc.exe"
    & $compiler /nologo /target:winexe "/win32icon:$PSScriptRoot/art/tizen-ornn.ico" /platform:x64 /optimize+ "/out:$Output" "/resource:$archive,payload.zip" /r:System.IO.Compression.dll /r:System.IO.Compression.FileSystem.dll /r:System.Windows.Forms.dll (Join-Path $PSScriptRoot PortableLauncher.cs)
    if ($LASTEXITCODE) { throw 'Portable launcher compilation failed' }
} finally {
    Remove-Item $stage -Force -Recurse
}
