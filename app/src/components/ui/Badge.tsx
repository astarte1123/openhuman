import type { HTMLAttributes } from 'react';

import { cn } from './cn';

export type BadgeVariant = 'neutral' | 'brand' | 'success' | 'warning' | 'danger';

const VARIANTS: Record<BadgeVariant, string> = {
  neutral: 'bg-surface-sunken text-ink-secondary',
  brand: 'bg-primary-50 text-primary-700',
  success: 'bg-sage-50 text-sage-700',
  warning: 'bg-amber-50 text-amber-700',
  danger: 'bg-coral-50 text-coral-700',
};

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
}

const Badge = (props: BadgeProps) => {
  const { variant = 'neutral', className, ...rest } = props;
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium',
        VARIANTS[variant],
        className
      )}
      {...rest}
    />
  );
};

export default Badge;
