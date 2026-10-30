# Proposta di redesign UI — Aura Desktop App

> Analisi completa dello stato attuale della UI (`app/src/`) e proposta di
> rinnovo: design system unificato, UI kit su Radix, migrazione per fasi.
> La proposta è **evolutiva, non rivoluzionaria**: il linguaggio visivo esistente
> (chiaro, calmo, premium) resta; cambia il modo in cui è implementato.

---

## 1. Diagnosi dello stato attuale

### 1.1 I token esistono, ma non comandano

`app/tailwind.config.js` ha buone fondamenta — scale `neutral`, `canvas`,
`primary` (blu Figma `#2F6EF4`), `sage`/`amber`/`coral`, font stack, `shadow-soft`.
Problemi riscontrati:

| Problema | Evidenza |
|---|---|
| **Tre palette grayscale parallele** in uso: `stone-*` (default Tailwind), `neutral-*`, `canvas-*` | `BottomTabBar` usa `stone-200/300`; `Welcome` usa `bg-white border-stone-200`; altri usano `neutral-0/900` — lo stesso concetto ("surface") ha 3 nomi diversi |
| **Cabinet Grotesk dichiarato mai caricato** | `fontFamily.display` lo referenzia, ma `index.css` importa solo Inter + JetBrains Mono da Google Fonts (Cabinet Grotesk sta su Fontshare, non su Google) → fallback silenzioso a Inter |
| **Dark mode configurata ma inesistente** | `darkMode: 'class'` nel config, **zero** occorrenze `dark:` in tutto `app/src` |
| **Token semantici solo per la ⌘K palette** | `cmd-surface/foreground/border` come CSS var; il commento nel config dice testualmente che il full reskin "è una decisione separata" — mai presa |
| **Docs vs codice divergenti** | AGENTS.md descrive primary "ocean `#4A83DD`"; il codice ha `#2F6EF4` "Complementary Blue (Figma)" |

### 1.2 Il design system c'è. Su carta. Adozione ~0%

- `app/src/components/ui/` contiene **un solo primitivo**: `Button.tsx` (ben fatto:
  varianti primary/secondary/ghost/danger, 5 size, focus ring).
- **`Button` è usato da 2 file su ~450**: `grep` conta **453 `<button>` raw**
  con stili inline ripetuti a mano (`bg-white rounded-lg shadow-soft…` copiato
  ovunque) e **4.207 stringhe `className=`** sparse in ~380 file tsx.
- **Icone senza sistema**: 7 SVG Heroicons disegnati a mano dentro `BottomTabBar.tsx`,
  altri in `MicComposer.tsx`; `react-icons` usato in 4 file — nessuna libreria
  canonica, stroke weight e viewBox incoerenti.
- **Modali**: Radix Dialog in 1 file; altrove `createPortal` artigianale
  (`VoiceSetupModal`, `OpenhumanLinkModal`) con overlay/gestione focus homemade.
- **Nessun sistema di toast/snackbar** — gli errori viaggiano su banner inline
  (`chatSendError`) ricostruiti a ogni call site.
- **Nessuna libreria di animazione** — solo `animate-ping/spin` Tailwind e keyframes
  CSS una tantum; le transizioni di pagina non esistono.
- ** layering ad-hoc**: 53 occorrenze `fixed`, 9 `z-[…]` custom, nessuna scala z.

### 1.3 Struttura

- **Navigazione**: `BottomTabBar` floating-pill con label espansi all'hover —
  pattern buono e distintivo, ma hardcoded (logica `isActive` con casi speciali
  per path, radii `rounded-sm` su claim "premium", badge notifiche artigianale).
- **Settings**: 31 panel in `components/settings/panels/`, form lunghi e piatti
  (`VoicePanel.tsx` = 1.029 righe di form unico), nessun raggruppamento visivo.
- **Chat**: `Conversations.tsx` = **1.737 righe** in un unico componente
  (lista messaggi + composer + thread + tool timeline + errori + dictation).
- **Pagine media**: Home 299, Intelligence 270, Skills 993 — tutte con card
  `bg-white rounded-2xl shadow-soft` reimplementate a mano ogni volta.

### 1.4 Sintomi d'insieme

1. **Incoerenza visiva strutturale**: stessa app, stesso schermo, 3 vocabolari
   cromatici e 5 raggi di bordo diversi.
2. **Costo di manutenzione**: ogni modifica richiede editing di classi sparse;
   nessun punto singolo per cambiare "come appare un bottone".
3. **Inaccessibilità di fatto**: i 453 raw button non hanno focus ring
   (`Button` sì) — la keyboard navigation è a geometria variabile.
4. **Onboarding visivo debole**: Welcome è una card bianca su grigio con uno
   spinner HTML artigianale; per un'app "premium calm" il primo contatto è il
   meno curato.

---

## 2. La proposta: "Aura Design System"

Tre livelli, in ordine di dipendenza. Ogni livello è shippabile di per sé.

### Livello 1 — Token layer (fondamenta, ~2-3 giorni)

**Un vocabolario semantico unico**, implementato come CSS variables con mappa
Tailwind. Fine delle palette parallele:

