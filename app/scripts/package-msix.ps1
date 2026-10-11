param(
  [Parameter(Mandatory = $true)][string]$ContentDir,
  [Parameter(Mandatory = $true)][string]$OutputPath
)
$ErrorActionPreference = "Stop"
$ContentDir = [IO.Path]::GetFullPath($ContentDir)
$OutputPath = [IO.Path]::GetFullPath($OutputPath)
$kits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
$sdk = Get-ChildItem $kits -Directory | Where-Object {
  (Test-Path (Join-Path $_.FullName "x64\makeappx.exe")) -and
  (Test-Path (Join-Path $_.FullName "x64\makepri.exe"))
} | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (-not $sdk) { throw "Install a Windows 10/11 SDK containing x64 MakePri.exe and MakeAppx.exe" }
$makeappx = Join-Path $sdk.FullName "x64\makeappx.exe"
$makepri = Join-Path $sdk.FullName "x64\makepri.exe"
$config = Join-Path (Split-Path $OutputPath) "priconfig.xml"
& $makepri createconfig /cf $config /dq en-US /o
if ($LASTEXITCODE -ne 0) { throw "MakePri createconfig failed ($LASTEXITCODE)" }
& $makepri new /pr $ContentDir /cf $config /of (Join-Path $ContentDir "resources.pri") /o
if ($LASTEXITCODE -ne 0) { throw "MakePri resource indexing failed ($LASTEXITCODE)" }
& $makeappx pack /d $ContentDir /p $OutputPath /o
if ($LASTEXITCODE -ne 0) { throw "MakeAppx semantic validation or packaging failed ($LASTEXITCODE)" }
