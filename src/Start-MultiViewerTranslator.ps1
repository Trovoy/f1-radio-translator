param([switch]$TranslateOnce)

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Security
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName WindowsBase
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
[Windows.Forms.Application]::EnableVisualStyles()

$script:CaptureBounds = [Drawing.Rectangle]::Empty
$script:OcrProcess = $null
$script:LastFrameHash = ''
$script:LastOcrLines = @()
$script:TranslationProvider = 'MyMemory'
$script:DeepLKey = ''
$script:OpenAIKey = ''
$script:OpenAIBaseUrl = 'https://api.openai.com/v1'
$script:AppScriptPath = $PSCommandPath
$script:AppDirectory = Split-Path -Parent $PSCommandPath
$script:Scanning = $false
$script:FocusMode = $false
$script:TranslationProcess = $null
$script:TranslationPendingText = ''
$script:TranslationPendingSpeaker = ''
$script:VisibleError = $false
$script:SessionDeepLKey = ''
$script:SessionOpenAIKey = ''
$script:RememberDeepLKey = $false
$script:RememberOpenAIKey = $false
$script:SavedDeepLKey = ''
$script:SavedOpenAIKey = ''
$script:SavedProviderIndex = 0
$script:LastProviderIndex = 0
$script:LoadingSettings = $true
$script:SettingsWarning = ''
$script:SettingsDirectory = Join-Path $env:LOCALAPPDATA 'MultiViewerRadioTranslator'
$script:SettingsPath = Join-Path $script:SettingsDirectory 'settings.json'
$script:Seen = [Collections.Generic.Dictionary[string, datetime]]::new([StringComparer]::OrdinalIgnoreCase)
$script:Queue = [Collections.Generic.Queue[object]]::new()
$script:OutputBlockGroups = [Collections.Generic.Queue[int]]::new()
$script:NextRequestAt = [DateTime]::MinValue
$script:BackoffSeconds = 0
$script:Busy = $false
$script:TempImage = Join-Path $env:TEMP 'multiviewer-radio-ocr.png'

