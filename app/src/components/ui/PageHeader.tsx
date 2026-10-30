import type { ReactNode } from 'react';

import { cn } from './cn';

export interface PageHeaderProps {
  title: string;
  description?: string;
  /** Right-aligned action row (buttons, badges). */
  actions?: ReactNode;
  className?: string;
}

/**
 * Canonical page header — display type, optional description, action row.
 * Every page renders one; keeps title hierarchy consistent app-wide.
 */
const PageHeader = (props: PageHeaderProps) => {
  const { title, description, actions, className } = props;
  return (
    <header className={cn('flex items-start justify-between gap-4', className)}>
      <div className="min-w-0">
        <h1 className="truncate font-display text-2xl font-medium tracking-tight text-ink-primary">
          {title}
        </h1>
        {description && <p className="mt-1 text-sm text-ink-secondary">{description}</p>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </header>
  );
};

export default PageHeader;
