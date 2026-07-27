# Aura

Rust + Tauri v2 rewrite of OpenHuman. Frontend in Yew/WebAssembly, backend in Rust.

## Prerequisiti

| Tool | Installazione |
|------|---------------|
| Rust (stable GNU) | `rustup default stable-x86_64-pc-windows-gnu` |
| WASM target | `rustup target add wasm32-unknown-unknown` |
| MinGW GCC | `C:\msys64\mingw64\bin` (MSYS2 già installato) |
| trunk | scaricato in `~/.cargo/bin/trunk.exe` |
| tauri-cli | scaricato in `~/.cargo/bin/cargo-tauri.exe` |

**GCC deve essere nel PATH:**
```powershell
$env:PATH += ";$env:USERPROFILE\.cargo\bin;C:\msys64\mingw64\bin"
```

## Avviare in modalità sviluppo

```powershell
# 1. Aggiungere GCC al PATH (ogni sessione)
$env:PATH += ";$env:USERPROFILE\.cargo\bin;C:\msys64\mingw64\bin"

# 2. Avviare il frontend dev server (in un terminale separato)
cd aura\frontend
trunk serve --port 6005 --open false

# 3. In un altro terminale: avviare Tauri
$env:PATH += ";$env:USERPROFILE\.cargo\bin;C:\msys64\mingw64\bin"
cd aura\src-tauri
cargo tauri dev
```

Tauri apre automaticamente la finestra dell'app caricando `http://127.0.0.1:6005`.

## Build del frontend WASM (standalone)

```powershell
$env:PATH += ";$env:USERPROFILE\.cargo\bin;C:\msys64\mingw64\bin"
cd aura\frontend
trunk build --release   # output in aura\dist\
```

## Build del binario Tauri

```powershell
$env:PATH += ";$env:USERPROFILE\.cargo\bin;C:\msys64\mingw64\bin"
cd aura\src-tauri
cargo build             # debug
cargo build --release   # release
```

## Struttura

```
aura/
├── frontend/           Yew + WebAssembly (compilato con trunk)
│   └── src/
│       ├── lib.rs      Entrypoint WASM (#[wasm_bindgen(start)])
│       ├── app/
│       │   ├── mod.rs  (app.rs) Router + pagine
│       │   └── api.rs  Bridge verso Tauri IPC
│       └── tauri.js    Wrapper JS per __TAURI_INTERNALS__.invoke
├── src-tauri/          Host Tauri v2 (Rust)
│   ├── src/
│   │   ├── main.rs     Comandi Tauri (core_invoke, start_core, …)
│   │   └── core_process.rs  Avvio/attach a openhuman-core
│   ├── tauri.conf.json Configurazione Tauri v2
│   ├── capabilities/   Permessi Tauri v2
│   └── icons/          Icone app
└── rust-toolchain.toml Forza stable-x86_64-pc-windows-gnu
```

## Note core OpenHuman

Il backend Tauri NON linka il crate `openhuman_core` direttamente (troppo pesante, richiede libclang per whisper-rs). Invece `core_process.rs`:

1. Prova a collegarsi a `openhuman-core` già in esecuzione su porta 7788.
2. Se non trovato, cerca il binario `openhuman-core` nel PATH o accanto all'eseguibile e lo lancia con `serve`.
3. Legge il token RPC da `~/.openhuman/core.token` o `~/.openhuman-staging/core.token`.

Per testare con il core reale:
```powershell
# Dal root del repo (non da aura/)
cargo build --bin openhuman-core
.\target\debug\openhuman-core serve
# Poi avviare aura in un altro terminale
```

## Toolchain

`aura/rust-toolchain.toml` seleziona `stable-x86_64-pc-windows-gnu` per isolarsi
dalla versione pinned `1.93.0-x86_64-pc-windows-msvc` del repo padre.
