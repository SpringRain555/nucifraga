@echo off
rem Double-click: build the CLI and run two sample backups inside %TEMP%. Logic lives in Try-SampleBackup.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Try-SampleBackup.ps1" %*
