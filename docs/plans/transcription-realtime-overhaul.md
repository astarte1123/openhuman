# Piano di ristrutturazione — Trascrizione vocale real-time (dictation + mic composer)

> Analisi archeologica dello stack trascrizione (voice/dictation/STT) e piano di
> ristrutturazione. Lo stato attuale è il risultato di **almeno 4 epoche di
> sviluppo sovrapposte** (whisper-cli subprocess → whisper-rs in-process →
> cloud STT proxy → WebSocket streaming mai collegato alla UI). Il risultato è
> una feature che "non funziona per nulla bene": pipeline duplicate, configurazioni
> concettualmente sovrapposte, endpoint morti e delivery del testo rotta su
> Windows/Linux.

---

## 1. Mappa dello stato attuale (cosa esiste davvero)

### 1.1 Cinque pipeline di trascrizione parallele

| # | Pipeline | Entry point | Motore | Consumatore |
|---|----------|-------------|--------|-------------|
| A | **Voice server** (hotkey globale → record → transcribe → insert) | `src/aura/voice/server.rs` | in-process whisper-rs, fallback whisper-cli (`voice::ops::voice_transcribe_bytes` → `LocalAiService::transcribe_with_prompt`) | paste OS (`text_input.rs`) o socket.io |
| B | **Dictation listener** (solo eventi hotkey) | `src/aura/voice/dictation_listener.rs` | nessuno — rdev → broadcast socket.io | `useDictationHotkey`, `OverlayApp` |
| C | **WebSocket streaming** (parziali real-time) | `src/aura/inference/voice/streaming.rs`, route `/ws/dictation` (`src/core/jsonrpc/router.rs:38`) | in-process whisper-rs su finestra scorrevole 15s | **NESSUNO — endpoint morto** |
| D | **Factory STT dispatch** (MicComposer) | RPC `aura.voice_stt_dispatch` → `src/aura/voice/factory.rs` | cloud (backend proxy) **o** whisper-cli, scelto da `config.local_ai.stt_provider` | `MicComposer.tsx` via `transcribeWithFactory` |
| E | **Meeting recorder** (Tauri shell) | `app/src-tauri/src/meeting_recorder/whisper_engine.rs` | whisper-rs **CPU-only, Windows-only**, modello `ggml-base.bin` hardcoded | meeting recorder |

Esistono inoltre RPC a semantica sovrapposta: `aura.voice_transcribe` (local-only),
`aura.voice_transcribe_bytes` (local-only), `aura.voice_cloud_transcribe` (cloud-only),
`aura.voice_stt_dispatch` (factory). Il renderer usa D; la pipeline A usa
`voice_transcribe_bytes`. L'utente non può capire quale via sta usando.

### 1.2 Tre meccanismi di hotkey globale

1. `src/aura/voice/hotkey.rs` (rdev) usato sia dal voice server (A) sia dal dictation listener (B) — con mutua esclusione fragile in `src/aura/credentials/ops.rs:35-41`.
2. Tauri global-shortcut: `register_dictation_hotkey` in `app/src-tauri/src/lib.rs:886` — **nessun call site nel frontend** (`registerDictationHotkey` in `app/src/utils/tauriCommands/voice.ts:180` non è mai invocata). API morta.
3. Spacebar nel renderer (`MicComposer.tsx:137`) — funziona solo quando il renderer ha il focus.

### 1.3 Due sezioni di config per la stessa feature

- `[dictation]` (`src/aura/config/schema/dictation.rs`): `enabled`, `hotkey`, `activation_mode`, `llm_refinement`, `streaming`, `streaming_interval_ms`. Default: `enabled=false`, `streaming=true`.
- `[voice_server]` (`src/aura/config/schema/voice_server.rs`): `auto_start`, `hotkey`, `activation_mode`, `skip_cleanup`, `min_duration_secs`, `silence_threshold`, `custom_dictionary`. Default: `auto_start=false`.

`VoicePanel.tsx` scrive solo `[voice_server]`; `useDictationHotkey` legge solo `[dictation]`.
La feature è comandata da due manopole che non si parlano.

---

## 2. Problemi riscontrati (con evidenze)

### P1 — La dettatura globale ignora la scelta del provider STT (BLOCCANTE)
`voice::ops::voice_transcribe_bytes` (`src/aura/voice/ops.rs:142`) chiama sempre
`local_ai::global(config).transcribe_with_prompt`, che fallisce con
`"local ai is disabled"` se `local_ai.runtime_enabled = false`
(`src/aura/inference/local/service/speech.rs:92-94`). Anche con runtime abilitato,
se il modello whisper non è installato l'utente con provider "cloud" configurato
si becca comunque l'errore del modello mancante. **MicComposer rispetta
`stt_provider=cloud`; la hotkey globale no.** Esperienza incoerente per design.

