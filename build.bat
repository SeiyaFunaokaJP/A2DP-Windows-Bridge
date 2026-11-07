@echo off
rem Runs build.ps1 (PowerShell 7) with the same switches, e.g. build.bat -Test
rem Double-clicked (no switches), it waits for a key at the end.
where pwsh >nul 2>nul
if errorlevel 1 (
    echo PowerShell 7 ^(pwsh^) is needed: winget install Microsoft.PowerShell
    if "%~1"=="" pause
    exit /b 1
)
pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
set "BLUECASTER_RC=%ERRORLEVEL%"
if "%~1"=="" pause
exit /b %BLUECASTER_RC%
