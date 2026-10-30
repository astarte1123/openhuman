import { ReactNode } from 'react';

import { cn } from '../../ui/cn';

interface SettingsMenuItemProps {
  icon: ReactNode;
  title: string;
  description?: string;
  onClick?: () => void;
  dangerous?: boolean;
  isFirst?: boolean;
  isLast?: boolean;
  rightElement?: ReactNode;
}

const SettingsMenuItem = ({
  icon,
  title,
  description,
  onClick,
  dangerous = false,
  isFirst = false,
  isLast = false,
  rightElement,
}: SettingsMenuItemProps) => {
  const content = (
    <>
      <div
        className={cn(
          'mr-3 flex h-5 w-5 flex-shrink-0 items-center justify-center opacity-60',
          dangerous ? 'text-coral-600' : 'text-ink-primary'
        )}>
        {icon}
      </div>
      <div className="flex-1">
        <div
          className={cn(
            'mb-1 text-sm font-medium',
            dangerous ? 'text-coral-600' : 'text-ink-primary'
          )}>
          {title}
        </div>
        {description && <p className="text-xs text-ink-secondary">{description}</p>}
      </div>
      {rightElement && <div className="ml-3 flex-shrink-0">{rightElement}</div>}
    </>
  );

  const rowClasses = cn(
    'flex w-full items-center justify-between px-4 py-3 text-left transition-colors duration-fast ease-standard',
    !isLast && 'border-b border-line-default',
    isFirst && 'first:rounded-t-3xl',
    isLast && 'last:rounded-b-3xl',
    onClick && 'cursor-pointer hover:bg-surface-raised active:scale-[0.99]'
  );

  if (onClick) {
    return (
      <button type="button" onClick={onClick} className={rowClasses}>
        {content}
      </button>
    );
  }

  return <div className={rowClasses}>{content}</div>;
};

export default SettingsMenuItem;
