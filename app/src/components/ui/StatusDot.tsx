import type { HTMLAttributes } from 'react';

import { cn } from './cn';

export type StatusDotTone = 'online' | 'busy' | 'offline' | 'warning';

const DOTS: Record<StatusDotTone, string> = {
  online: 'bg-sage-500',
  busy: 'bg-amber-500',
  warning: 'bg-amber-500',
  offline: 'bg-neutral-400',
};

export interface StatusDotProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: StatusDotTone;
}

/** Presence/status indicator — pairs with Badge for connection states. */
const StatusDot = (props: StatusDotProps) => {
  const { tone = 'online', className, ...rest } = props;
  return (
    <span
      role="status"
      className={cn('inline-block h-2 w-2 rounded-full', DOTS[tone], className)}
      {...rest}
    />
  );
};

export default StatusDot;