function Status([string]$Text) {
  if ($script:StatusText) { $script:StatusText.Text = $Text }
}
function Protect-LocalText([string]$Text) {
  if (-not $Text) { return '' }
  $bytes=[Text.Encoding]::UTF8.GetBytes($Text)
  return [Convert]::ToBase64String([Security.Cryptography.ProtectedData]::Protect($bytes,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser))
}
function Unprotect-LocalText([string]$ProtectedText) {
  if (-not $ProtectedText) { return '' }
  $bytes=[Convert]::FromBase64String($ProtectedText)
  $plain=[Security.Cryptography.ProtectedData]::Unprotect($bytes,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)
  return [Text.Encoding]::UTF8.GetString($plain)
}
function Save-Settings {
  try {
    if (-not (Test-Path $script:SettingsDirectory)) { [void](New-Item -ItemType Directory -Path $script:SettingsDirectory -Force) }
    $capture=$null
    if (-not $script:CaptureBounds.IsEmpty) {
      $capture=@{ left=$script:CaptureBounds.Left; top=$script:CaptureBounds.Top; width=$script:CaptureBounds.Width; height=$script:CaptureBounds.Height }
    }
    $providerIndex=$script:SavedProviderIndex
    if ($script:ProviderBox -and $script:ProviderBox.SelectedIndex -ge 0) { $providerIndex=$script:ProviderBox.SelectedIndex }
    $settings=@{ version=1; captureBounds=$capture; providerIndex=$providerIndex; deepLKeyProtected=$script:SavedDeepLKey; openAIKeyProtected=$script:SavedOpenAIKey; openAIBaseUrl=$script:OpenAIBaseUrl }
    $json=$settings | ConvertTo-Json -Depth 5
    $temporaryPath=$script:SettingsPath+'.tmp'
    [IO.File]::WriteAllText($temporaryPath,$json,[Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporaryPath -Destination $script:SettingsPath -Force
  } catch { Status ('设置保存失败：'+$_.Exception.Message) }
}
function Save-CurrentRememberedKey {
  if ($script:LoadingSettings -or -not $script:ApiKeyBox) { return }
  if ($script:ProviderBox.SelectedIndex -eq 1) {
    $script:SessionDeepLKey=$script:ApiKeyBox.Password
    if ($script:RememberKeyCheck.IsChecked -eq $true) { $script:SavedDeepLKey=Protect-LocalText $script:SessionDeepLKey; $script:RememberDeepLKey=$true }
  } elseif ($script:ProviderBox.SelectedIndex -eq 2) {
    $script:SessionOpenAIKey=$script:ApiKeyBox.Password
    if ($script:RememberKeyCheck.IsChecked -eq $true) { $script:SavedOpenAIKey=Protect-LocalText $script:SessionOpenAIKey; $script:RememberOpenAIKey=$true }
  }
  Save-Settings
}
function Load-Settings {
  if (-not (Test-Path $script:SettingsPath)) { return }
  try {
    $settings=[IO.File]::ReadAllText($script:SettingsPath,[Text.Encoding]::UTF8) | ConvertFrom-Json
    if ($settings.captureBounds) {
      $candidate=[Drawing.Rectangle]::new([int]$settings.captureBounds.left,[int]$settings.captureBounds.top,[int]$settings.captureBounds.width,[int]$settings.captureBounds.height)
      $screen=[Windows.Forms.SystemInformation]::VirtualScreen
      if ($candidate.Width -ge 50 -and $candidate.Height -ge 20 -and $candidate.Left -ge $screen.Left -and $candidate.Top -ge $screen.Top -and $candidate.Right -le $screen.Right -and $candidate.Bottom -le $screen.Bottom) { $script:CaptureBounds=$candidate }
    }
    if ($null -ne $settings.providerIndex -and [int]$settings.providerIndex -ge 0 -and [int]$settings.providerIndex -le 2) { $script:SavedProviderIndex=[int]$settings.providerIndex }
    if ($settings.deepLKeyProtected) {
      try { $script:SessionDeepLKey=Unprotect-LocalText ([string]$settings.deepLKeyProtected); $script:SavedDeepLKey=[string]$settings.deepLKeyProtected; $script:RememberDeepLKey=($script:SessionDeepLKey.Length -gt 0) }
      catch { $script:SettingsWarning='无法读取已保存的 DeepL Key；请重新输入并保存。' }
    }
    if ($settings.openAIKeyProtected) {
      try { $script:SessionOpenAIKey=Unprotect-LocalText ([string]$settings.openAIKeyProtected); $script:SavedOpenAIKey=[string]$settings.openAIKeyProtected; $script:RememberOpenAIKey=($script:SessionOpenAIKey.Length -gt 0) }
      catch { $script:SettingsWarning='无法读取已保存的 OpenAI Key；请重新输入并保存。' }
    }
    if ($settings.openAIBaseUrl -and [string]$settings.openAIBaseUrl -match '^https?://') { $script:OpenAIBaseUrl=([string]$settings.openAIBaseUrl).Trim().TrimEnd('/') }
  } catch { $script:SettingsWarning='设置文件无法读取；重新框选区域或输入密钥后会重新保存。' }
}
if (-not $TranslateOnce) { Load-Settings }
function Get-CaptureThumbnail([Drawing.Rectangle]$Bounds,[int]$DecodeWidth=312) {
  $bitmap=[Drawing.Bitmap]::new($Bounds.Width,$Bounds.Height)
  $graphics=$null
  $stream=[IO.MemoryStream]::new()
  try {
    $graphics=[Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($Bounds.Location,[Drawing.Point]::Empty,$Bounds.Size,[Drawing.CopyPixelOperation]::SourceCopy)
    $bitmap.Save($stream,[Drawing.Imaging.ImageFormat]::Png)
    $stream.Position=0
    $thumbnail=[Windows.Media.Imaging.BitmapImage]::new()
    $thumbnail.BeginInit()
    $thumbnail.CacheOption=[Windows.Media.Imaging.BitmapCacheOption]::OnLoad
    if ($DecodeWidth -gt 0 -and $Bounds.Width -gt $DecodeWidth) { $thumbnail.DecodePixelWidth=$DecodeWidth }
    $thumbnail.StreamSource=$stream
    $thumbnail.EndInit()
    $thumbnail.Freeze()
    return $thumbnail
  } finally {
    if ($graphics) { $graphics.Dispose() }
    $bitmap.Dispose()
    $stream.Dispose()
  }
}
function Update-CapturePreview {
  if (-not $script:CapturePreviewImage) { return }
  $bounds=$script:CaptureBounds
  $script:CapturePreviewImage.Source=$null
  $script:CapturePreviewPlaceholder.Visibility=[Windows.Visibility]::Visible
  $script:RefreshPreviewButton.IsEnabled=(-not $bounds.IsEmpty)
  $script:ClearAreaButton.IsEnabled=(-not $bounds.IsEmpty)
  $script:EnlargePreviewButton.IsEnabled=(-not $bounds.IsEmpty)
  if ($bounds.IsEmpty) {
    $script:CapturePreviewPlaceholder.Text='尚未框选'
    $script:CapturePreviewDetails.Text='尚未选择字幕区域'
    $script:CapturePreviewHint.Text='框选时请包含卡片正文及下方姓名行。'
    return
  }
  $script:CapturePreviewDetails.Text=('屏幕位置 ({0}, {1})  ·  {2} × {3} px' -f $bounds.Left,$bounds.Top,$bounds.Width,$bounds.Height)
  try {
    $script:CapturePreviewImage.Source=Get-CaptureThumbnail $bounds
    $script:CapturePreviewPlaceholder.Visibility=[Windows.Visibility]::Collapsed
    $script:CapturePreviewHint.Text='点击缩略图放大 · 每 10 秒自动刷新。'
  } catch {
    $script:CapturePreviewPlaceholder.Text='预览不可用'
    $script:CapturePreviewHint.Text='暂时无法截取区域，点击“刷新预览”重试。'
  }
}
function Set-CapturePreviewZoom([double]$Scale,[switch]$Fit) {
  if (-not $script:LargePreviewImage -or -not $script:LargePreviewImage.Source) { return }
  $pixelWidth=$script:LargePreviewImage.Source.PixelWidth
  $pixelHeight=$script:LargePreviewImage.Source.PixelHeight
  if ($Fit) {
    $availableWidth=[Math]::Max(1,$script:LargePreviewScroller.ActualWidth-32)
    $availableHeight=[Math]::Max(1,$script:LargePreviewScroller.ActualHeight-32)
    $Scale=[Math]::Min($availableWidth/$pixelWidth,$availableHeight/$pixelHeight)
    $script:LargePreviewFit=$true
  } else { $script:LargePreviewFit=$false }
  $script:LargePreviewScale=[Math]::Max(0.02,[Math]::Min(8,$Scale))
  $script:LargePreviewImage.Width=$pixelWidth*$script:LargePreviewScale
  $script:LargePreviewImage.Height=$pixelHeight*$script:LargePreviewScale
  $script:LargePreviewZoomText.Text=('{0:0}%' -f ($script:LargePreviewScale*100))
}
function Show-CapturePreview {
  if ($script:CaptureBounds.IsEmpty) { Status '请先框选字幕区域。'; return }
  $resumeScan=$script:Scanning
  $script:ScanTimer.Stop()
  $script:PreviewTimer.Stop()
  $script:AutoPreviewTimer.Stop()
  try {
    # Capture the full-resolution image before opening a window over the desktop.
    $previewSource=Get-CaptureThumbnail $script:CaptureBounds 0
    $previewXaml=@'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="识别区域预览" Width="900" Height="650" MinWidth="540" MinHeight="360"
        WindowStartupLocation="CenterOwner" Background="#111720" Foreground="#EDF4FC"
        FontFamily="Microsoft YaHei UI" UseLayoutRounding="True">
  <Grid Margin="20">
    <Grid.RowDefinitions>
      <RowDefinition Height="Auto" /><RowDefinition Height="*" /><RowDefinition Height="Auto" />
    </Grid.RowDefinitions>
    <StackPanel Margin="0,0,0,14">
      <TextBlock Text="识别区域" FontSize="21" FontWeight="SemiBold" />
      <TextBlock x:Name="LargePreviewDetails" Foreground="#94A7BE" FontSize="12" Margin="0,5,0,0" />
    </StackPanel>
    <Border Grid.Row="1" Background="#0C1118" BorderBrush="#29384A" BorderThickness="1" CornerRadius="10" ClipToBounds="True">
      <ScrollViewer x:Name="LargePreviewScroller" HorizontalScrollBarVisibility="Auto"
                    VerticalScrollBarVisibility="Auto" Padding="8" CanContentScroll="False">
        <Grid>
          <Image x:Name="LargePreviewImage" Stretch="Fill" HorizontalAlignment="Center"
                 VerticalAlignment="Center" RenderOptions.BitmapScalingMode="HighQuality" />
        </Grid>
      </ScrollViewer>
    </Border>
    <Grid Grid.Row="2" Margin="0,14,0,0">
      <Grid.ColumnDefinitions><ColumnDefinition Width="*" /><ColumnDefinition Width="Auto" /></Grid.ColumnDefinitions>
      <StackPanel>
        <StackPanel Orientation="Horizontal">
          <Button x:Name="ZoomOutButton" Content="−" Width="34" Padding="0,6" ToolTip="缩小" />
          <TextBlock x:Name="LargePreviewZoomText" Text="100%" Width="60" TextAlignment="Center" VerticalAlignment="Center" FontSize="12" />
          <Button x:Name="ZoomInButton" Content="+" Width="34" Padding="0,6" ToolTip="放大" Margin="0,0,10,0" />
          <Button x:Name="FitPreviewButton" Content="自适应" Padding="10,6" FontSize="12" Margin="0,0,8,0" />
          <Button x:Name="ActualPreviewButton" Content="100%" Padding="10,6" FontSize="12" />
        </StackPanel>
        <TextBlock Text="滚轮上下浏览 · Ctrl + 滚轮缩放 · Esc 关闭" Foreground="#75859A" FontSize="11" Margin="0,8,0,0" />
      </StackPanel>
      <Button x:Name="ClosePreviewButton" Grid.Column="1" Content="关闭预览" Padding="12,7"
              FontSize="12" VerticalAlignment="Top" Margin="12,0,0,0" />
    </Grid>
  </Grid>
</Window>
'@
    $script:CapturePreviewWindow=[Windows.Markup.XamlReader]::Load([Xml.XmlNodeReader]::new([xml]$previewXaml))
    $script:CapturePreviewWindow.Owner=$script:Window
    $script:CapturePreviewWindow.Resources=$script:Window.Resources
    $workArea=[Windows.SystemParameters]::WorkArea
    $script:CapturePreviewWindow.Width=[Math]::Min(900,$workArea.Width)
    $script:CapturePreviewWindow.Height=[Math]::Min(650,$workArea.Height)
    $script:LargePreviewImage=$script:CapturePreviewWindow.FindName('LargePreviewImage')
    $script:LargePreviewScroller=$script:CapturePreviewWindow.FindName('LargePreviewScroller')
    $script:LargePreviewZoomText=$script:CapturePreviewWindow.FindName('LargePreviewZoomText')
    $script:LargePreviewImage.Source=$previewSource
    $script:LargePreviewFit=$true
    $script:LargePreviewScale=1.0
    $script:CapturePreviewWindow.FindName('LargePreviewDetails').Text=('屏幕位置 ({0}, {1}) · {2} × {3} px · 本次截图' -f $script:CaptureBounds.Left,$script:CaptureBounds.Top,$script:CaptureBounds.Width,$script:CaptureBounds.Height)
    $script:CapturePreviewWindow.FindName('ZoomOutButton').Add_Click({ Set-CapturePreviewZoom ($script:LargePreviewScale/1.25) })
    $script:CapturePreviewWindow.FindName('ZoomInButton').Add_Click({ Set-CapturePreviewZoom ($script:LargePreviewScale*1.25) })
    $script:CapturePreviewWindow.FindName('FitPreviewButton').Add_Click({ Set-CapturePreviewZoom 1 -Fit })
    $script:CapturePreviewWindow.FindName('ActualPreviewButton').Add_Click({ Set-CapturePreviewZoom 1 })
    $script:CapturePreviewWindow.FindName('ClosePreviewButton').Add_Click({ $script:CapturePreviewWindow.Close() })
    $script:CapturePreviewWindow.Add_ContentRendered({ Set-CapturePreviewZoom 1 -Fit })
    $script:LargePreviewScroller.Add_SizeChanged({ if ($script:LargePreviewFit) { Set-CapturePreviewZoom 1 -Fit } })
    $script:LargePreviewScroller.Add_PreviewMouseWheel({
      param($sender,$e)
      if (([Windows.Input.Keyboard]::Modifiers -band [Windows.Input.ModifierKeys]::Control) -ne 0) {
        $factor=if ($e.Delta -gt 0) { 1.25 } else { 0.8 }
        Set-CapturePreviewZoom ($script:LargePreviewScale*$factor)
        $e.Handled=$true
      }
    })
    $script:CapturePreviewWindow.Add_KeyDown({ param($sender,$e) if ($e.Key -eq [Windows.Input.Key]::Escape) { $sender.Close(); $e.Handled=$true } })
    [void]$script:CapturePreviewWindow.ShowDialog()
  } catch { Status ('无法放大预览：'+$_.Exception.Message) }
  finally {
    if ($script:CapturePreviewWindow -and $script:CapturePreviewWindow.IsLoaded) { $script:CapturePreviewWindow.Close() }
    if ($script:LargePreviewImage) { $script:LargePreviewImage.Source=$null }
    $script:LargePreviewImage=$null
    $script:LargePreviewScroller=$null
    $script:LargePreviewZoomText=$null
    $script:CapturePreviewWindow=$null
    if (-not $script:ShuttingDown) {
      $script:PreviewTimer.Start()
      $script:AutoPreviewTimer.Start()
      if ($resumeScan -and $script:Scanning) { $script:ScanTimer.Start() }
    }
  }
}
function Select-Area {
  $script:PreviewTimer.Stop()
  $script:AutoPreviewTimer.Stop()
  $resumeScan=$script:Scanning
  $script:ScanTimer.Stop()
  $overlay=$null
  $accepted=$false
  $selectionError=''
  try {
    if (-not ('MultiViewerRadio.RegionSelectionForm' -as [type])) {
      $selectorAssembly=Join-Path $script:AppDirectory 'RegionSelector.dll'
      if (Test-Path -LiteralPath $selectorAssembly) { Add-Type -Path $selectorAssembly }
      else {
        $selectorSource=[IO.File]::ReadAllText((Join-Path $script:AppDirectory 'RegionSelector.cs'),[Text.Encoding]::UTF8)
        Add-Type -TypeDefinition $selectorSource -ReferencedAssemblies 'System.Windows.Forms','System.Drawing' -ErrorAction Stop
      }
    }
    $overlay=[MultiViewerRadio.RegionSelectionForm]::CreateForDesktop($script:CaptureBounds)
    $script:RegionSelectorWindow=$overlay
    if ($overlay.ShowDialog() -eq [Windows.Forms.DialogResult]::OK) {
      $script:CaptureBounds=$overlay.SelectedScreenBounds
      $script:LastFrameHash=''
      $script:LastOcrLines=@()
      Save-Settings
      $accepted=$true
    }
  } catch { $selectionError=$_.Exception.Message }
  finally {
    if ($overlay) { $overlay.Dispose() }
    $script:RegionSelectorWindow=$null
    if (-not $script:ShuttingDown) {
      $script:PreviewTimer.Start()
      $script:AutoPreviewTimer.Start()
      if ($resumeScan -and $script:Scanning) { $script:ScanTimer.Start() }
    }
  }
  if ($selectionError) { Status ('无法框选区域：'+$selectionError) }
  elseif ($accepted) { Status ('已选区域 {0} × {1}（自动保存）。' -f $script:CaptureBounds.Width,$script:CaptureBounds.Height) }
  elseif ($script:CaptureBounds.IsEmpty) { Status '已取消框选；请先选择字幕区域。' }
  else { Status '已取消修改，保留原来的识别区域。' }
}
function Clear-SelectedArea {
  if ($script:Scanning) { Stop-Translation }
  $script:PreviewTimer.Stop()
  $script:AutoPreviewTimer.Stop()
  $script:CaptureBounds=[Drawing.Rectangle]::Empty
  $script:LastFrameHash=''
  $script:LastOcrLines=@()
  $script:Seen.Clear()
  $script:Queue.Clear()
  $script:TranslationPendingText=''
  $script:TranslationPendingSpeaker=''
  Save-Settings
  Update-CapturePreview
  Status '选区已清除；重新框选后可以开始翻译。'
}
function Write-OcrCommand([string]$Command) {
  if (-not $script:OcrProcess -or $script:OcrProcess.HasExited) { throw '本地 OCR 进程已退出。' }
  # .NET Framework does not expose ProcessStartInfo.StandardInputEncoding.
  # Write UTF-8 bytes directly to the redirected pipe, without a BOM.
  $commandBytes=[Text.UTF8Encoding]::new($false).GetBytes($Command+"`n")
  $inputStream=$script:OcrProcess.StandardInput.BaseStream
  $inputStream.Write($commandBytes,0,$commandBytes.Length)
  $inputStream.Flush()
}
function Start-OcrWorker {
  if ($script:OcrProcess -and -not $script:OcrProcess.HasExited) { return }
  $appDir = $script:AppDirectory
  $pythonDeps = Join-Path $appDir 'python-deps'
  $worker = Join-Path $appDir 'rapidocr_worker.py'
  $pythonCandidates = @(
    (Join-Path $appDir 'python-runtime\python.exe'),
    (Join-Path $env:LOCALAPPDATA 'Programs\Python\Python312\python.exe'),
    (Join-Path $env:ProgramFiles 'Python312\python.exe')
  )
  $python = $pythonCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
  if (-not $python) {
    $cmd = Get-Command python.exe -ErrorAction SilentlyContinue
    if ($cmd) { $python = $cmd.Source }
  }
  if (-not $python) { throw '找不到 Python 3.12。此版本 OCR 需要 Python 3.12；请安装 Python 3.12 后重试。' }
  $versionInfo=[Diagnostics.ProcessStartInfo]::new()
  $versionInfo.FileName=$python
  $versionInfo.Arguments='-c "import sys; print(sys.version_info[0], sys.version_info[1])"'
  $versionInfo.UseShellExecute=$false
  $versionInfo.CreateNoWindow=$true
  $versionInfo.RedirectStandardOutput=$true
  $versionInfo.RedirectStandardError=$true
  $versionInfo.StandardOutputEncoding=[Text.Encoding]::UTF8
  $versionProcess=[Diagnostics.Process]::new()
  $versionProcess.StartInfo=$versionInfo
  try {
    if (-not $versionProcess.Start()) { throw '无法启动 Python 运行环境。' }
    if (-not $versionProcess.WaitForExit(5000)) { $versionProcess.Kill(); throw 'Python 运行环境检查超时。' }
    $version=$versionProcess.StandardOutput.ReadToEnd()
  } finally { $versionProcess.Dispose() }
  if ($version.Trim() -ne '3 12') { throw ('检测到 Python {0}；RapidOCR 依赖需要 Python 3.12。' -f $version) }
  if (-not (Test-Path (Join-Path $pythonDeps 'rapidocr'))) { throw '缺少本地 OCR 依赖目录；请解压完整 ZIP 后再运行。' }

  $psi = [Diagnostics.ProcessStartInfo]::new()
  $psi.FileName = $python
  $psi.Arguments = ('-u "{0}"' -f $worker.Replace('"','\"'))
  $psi.WorkingDirectory = $appDir
  $psi.UseShellExecute = $false
  $psi.CreateNoWindow = $true
  $psi.RedirectStandardInput = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $false
  $psi.StandardOutputEncoding=[Text.Encoding]::UTF8
  $psi.EnvironmentVariables['PYTHONUTF8']='1'
  $psi.EnvironmentVariables['PYTHONIOENCODING']='utf-8'
  $oldPythonPath = $psi.EnvironmentVariables['PYTHONPATH']
  if ($python -eq (Join-Path $appDir 'python-runtime\python.exe')) {
    $psi.EnvironmentVariables['PYTHONHOME']=Join-Path $appDir 'python-runtime'
    $psi.EnvironmentVariables['PYTHONPATH']=$pythonDeps
    $psi.EnvironmentVariables['PYTHONNOUSERSITE']='1'
  } else {
    $psi.EnvironmentVariables['PYTHONPATH'] = if ($oldPythonPath) { $pythonDeps + [IO.Path]::PathSeparator + $oldPythonPath } else { $pythonDeps }
  }
  $script:OcrProcess = [Diagnostics.Process]::new()
  $script:OcrProcess.StartInfo = $psi
  if (-not $script:OcrProcess.Start()) { throw '无法启动本地 RapidOCR 进程。' }
  $readyLine = $script:OcrProcess.StandardOutput.ReadLine()
  if (-not $readyLine) { throw 'RapidOCR 启动失败。请查看 PowerShell 窗口中的错误信息。' }
  $ready = $readyLine | ConvertFrom-Json
  if (-not $ready.ready) { throw ('RapidOCR 初始化失败：' + $ready.error) }
}
function Read-Ocr([Drawing.Rectangle]$Bounds) {
  $bmp=[Drawing.Bitmap]::new($Bounds.Width,$Bounds.Height); $g=[Drawing.Graphics]::FromImage($bmp)
  try {
    $g.CopyFromScreen($Bounds.Location,[Drawing.Point]::Empty,$Bounds.Size,[Drawing.CopyPixelOperation]::SourceCopy)
    $pixels=$bmp.LockBits([Drawing.Rectangle]::new(0,0,$bmp.Width,$bmp.Height),[Drawing.Imaging.ImageLockMode]::ReadOnly,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try {
      $byteCount=[Math]::Abs($pixels.Stride)*$bmp.Height
      $pixelBytes=[byte[]]::new($byteCount)
      [Runtime.InteropServices.Marshal]::Copy($pixels.Scan0,$pixelBytes,0,$byteCount)
    } finally { $bmp.UnlockBits($pixels) }
    $sha=[Security.Cryptography.SHA256]::Create()
    try { $frameHash=[Convert]::ToBase64String($sha.ComputeHash($pixelBytes)) } finally { $sha.Dispose() }
    if ($frameHash -eq $script:LastFrameHash) { return @($script:LastOcrLines) }
    $script:LastFrameHash=$frameHash
    $bmp.Save($script:TempImage,[Drawing.Imaging.ImageFormat]::Png)
  } finally { $g.Dispose(); $bmp.Dispose() }
  if (-not $script:OcrProcess -or $script:OcrProcess.HasExited) { throw '本地 OCR 进程已退出，请暂停后重新开始。' }
  Write-OcrCommand $script:TempImage
  $responseLine = $script:OcrProcess.StandardOutput.ReadLine()
  if (-not $responseLine) { throw '本地 OCR 没有返回结果。请暂停后重新开始。' }
  $response = $responseLine | ConvertFrom-Json
  if ($response.error) { throw $response.error }
  $script:LastOcrLines=@($response.lines)
  return @($script:LastOcrLines)
}
function Is-Transcript([string]$Text) {
  if (-not $Text -or $Text.Length -lt 3 -or $Text.Length -gt 300 -or $Text -notmatch '\p{L}') { return $false }
  $digits=([regex]::Matches($Text,'\d')).Count
  if ($digits/[Math]::Max(1,$Text.Length) -gt .22) { return $false }
  $words=([regex]::Matches($Text,'\p{L}+')).Count
  return ($words -ge 2 -or $Text -match '[.!?。？！]')
}
function Invoke-JsonUtf8([string]$Uri,[hashtable]$Headers,[string]$Body,[int]$TimeoutSec) {
  $request=[Net.HttpWebRequest]::Create($Uri)
  $request.Method='POST'
  $request.ContentType='application/json; charset=utf-8'
  $request.Accept='application/json'
  $request.Timeout=$TimeoutSec*1000
  $request.ReadWriteTimeout=$TimeoutSec*1000
  foreach ($name in $Headers.Keys) { $request.Headers[$name]=[string]$Headers[$name] }
  $bytes=[Text.Encoding]::UTF8.GetBytes($Body)
  $request.ContentLength=$bytes.Length
  $requestStream=$request.GetRequestStream()
  try { $requestStream.Write($bytes,0,$bytes.Length) } finally { $requestStream.Dispose() }
  try {
    $response=$request.GetResponse()
  } catch [Net.WebException] {
    $httpResponse=$_.Exception.Response
    if (-not $httpResponse) { throw }
    try {
      $errorReader=[IO.StreamReader]::new($httpResponse.GetResponseStream(),[Text.Encoding]::UTF8)
      try { $errorText=$errorReader.ReadToEnd() } finally { $errorReader.Dispose() }
      $detail=$errorText
      try { $errorData=$errorText | ConvertFrom-Json; if ($errorData.message) { $detail=[string]$errorData.message } elseif ($errorData.error.message) { $detail=[string]$errorData.error.message } } catch { }
      throw ('HTTP {0}: {1}' -f [int]$httpResponse.StatusCode,$detail)
    } finally { $httpResponse.Dispose() }
  }
  try {
    $reader=[IO.StreamReader]::new($response.GetResponseStream(),[Text.Encoding]::UTF8)
    try { $json=$reader.ReadToEnd() } finally { $reader.Dispose() }
  } finally { $response.Dispose() }
  return ($json | ConvertFrom-Json)
}
function Translate([string]$Text) {
  if ($script:TranslationProvider -eq 'OpenAI') {
    $headers = @{ Authorization = ('Bearer ' + $script:OpenAIKey.Trim()) }
    $baseUrl = $script:OpenAIBaseUrl.Trim().TrimEnd('/')
    if (-not $baseUrl) { $baseUrl = 'https://api.openai.com/v1' }
    if ($baseUrl -match '/chat/completions$') { $requestUri = $baseUrl }
    elseif ($baseUrl -match '/v\d+(?:beta\d+)?$') { $requestUri = $baseUrl + '/chat/completions' }
    else { $requestUri = $baseUrl + '/v1/chat/completions' }
    $instructions = 'Translate live Formula 1 team-radio transcript text into concise, natural Simplified Chinese. This is motorsport radio during a race. Preserve driver names, team names, car numbers, and turn/lap numbers. Interpret track limits as 超出赛道界限, stint as 赛段, box as 进站, lift and coast as 松油滑行, and strikes in a track-limits context as recorded warnings or infringements rather than physical hits. The input may contain speech-recognition errors or an unfinished sentence: use the F1 context, do not invent missing facts, and return only the Chinese translation with no explanation.'
    $body = @{
      model = 'gpt-6-luna'
      reasoning_effort = 'none'
      messages = @(
        @{ role = 'system'; content = $instructions }
        @{ role = 'user'; content = $Text }
      )
      max_completion_tokens = 180
    } | ConvertTo-Json -Depth 5 -Compress
    $data = Invoke-JsonUtf8 -Uri $requestUri -Headers $headers -Body $body -TimeoutSec 20
    $translated = [string]$data.choices[0].message.content
    $translated = $translated.Trim()
    if (-not $translated) { throw 'OpenAI did not return a translation.' }
    return $translated
  }
  if ($script:TranslationProvider -eq 'DeepL') {
    $baseUrl = if ($script:DeepLKey.Trim().EndsWith(':fx',[StringComparison]::OrdinalIgnoreCase)) { 'https://api-free.deepl.com' } else { 'https://api.deepl.com' }
    $headers = @{ Authorization = ('DeepL-Auth-Key ' + $script:DeepLKey.Trim()) }
    $body = @{
      text = @($Text)
      source_lang = 'EN'
      target_lang = 'ZH'
      model_type = 'quality_optimized'
      context = 'Live Formula 1 driver-engineer radio. Short spoken instructions and fragments from a race; likely subjects include energy deployment, overtaking, flags, track limits, pit entry, tyres, stints, lap and turn numbers. For 2026 F1, Boost is the driver-operated ERS energy-deployment / maximum-power button; Overtake Mode is a separate passing aid. They are not interchangeable.'
      split_sentences = '0'
      custom_instructions = @(
        'Translate short live Formula 1 driver-engineer radio into concise, natural Simplified Chinese. Keep the direct spoken tone; return only the translation. Preserve spoken names, numbers, decimals and units exactly; do not add a speaker name or infer a software version from a bare decimal.',
        'In F1, Boost or Boost Button means the driver''s ERS energy-deployment / maximum-power button. Translate use boost as 使用 Boost（能量部署） or a natural equivalent; never translate it as turbocharging or 涡轮增压. Keep Boost distinct from Overtake Mode.',
        'Overtake means 超车; Overtake Mode means 超车模式. Pit entry means 维修区入口 or 进站入口, not 进站道上. Box as a radio instruction means 进站; double yellows means 双黄旗; track limits means 赛道界限.',
        'The transcript may contain speech-recognition errors and [unclear]. Translate [unclear] as [听不清]. Correct an obvious recognition error only when context makes it clear; otherwise preserve uncertainty. Do not invent missing words, race events, names, or explanations.',
        'Use consistent F1 terms: stint means 赛段 or 轮胎使用阶段 depending on the sentence; lift and coast means 松油滑行; recharge means 回收能量 or 充电 depending on context; strike in a track-limits context means a warning or infringement, not a physical hit.'
      )
    } | ConvertTo-Json -Depth 5 -Compress
    $data = Invoke-JsonUtf8 -Uri ($baseUrl + '/v2/translate') -Headers $headers -Body $body -TimeoutSec 15
    $translated = [string]$data.translations[0].text
    if (-not $translated.Trim()) { throw 'DeepL did not return a translation.' }
    return $translated.Trim()
  }
  $q=[Uri]::EscapeDataString($Text)
  $uri='https://api.mymemory.translated.net/get?q='+$q+'&langpair=en%7Czh-CN'
  $data=Invoke-RestMethod -Uri $uri -Method Get -TimeoutSec 10
  if ($data.responseStatus -and [int]$data.responseStatus -ne 200) {
    throw ('MyMemory returned status {0}: {1}' -f $data.responseStatus,$data.responseDetails)
  }
  $translated=[Net.WebUtility]::HtmlDecode([string]$data.responseData.translatedText).Trim()
  if (-not $translated) { throw 'MyMemory did not return a translation.' }
  return $translated
}
if ($TranslateOnce) {
  try {
    [Console]::InputEncoding=[Text.Encoding]::UTF8
    [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
    $encoded=[Console]::In.ReadLine()
    if (-not $encoded) { throw '没有收到翻译请求。' }
    $job=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($encoded)) | ConvertFrom-Json
    $script:TranslationProvider=[string]$job.Provider
    $script:DeepLKey=[string]$job.DeepLKey
    $script:OpenAIKey=[string]$job.OpenAIKey
    $script:OpenAIBaseUrl=[string]$job.OpenAIBaseUrl
    $result=Translate ([string]$job.Text)
    [Console]::Out.WriteLine((@{ ok=$true; translation=$result } | ConvertTo-Json -Compress))
    exit 0
  } catch {
    [Console]::Out.WriteLine((@{ ok=$false; error=$_.Exception.Message } | ConvertTo-Json -Compress))
    exit 1
  }
}
function Append-Result([string]$Original,[string]$Chinese,[string]$Speaker) {
  $groupSize=2
  if ($Speaker) {
    $speakerParagraph=[Windows.Documents.Paragraph]::new()
    $speakerParagraph.SetValue([Windows.Documents.TextElement]::FontSizeProperty,[double]12)
    $speakerParagraph.SetValue([Windows.Documents.TextElement]::ForegroundProperty,[Windows.Media.BrushConverter]::new().ConvertFromString('#56C7F2'))
    $speakerParagraph.SetValue([Windows.Documents.Block]::MarginProperty,[Windows.Thickness]::new(0,0,0,3))
    [void]$speakerParagraph.Inlines.Add([Windows.Documents.Run]::new($Speaker))
    [void]$script:OutputDocument.Blocks.Add($speakerParagraph)
    $groupSize=3
  }
  $englishParagraph=[Windows.Documents.Paragraph]::new()
  $englishParagraph.SetValue([Windows.Documents.TextElement]::FontSizeProperty,[double]13)
  $englishParagraph.SetValue([Windows.Documents.TextElement]::ForegroundProperty,[Windows.Media.BrushConverter]::new().ConvertFromString('#8998AD'))
  $englishParagraph.SetValue([Windows.Documents.Block]::MarginProperty,[Windows.Thickness]::new(0,0,0,3))
  [void]$englishParagraph.Inlines.Add([Windows.Documents.Run]::new($Original))
  $translationParagraph=[Windows.Documents.Paragraph]::new()
  $translationParagraph.SetValue([Windows.Documents.TextElement]::FontSizeProperty,[double]21)
  $translationParagraph.SetValue([Windows.Documents.TextElement]::ForegroundProperty,[Windows.Media.Brushes]::White)
  $translationParagraph.SetValue([Windows.Documents.Block]::MarginProperty,[Windows.Thickness]::new(0,0,0,23))
  [void]$translationParagraph.Inlines.Add([Windows.Documents.Run]::new($Chinese))
  [void]$script:OutputDocument.Blocks.Add($englishParagraph)
  [void]$script:OutputDocument.Blocks.Add($translationParagraph)
  $script:OutputBlockGroups.Enqueue($groupSize)
  while ($script:OutputBlockGroups.Count -gt 120) {
    $removeCount=$script:OutputBlockGroups.Dequeue()
    for ($i=0; $i -lt $removeCount; $i++) { $script:OutputDocument.Blocks.Remove($script:OutputDocument.Blocks.FirstBlock) }
  }
  $script:OutputViewer.UpdateLayout()
  $pendingVisuals=[Collections.Generic.Stack[Windows.DependencyObject]]::new()
  $pendingVisuals.Push($script:OutputViewer)
  while ($pendingVisuals.Count -gt 0) {
    $visual=$pendingVisuals.Pop()
    if ($visual -is [Windows.Controls.ScrollViewer]) { $visual.ScrollToEnd(); break }
    for ($index=0; $index -lt [Windows.Media.VisualTreeHelper]::GetChildrenCount($visual); $index++) {
      $child=[Windows.Media.VisualTreeHelper]::GetChild($visual,$index)
      if ($child -is [Windows.DependencyObject]) { $pendingVisuals.Push($child) }
    }
  }
}
function Worker-Status([object]$Worker,[string]$Text) {
  if (-not $script:UserPaused -and -not $script:VisibleError) {
    if ($script:FocusMode) { $script:StatusBar.Visibility=[Windows.Visibility]::Collapsed }
    $script:StatusDot.Fill=[Windows.Media.Brushes]::LightGreen
    Status $Text
  }
}
function Worker-Error([object]$Worker,[string]$Text) {
  $script:VisibleError=$true
  if ($script:FocusMode) { $script:StatusBar.Visibility=[Windows.Visibility]::Visible }
  $script:StatusDot.Fill=[Windows.Media.Brushes]::Tomato
  Status $Text
}
function Start-TranslationRequest([string]$Text,[string]$Speaker) {
  $psi=[Diagnostics.ProcessStartInfo]::new()
  $psi.FileName=Join-Path $PSHOME $(if ($PSVersionTable.PSEdition -eq 'Core') { 'pwsh.exe' } else { 'powershell.exe' })
  $psi.Arguments='-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}" -TranslateOnce' -f $script:AppScriptPath.Replace('"','\"')
  $psi.WorkingDirectory=$script:AppDirectory
  $psi.UseShellExecute=$false
  $psi.CreateNoWindow=$true
  $psi.RedirectStandardInput=$true
  $psi.RedirectStandardOutput=$true
  $psi.RedirectStandardError=$false
  $psi.StandardOutputEncoding=[Text.Encoding]::UTF8
  $process=[Diagnostics.Process]::new()
  $process.StartInfo=$psi
  if (-not $process.Start()) { throw '无法启动隔离的 GPT 翻译进程。' }
  $job=@{
    Provider=$script:TranslationProvider
    DeepLKey=$script:DeepLKey
    OpenAIKey=$script:OpenAIKey
    OpenAIBaseUrl=$script:OpenAIBaseUrl
    Text=$Text
  } | ConvertTo-Json -Compress -Depth 4
  $encoded=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($job))
  try { $process.StandardInput.WriteLine($encoded); $process.StandardInput.Close() }
  catch { try { $process.Kill() } catch { }; $process.Dispose(); throw }
  $script:TranslationProcess=$process
  $script:TranslationPendingText=$Text
  $script:TranslationPendingSpeaker=$Speaker
}
function Complete-TranslationRequest {
  $process=$script:TranslationProcess
  if (-not $process -or -not $process.HasExited) { return }
  $output=$process.StandardOutput.ReadToEnd().Trim()
  $exitCode=$process.ExitCode
  $process.Dispose()
  $script:TranslationProcess=$null
  try { $response=$output | ConvertFrom-Json -ErrorAction Stop }
  catch {
    $errorText=$output
    if (-not $errorText) { $errorText='翻译进程退出，未返回错误详情（exit code {0}）。' -f $exitCode }
    $response=[pscustomobject]@{ ok=$false; error=$errorText }
  }
  $text=$script:TranslationPendingText
  $speaker=$script:TranslationPendingSpeaker
  $script:TranslationPendingText=''
  $script:TranslationPendingSpeaker=''
  if ($response.ok -and $response.translation) {
    if ($script:Queue.Count -gt 0 -and $script:Queue.Peek().Text -eq $text -and $script:Queue.Peek().Speaker -eq $speaker) { [void]$script:Queue.Dequeue() }
    $seenKey=$speaker + "`n" + $text
    $script:Seen[$seenKey]=[DateTime]::UtcNow
    $script:BackoffSeconds=0
    $script:VisibleError=$false
    $script:NextRequestAt=[DateTime]::UtcNow.AddSeconds(3)
    Append-Result $text ([string]$response.translation) $speaker
    $script:StatusDot.Fill=[Windows.Media.Brushes]::LightGreen
    if ($script:FocusMode) { $script:StatusBar.Visibility=[Windows.Visibility]::Collapsed }
    Status ('正在监测；已翻译 {0} 条，排队 {1} 条。' -f $script:Seen.Count,$script:Queue.Count)
  } else {
    $message=[string]$response.error
    if (-not $message) { $message='翻译进程未返回译文。' }
    if ($message -match '(429|Too Many Requests)') {
      if ($script:BackoffSeconds -eq 0) { $script:BackoffSeconds=60 }
      else { $script:BackoffSeconds=[Math]::Min(600,$script:BackoffSeconds*2) }
    } else { $script:BackoffSeconds=[Math]::Max(15,$script:BackoffSeconds) }
    $script:NextRequestAt=[DateTime]::UtcNow.AddSeconds($script:BackoffSeconds)
    Worker-Error $null ('翻译暂不可用；{0} 秒后自动重试，排队 {1} 条。{2}' -f $script:BackoffSeconds,$script:Queue.Count,$message)
  }
}
function Scan-Worker([object]$Worker) {
  if ($script:ClearRequested) { $script:Queue.Clear(); $script:Seen.Clear(); $script:ClearRequested=$false }
  Complete-TranslationRequest
  $bounds=$script:CaptureBounds
  if ($bounds.IsEmpty) { return }
  try { $lines=Read-Ocr $bounds }
  catch { Worker-Error $null ('OCR 识别失败：'+$_.Exception.Message); return }
  foreach ($line in $lines) {
    if ($line -is [string]) { $text=([string]$line -replace '\s+',' ').Trim(); $speaker='' }
    else { $text=([string]$line.text -replace '\s+',' ').Trim(); $speaker=([string]$line.speaker).Trim() }
    if (-not (Is-Transcript $text)) { continue }
    $seenKey=$speaker + "`n" + $text
    $alreadyQueued=$false
    foreach ($queuedItem in $script:Queue) { if ($queuedItem.Text -eq $text -and $queuedItem.Speaker -eq $speaker) { $alreadyQueued=$true; break } }
    if ($script:Seen.ContainsKey($seenKey) -or $alreadyQueued) { continue }
    if ($script:Queue.Count -lt 200) { $script:Queue.Enqueue([pscustomobject]@{ Text=$text; Speaker=$speaker }) }
  }
  if ($script:TranslationProcess) { Worker-Status $null ('正在翻译；排队 {0} 条。' -f $script:Queue.Count); return }
  $now=[DateTime]::UtcNow
  if ($now -lt $script:NextRequestAt) {
    $wait=[Math]::Ceiling(($script:NextRequestAt-$now).TotalSeconds)
    if ($script:BackoffSeconds -gt 0) { Worker-Status $null ('翻译服务限流，暂停 {0} 秒；排队 {1} 条。' -f $wait,$script:Queue.Count) }
    else { Worker-Status $null ('正在监测；下一条将在 {0} 秒后翻译，排队 {1} 条。' -f $wait,$script:Queue.Count) }
    return
  }
  if ($script:Queue.Count -eq 0) { Worker-Status $null ('正在监测字幕区域；识别到 {0} 条。' -f $lines.Count); return }
  $item=$script:Queue.Peek()
  try { Start-TranslationRequest ([string]$item.Text) ([string]$item.Speaker); Worker-Status $null ('正在发送翻译请求；排队 {0} 条。' -f $script:Queue.Count) }
  catch { $script:BackoffSeconds=[Math]::Max(15,$script:BackoffSeconds); $script:NextRequestAt=[DateTime]::UtcNow.AddSeconds($script:BackoffSeconds); Worker-Error $null ('无法启动翻译请求：'+$_.Exception.Message) }
}
function Stop-TranslationProcess {
  $process=$script:TranslationProcess
  $script:TranslationProcess=$null
  $script:TranslationPendingText=''
  $script:TranslationPendingSpeaker=''
  if (-not $process) { return }
  try {
    if (-not $process.HasExited) { $process.Kill(); [void]$process.WaitForExit(1500) }
  } catch { }
  finally { $process.Dispose() }
}
function Stop-OcrWorker([switch]$Force) {
  if ($script:OcrProcess -and -not $script:OcrProcess.HasExited) {
    try {
      if ($Force) { $script:OcrProcess.Kill(); [void]$script:OcrProcess.WaitForExit(1500) }
      else {
        Write-OcrCommand '__quit__'
        $script:OcrProcess.StandardInput.Close()
        if (-not $script:OcrProcess.WaitForExit(1500)) { $script:OcrProcess.Kill(); [void]$script:OcrProcess.WaitForExit(1500) }
      }
    } catch { try { if (-not $script:OcrProcess.HasExited) { $script:OcrProcess.Kill(); [void]$script:OcrProcess.WaitForExit(1500) } } catch { } }
  }
  if ($script:OcrProcess) { $script:OcrProcess.Dispose(); $script:OcrProcess=$null }
}
$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="MultiViewer F1 Radio Translator" Width="940" Height="720"
        MinWidth="760" MinHeight="570" WindowStartupLocation="CenterScreen"
        Background="#111720" Foreground="#EAF1FA" FontFamily="Segoe UI"
        UseLayoutRounding="True" SnapsToDevicePixels="True"
        TextOptions.TextFormattingMode="Display">
  <Window.Resources>
    <SolidColorBrush x:Key="AccentBrush" Color="#56C7F2" />
    <SolidColorBrush x:Key="PanelBrush" Color="#1B2431" />
    <Style x:Key="ScrollPageButton" TargetType="{x:Type RepeatButton}">
      <Setter Property="Focusable" Value="False" />
      <Setter Property="IsTabStop" Value="False" />
      <Setter Property="Template">
        <Setter.Value><ControlTemplate TargetType="{x:Type RepeatButton}"><Border Background="Transparent" /></ControlTemplate></Setter.Value>
      </Setter>
    </Style>
    <Style x:Key="SlimScrollThumb" TargetType="{x:Type Thumb}">
      <Setter Property="MinHeight" Value="28" />
      <Setter Property="Template">
        <Setter.Value>
          <ControlTemplate TargetType="{x:Type Thumb}">
            <Border x:Name="Grip" Background="#52657B" CornerRadius="3" Margin="4,0" />
            <ControlTemplate.Triggers>
              <DataTrigger Binding="{Binding Orientation, RelativeSource={RelativeSource AncestorType={x:Type ScrollBar}}}" Value="Horizontal">
                <Setter TargetName="Grip" Property="Margin" Value="0,4" />
              </DataTrigger>
              <Trigger Property="IsMouseOver" Value="True"><Setter TargetName="Grip" Property="Background" Value="#89A3BD" /></Trigger>
              <Trigger Property="IsDragging" Value="True"><Setter TargetName="Grip" Property="Background" Value="#56C7F2" /></Trigger>
            </ControlTemplate.Triggers>
          </ControlTemplate>
        </Setter.Value>
      </Setter>
      <Style.Triggers>
        <DataTrigger Binding="{Binding Orientation, RelativeSource={RelativeSource AncestorType={x:Type ScrollBar}}}" Value="Horizontal">
          <Setter Property="MinHeight" Value="0" /><Setter Property="MinWidth" Value="28" />
        </DataTrigger>
      </Style.Triggers>
    </Style>
    <Style x:Key="SlimScrollBar" TargetType="{x:Type ScrollBar}">
      <Setter Property="Width" Value="12" />
      <Setter Property="Background" Value="Transparent" />
      <Setter Property="Focusable" Value="False" />
      <Setter Property="IsTabStop" Value="False" />
      <Setter Property="Opacity" Value="0.65" />
      <Setter Property="Template">
        <Setter.Value>
          <ControlTemplate TargetType="{x:Type ScrollBar}">
            <Grid Background="Transparent">
              <Track x:Name="PART_Track" IsDirectionReversed="True"
                     Orientation="{TemplateBinding Orientation}" Minimum="{TemplateBinding Minimum}"
                     Maximum="{TemplateBinding Maximum}" ViewportSize="{TemplateBinding ViewportSize}"
                     Value="{Binding Value, RelativeSource={RelativeSource TemplatedParent}, Mode=TwoWay}">
                <Track.DecreaseRepeatButton>
                  <RepeatButton x:Name="DecreasePage" Command="{x:Static ScrollBar.PageUpCommand}" Style="{StaticResource ScrollPageButton}" />
                </Track.DecreaseRepeatButton>
                <Track.Thumb><Thumb Style="{StaticResource SlimScrollThumb}" /></Track.Thumb>
                <Track.IncreaseRepeatButton>
                  <RepeatButton x:Name="IncreasePage" Command="{x:Static ScrollBar.PageDownCommand}" Style="{StaticResource ScrollPageButton}" />
                </Track.IncreaseRepeatButton>
              </Track>
            </Grid>
            <ControlTemplate.Triggers>
              <Trigger Property="Orientation" Value="Horizontal">
                <Setter TargetName="PART_Track" Property="IsDirectionReversed" Value="False" />
                <Setter TargetName="DecreasePage" Property="Command" Value="{x:Static ScrollBar.PageLeftCommand}" />
                <Setter TargetName="IncreasePage" Property="Command" Value="{x:Static ScrollBar.PageRightCommand}" />
              </Trigger>
            </ControlTemplate.Triggers>
          </ControlTemplate>
        </Setter.Value>
      </Setter>
      <Style.Triggers>
        <Trigger Property="Orientation" Value="Horizontal">
          <Setter Property="Width" Value="Auto" /><Setter Property="Height" Value="12" />
        </Trigger>
        <Trigger Property="IsMouseOver" Value="True"><Setter Property="Opacity" Value="1" /></Trigger>
        <Trigger Property="IsEnabled" Value="False"><Setter Property="Opacity" Value="0.3" /></Trigger>
      </Style.Triggers>
    </Style>
    <Style TargetType="{x:Type ScrollBar}" BasedOn="{StaticResource SlimScrollBar}" />
    <Style TargetType="{x:Type Button}">
      <Setter Property="Background" Value="#263344" />
      <Setter Property="Foreground" Value="#EDF4FC" />
      <Setter Property="BorderBrush" Value="#394B61" />
      <Setter Property="BorderThickness" Value="1" />
      <Setter Property="Padding" Value="15,10" />
      <Setter Property="FontSize" Value="14" />
      <Setter Property="FontWeight" Value="SemiBold" />
      <Setter Property="Cursor" Value="Hand" />
      <Setter Property="Template">
        <Setter.Value>
          <ControlTemplate TargetType="{x:Type Button}">
            <Border x:Name="ButtonBorder" Background="{TemplateBinding Background}"
                    BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}"
                    CornerRadius="8" Padding="{TemplateBinding Padding}">
              <ContentPresenter HorizontalAlignment="Center" VerticalAlignment="Center" />
            </Border>
            <ControlTemplate.Triggers>
              <Trigger Property="IsMouseOver" Value="True"><Setter TargetName="ButtonBorder" Property="Background" Value="#34465B" /></Trigger>
              <Trigger Property="IsPressed" Value="True"><Setter TargetName="ButtonBorder" Property="Background" Value="#172230" /></Trigger>
              <Trigger Property="IsEnabled" Value="False"><Setter TargetName="ButtonBorder" Property="Opacity" Value="0.45" /></Trigger>
            </ControlTemplate.Triggers>
          </ControlTemplate>
        </Setter.Value>
      </Setter>
    </Style>
    <Style x:Key="PrimaryButton" TargetType="{x:Type Button}" BasedOn="{StaticResource {x:Type Button}}">
      <Setter Property="Background" Value="#167CB8" />
      <Setter Property="BorderBrush" Value="#2D9DD8" />
    </Style>
    <Style x:Key="DarkComboBoxItem" TargetType="{x:Type ComboBoxItem}">
      <Setter Property="Background" Value="#141C27" />
      <Setter Property="Foreground" Value="#EDF4FC" />
      <Setter Property="Padding" Value="10,8" />
      <Setter Property="HorizontalContentAlignment" Value="Stretch" />
      <Setter Property="Template">
        <Setter.Value>
          <ControlTemplate TargetType="{x:Type ComboBoxItem}">
            <Border x:Name="ItemBorder" Background="{TemplateBinding Background}" Padding="{TemplateBinding Padding}">
              <ContentPresenter HorizontalAlignment="{TemplateBinding HorizontalContentAlignment}"
                                VerticalAlignment="Center" TextElement.Foreground="{TemplateBinding Foreground}" />
            </Border>
            <ControlTemplate.Triggers>
              <Trigger Property="IsHighlighted" Value="True"><Setter TargetName="ItemBorder" Property="Background" Value="#233C53" /></Trigger>
              <Trigger Property="IsSelected" Value="True"><Setter TargetName="ItemBorder" Property="Background" Value="#176A98" /></Trigger>
              <Trigger Property="IsEnabled" Value="False"><Setter Property="Foreground" Value="#68778A" /></Trigger>
            </ControlTemplate.Triggers>
          </ControlTemplate>
        </Setter.Value>
      </Setter>
    </Style>
    <Style TargetType="{x:Type ComboBox}">
      <Setter Property="Background" Value="#141C27" />
      <Setter Property="Foreground" Value="#EDF4FC" />
      <Setter Property="BorderBrush" Value="#394B61" />
      <Setter Property="BorderThickness" Value="1" />
      <Setter Property="Padding" Value="10,8" />
      <Setter Property="FontSize" Value="14" />
      <Setter Property="MinHeight" Value="40" />
      <Setter Property="ItemContainerStyle" Value="{StaticResource DarkComboBoxItem}" />
      <Setter Property="Template">
        <Setter.Value>
          <ControlTemplate TargetType="{x:Type ComboBox}">
            <Grid>
              <ToggleButton x:Name="ToggleButton" Focusable="False"
                            IsChecked="{Binding IsDropDownOpen, RelativeSource={RelativeSource TemplatedParent}, Mode=TwoWay}"
                            ClickMode="Press" Background="{TemplateBinding Background}"
                            BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}">
                <ToggleButton.Template>
                  <ControlTemplate TargetType="{x:Type ToggleButton}">
                    <Border x:Name="ComboBorder" Background="{TemplateBinding Background}"
                            BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}"
                            CornerRadius="7">
                      <Path Data="M 0 0 L 4 4 L 8 0 Z" Width="8" Height="4" Fill="#91A4B9"
                            HorizontalAlignment="Right" VerticalAlignment="Center" Margin="0,0,12,0" />
                    </Border>
                    <ControlTemplate.Triggers>
                      <Trigger Property="IsMouseOver" Value="True"><Setter TargetName="ComboBorder" Property="Background" Value="#1B2735" /></Trigger>
                      <Trigger Property="IsPressed" Value="True"><Setter TargetName="ComboBorder" Property="Background" Value="#202F40" /></Trigger>
                    </ControlTemplate.Triggers>
                  </ControlTemplate>
                </ToggleButton.Template>
              </ToggleButton>
              <ContentPresenter IsHitTestVisible="False" Margin="{TemplateBinding Padding}"
                                Content="{TemplateBinding SelectionBoxItem}"
                                ContentTemplate="{TemplateBinding SelectionBoxItemTemplate}"
                                ContentTemplateSelector="{TemplateBinding ItemTemplateSelector}"
                                VerticalAlignment="Center" HorizontalAlignment="Left" />
              <Popup x:Name="PART_Popup" Placement="Bottom" AllowsTransparency="True" Focusable="False"
                     PopupAnimation="Fade" PlacementTarget="{Binding RelativeSource={RelativeSource TemplatedParent}}"
                     IsOpen="{Binding IsDropDownOpen, RelativeSource={RelativeSource TemplatedParent}}">
                <Border Background="#141C27" BorderBrush="#394B61" BorderThickness="1" CornerRadius="7"
                        MinWidth="190" MaxHeight="{TemplateBinding MaxDropDownHeight}">
                  <ScrollViewer Margin="2" CanContentScroll="True">
                    <ItemsPresenter KeyboardNavigation.DirectionalNavigation="Contained" />
                  </ScrollViewer>
                </Border>
              </Popup>
            </Grid>
          </ControlTemplate>
        </Setter.Value>
      </Setter>
    </Style>
    <Style TargetType="{x:Type PasswordBox}">
      <Setter Property="Background" Value="#141C27" />
      <Setter Property="Foreground" Value="#EDF4FC" />
      <Setter Property="BorderBrush" Value="#394B61" />
      <Setter Property="BorderThickness" Value="1" />
      <Setter Property="Padding" Value="10,6" />
      <Setter Property="FontSize" Value="14" />
      <Setter Property="Height" Value="40" />
      <Setter Property="MinHeight" Value="40" />
      <Setter Property="VerticalContentAlignment" Value="Center" />
      <Setter Property="CaretBrush" Value="#EDF4FC" />
      <Setter Property="Template">
        <Setter.Value>
          <ControlTemplate TargetType="{x:Type PasswordBox}">
            <Border x:Name="PasswordBorder" CornerRadius="7"
                    Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}"
                    BorderThickness="{TemplateBinding BorderThickness}">
              <ScrollViewer x:Name="PART_ContentHost" Margin="0" />
            </Border>
            <ControlTemplate.Triggers>
              <Trigger Property="IsMouseOver" Value="True">
                <Setter TargetName="PasswordBorder" Property="BorderBrush" Value="#58708D" />
              </Trigger>
              <Trigger Property="IsKeyboardFocusWithin" Value="True">
                <Setter TargetName="PasswordBorder" Property="BorderBrush" Value="#56C7F2" />
              </Trigger>
              <Trigger Property="IsEnabled" Value="False">
                <Setter TargetName="PasswordBorder" Property="Opacity" Value="0.5" />
              </Trigger>
            </ControlTemplate.Triggers>
          </ControlTemplate>
        </Setter.Value>
      </Setter>
    </Style>
    <Style TargetType="{x:Type TextBox}">
      <Setter Property="Background" Value="#141C27" />
      <Setter Property="Foreground" Value="#EDF4FC" />
      <Setter Property="CaretBrush" Value="#EDF4FC" />
      <Setter Property="SelectionBrush" Value="#167CB8" />
      <Setter Property="BorderBrush" Value="#394B61" />
      <Setter Property="BorderThickness" Value="1" />
      <Setter Property="Padding" Value="10,6" />
      <Setter Property="FontSize" Value="14" />
      <Setter Property="Height" Value="40" />
      <Setter Property="MinHeight" Value="40" />
      <Setter Property="VerticalContentAlignment" Value="Center" />
      <Setter Property="Template">
        <Setter.Value>
          <ControlTemplate TargetType="{x:Type TextBox}">
            <Border x:Name="InputBorder" CornerRadius="7"
                    Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}"
                    BorderThickness="{TemplateBinding BorderThickness}">
              <ScrollViewer x:Name="PART_ContentHost" Margin="0" />
            </Border>
            <ControlTemplate.Triggers>
              <Trigger Property="IsMouseOver" Value="True">
                <Setter TargetName="InputBorder" Property="BorderBrush" Value="#58708D" />
              </Trigger>
              <Trigger Property="IsKeyboardFocusWithin" Value="True">
                <Setter TargetName="InputBorder" Property="BorderBrush" Value="#56C7F2" />
              </Trigger>
              <Trigger Property="IsEnabled" Value="False">
                <Setter TargetName="InputBorder" Property="Opacity" Value="0.5" />
              </Trigger>
            </ControlTemplate.Triggers>
          </ControlTemplate>
        </Setter.Value>
      </Setter>
    </Style>
    <Style TargetType="{x:Type CheckBox}">
      <Setter Property="Foreground" Value="#AAB7C8" />
      <Setter Property="FontSize" Value="12" />
      <Setter Property="VerticalAlignment" Value="Center" />
    </Style>
  </Window.Resources>
  <Grid x:Name="RootGrid" Margin="24">
    <Grid.RowDefinitions>
      <RowDefinition Height="Auto" />
      <RowDefinition Height="*" />
      <RowDefinition Height="Auto" />
    </Grid.RowDefinitions>

    <StackPanel x:Name="ControlPanel" Grid.Row="0" Margin="0,0,0,18">
      <DockPanel LastChildFill="True" Margin="0,0,0,18">
        <Border DockPanel.Dock="Right" Background="#1A2C3C" BorderBrush="#28445A" BorderThickness="1"
                CornerRadius="10" Padding="12,7" VerticalAlignment="Center">
          <StackPanel Orientation="Horizontal">
            <Ellipse Width="7" Height="7" Fill="#58D6A3" Margin="0,0,8,0" VerticalAlignment="Center" />
            <TextBlock Text="LOCAL OCR" Foreground="#AFC4D5" FontSize="11" FontWeight="Bold" />
          </StackPanel>
        </Border>
        <StackPanel Orientation="Horizontal">
          <Image x:Name="ApplicationLogo" Width="60" Height="60" Margin="0,0,14,0"
                 VerticalAlignment="Center" RenderOptions.BitmapScalingMode="HighQuality" />
          <StackPanel>
            <TextBlock Text="MULTIVIEWER  /  F1 RADIO" Foreground="{StaticResource AccentBrush}" FontSize="11" FontWeight="Bold" />
            <TextBlock Text="无线电实时翻译" FontSize="27" FontWeight="SemiBold" Margin="0,4,0,3" />
            <TextBlock Text="本地识别字幕 · 英文原文与简体中文译文" Foreground="#9BAABD" FontSize="13" />
          </StackPanel>
        </StackPanel>
      </DockPanel>

      <Border Background="{StaticResource PanelBrush}" BorderBrush="#29384A" BorderThickness="1"
              CornerRadius="13" Padding="16">
        <Grid>
          <Grid.RowDefinitions>
            <RowDefinition Height="Auto" />
            <RowDefinition Height="14" />
            <RowDefinition Height="Auto" />
          </Grid.RowDefinitions>
          <WrapPanel Grid.Row="0" Orientation="Horizontal">
            <Button x:Name="SelectAreaButton" Content="框选字幕区域" Margin="0,0,9,0" />
            <Button x:Name="StartButton" Content="开始翻译" Style="{StaticResource PrimaryButton}" Margin="0,0,9,0" />
            <Button x:Name="PauseButton" Content="暂停" Margin="0,0,9,0" />
            <Button x:Name="StopButton" Content="结束翻译" IsEnabled="False" Margin="0,0,9,0"
                    Background="#39252B" BorderBrush="#654449" Foreground="#FFD0CC" />
            <Button x:Name="ClearButton" Content="清空译文" Margin="0,0,9,0" />
            <Button x:Name="FocusModeButton" Content="简洁模式" ToolTip="只显示译文；按 F8 或点击控制面板返回。" />
            <Button x:Name="HideToTrayButton" Content="收起到托盘" Margin="9,0,0,0" ToolTip="后台继续运行；双击托盘图标恢复窗口。点 X 会退出软件。" />
          </WrapPanel>
          <StackPanel Grid.Row="2">
            <Grid>
              <Grid.ColumnDefinitions>
                <ColumnDefinition Width="190" />
                <ColumnDefinition Width="14" />
                <ColumnDefinition Width="*" />
                <ColumnDefinition Width="18" />
                <ColumnDefinition Width="Auto" />
              </Grid.ColumnDefinitions>
              <StackPanel Grid.Column="0">
                <TextBlock Text="翻译服务" Foreground="#9BAABD" FontSize="11" Margin="2,0,0,6" />
                <ComboBox x:Name="ProviderBox">
                  <ComboBoxItem Content="MyMemory · 免密备用" />
                  <ComboBoxItem Content="DeepL API" />
                  <ComboBoxItem Content="GPT-6 Luna" />
                </ComboBox>
              </StackPanel>
              <StackPanel Grid.Column="2">
                <TextBlock x:Name="KeyLabel" Text="API Key" Foreground="#9BAABD" FontSize="11" Margin="2,0,0,6" />
                <Grid>
                  <Grid.ColumnDefinitions>
                    <ColumnDefinition Width="*" />
                    <ColumnDefinition Width="8" />
                    <ColumnDefinition Width="68" />
                  </Grid.ColumnDefinitions>
                  <PasswordBox x:Name="ApiKeyBox" Grid.Column="0" />
                  <TextBox x:Name="ApiKeyTextBox" Grid.Column="0" Visibility="Collapsed" />
                  <Button x:Name="ToggleKeyVisibilityButton" Grid.Column="2" Content="显示" Padding="8,0" />
                </Grid>
              </StackPanel>
              <CheckBox x:Name="RememberKeyCheck" Grid.Column="4" Content="记住密钥"
                        VerticalAlignment="Bottom" Margin="0,0,2,12" Visibility="Collapsed" />
            </Grid>
            <StackPanel x:Name="OpenAIEndpointPanel" Visibility="Collapsed" Margin="0,12,0,0">
              <TextBlock Text="GPT 上游地址（Base URL；可留空使用 OpenAI 官方）" Foreground="#9BAABD" FontSize="11" Margin="2,0,0,6" />
              <TextBox x:Name="OpenAIBaseUrlBox" ToolTip="填写上游提供的 OpenAI 兼容 Base URL，例如 https://api.example.com/v1；请求会发送到 /chat/completions。" />
            </StackPanel>
          </StackPanel>
        </Grid>
      </Border>
      <Border Background="#18212C" BorderBrush="#29384A" BorderThickness="1"
              CornerRadius="10" Padding="12" Margin="0,12,0,0">
        <Grid>
          <Grid.ColumnDefinitions>
            <ColumnDefinition Width="156" />
            <ColumnDefinition Width="14" />
            <ColumnDefinition Width="*" />
            <ColumnDefinition Width="Auto" />
          </Grid.ColumnDefinitions>
          <Button x:Name="EnlargePreviewButton" Grid.Column="0" Height="72" Background="#10161F"
                  BorderBrush="#394B61" Padding="0" IsEnabled="False" HorizontalContentAlignment="Stretch"
                  VerticalContentAlignment="Stretch" ToolTip="点击放大识别区域；支持缩放和滚动查看。">
            <Button.Template>
              <ControlTemplate TargetType="{x:Type Button}">
                <Border x:Name="PreviewBorder" Background="{TemplateBinding Background}" CornerRadius="6"
                        BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="1" ClipToBounds="True">
                  <ContentPresenter HorizontalAlignment="Stretch" VerticalAlignment="Stretch" />
                </Border>
                <ControlTemplate.Triggers>
                  <Trigger Property="IsMouseOver" Value="True"><Setter TargetName="PreviewBorder" Property="BorderBrush" Value="#56C7F2" /></Trigger>
                </ControlTemplate.Triggers>
              </ControlTemplate>
            </Button.Template>
            <Grid>
              <Image x:Name="CapturePreviewImage" Stretch="Uniform" Margin="3" />
              <TextBlock x:Name="CapturePreviewPlaceholder" Text="尚未框选" FontSize="12" Foreground="#75859A"
                         HorizontalAlignment="Center" VerticalAlignment="Center" />
            </Grid>
          </Button>
          <StackPanel Grid.Column="2" VerticalAlignment="Center">
            <TextBlock Text="当前识别区域" Foreground="#56C7F2" FontSize="12" FontWeight="SemiBold" />
            <TextBlock x:Name="CapturePreviewDetails" Text="尚未选择字幕区域" Foreground="#D4DEEB" FontSize="12" Margin="0,5,0,0" TextTrimming="CharacterEllipsis" />
            <TextBlock x:Name="CapturePreviewHint" Text="框选时请包含卡片正文及下方姓名行。" Foreground="#8696AB" FontSize="11" Margin="0,5,10,0" TextWrapping="Wrap" />
          </StackPanel>
          <StackPanel Grid.Column="3" VerticalAlignment="Center" Margin="10,0,0,0">
            <Button x:Name="RefreshPreviewButton" Content="刷新预览" IsEnabled="False"
                    Padding="10,6" FontSize="12" Margin="0,0,0,6" />
            <Button x:Name="ClearAreaButton" Content="清除选区" IsEnabled="False"
                    Padding="10,6" FontSize="12" Foreground="#B9C8DB"
                    ToolTip="清除已保存的识别区域；运行中的翻译会结束。" />
          </StackPanel>
        </Grid>
      </Border>
    </StackPanel>

    <Border x:Name="OutputCard" Grid.Row="1" Background="#151C26" BorderBrush="#273545"
            BorderThickness="1" CornerRadius="13" Padding="20" Margin="0,0,0,12">
      <FlowDocumentScrollViewer x:Name="OutputViewer" Background="Transparent" Foreground="#EDF4FC"
                                IsToolBarVisible="False" VerticalScrollBarVisibility="Auto"
                                HorizontalScrollBarVisibility="Disabled" Padding="0">
        <FlowDocumentScrollViewer.Resources>
          <Style TargetType="{x:Type ScrollBar}" BasedOn="{StaticResource SlimScrollBar}" />
        </FlowDocumentScrollViewer.Resources>
        <FlowDocument PagePadding="0" Background="Transparent" FontFamily="Microsoft YaHei UI" FontSize="18" />
      </FlowDocumentScrollViewer>
    </Border>

    <DockPanel x:Name="StatusBar" Grid.Row="2" LastChildFill="True" Margin="2,0,2,0">
      <TextBlock x:Name="HotkeyText" DockPanel.Dock="Right" Text="F8 控制面板   ·   F9 暂停/继续   ·   F10 结束"
                 Foreground="#75859A" FontSize="11" VerticalAlignment="Center" />
      <StackPanel Orientation="Horizontal" VerticalAlignment="Center">
        <Ellipse x:Name="StatusDot" Width="7" Height="7" Fill="#7F91A8" Margin="0,0,9,0" />
        <TextBlock x:Name="StatusText" Text="先框选 MultiViewer 无线电转录区域。"
                   Foreground="#AAB7C8" FontSize="12" TextTrimming="CharacterEllipsis" />
      </StackPanel>
    </DockPanel>
    <Button x:Name="ReturnControlButton" Content="控制面板" Visibility="Collapsed"
            Grid.Row="0" Grid.RowSpan="3" Panel.ZIndex="1" HorizontalAlignment="Right" VerticalAlignment="Top"
            Margin="0,6,6,0" Padding="9,4" FontSize="11" FontWeight="Normal" Opacity="0.8"
            ToolTip="返回控制面板（F8）" />
  </Grid>