### P2 — Su Windows/Linux la trascrizione viene incollata nel posto sbagliato (BLOCCANTE)
`capture_expected_app_name()` è macOS-only: fuori da macOS restituisce sempre `None`
(`src/aura/voice/server.rs:682-685`), quindi `is_self` è sempre `false`
(`server.rs:874-877`) e il risultato va **sempre** in `text_input::insert_text`
(paste Ctrl+V sul focus esterno). Il percorso socket.io
(`dictation:transcription` → `Conversations.tsx:356`) è quindi **de facto macOS-only**.
Su Windows — la piattaforma principale del progetto — dettare mentre Aura è aperta
incolla il testo nell'app dietro Aura (o nel vuoto), mai nella chat.

### P3 — Endpoint streaming real-time morto (e difettoso anche se usato)
Route `/ws/dictation` registrata in `src/core/jsonrpc/router.rs:38`; **nessun
client WebSocket la usa** (unico `new WebSocket` in `app/src` è
`MascotFrameProducer.tsx:177`, per i frame del mascot). Inoltre, anche se lo usasse:
- sliding window di 15s con partial = trascrizione dell'intera finestra ogni
  `max(interval_ms, 500)` → costo O(n) crescente a ogni tick, con lock globale
  del motore (`whisper_engine.rs:141` `handle.lock()` durante tutta l'inferenza):
  ogni partial blocca tutte le altre trascrizioni del processo;
- nessun testo "committed": i partial si sovrascrivono integralmente → testo che
  "balla" in UI, impossibile da renderere stabile senza diff lato client;
- `config.dictation.streaming=true` di default ma senza effetto alcuno.

### P4 — Due listener rdev e configurazione fragile
`start_login_gated_services` avvia il voice server se `voice_server.auto_start`,
altrimenti il dictation listener (`src/aura/credentials/ops.rs:35-41`). Il voice
server re-inoltra gli eventi al dictation bus (`server.rs:232-251`) per mantenere
una sola sorgente. È corretto solo se le due config restano disgiunte; un utente
che abilita `dictation.enabled` **e** `voice_server.auto_start` con hotkey diversi
ottiene due listener globali su combinazioni diverse con semantiche diverse
(push vs toggle). Nessun guard centralizzato, nessun errore di configurazione esposto.

### P5 — `insert_text` sincrono dentro il runtime async + clipboard race
`text_input::insert_text` fa `std::thread::sleep(120ms)` e simula la tastiera con
enigo, chiamato da `process_recording_bg` (task async): **blocca il thread dell'executor
del core** a ogni dettatura. In più la clipboard viene salvata e ripristinata dopo
450ms — se l'utente copia qualcosa nel frattempo, la sua clipboard viene sovrascritta
dal restore (race nota ma non gestita).

### P6 — Qualità audio: doppia cattura con due barre qualitative
- Pipeline A (core): cpal → mono → resample **lineare** 48k→16k
  (`audio_capture.rs:517-535`, dichiarato "good enough, not audiophile") + SilenceGate
  con soglia RMS fissa 0.002 hardcoded (`audio_capture.rs:22`), non collegata a
  `config.voice_server.silence_threshold`.
- Pipeline D (renderer): MediaRecorder con echoCancellation/noiseSuppression/autoGain
  attivi (`MicComposer.tsx:241-249`) e blob nativo — qualità nettamente superiore.
Chi usa la hotkey globale ottiene audio peggiore di chi usa il microfono in app.

### P7 — Commenti e documentazione contraddittori (segnali archeologici)
- `MicComposer.tsx:16-23`: "AAC-in-MP4 is preferred because the hosted STT rejected
  Opus-in-WebM" vs `MicComposer.tsx:383-388`: "Send the recorder's native blob first
  (Opus-in-WebM) — Scribe accepts it natively". I due commenti si contraddicono;
  l'uno o l'altro descrive una realtà superata.
- AGENTS.md non elenca i domini `inference` e `meetings` (esistono e sono centrali).
- `DictationHotkeyManager.tsx` gestisce `toggleCount` ma ci fa solo un
  `console.debug` — resto di una UI mai costruita.

### P8 — Tre connessioni socket.io parallele
`useDictationHotkey` apre una socket dedicata (`reconnectionAttempts: Infinity`,
`forceNew: true`), `OverlayApp` ne apre una seconda identica, `socketService` una terza.
Triplice connessione al core per eventi che potrebbero viaggiare su una sola.

### P9 — Post-processing asimmetrico
`whisper-guard` (pulizia segmenti) viene applicato **solo** nel motore in-process
(`whisper_engine.rs:284`), non nella via whisper-cli né cloud. Risultati diversi a
seconda del provider attivo, con la UI che non lo segnala.

### P10 — Default disabilitati + UI che promette il contrario
`dictation.enabled=false` e `voice_server.auto_start=false` di default: fuori dalla
scatola la dettatura globale non esiste. `VoicePanel.tsx:972` mostra "Voice dictation
is disabled until the local STT model is downloaded" — legando la feature alla sola
via locale (vedi P1) anche quando l'utente ha scelto il cloud.

---

## 3. Obiettivo di destinazione

**Una sola pipeline di dettatura**, con **una sola voce di config**, che:
1. rispetta `stt_provider` (cloud/whisper) ovunque — hotkey globale inclusa;
2. mostra la trascrizione **in tempo reale** (parziali stabili, testo committed)
   nella finestra Aura, con possibilità di editare prima dell'invio;
3. incolla nel focus esterno **solo** quando il focus è davvero esterno (tutte le
   piattaforme, non solo macOS);
4. unifica MicComposer, hotkey globale e overlay su un unico protocollo di eventi;
5. elimina codice morto: endpoint `/ws/dictation` o viene adottato dalla UI o va rimosso;
   `register_dictation_hotkey` Tauri va rimosso (nessun caller).

---

## 4. Piano di intervento

### Fase 0 — Strumentazione (mezzo giorno)
- Aggiungere metriche/timing strutturati a ogni stadio della pipeline A
  (già parzialmente presenti in `server.rs`, estendere a capture, gate, transcribe,
  cleanup, deliver) con prefisso unico `[dictation]`.
- Test E2E di riferimento prima di toccare codice: spec WDIO "dictation flow"
  che registra, trascrive e verifica la consegna (oggi assente).

### Fase 1 — Consolidamento config e RPC (1 giorno)
1. **Config unica**: deprecare `[dictation]` in lettura con migrazione one-shot verso
   `[voice_server]` (campi: `enabled` ← `dictation.enabled`, `streaming*` spostati,
   `llm_refinement` unificato con `skip_cleanup` in un unico flag `postprocess`).
   Mantenere i vecchi campi in `serde(default)` per backward-compat dei file esistenti.
2. **RPC unico**: `aura.voice_transcribe_bytes` diventa un wrapper della factory
   (`factory::create_stt_provider` con `config.local_ai.stt_provider`), risolvendo P1.
   `aura.voice_transcribe` e `aura.voice_cloud_transcribe` restano come alias
   documentati (deprecati in schema) per non rompere il renderer fino alla Fase 4.
3. **Guard configurazione**: se `enabled=true` e `hotkey` vuoto o non parsabile,
   esporre l'errore in `aura.voice_server_status.last_error` e nel pannello
   (oggi log-only).

### Fase 2 — Fix delivery cross-platform (1-2 giorni)
1. Implementare `capture_expected_app_name()` su Windows (finestra foreground via
   `GetForegroundWindow` + `GetWindowText`) e Linux/X11 (`xdo`/`xprop`), dietro
   feature-flag con fallback al comportamento attuale.
2. Su "self focus": consegnare **sempre** via socket.io indipendentemente dalla
   piattaforma; il paste esterno scatta solo quando il focus è confermato esterno.
3. Rendere `insert_text` non bloccante: spostare il paste in `tokio::task::spawn_blocking`
   e serializzare con una `Mutex` dedicata (evita paste sovrapposte su dettature
   ravvicinate). Gestire la clipboard race: ripristinare solo se la clipboard
   attuale è ancora il testo che abbiamo incollato (confronto prima del restore).
4. Agganciare la soglia del SilenceGate a `config.voice_server.silence_threshold`.

### Fase 3 — Streaming real-time che funziona (2-3 giorni)
**Decisione architetturale raccomandata**: resuscitare `/ws/dictation` invece di
buttarlo, ma con protocollo a commitment:
- Il server mantiene testo **committed** + delta di partial. Ogni tick di inferenza
  lavora solo sul **nuovo audio** dopo l'ultimo commit, con `initial_prompt` = testo
  committed (whisper lo supporta nativamente): costo O(chunk) invece di O(finestra),
  niente testo che balla.
- VAD sul client (renderer) per segmentare e ridurre invii: il renderer ha già
  `getUserMedia` + AudioWorklet disponibili; invia chunk PCM16 solo quando il VAD
  segna speech.
- Lock del motore whisper già esistente (`handle.lock()`): introducere una coda
  FIFO di inferenze con priorità "final" > "partial" così lo stop dell'utente non
  aspetta un partial in corso.
- Fallback: se `stt_provider=cloud`, gli stessi eventi viaggiano su socket.io
  (chunk base64 → `aura.voice_cloud_transcribe` per chunk) con lo stesso schema di
  eventi lato client: **un solo consumatore UI**, due transport.

### Fase 4 — Unificazione frontend (2-3 giorni)
1. **Un solo hook `useDictation`** che espone `{ state, partialText, finalText,
   error, start, stop }` e si alimenta dagli stessi eventi socket sia che la
   registrazione parta dal MicComposer, dalla hotkey globale o dall'overlay.
2. **Un solo componente `VoiceComposer`** usato da HumanPage e Conversations:
   durante la registrazione mostra il partial live; allo stop mostra il testo
   finale **editabile** in un mini-composer con Invio per inviare / Esc per
   scartare (oggi `onSubmit` spara il messaggio immediatamente — nessuna chance
   di correzione, e un errore di trascrizione diventa un messaggio inviato).
3. Eliminare `DictationHotkeyManager` (console.debug-only) e fondere la sua logica
   nello hook unico. La socket dedicata di `useDictationHotkey` viene sostituita
   dagli eventi sul socketService esistente (il core emette già broadcast senza auth).
4. Overlay: sostituire il placeholder statico con gli stessi partial live
   (bubble che si aggiorna a ogni `partial`).

### Fase 5 — Restyling UI (2 giorni) — *la modifica estetica richiesta*

Obiettivo: un'esperienza coerente col design system (ocean `#4A83DD` primary, coral
semantico, Inter/Cabinet Grotesk, raggi/ombre da `app/tailwind.config.js`).

