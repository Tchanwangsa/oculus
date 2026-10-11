param(
  [Parameter(Mandatory = $true)][string]$ContentDir,
  [Parameter(Mandatory = $true)][string]$IconPath,
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
Add-Type -AssemblyName System.Drawing
$assets = Join-Path $ContentDir "Assets"
New-Item $assets -ItemType Directory -Force | Out-Null
$image = [Drawing.Image]::FromFile($IconPath)
try {
  foreach ($logo in @(@{ Name = "Square44x44Logo"; Size = 44 }, @{ Name = "Square150x150Logo"; Size = 150 }, @{ Name = "StoreLogo"; Size = 50 })) {
    foreach ($scale in @(100, 150, 200)) {
      $size = [int]($logo.Size * $scale / 100)
      $bitmap = [Drawing.Bitmap]::new($size, $size)
      $graphics = [Drawing.Graphics]::FromImage($bitmap)
      try {
        $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.Clear([Drawing.Color]::Transparent)
        $graphics.DrawImage($image, 0, 0, $size, $size)
        $bitmap.Save((Join-Path $assets "$($logo.Name).scale-$scale.png"), [Drawing.Imaging.ImageFormat]::Png)
      } finally { $graphics.Dispose(); $bitmap.Dispose() }
    }
  }
} finally { $image.Dispose() }
$config = Join-Path (Split-Path $OutputPath) "priconfig.xml"
& $makepri createconfig /cf $config /dq en-US /o
if ($LASTEXITCODE -ne 0) { throw "MakePri createconfig failed ($LASTEXITCODE)" }
& $makepri new /pr $ContentDir /cf $config /of (Join-Path $ContentDir "resources.pri") /o
if ($LASTEXITCODE -ne 0) { throw "MakePri resource indexing failed ($LASTEXITCODE)" }
& $makeappx pack /d $ContentDir /p $OutputPath /o
if ($LASTEXITCODE -ne 0) { throw "MakeAppx semantic validation or packaging failed ($LASTEXITCODE)" }