</Window>
'@

$script:Window=[Windows.Markup.XamlReader]::Load([Xml.XmlNodeReader]::new([xml]$xaml))
$applicationIcon=Join-Path $script:AppDirectory 'Translator.ico'
if (Test-Path -LiteralPath $applicationIcon) { $script:Window.Icon=[Windows.Media.Imaging.BitmapFrame]::Create([Uri]::new($applicationIcon)) }
$applicationLogoPath=Join-Path $script:AppDirectory 'Translator-logo.png'
if (Test-Path -LiteralPath $applicationLogoPath) {
  $script:Window.FindName('ApplicationLogo').Source=[Windows.Media.Imaging.BitmapImage]::new([Uri]::new($applicationLogoPath))
}
$script:ControlPanel=$script:Window.FindName('ControlPanel')
$script:StatusBar=$script:Window.FindName('StatusBar')
$script:OutputCard=$script:Window.FindName('OutputCard')
$script:OutputViewer=$script:Window.FindName('OutputViewer')
$script:OutputDocument=$script:OutputViewer.Document
$script:StatusText=$script:Window.FindName('StatusText')
$script:StatusDot=$script:Window.FindName('StatusDot')
$script:HotkeyText=$script:Window.FindName('HotkeyText')
$script:SelectAreaButton=$script:Window.FindName('SelectAreaButton')
$script:StartButton=$script:Window.FindName('StartButton')
$script:PauseButton=$script:Window.FindName('PauseButton')
$script:StopButton=$script:Window.FindName('StopButton')
$script:ClearButton=$script:Window.FindName('ClearButton')
$script:FocusModeButton=$script:Window.FindName('FocusModeButton')
$script:HideToTrayButton=$script:Window.FindName('HideToTrayButton')
$script:ReturnControlButton=$script:Window.FindName('ReturnControlButton')
$script:CapturePreviewImage=$script:Window.FindName('CapturePreviewImage')
$script:CapturePreviewPlaceholder=$script:Window.FindName('CapturePreviewPlaceholder')
$script:CapturePreviewDetails=$script:Window.FindName('CapturePreviewDetails')
$script:CapturePreviewHint=$script:Window.FindName('CapturePreviewHint')
$script:RefreshPreviewButton=$script:Window.FindName('RefreshPreviewButton')
$script:ClearAreaButton=$script:Window.FindName('ClearAreaButton')
$script:EnlargePreviewButton=$script:Window.FindName('EnlargePreviewButton')
$script:PreviewTimer=[Windows.Threading.DispatcherTimer]::new()
$script:PreviewTimer.Interval=[TimeSpan]::FromMilliseconds(250)
$script:PreviewTimer.Add_Tick({ $script:PreviewTimer.Stop(); Update-CapturePreview })
$script:AutoPreviewTimer=[Windows.Threading.DispatcherTimer]::new()
$script:AutoPreviewTimer.Interval=[TimeSpan]::FromSeconds(10)
$script:AutoPreviewTimer.Add_Tick({ if (-not $script:CaptureBounds.IsEmpty) { Update-CapturePreview } })
$script:PreviewInitialized=$false
$script:ProviderBox=$script:Window.FindName('ProviderBox')
$script:ApiKeyBox=$script:Window.FindName('ApiKeyBox')
$script:ApiKeyTextBox=$script:Window.FindName('ApiKeyTextBox')
$script:ToggleKeyVisibilityButton=$script:Window.FindName('ToggleKeyVisibilityButton')
$script:SyncingKey=$false
$script:RememberKeyCheck=$script:Window.FindName('RememberKeyCheck')
$script:KeyLabel=$script:Window.FindName('KeyLabel')
$script:OpenAIEndpointPanel=$script:Window.FindName('OpenAIEndpointPanel')
$script:OpenAIBaseUrlBox=$script:Window.FindName('OpenAIBaseUrlBox')
$script:ProviderBox.SelectedIndex=$script:SavedProviderIndex
$script:LastProviderIndex=$script:ProviderBox.SelectedIndex
$script:PauseGate=[Threading.ManualResetEventSlim]::new($true)
$script:UserPaused=$false
$script:ShutdownRequested=$false
$script:ShutdownReady=$false
$script:ShuttingDown=$false
$script:LastWindowState=[Windows.WindowState]::Normal
$script:TrayIcon=$null
$script:TrayMenu=$null
$script:TrayIconImage=$null
$script:ClearRequested=$false

