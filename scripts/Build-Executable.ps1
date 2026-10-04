param(
  [string]$PythonExe='',
  [string]$PythonDirectory='',
  [string]$OutputDirectory=''
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Resolve-Python312.ps1')
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.IO.Compression.FileSystem

$workspaceRoot=Split-Path -Parent $PSScriptRoot
$applicationDirectory=Join-Path $workspaceRoot 'src'
if ($PythonDirectory) {
  $pythonDirectory=(Resolve-Path -LiteralPath $PythonDirectory).Path
  [void](Resolve-Python312 (Join-Path $pythonDirectory 'python.exe'))
} else {
  $pythonDirectory=Split-Path -Parent (Resolve-Python312 $PythonExe)
}
if (-not $OutputDirectory) { $OutputDirectory=Join-Path $workspaceRoot 'dist' }
$OutputDirectory=[IO.Path]::GetFullPath($OutputDirectory)
[void][IO.Directory]::CreateDirectory($OutputDirectory)
$buildDirectory=Join-Path $OutputDirectory '.build'
[void][IO.Directory]::CreateDirectory($buildDirectory)
$compiler=Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$executablePath=Join-Path $OutputDirectory 'F1-Radio-Translator-1.0.3.exe'
$stageDirectory=Join-Path $buildDirectory ('stage-'+[Guid]::NewGuid().ToString('N'))
$payloadPath=Join-Path $buildDirectory 'payload.zip'
$payloadIdPath=Join-Path $buildDirectory 'payload.id'
$iconPath=Join-Path $applicationDirectory 'Translator.ico'
$logoPath=Join-Path $applicationDirectory 'Translator-logo.png'

if (-not (Test-Path -LiteralPath $compiler)) { throw 'The Windows .NET Framework C# compiler is unavailable.' }
if (-not (Test-Path -LiteralPath (Join-Path $pythonDirectory 'python312.dll'))) { throw 'A full Python 3.12 installation is required.' }
if (-not (Test-Path -LiteralPath (Join-Path $applicationDirectory 'python-deps\rapidocr'))) { throw 'Run scripts\Setup-Dependencies.ps1 before building.' }

function Copy-PayloadTree([string]$Source,[string]$Destination,[string[]]$SkipAtRoot=@()) {
  [void][IO.Directory]::CreateDirectory($Destination)
  foreach ($item in Get-ChildItem -LiteralPath $Source -Force) {
    if ($item.Name -in $SkipAtRoot -or $item.Name -eq '__pycache__' -or $item.Extension -in @('.pyc','.pyo')) { continue }
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('Unexpected linked payload item: '+$item.FullName) }
    $targetPath=Join-Path $Destination $item.Name
    if ($item.PSIsContainer) { Copy-PayloadTree $item.FullName $targetPath }
    else { [IO.File]::Copy($item.FullName,$targetPath,$true) }
  }
}

