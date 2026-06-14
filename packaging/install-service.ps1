# PowerShell Script to install Aura as a Windows Service
# Must be run as Administrator

$ExePath = Join-Path $PSScriptRoot "aura.exe"
if (-not (Test-Path $ExePath)) {
    $ExePath = "C:\Program Files\Aura\aura.exe"
}

if (-not (Test-Path $ExePath)) {
    Write-Error "Aura executable not found. Please place aura.exe in this folder or at C:\Program Files\Aura\aura.exe"
    exit 1
}

Write-Host "Registering Aura Download Engine as a Windows Service..."
New-Service -Name "AuraDaemon" -BinaryPathName "`"$ExePath`" daemon --windows-service" -DisplayName "Aura Download Service" -StartupType Automatic
Start-Service -Name "AuraDaemon"
Write-Host "Aura service registered and started."