function Set-FocusMode([bool]$Enabled) {
  $script:FocusMode=$Enabled
  if ($Enabled) {
    $script:Window.Title='F1 实时译文'
    $script:ControlPanel.Visibility=[Windows.Visibility]::Collapsed
    $script:StatusBar.Visibility=[Windows.Visibility]::Collapsed
    $script:ReturnControlButton.Visibility=[Windows.Visibility]::Visible
    $script:OutputCard.SetValue([Windows.Controls.Grid]::RowProperty,0)
    $script:OutputCard.SetValue([Windows.Controls.Grid]::RowSpanProperty,3)
    $script:OutputCard.SetValue([Windows.FrameworkElement]::MarginProperty,[Windows.Thickness]::new(0))
    $script:OutputCard.Padding=[Windows.Thickness]::new(18,44,18,18)
    $script:OutputCard.BorderThickness=[Windows.Thickness]::new(0)
  } else {
    $script:Window.Title='MultiViewer F1 Radio Translator'
    $script:ControlPanel.Visibility=[Windows.Visibility]::Visible
    $script:StatusBar.Visibility=[Windows.Visibility]::Visible
    $script:ReturnControlButton.Visibility=[Windows.Visibility]::Collapsed
    $script:OutputCard.SetValue([Windows.Controls.Grid]::RowProperty,1)
    $script:OutputCard.SetValue([Windows.Controls.Grid]::RowSpanProperty,1)
    $script:OutputCard.SetValue([Windows.FrameworkElement]::MarginProperty,[Windows.Thickness]::new(0,0,0,12))
    $script:OutputCard.Padding=[Windows.Thickness]::new(20)
    $script:OutputCard.BorderThickness=[Windows.Thickness]::new(1)
  }
}
function Toggle-Pause {
  if (-not $script:Scanning) { Status '请先开始翻译。'; return }
  if ($script:PauseGate.IsSet) {
    $script:PauseGate.Reset(); $script:UserPaused=$true; $script:PauseButton.Content='继续'; $script:StatusDot.Fill=[Windows.Media.Brushes]::Orange; Status '已暂停。按 F9 或点击“继续”恢复。'
  } else {
    $script:UserPaused=$false; $script:PauseGate.Set(); $script:PauseButton.Content='暂停'; $script:StatusDot.Fill=[Windows.Media.Brushes]::LightGreen; Status '正在恢复识别。'
  }
  Update-TrayMenu
}
function Stop-Translation {
  if (-not $script:Scanning) { Status '当前没有运行中的翻译。'; return }
  $script:ScanTimer.Stop()
  $script:Scanning=$false
  $script:UserPaused=$false
  $script:PauseGate.Set()
  $script:Queue.Clear()
  $script:TranslationPendingText=''
  Stop-TranslationProcess
  Stop-OcrWorker
  $script:StartButton.IsEnabled=$true
  $script:PauseButton.IsEnabled=$false
  $script:StopButton.IsEnabled=$false
  $script:PauseButton.Content='暂停'
  $script:VisibleError=$false
  $script:StatusDot.Fill=[Windows.Media.Brushes]::Gray
  Set-FocusMode $false
  Status '翻译已结束。可以重新开始或关闭窗口。'
  Update-TrayMenu
}

