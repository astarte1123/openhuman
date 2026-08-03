@echo off
setlocal EnableDelayedExpansion

title Youma Licensing Server [DEV]
cd /d "%~dp0"

echo ============================================================
echo   Youma Licensing Server - Avvio DEV (cargo run)
echo ============================================================
echo.

:: ── Verifica .env ─────────────────────────────────────────────
if not exist ".env" (
    if exist ".env.example" (
        echo [setup] .env non trovato — copio da .env.example
        copy ".env.example" ".env" >nul
        echo [setup] Apri .env e imposta ADMIN_TOKEN e SIGNING_SECRET prima di continuare.
        echo.
        echo Premi un tasto per avviare comunque con token generati automaticamente...
        pause >nul
    ) else (
        echo [setup] Nessun file .env trovato. Il server genererà token casuali.
        echo.
    )
) else (
    echo [setup] .env trovato OK
)

:: ── Verifica Rust ─────────────────────────────────────────────
where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERRORE] cargo non trovato nel PATH.
    echo Installa Rust da: https://rustup.rs
    pause
    exit /b 1
)
echo [setup] Rust/cargo OK

:: ── Avvio ─────────────────────────────────────────────────────
echo.
echo [avvio] cargo run (modalita debug)
echo [avvio] Admin UI: http://localhost:8080/admin
echo [avvio] API:      http://localhost:8080/api/v1
echo [avvio] Premi Ctrl+C per fermare il server
echo.

cargo run 2>&1

if errorlevel 1 (
    echo.
    echo [ERRORE] Il server si e fermato con errore.
    pause
)
endlocal
