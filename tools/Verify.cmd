@echo off
rem Double-click to run every project check. Logic lives in Verify.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Verify.ps1" %*
echo.
pause