```css
:root {
  /* Surface (uno solo — depreca stone/canvas/neutral a favore dei semantic) */
  --surface-app:        #F5F5F5;   /* canvas-100: sfondo app */
  --surface-base:       #FFFFFF;   /* card, sheet */
  --surface-raised:     #FAFAFA;   /* hover, inset */
  --surface-sunken:     #EFEFEF;   /* well, input bg */
  /* Border */
  --border-default:     #E5E5E5;
  --border-strong:      #D4D4D4;
  /* Text */
  --text-primary:       #171717;
  --text-secondary:     #525252;
  --text-tertiary:      #737373;
  /* Brand + semantic (invariati) */
  --brand:              #2F6EF4;   /* primary-500 */
  --success: #34C759;  --warning: #F59E0B;  --danger: #F97066;
  /* Radius */
  --radius-sm: 8px;  --radius-md: 12px;  --radius-lg: 16px;  --radius-xl: 24px;
  /* Elevation */
  --shadow-rest:   0 1px 2px rgb(0 0 0 / 0.04);
  --shadow-raised: 0 4px 16px -4px rgb(0 0 0 / 0.08);  /* ex shadow-soft */
  --shadow-overlay: 0 12px 40px -8px rgb(0 0 0 / 0.18);
  /* Z-scale */
  --z-nav: 50;  --z-sticky: 40;  --z-overlay: 80;  --z-modal: 90;  --z-toast: 100;
}
```

- Tailwind mappa i token: `bg-surface-base`, `text-secondary`, `border-default`,
  `rounded-md`→`var(--radius-md)` (override theme), `shadow-raised`.
- **Font**: caricare Cabinet Grotesk da Fontshare (`@import url(fontshare)`)
  o rimuovere il token `display` — niente fallback silenziosi.
- **Type scale** esplicita: `text-caption(11) / text-ui(13) / text-body(14) /
  text-subhead(16) / text-title(20) / text-display(28)` come utility custom.
- **Dark-ready**: variabili HSL `h s% l%` → dark via `.dark` senza toccare i
  componenti (decisione prodotto successiva, ma il layer non blocca).
- ESLint `no-restricted-syntax` su `stone-*` per i file nuovi (regola warn,
  migrazione progressiva).

### Livello 2 — UI kit (il cuore, ~1 settimana)

**Componenti in `components/ui/` costruiti su Radix** (già dipendenza, a11y
gratis: focus trap, aria, dismissable layer) con `cva` + `tailwind-merge` per
le varianti. "Oggetti migliori", concretamente:

| Componente | Costruito su | Sostituisce |
|---|---|---|
| `Button` (esteso) | esistente | + `loading` prop (spinner integrato), `iconOnly`, asChild |
| `IconButton` | Button | i 60+ bottoni-icona ad-hoc |
| `Icon` | **lucide-react** (unica libreria) | SVG inline + react-icons |
| `Badge` / `StatusDot` | — | badge notifiche/notion-style artigianali |
| `Card` (`Surface`) | — | le centinaia di `bg-white rounded-2xl shadow-soft` |
| `Input` / `Textarea` | — | input raw, con error state + focus ring unico |
| `Select` | Radix Select | le `<select>` native (es. selettore microfono) |
| `Switch` / `Slider` | Radix | toggle custom sparsi nei settings |
| `Dialog` / `Sheet` | Radix Dialog | createPortal artigianale |
| `Tooltip` / `Popover` | Radix | title= nativi mai usati |
| `Toast` | Radix Toast (o sonner) | banner errore ricostruiti a mano |
| `Tabs` / `SegmentedControl` | Radix Tabs | tab switcher homemade |
| `Skeleton` | — | spinner-only loading |
| `EmptyState` | — | empty state inventati di pagina in pagina |
| `Kbd` | — | scorciatoie testuali |
| `PageHeader` | — | header pagina (titolo+azioni+breadcrumb) |
| `Field` (label+control+hint+error) | — | form layout ripetuto |

Più due utility: `cn()` (clsx+tailwind-merge) e hook `useReducedMotion`
rispettato da tutte le animazioni.

**Animazioni**: CSS-first (transizioni su token `duration-fast 150ms /
base 250ms`, easing unico). Micro-interazioni mirate via keyframes esistenti;
entrata dialog/toast con slide+fade; **indicatore navigazione animato**
(indicatore pill che scorre con `transform`, non ricostruzione DOM).

### Livello 3 — Schermate (~2-3 settimane, per area)

1. **AppShell + navigazione**: `BottomTabBar` → `NavBar` (stesso pattern
   floating-pill — è buono — ma con token, icone lucide, indicatore attivo
   animato, badge via `Badge`). `AppShell` con container a larghezza unica e
   `PageHeader` per ogni pagina. Transizioni route fade/slide 150ms.
2. **Settings**: sidebar con sezioni raggruppate
   *(Profilo · AI e modelli · Connessioni · Automazioni · Team · Avanzate)*
   con `SettingsNav`; ogni panel riscritto su `Card` sezionata + `Field`;
   VoicePanel → 3 card (Provider / Hotkey / Qualità) come da piano trascrizione.