function Update-TrayMenu {
  if (-not $script:TrayIcon) { return }
  $script:TrayPauseItem.Enabled=$script:Scanning
  $script:TrayStopItem.Enabled=$script:Scanning
  $script:TrayPauseItem.Text=if ($script:UserPaused) { '继续翻译' } else { '暂停翻译' }
  $state=if (-not $script:Scanning) { '未开始' } elseif ($script:UserPaused) { '已暂停' } else { '翻译中' }
  $script:TrayIcon.Text='F1 无线电翻译 · '+$state
}
function Restore-FromTray {
  if ($script:ShuttingDown) { return }
  $script:Window.Show()
  $script:Window.WindowState=$script:LastWindowState
  [void]$script:Window.Activate()
}
function Hide-ToTray {
  if ($script:ShuttingDown -or -not $script:TrayIcon) { return }
  Update-TrayMenu
  $script:Window.Hide()
}
function Initialize-TrayIcon {
  $script:TrayMenu=[Windows.Forms.ContextMenuStrip]::new()
  $script:TrayMenu.Font=[Drawing.Font]::new('Microsoft YaHei UI',[single]9)
  $showItem=[Windows.Forms.ToolStripMenuItem]::new('显示窗口')
  $script:TrayPauseItem=[Windows.Forms.ToolStripMenuItem]::new('暂停翻译')
  $script:TrayStopItem=[Windows.Forms.ToolStripMenuItem]::new('结束翻译')
  $exitItem=[Windows.Forms.ToolStripMenuItem]::new('退出软件')
  [void]$script:TrayMenu.Items.Add($showItem)
  [void]$script:TrayMenu.Items.Add($script:TrayPauseItem)
  [void]$script:TrayMenu.Items.Add($script:TrayStopItem)
  [void]$script:TrayMenu.Items.Add([Windows.Forms.ToolStripSeparator]::new())
  [void]$script:TrayMenu.Items.Add($exitItem)
  $showItem.Add_Click({ Restore-FromTray })
  $script:TrayPauseItem.Add_Click({ Toggle-Pause; Update-TrayMenu })
  $script:TrayStopItem.Add_Click({ Stop-Translation; Update-TrayMenu })
  $exitItem.Add_Click({ $script:Window.Close() })
  $script:TrayMenu.Add_Opening({ Update-TrayMenu })
  $script:TrayIcon=[Windows.Forms.NotifyIcon]::new()
  $script:TrayIconImage=[Drawing.Icon]::new((Join-Path $script:AppDirectory 'Translator.ico'),[Drawing.Size]::new(32,32))
  $script:TrayIcon.Icon=$script:TrayIconImage
  $script:TrayIcon.ContextMenuStrip=$script:TrayMenu
  $script:TrayIcon.Add_DoubleClick({ Restore-FromTray })
  Update-TrayMenu
  $script:TrayIcon.Visible=$true
}
function Shutdown-ApplicationRuntime {
  if ($script:ShuttingDown) { return }
  $script:ShuttingDown=$true
  $script:ShutdownRequested=$true
  $script:Scanning=$false
  $script:PreviewTimer.Stop()
  $script:AutoPreviewTimer.Stop()
  $script:ScanTimer.Stop()
  $script:PauseGate.Set()
  $script:Queue.Clear()
  if ($script:RegionSelectorWindow) { $script:RegionSelectorWindow.Close() }
  Stop-TranslationProcess
  Stop-OcrWorker -Force
  if ($script:TrayIcon) { $script:TrayIcon.Visible=$false; $script:TrayIcon.Dispose(); $script:TrayIcon=$null }
  if ($script:TrayMenu) { $script:TrayMenu.Font.Dispose(); $script:TrayMenu.Dispose(); $script:TrayMenu=$null }
  if ($script:TrayIconImage) { $script:TrayIconImage.Dispose(); $script:TrayIconImage=$null }
  if (Test-Path -LiteralPath $script:TempImage) { Remove-Item -LiteralPath $script:TempImage -Force -ErrorAction SilentlyContinue }
}