if (-not (Test-Path -LiteralPath $logoPath)) { throw 'The application logo PNG is unavailable.' }
$logoImage=[Drawing.Image]::FromFile($logoPath)
$iconFrames=[Collections.Generic.List[byte[]]]::new()
$iconSizes=@(16,20,24,32,40,48,64,128,256)
try {
  foreach ($size in $iconSizes) {
    $frameBitmap=[Drawing.Bitmap]::new($size,$size,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $frameGraphics=[Drawing.Graphics]::FromImage($frameBitmap)
    $frameStream=[IO.MemoryStream]::new()
    try {
      $frameGraphics.Clear([Drawing.Color]::Transparent)
      $frameGraphics.CompositingQuality=[Drawing.Drawing2D.CompositingQuality]::HighQuality
      $frameGraphics.InterpolationMode=[Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
      $frameGraphics.PixelOffsetMode=[Drawing.Drawing2D.PixelOffsetMode]::HighQuality
      $frameGraphics.DrawImage($logoImage,[Drawing.Rectangle]::new(0,0,$size,$size))
      $frameBitmap.Save($frameStream,[Drawing.Imaging.ImageFormat]::Png)
      $iconFrames.Add($frameStream.ToArray())
    } finally { $frameGraphics.Dispose(); $frameBitmap.Dispose(); $frameStream.Dispose() }
  }
} finally { $logoImage.Dispose() }
# ICO embeds independent PNG frames so Windows can pick the right size and DPI.
$iconWriter=[IO.BinaryWriter]::new([IO.File]::Create($iconPath))
try {
  $iconWriter.Write([UInt16]0); $iconWriter.Write([UInt16]1); $iconWriter.Write([UInt16]$iconSizes.Count)
  $frameOffset=6+16*$iconSizes.Count
  for ($frameIndex=0; $frameIndex -lt $iconSizes.Count; $frameIndex++) {
    $dimension=if ($iconSizes[$frameIndex] -eq 256) { 0 } else { $iconSizes[$frameIndex] }
    $iconWriter.Write([byte]$dimension); $iconWriter.Write([byte]$dimension)
    $iconWriter.Write([byte]0); $iconWriter.Write([byte]0)
    $iconWriter.Write([UInt16]1); $iconWriter.Write([UInt16]32)
    $iconWriter.Write([UInt32]$iconFrames[$frameIndex].Length); $iconWriter.Write([UInt32]$frameOffset)
    $frameOffset+=$iconFrames[$frameIndex].Length
  }
  foreach ($frameBytes in $iconFrames) { $iconWriter.Write([byte[]]$frameBytes) }
} finally { $iconWriter.Dispose() }

try {
  Write-Output 'Collecting application, OCR models, and Python runtime...'
  [void][IO.Directory]::CreateDirectory($stageDirectory)
  foreach ($sourceFile in @('Start-MultiViewerTranslator.ps1','Start-MultiViewerTranslator.bat','rapidocr_worker.py','RegionSelector.cs','DesktopIntegration.cs','Translator-logo.png','Translator.ico')) {
    [IO.File]::Copy((Join-Path $applicationDirectory $sourceFile),(Join-Path $stageDirectory $sourceFile),$true)
  }
  Copy-PayloadTree (Join-Path $applicationDirectory 'python-deps') (Join-Path $stageDirectory 'python-deps')
  foreach ($noticeFile in @('LICENSE','THIRD_PARTY_NOTICES.md')) {
    [IO.File]::Copy((Join-Path $workspaceRoot $noticeFile),(Join-Path $stageDirectory $noticeFile),$true)
  }
  Write-Output 'Compiling the screen region selector...'
  & $compiler '/nologo' '/target:library' '/platform:x64' '/optimize+' '/codepage:65001' '/reference:System.Windows.Forms.dll' '/reference:System.Drawing.dll' ('/out:'+(Join-Path $stageDirectory 'RegionSelector.dll')) (Join-Path $applicationDirectory 'RegionSelector.cs')
  if ($LASTEXITCODE -ne 0) { throw 'Screen region selector compilation failed.' }
  & $compiler '/nologo' '/target:library' '/platform:x64' '/optimize+' '/codepage:65001' ('/out:'+(Join-Path $stageDirectory 'DesktopIntegration.dll')) (Join-Path $applicationDirectory 'DesktopIntegration.cs')
  if ($LASTEXITCODE -ne 0) { throw 'Desktop integration compilation failed.' }
  $runtimeDestination=Join-Path $stageDirectory 'python-runtime'
  [void][IO.Directory]::CreateDirectory($runtimeDestination)
  foreach ($runtimeFile in Get-ChildItem -LiteralPath $pythonDirectory -File) {
    [IO.File]::Copy($runtimeFile.FullName,(Join-Path $runtimeDestination $runtimeFile.Name),$true)
  }
  Copy-PayloadTree (Join-Path $pythonDirectory 'DLLs') (Join-Path $runtimeDestination 'DLLs')
  Copy-PayloadTree (Join-Path $pythonDirectory 'Lib') (Join-Path $runtimeDestination 'Lib') @('site-packages','test','idlelib','turtledemo','ensurepip','tkinter')

  Write-Output 'Compressing the self-contained application payload...'
  if (Test-Path -LiteralPath $payloadPath) { Remove-Item -LiteralPath $payloadPath }
  [IO.Compression.ZipFile]::CreateFromDirectory($stageDirectory,$payloadPath,[IO.Compression.CompressionLevel]::Optimal,$false)
  $releaseId=(Get-FileHash -LiteralPath $payloadPath -Algorithm SHA256).Hash.Substring(0,20).ToLowerInvariant()
  [IO.File]::WriteAllText($payloadIdPath,$releaseId,[Text.UTF8Encoding]::new($false))

  Write-Output 'Compiling the Windows executable...'
  $compilerArguments=@(
    '/nologo','/target:winexe','/platform:x64','/optimize+','/codepage:65001',
    ('/out:'+$executablePath),('/win32icon:'+$iconPath),
    ('/win32manifest:'+(Join-Path $workspaceRoot 'build\Launcher.manifest')),
    '/reference:System.Windows.Forms.dll','/reference:System.Drawing.dll',
    '/reference:System.IO.Compression.dll','/reference:System.IO.Compression.FileSystem.dll',
    ('/resource:'+$payloadPath+',Translator.Payload'),
    ('/resource:'+$payloadIdPath+',Translator.PayloadId'),
    ('/resource:'+$logoPath+',Translator.Logo'),
    (Join-Path $workspaceRoot 'build\Launcher.cs'),
    (Join-Path $applicationDirectory 'DesktopIntegration.cs')
  )
  & $compiler @compilerArguments
  if ($LASTEXITCODE -ne 0) { throw ('Executable compilation failed: '+$LASTEXITCODE) }
  Get-Item -LiteralPath $executablePath | Select-Object FullName,Length,LastWriteTime
} finally {
  # Delete only the unique staging directory created by this build.
  $resolvedStage=[IO.Path]::GetFullPath($stageDirectory)
  $buildRoot=[IO.Path]::GetFullPath($buildDirectory).TrimEnd('\')+'\'
  if (-not $resolvedStage.StartsWith($buildRoot,[StringComparison]::OrdinalIgnoreCase) -or
      [IO.Path]::GetFileName($resolvedStage) -notmatch '^stage-[a-f0-9]{32}$') {
    throw 'Refusing to remove a staging directory outside the package workspace.'
  }
  if (Test-Path -LiteralPath $resolvedStage) { Remove-Item -LiteralPath $resolvedStage -Recurse -Force }
}
