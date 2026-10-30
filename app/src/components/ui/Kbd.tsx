import type { HTMLAttributes } from 'react';

import { cn } from './cn';

/** Keyboard shortcut chip, e.g. <Kbd>Ctrl</Kbd>+<Kbd>Space</Kbd>. */
const Kbd = (props: HTMLAttributes<HTMLElement>) => {
  const { className, ...rest } = props;
  return (
    <kbd
      className={cn(
        'inline-flex h-5 min-w-5 items-center justify-center rounded-xs border border-line-default ' +
          'bg-surface-raised px-1.5 font-mono text-xs text-ink-secondary shadow-subtle',
        className
      )}
      {...rest}
    />
  );
};

export default Kbd;
