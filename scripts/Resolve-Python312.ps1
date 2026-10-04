function Resolve-Python312([string]$PythonExe) {
  if ($PythonExe) { $candidates=@($PythonExe) }
  else {
    $candidates=@(
      (Join-Path $env:LOCALAPPDATA 'Programs\Python\Python312\python.exe'),
      (Join-Path $env:ProgramFiles 'Python312\python.exe')
    )
    $launcher=Get-Command py.exe -ErrorAction SilentlyContinue
    if ($launcher) {
      $resolvedPython=& $launcher.Source -3.12 -c 'import sys; print(sys.executable)' 2>$null
      if ($LASTEXITCODE -eq 0 -and $resolvedPython) { $candidates=@([string]$resolvedPython)+$candidates }
    }
    $pythonCommand=Get-Command python.exe -ErrorAction SilentlyContinue
    if ($pythonCommand) { $candidates+=@($pythonCommand.Source) }
  }
  foreach ($candidate in $candidates) {
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }
    $version=& $candidate -I -c 'import sys, struct; print("%d.%d-%d" % (sys.version_info[0], sys.version_info[1], struct.calcsize("P") * 8))' 2>$null
    if ($LASTEXITCODE -eq 0 -and ([string]$version).Trim() -eq '3.12-64') { return (Resolve-Path -LiteralPath $candidate).Path }
  }
  throw 'Install 64-bit Python 3.12, or supply -PythonExe with its full executable path.'
}
