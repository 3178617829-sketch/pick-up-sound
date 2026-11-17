@echo off
rem Host is ARM64 but there is no ARM64 MSVC toolchain; build x64 instead.
rem Use the x64 Rust toolchain (runs under emulation) + MSVC x64 env.
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64 >nul 2>&1
if errorlevel 1 goto _vc_fail
cd /d "%~dp0src-tauri"
cargo +stable-x86_64-pc-windows-msvc %*
exit /b %errorlevel%

:_vc_fail
echo [ERROR] vcvarsall failed
exit /b 1
