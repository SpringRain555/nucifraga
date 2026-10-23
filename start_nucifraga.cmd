@echo off
rem Double-click to start nucifraga. All logic and messages live in tools\Launch.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\Launch.ps1" %*