1. **`VoiceComposer` (nuovo, sostituisce il bottone microfono in `MicComposer`)**
   - Stato idle: pill con icona mic 16px, ring ocean a riposo.
   - Stato recording: anello pulse coral (`animate-ping` già presente, ma ridisegnato:
     anello esterno espandente + waveform live). **Waveform**: barre verticali 3px
     animate da `AudioWorklet` RMS (dati già disponibili per il VAD — riuso, zero costo).
   - Stato transcribing: la pill si espande in **mini-composer** (textarea con il
     testo, bottone invio ocean, bottone scarta ghost). Il testo si può correggere.
   - Tipografia: Cabinet Grotesk per il testo della trascrizione, Inter per le label;
     label di stato maiuscole 11px tracking-wide, colore sage per "listening",
     amber per "transcribing".
2. **Badge hotkey**: sotto il composer, kbd-style chip (JetBrains Mono) che mostra
   la hotkey configurata (`Fn`, `Ctrl+Space`, …) così l'utente la impara. Oggi la
   hotkey è invisibile in UI.
3. **Overlay bubble** (`OverlayApp.tsx`): bubble floating con sfondo blur ocean-950/70,
   testo partial in bianco, puntini animati durante l'ascolto; al `final` il testo
   resta 2.5s poi fade-out (sostituisce le virgolette hardcoded attuali).