3. **Chat**: il monolite `Conversations.tsx` (1.737 righe) smontato in
   `features/chat/` — `MessageList`, `MessageBubble` (user: tint brand, radius
   asimmetrico; assistant: `Surface` piatto), `Composer` unico mult modalità
   (testo ⇄ voce con la review card già costruita), `ThreadRail`, `ToolTimeline`
   su `Accordion`. Streaming con skeleton + cursore.
4. **Welcome/Onboarding**: card centrata su MeshGradient (già presente) con
   blur, logo, bottoni OAuth a riga intera con icona provider, `Skeleton` al
   posto dello spinner artigianale.
5. **Home/Intelligence**: banner → `Card` con `Icon`+titolo+azione; empty state
   con mascot + CTA (`EmptyState`).
6. **Skills**: card grid su `Card`, barra ricerca/filtri (cmdk pattern già in
   casa), `StatusDot` per lo stato connessione.

### Livello 4 — Governance (permanente)

- ESLint: divieto `raw <button>`/`stone-*` fuori dal kit (regola `no-restricted-syntax`,
  warn → error per fasi).
- **Codemod di migrazione**: i 453 `<button>` sono meccanici — script di
  trasformazione (jscodeshift) che mappa le classi più comuni alle varianti
  Button; il resto a mano area per area.
- Una pagina `/design-system` in dev-only che renderizza ogni variante del kit
  (poor-man's Storybook, zero infrastruttura nuova).
- Docs: `gitbooks/developing/design-system.md` + aggiornamento AGENTS.md.

---

## 3. Roadmap di migrazione (strangler, niente big-bang)

| Fase | Contenuto | Risultato visibile |
|---|---|---|
| 0 | Token layer + `cn()` + font fix | Nulla di visibile, tutto di strutturale |
| 1 | UI kit completo + pagina `/design-system` | Oggetti pronti, non ancora usati |
| 2 | NavBar + AppShell + Welcome | Prima impronta visiva nuova |
| 3 | Settings (31 panel → 6 sezioni) | La superficie più grande e più piatta |
| 4 | Chat (smontaggio monolite + composer unico) | Cuore del prodotto |
| 5 | Home, Intelligence, Skills, Meetings, resto | Completezza |
| 6 | ESLint strict + pulizia `stone-*` e SVG inline | Lock del guadagno |

Ogni fase = 1 PR indipendente, testabile, con copertura ≥80% sulle righe
cambiate (merge gate esistente).

## 4. Dipendenze da aggiungere

- `lucide-react` (icone) — rimozione graduale `react-icons`
- `class-variance-authority` + `clsx` + `tailwind-merge` (varianti)
- `@radix-ui/react-select`, `react-switch`, `react-slider`, `react-tooltip`,
  `react-popover`, `react-tabs`, `react-toast` — estensione del Radix già in uso

Niente framer-motion (CSS-first basta), niente tailwind plugin esterni.

## 5. Rischi

| Rischio | Mitigazione |
|---|---|
| 25 cataloghi i18n toccati dalle stringhe nuove | Chiavi nuove via `translate-catalogs.mjs` come già fatto |
| E2E WDIO legati a selettori/classi | Migrare via `data-testid`/role, non classi; i helper `element-helpers.ts` già astraggono |
| La review di 450 bottoni in PR è ingestibile | Codemod in PR separata (fase 1), review per campionamento |
| Gusto soggettivo sul "moderno" | Fase 2 (Welcome+Nav) come **prototipo visivo**: si approva lì prima di proseguire |

---

## 6. Stato di implementazione (2026-10-30, branch `feat/ui-design-system`)

**Fase 0 + Fase 1 implementate:**

- Token layer: CSS variables semantiche in `index.css` (`--surface-*`,
  `--ink-*`, `--line-*`, dark-ready), scale Tailwind `surface/ink/line`,
  scala z (`z-nav/overlay/modal/toast`), Cabinet Grotesk caricato da Fontshare
  (era dichiarato mai caricato). Deviazioni dal documento: raggi e ombre NON
  ridichiarati — la config aveva già scale complete (`rounded-xs…4xl`,
  `shadow-soft…float`); il kit le riusa.
- UI kit in `components/ui/`: Button (esteso con `loading`), IconButton, Icon
  (lucide), Card, Badge, StatusDot, Input/Textarea, Select, Switch, Dialog,
  Tooltip, Toast (Provider + `useToast`), Skeleton, EmptyState, Kbd,
  PageHeader, `cn()`. Costruiti su Radix (già dipendenza). Deferred a fase
  successiva: Tabs, Popover, Slider, Field.
- Pagina showcase `/design-system` (dev-only via `import.meta.env.DEV` sulla
  route; import statico per la regola no-dynamic-imports).
- Test: 22 (cn, Button, Card, Badge, Input, Toast) — verdi; tsc, ESLint
  (0 errori), Prettier verdi.

Prossimo: Fase 2 (NavBar + AppShell + Welcome) come prototipo visivo da
approvare prima di continuare.
