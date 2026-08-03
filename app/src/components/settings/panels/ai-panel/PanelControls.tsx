// ─────────────────────────────────────────────────────────────────────────────
// Small shared primitives used by BackgroundLoopControls: a labeled toggle
// switch, a compact metric tile, and a formula row (label + value + detail).
// ─────────────────────────────────────────────────────────────────────────────

export const LoopToggle = ({
  label,
  description,
  checked,
  busy,
  onToggle,
}: {
  label: string;
  description: string;
  checked: boolean;
  busy: boolean;
  onToggle: () => void;
}) => (
  <div className="flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-white px-3 py-2">
    <div className="min-w-0">
      <div className="text-sm font-medium text-stone-900">{label}</div>
      <div className="text-xs text-stone-500">{description}</div>
    </div>
    <button
      type="button"
      role="switch"
      aria-label={label}
      aria-checked={checked}
      disabled={busy}
      onClick={onToggle}
      className={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors disabled:cursor-wait disabled:opacity-60 ${checked ? 'bg-primary-500' : 'bg-stone-300'}`}>
      <span
        aria-hidden
        className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${checked ? 'translate-x-4' : 'translate-x-0.5'}`}
      />
    </button>
  </div>
);

export const MetricTile = ({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail?: string;
}) => (
  <div className="rounded-md bg-stone-50 px-3 py-2">
    <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-400">{label}</div>
    <div className="mt-1 text-sm font-semibold text-stone-900">{value}</div>
    {detail ? <div className="mt-0.5 text-[11px] text-stone-500">{detail}</div> : null}
  </div>
);

export const FormulaRow = ({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail: string;
}) => (
  <div className="rounded-md border border-stone-200 bg-white px-3 py-2">
    <div className="flex items-center justify-between gap-3">
      <span className="text-xs font-medium text-stone-800">{label}</span>
      <span className="font-mono text-xs text-stone-600">{value}</span>
    </div>
    <div className="mt-1 text-[11px] text-stone-500">{detail}</div>
  </div>
);
