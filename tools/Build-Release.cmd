@echo off
rem Double-click to build the release files (installer, portable zip, SHA256SUMS). Logic lives in Build-Release.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Build-Release.ps1" %*
