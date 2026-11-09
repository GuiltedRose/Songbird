@echo off
setlocal
if "%~1"=="" (
  "%~dp0songbird.exe" "%USERPROFILE%\Music\Songbird"
) else (
  "%~dp0songbird.exe" "%~1"
)
if errorlevel 1 pause