$script:ProviderBox.Add_SelectionChanged({
  if ($script:LastProviderIndex -eq 1) { $script:SessionDeepLKey=$script:ApiKeyBox.Password }
  elseif ($script:LastProviderIndex -eq 2) { $script:SessionOpenAIKey=$script:ApiKeyBox.Password }
  if ($script:ProviderBox.SelectedIndex -eq 1) { $script:KeyLabel.Text='DeepL API Key' }
  elseif ($script:ProviderBox.SelectedIndex -eq 2) { $script:KeyLabel.Text='OpenAI API Key' }
  else { $script:KeyLabel.Text='API Key' }
  $script:OpenAIEndpointPanel.Visibility=if ($script:ProviderBox.SelectedIndex -eq 2) { [Windows.Visibility]::Visible } else { [Windows.Visibility]::Collapsed }
  $script:LoadingSettings=$true
  if ($script:ProviderBox.SelectedIndex -eq 1) { $script:ApiKeyBox.Password=$script:SessionDeepLKey; $script:RememberKeyCheck.IsChecked=$script:RememberDeepLKey }
  elseif ($script:ProviderBox.SelectedIndex -eq 2) { $script:ApiKeyBox.Password=$script:SessionOpenAIKey; $script:RememberKeyCheck.IsChecked=$script:RememberOpenAIKey }
  else { $script:ApiKeyBox.Clear(); $script:RememberKeyCheck.IsChecked=$false }
  $script:RememberKeyCheck.Visibility=if ($script:ProviderBox.SelectedIndex -in @(1,2)) { [Windows.Visibility]::Visible } else { [Windows.Visibility]::Collapsed }
  $script:LoadingSettings=$false
  $script:LastProviderIndex=$script:ProviderBox.SelectedIndex
  $script:SavedProviderIndex=$script:ProviderBox.SelectedIndex
  Save-Settings
})
$script:RememberKeyCheck.Add_Checked({ if (-not $script:LoadingSettings) { Save-CurrentRememberedKey } })
$script:RememberKeyCheck.Add_Unchecked({
  if ($script:LoadingSettings) { return }
  if ($script:ProviderBox.SelectedIndex -eq 1) { $script:RememberDeepLKey=$false; $script:SavedDeepLKey='' }
  elseif ($script:ProviderBox.SelectedIndex -eq 2) { $script:RememberOpenAIKey=$false; $script:SavedOpenAIKey='' }
  Save-Settings
})
$script:ApiKeyBox.Add_PasswordChanged({
  if (-not $script:SyncingKey) {
    $script:SyncingKey=$true
    try { $script:ApiKeyTextBox.Text=$script:ApiKeyBox.Password } finally { $script:SyncingKey=$false }
  }
  if ($script:RememberKeyCheck.IsChecked -eq $true) { Save-CurrentRememberedKey }
})
$script:ApiKeyTextBox.Add_TextChanged({
  if ($script:SyncingKey) { return }
  $script:SyncingKey=$true
  try { $script:ApiKeyBox.Password=$script:ApiKeyTextBox.Text } finally { $script:SyncingKey=$false }
  if ($script:RememberKeyCheck.IsChecked -eq $true) { Save-CurrentRememberedKey }
})
$script:ToggleKeyVisibilityButton.Add_Click({
  if ($script:ApiKeyBox.Visibility -eq [Windows.Visibility]::Visible) {
    $script:ApiKeyTextBox.Text=$script:ApiKeyBox.Password
    $script:ApiKeyBox.Visibility=[Windows.Visibility]::Collapsed
    $script:ApiKeyTextBox.Visibility=[Windows.Visibility]::Visible
    $script:ToggleKeyVisibilityButton.Content='隐藏'
  } else {
    $script:ApiKeyBox.Password=$script:ApiKeyTextBox.Text
    $script:ApiKeyTextBox.Visibility=[Windows.Visibility]::Collapsed
    $script:ApiKeyBox.Visibility=[Windows.Visibility]::Visible
    $script:ToggleKeyVisibilityButton.Content='显示'
  }
})
$script:OpenAIBaseUrlBox.Text=$script:OpenAIBaseUrl
$script:OpenAIBaseUrlBox.Add_TextChanged({
  if ($script:LoadingSettings) { return }
  $script:OpenAIBaseUrl=$script:OpenAIBaseUrlBox.Text.Trim()
  Save-Settings
})

