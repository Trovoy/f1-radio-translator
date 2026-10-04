param([string]$PythonExe='')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Resolve-Python312.ps1')
$repositoryRoot=Split-Path -Parent $PSScriptRoot
$python=Resolve-Python312 $PythonExe
$target=Join-Path $repositoryRoot 'src\python-deps'
& $python -m pip install --disable-pip-version-check --upgrade --target $target -r (Join-Path $repositoryRoot 'requirements.txt')
if ($LASTEXITCODE -ne 0) { throw 'Installing OCR dependencies failed.' }
Write-Output 'Dependencies ready. Start src\Start-MultiViewerTranslator.bat or build an EXE.'