4. **`VoicePanel` (Settings)**: riorganizzare in 3 card — "Provider" (cloud/whisper
   + stato modello), "Global hotkey" (hotkey, push/toggle, test button che fa un
   beep di prova), "Quality" (soglia silenzio, dizionario, post-processing). Oggi
   è un form lungo 1029 righe piatto.
5. **Stati vuoti/errori**: empty state con illustrazione mascot che parla +
   CTA "Enable dictation" che accende `enabled` e registra la hotkey in un click
   (oggi l'utente deve indovinare la combinazione di toggle).

Deliverable visivo: storybook/vitest snapshot per i 4 stati del composer
(aligned con i test esistenti `MicComposer.test.tsx`).

### Fase 6 — Pulizia archeologica (1 giorno)
- Rimozione `register_dictation_hotkey`/`unregister_dictation_hotkey` (Tauri) e
  `expand_dictation_shortcuts` — zero caller.
- Rimozione o adozione definitiva di `/ws/dictation` (dipende dalla Fase 3).
- Valutazione whisper-engine Tauri meeting recorder (E): accettare come special-case
  documentato (modello dedicato piccolo per i meeting) **ma** spostare il download
  del modello sullo stesso resolver di `inference::paths` per non avere due directory
  modelli.
- Unificare whisper-guard su tutte e tre le vie (in-process, CLI, cloud) applicando
  la pulizia sul testo grezzo in `postprocess` invece che dentro il motore.
- Aggiornare AGENTS.md (aggiungere domini `inference` e `meetings` all'elenco) e
  `gitbooks/developing/architecture.md` con la pipeline unificata.

---

## 5. Verifica

- **Rust**: `pnpm test:rust` (con mock backend) + `cargo test -p aura voice::`
  (hotkey, silence gate, state machine, config migration).
- **Frontend**: `pnpm debug unit` sui nuovi hook/componenti; copertura ≥80% sulle
  righe modificate (merge gate).
- **E2E**: nuovo spec `app/test/e2e/specs/dictation-flow.spec.ts` — hotkey →
  recording → partial visibile → final editabile → invio → messaggio in thread;
  più variante "focus esterno" con mock di foreground app.
- **Manuale cross-platform**: Windows (focus esterno Ctrl+V), macOS (globe key +
  socket.io path), Linux X11.
- Gate pre-PR: `pnpm typecheck`, `pnpm lint`, `pnpm format:check`, `cargo check`
  (GGML_NATIVE=OFF su macOS Tahoe).

---

## 6. Rischi e decisioni aperte

| Rischio | Mitigazione |
|---|---|
| Resurrezione di `/ws/dictation` costa più del previsto | Piano B: streaming via socket.io chunk+RPC (Fase 3 lo prevede come transport alternativo) |
| Focus detection su Windows/Linux richiede permessi/API native diverse | Feature-flag `dictation.detect_focus`; fallback = sempre socket.io (più sicuro del paste cieco attuale) |
| Migrazione config può rompere installazioni esistenti | `serde(default)` + test di caricamento con file TOML vecchi (`load_tests.rs`) |
| Unificazione RPC tocca il renderer in punti non coperti dai test | Alias deprecati mantenuti per un ciclo di release |

**Decisione da prendere prima della Fase 3** (richiede input prodotto): il
trasporto streaming è WebSocket dedicato (più pulito, già esistente) oppure
socket.io (una sola connessione, riuso auth)? Raccomandazione: **socket.io**,
dato che P8 vuole ridurre le connessioni parallele.

---

## 7. Stato di implementazione (2026-10-29, branch `fix/transcription-pipeline`)

Implementato in questa passata:

- **P1** — `aura.voice_transcribe` / `aura.voice_transcribe_bytes` passano dalla STT
  factory e rispettano `config.local_ai.stt_provider` (cloud/whisper); la via whisper
  tenta prima il motore in-process whisper-rs e ricade su whisper-cli
  (`src/aura/voice/ops.rs`, `src/aura/inference/voice/local_transcribe.rs`).
- **P2** — rilevamento foreground su Windows via FFI user32/kernel32: se il processo
  proprietario ha il focus, la trascrizione viene consegnata via Socket.IO invece di
  essere incollata nella finestra esterna (`src/aura/voice/focus.rs`, `server.rs`).
- **P5** — paste eseguito in `spawn_blocking` (non blocca più l'executor del core) e
  race sulla clipboard risolta: il restore avviene solo se la clipboard è ancora il
  testo incollato (`src/aura/voice/server.rs`, `text_input.rs`).
- **P6** — la soglia del SilenceGate in cattura arriva da `config.voice_server.silence_threshold`
  (`src/aura/voice/audio_capture.rs`).
- **Fase 5 (restyling parziale)** — `MicComposer` con stato **review**: la trascrizione
  è editabile (textarea auto-focusata) prima dell'invio; Enter conferma, Esc scarta,
  Send/Discard in UI; nuove chiavi i18n propagate a tutti i cataloghi.
- **Cleanup** — rimosso il meccanismo hotkey Tauri morto (`register_dictation_hotkey`,
  `unregister_dictation_hotkey`, plugin `tauri-plugin-global-shortcut`, permessi,
  esport TS); `DictationHotkeyManager` resta perché è l'unico mount point di
  `useDictationHotkey` (ponte `dictation://insert-text` verso Conversations).

Verifiche eseguite: `cargo check` (core + Tauri shell), `cargo test --lib voice`,
`pnpm typecheck`, ESLint/Prettier sui file toccati, Vitest `features/human` (224),
cataloghi i18n (50), `App.boot`.

**Ancora aperto** (richiede la decisione prodotto sulla Fase 3): trasporto streaming
parziale (WebSocket dedicato vs socket.io), unificazione config `[dictation]`/`[voice_server]`,
copia UI che lega ancora la dettatura al solo modello locale (`VoicePanel.tsx:972`),
waveform live nel composer, rilevamento focus su Linux/X11.
