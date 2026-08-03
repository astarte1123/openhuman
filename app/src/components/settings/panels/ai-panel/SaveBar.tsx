import { LuCheck, LuCircleAlert } from 'react-icons/lu';

// ─────────────────────────────────────────────────────────────────────────────
// Save bar (sticky)
// ─────────────────────────────────────────────────────────────────────────────

const SaveBar = ({
  diffSummary,
  changeCount,
  onSave,
  onDiscard,
}: {
  diffSummary: string[];
  changeCount: number;
  onSave: () => void;
  onDiscard: () => void;
}) => (
  <div className="pointer-events-none sticky bottom-3 z-20 flex justify-center px-4">
    <div className="pointer-events-auto flex w-full items-center gap-2 rounded-lg border border-stone-200 bg-white/95 px-3 py-2 shadow-float backdrop-blur-md animate-fade-up">
      <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded bg-amber-50 text-amber-600">
        <LuCircleAlert className="h-3.5 w-3.5" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-xs font-medium text-stone-900">
          {changeCount} unsaved change{changeCount === 1 ? '' : 's'}
        </div>
        <div className="truncate font-mono text-[10px] text-stone-500">
          {diffSummary.slice(0, 2).join(' · ')}
          {diffSummary.length > 2 ? ` · +${diffSummary.length - 2}` : ''}
        </div>
      </div>
      <button
        onClick={onDiscard}
        className="rounded-md border border-stone-200 px-2 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50">
        Discard
      </button>
      <button
        onClick={onSave}
        className="inline-flex items-center gap-1 rounded-md bg-primary-500 px-2.5 py-1 text-xs font-medium text-white hover:bg-primary-600">
        <LuCheck className="h-3 w-3" />
        Save
      </button>
    </div>
  </div>
);

export default SaveBar;
