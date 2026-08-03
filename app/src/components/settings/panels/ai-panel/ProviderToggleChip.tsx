import { slugTone } from './transforms';

// ─────────────────────────────────────────────────────────────────────────────
// Cloud provider toggle chip — pill with an inline switch, used for the
// built-in cloud provider connect/disconnect affordance.
// ─────────────────────────────────────────────────────────────────────────────

const ProviderToggleChip = ({
  slug,
  label,
  enabled,
  busy,
  onToggle,
}: {
  slug: string;
  label: string;
  enabled: boolean;
  busy?: boolean;
  onToggle: () => void;
}) => {
  const tone = slugTone(slug);
  return (
    <div
      className={`inline-flex items-center gap-2 rounded-full px-2.5 py-1 text-xs font-medium ring-1 transition-colors ${tone}`}>
      <span>{label}</span>
      <button
        type="button"
        role="switch"
        aria-checked={enabled}
        aria-label={`${enabled ? 'Disconnect' : 'Connect'} ${label}`}
        disabled={busy}
        onClick={onToggle}
        className={`relative inline-flex h-4 w-7 shrink-0 items-center rounded-full transition-colors disabled:cursor-wait disabled:opacity-60 ${enabled ? 'bg-primary-500' : 'bg-stone-300'}`}>
        <span
          aria-hidden
          className={`inline-block h-3 w-3 transform rounded-full bg-white shadow transition-transform ${enabled ? 'translate-x-3.5' : 'translate-x-0.5'}`}
        />
      </button>
    </div>
  );
};

export default ProviderToggleChip;
