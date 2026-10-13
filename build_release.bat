@echo off
chcp 65001 >nul
cd /d "%~dp0"
cargo build --release
if errorlevel 1 (
  echo Build failed. Read the errors above.
  pause
  exit /b 1
)
echo Build succeeded: target\release\kiseki-storage.exe
pause