$script:LoadingSettings=$true
if ($script:ProviderBox.SelectedIndex -eq 1) {
  $script:KeyLabel.Text='DeepL API Key'; $script:ApiKeyBox.Password=$script:SessionDeepLKey; $script:RememberKeyCheck.IsChecked=$script:RememberDeepLKey; $script:RememberKeyCheck.Visibility=[Windows.Visibility]::Visible
} elseif ($script:ProviderBox.SelectedIndex -eq 2) {
  $script:KeyLabel.Text='OpenAI API Key'; $script:ApiKeyBox.Password=$script:SessionOpenAIKey; $script:RememberKeyCheck.IsChecked=$script:RememberOpenAIKey; $script:RememberKeyCheck.Visibility=[Windows.Visibility]::Visible; $script:OpenAIEndpointPanel.Visibility=[Windows.Visibility]::Visible
} else { $script:RememberKeyCheck.Visibility=[Windows.Visibility]::Collapsed }
$script:LoadingSettings=$false
$script:LastProviderIndex=$script:ProviderBox.SelectedIndex

$script:ScanTimer=[Windows.Threading.DispatcherTimer]::new()
$script:ScanTimer.Interval=[TimeSpan]::FromMilliseconds(1200)
$script:ScanTimer.Add_Tick({
  if (-not $script:Scanning -or $script:UserPaused) { return }
  try { Scan-Worker $null }
  catch {
    $caughtMessage=$_.Exception.Message
    $caughtStack=$_.ScriptStackTrace
    $script:ScanTimer.Stop(); $script:Scanning=$false; $script:StartButton.IsEnabled=$true; $script:PauseButton.IsEnabled=$false; $script:StopButton.IsEnabled=$false
    Stop-OcrWorker
    if ($script:TranslationProcess) { try { if (-not $script:TranslationProcess.HasExited) { $script:TranslationProcess.Kill() } } catch { }; $script:TranslationProcess.Dispose(); $script:TranslationProcess=$null }
    Set-FocusMode $false
    $detail=$caughtMessage
    Worker-Error $null ('扫描失败：'+$detail)
    $dialogText='实时扫描失败：' + $detail
    if ($caughtStack) { $dialogText += "`n`n调用位置：`n" + $caughtStack }
    [void][Windows.MessageBox]::Show($dialogText,'MultiViewer F1 翻译器',[Windows.MessageBoxButton]::OK,[Windows.MessageBoxImage]::Error)
  }
})

