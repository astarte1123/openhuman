import type { ReactNode } from 'react';

import { cn } from './cn';

export interface EmptyStateProps {
  title: string;
  description?: string;
  /** Mascot/illustration slot. */
  visual?: ReactNode;
  /** Call-to-action slot (Button or link). */
  action?: ReactNode;
  className?: string;
}

/** Consistent "nothing here yet" — icon/visual, title, hint, one CTA. */
const EmptyState = (props: EmptyStateProps) => {
  const { title, description, visual, action, className } = props;
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center gap-3 px-6 py-12 text-center',
        className
      )}>
      {visual && <div className="text-ink-tertiary">{visual}</div>}
      <h3 className="font-display text-lg font-medium text-ink-primary">{title}</h3>
      {description && <p className="max-w-sm text-sm text-ink-secondary">{description}</p>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
};

export default EmptyState;
