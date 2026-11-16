# IrisPhone installer for Windows.
#   irm https://raw.githubusercontent.com/Iris-universe/IrisPhone/main/install.ps1 | iex
# Downloads the latest irisphone.exe, adds it to your PATH (current user only), then runs `irisphone setup`.
# Set $env:IRISPHONE_NO_SETUP = "1" to skip the setup step,
# $env:IRISPHONE_DOWNLOAD_URL to download the binary from somewhere else (mirror, local test build).

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest is very slow with the progress bar

$Repo = 'Iris-universe/IrisPhone'
$Arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
$Asset = "irisphone-windows-$Arch.exe"
$Dir = Join-Path $env:LOCALAPPDATA 'Programs\IrisPhone'
$Exe = Join-Path $Dir 'irisphone.exe'

Write-Host "==> Downloading IrisPhone ($Arch)..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $Dir | Out-Null
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$tmp = "$Exe.download"
$Url = if ($env:IRISPHONE_DOWNLOAD_URL) { $env:IRISPHONE_DOWNLOAD_URL } else { "https://github.com/$Repo/releases/latest/download/$Asset" }
Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $tmp
Move-Item -Force $tmp $Exe

# Read the raw user PATH from the registry so %VARIABLES% in it stay unexpanded.
$EnvKey = 'HKCU:\Environment'
$rawPath = (Get-Item $EnvKey).GetValue('Path', '', 'DoNotExpandEnvironmentNames')
if (($rawPath -split ';') -notcontains $Dir) {
    $newPath = ($rawPath.TrimEnd(';') + ";$Dir").TrimStart(';')
    Set-ItemProperty -Path $EnvKey -Name Path -Value $newPath -Type ExpandString
    # Setting any variable through .NET broadcasts the change to running programs (Explorer, terminals).
    [Environment]::SetEnvironmentVariable('IRISPHONE_INSTALLED', '1', 'User')
    Write-Host "    added $Dir to your PATH (new terminals will see it)" -ForegroundColor Green
}
if (($env:Path -split ';') -notcontains $Dir) { $env:Path += ";$Dir" }

& $Exe --version
if ($env:IRISPHONE_NO_SETUP -ne '1') {
    & $Exe setup
}