$script:SelectAreaButton.Add_Click({ Select-Area })
$script:RefreshPreviewButton.Add_Click({ $script:PreviewTimer.Stop(); $script:PreviewTimer.Start() })
$script:ClearAreaButton.Add_Click({ Clear-SelectedArea })
$script:EnlargePreviewButton.Add_Click({ Show-CapturePreview })
$script:Window.Add_ContentRendered({
  if (-not $script:PreviewInitialized) { $script:PreviewInitialized=$true; $script:PreviewTimer.Start(); $script:AutoPreviewTimer.Start() }
})
$script:StartButton.Add_Click({
  if ($script:CaptureBounds.IsEmpty) { Status '请先框选 MultiViewer 字幕区域。'; return }
  if ($script:ProviderBox.SelectedIndex -in @(1,2) -and -not $script:ApiKeyBox.Password.Trim()) { Status '当前翻译服务需要 API Key。'; return }
  if ($script:Scanning) { Status '翻译任务已经在运行。'; return }
  Save-CurrentRememberedKey
  $script:TranslationProvider=if ($script:ProviderBox.SelectedIndex -eq 1) { 'DeepL' } elseif ($script:ProviderBox.SelectedIndex -eq 2) { 'OpenAI' } else { 'MyMemory' }
  $script:DeepLKey=if ($script:TranslationProvider -eq 'DeepL') { $script:ApiKeyBox.Password.Trim() } else { '' }
  $script:OpenAIKey=if ($script:TranslationProvider -eq 'OpenAI') { $script:ApiKeyBox.Password.Trim() } else { '' }
  $script:PauseGate.Set(); $script:UserPaused=$false; $script:NextRequestAt=[DateTime]::MinValue; $script:BackoffSeconds=0
  $script:StartButton.IsEnabled=$false; $script:PauseButton.IsEnabled=$true; $script:PauseButton.Content='暂停'; $script:StatusDot.Fill=[Windows.Media.Brushes]::DodgerBlue
  $script:StopButton.IsEnabled=$true
  Status '正在载入本地 OCR 模型…'
  try {
    Start-OcrWorker
    $script:Scanning=$true
    $script:ScanTimer.Start()
    Update-TrayMenu
  } catch {
    $script:Scanning=$false
    $script:StartButton.IsEnabled=$true
    $script:PauseButton.IsEnabled=$false
    $script:StopButton.IsEnabled=$false
    Stop-OcrWorker
    Set-FocusMode $false
    $detail=$_.Exception.Message
    Worker-Error $null ('无法开始翻译：'+$detail)
    [void][Windows.MessageBox]::Show(('无法开始实时翻译：' + $detail),'MultiViewer F1 翻译器',[Windows.MessageBoxButton]::OK,[Windows.MessageBoxImage]::Error)
  }
})
$script:PauseButton.Add_Click({ Toggle-Pause })
$script:StopButton.Add_Click({ Stop-Translation })
$script:FocusModeButton.Add_Click({ Set-FocusMode $true })
$script:HideToTrayButton.Add_Click({ Hide-ToTray })
$script:ReturnControlButton.Add_Click({ Set-FocusMode $false })
$script:ClearButton.Add_Click({ $script:OutputDocument.Blocks.Clear(); $script:OutputBlockGroups.Clear(); $script:ClearRequested=$true; Status '译文已清空。' })
$script:Window.Add_KeyDown({
  param($sender,$e)
  if ($e.Key -eq [Windows.Input.Key]::F8) { Set-FocusMode (-not $script:FocusMode); $e.Handled=$true }
  elseif ($e.Key -eq [Windows.Input.Key]::F9) { Toggle-Pause; $e.Handled=$true }
  elseif ($e.Key -eq [Windows.Input.Key]::F10) { Stop-Translation; $e.Handled=$true }
})
$script:Window.Add_Closing({
  param($sender,$e)
  if ($script:ShuttingDown) { return }
  Save-Settings
  Shutdown-ApplicationRuntime
  $script:LoadingSettings=$true
  $script:DeepLKey=''; $script:OpenAIKey=''; $script:ApiKeyBox.Clear(); $script:ApiKeyTextBox.Clear()
})
$script:Window.Add_StateChanged({
  if ($script:Window.WindowState -eq [Windows.WindowState]::Minimized) { Hide-ToTray }
  else { $script:LastWindowState=$script:Window.WindowState }
})
$script:Window.Add_Closed({
  if ($script:WpfApplication) { $script:WpfApplication.Shutdown(0) }
})
if ($script:SettingsWarning) { Status $script:SettingsWarning }
elseif (-not $script:CaptureBounds.IsEmpty) { Status '已恢复上次框选区域；确认 MultiViewer 位置后开始翻译。' }
else { Status '先框选 MultiViewer 无线电转录区域。' }
try {
  $script:WpfApplication=[Windows.Application]::new()
  $script:WpfApplication.ShutdownMode=[Windows.ShutdownMode]::OnExplicitShutdown
  Initialize-TrayIcon
  [void]$script:WpfApplication.Run($script:Window)
} finally { Shutdown-ApplicationRuntime }
