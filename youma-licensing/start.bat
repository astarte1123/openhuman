@echo off
setlocal EnableDelayedExpansion

title Youma Licensing Server
cd /d "%~dp0"

echo ============================================================
echo   Youma Licensing Server v1.0.0 - Avvio PRODUZIONE
echo ============================================================
echo.

:: ── Verifica .env ─────────────────────────────────────────────
if not exist ".env" (
    if exist ".env.example" (
        echo [setup] .env non trovato — copio da .env.example
        copy ".env.example" ".env" >nul
        echo.
        echo [ATTENZIONE] Imposta ADMIN_TOKEN e SIGNING_SECRET in .env prima di usare in produzione!
        echo Genera valori sicuri con:  openssl rand -hex 32
        echo.
        echo Premi un tasto per continuare con token auto-generati...
        pause >nul
    ) else (
        echo [setup] Nessun file .env trovato. Il server generera token casuali ^(vedi stderr^).
    )
) else (
    echo [setup] .env trovato OK
    :: Carica variabili da .env (righe non vuote, non commenti)
    for /f "usebackq tokens=1,* delims==" %%A in (`findstr /v "^#" .env ^| findstr /v "^$"`) do (
        set "%%A=%%B"
    )
)

:: ── Verifica Rust ─────────────────────────────────────────────
where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERRORE] cargo non trovato nel PATH.
    echo Installa Rust da: https://rustup.rs
    pause
    exit /b 1
)

:: ── Build release (se necessario) ─────────────────────────────
set BINARY=target\release\youma-licensing.exe

if not exist "%BINARY%" (
    echo [build] Binary release non trovato — avvio compilazione...
    echo [build] Questo richiede alcuni minuti la prima volta.
    echo.
    cargo build --release
    if errorlevel 1 (
        echo.
        echo [ERRORE] Compilazione fallita. Controlla gli errori sopra.
        pause
        exit /b 1
    )
    echo.
    echo [build] Compilazione completata.
) else (
    echo [build] Binary trovato: %BINARY%

    :: Controlla se i sorgenti sono piu recenti del binary
    for /f "delims=" %%F in ('dir /b /s /a-d src\*.rs Cargo.toml 2^>nul ^| head -1') do (
        set "NEWEST_SRC=%%F"
    )
    echo [build] Per ricompilare elimina %BINARY% e rilancia lo script.
)

:: ── Mostra configurazione attiva ──────────────────────────────
echo.
echo [config] DATABASE_PATH  = !DATABASE_PATH!
echo [config] BIND_ADDR      = !BIND_ADDR!
echo [config] LOG_LEVEL      = !LOG_LEVEL!
echo [config] LOG_FORMAT     = !LOG_FORMAT!
echo [config] MAX_RATE_MIN   = !MAX_RATE_PER_MIN!
if defined ADMIN_TOKEN (
    set "MASKED_TOKEN=!ADMIN_TOKEN:~0,8!..."
    echo [config] ADMIN_TOKEN    = !MASKED_TOKEN! ^(mascherato^)
) else (
    echo [config] ADMIN_TOKEN    = ^(auto-generato al primo avvio^)
)
echo.

:: ── Avvio server ──────────────────────────────────────────────
set /a BIND_PORT=8080
if defined BIND_ADDR (
    for /f "tokens=2 delims=:" %%P in ("!BIND_ADDR!") do set "BIND_PORT=%%P"
)

echo [avvio] Youma Licensing Server in partenza...
echo [avvio] Admin UI: http://localhost:!BIND_PORT!/admin
echo [avvio] API:      http://localhost:!BIND_PORT!/api/v1
echo [avvio] Health:   http://localhost:!BIND_PORT!/api/v1/health
echo [avvio] Premi Ctrl+C per fermare il server
echo.
echo ============================================================
echo.

"%BINARY%"

if errorlevel 1 (
    echo.
    echo [ERRORE] Il server si e fermato con codice di errore %errorlevel%.
    pause
)
endlocal
