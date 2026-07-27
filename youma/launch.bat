@echo off
title Youma
set PATH=C:\msys64\mingw64\bin;C:\Users\ipezzoli\.cargo\bin;%PATH%
cd /d "%~dp0"

:: Build cache in Temp — AppData\Local\Temp is excluded from Bitdefender real-time scanning.
:: AppData\Local\youma-target is NOT excluded and causes LNK1104 on proc-macro DLLs.
set CARGO_TARGET_DIR=C:\Users\ipezzoli\AppData\Local\Temp\youma-target

:: AI provider — Qwen3 on local network server (OpenAI-compatible)
set CUSTOM_AI_URL=http://10.20.10.24:8080/v1/
set CUSTOM_AI_MODEL=qwen3.6-mtp
set CUSTOM_AI_KEY=
set CUSTOM_AI_MAX_TOKENS=16384

:: Kill leftover processes
echo [launch] Cleaning up stale processes...
taskkill /F /IM youma.exe /T >nul 2>&1
taskkill /F /IM cargo-tauri.exe /T >nul 2>&1

:: Free port 6005 if occupied
for /f "tokens=5" %%p in ('netstat -ano ^| findstr ":6005 " ^| findstr "LISTENING"') do (
    echo [launch] Killing PID %%p on port 6005
    taskkill /F /PID %%p >nul 2>&1
)

:: Free port 1420 (Tauri default fallback) if occupied
for /f "tokens=5" %%p in ('netstat -ano ^| findstr ":1420 " ^| findstr "LISTENING"') do (
    echo [launch] Killing PID %%p on port 1420
    taskkill /F /PID %%p >nul 2>&1
)

echo [launch] Starting Youma...
cargo tauri dev
