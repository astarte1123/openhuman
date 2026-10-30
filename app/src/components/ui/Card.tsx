import { forwardRef, type HTMLAttributes } from 'react';

import { cn } from './cn';

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  /** Hover lift + pointer cursor — for clickable cards. */
  interactive?: boolean;
}

/**
 * The one card. `bg-white rounded-2xl shadow-soft border` was reimplemented
 * by hand in dozens of places; this is the canonical version, token-driven.
 */
const Card = forwardRef<HTMLDivElement, CardProps>((props, ref) => {
  const { interactive = false, className, ...rest } = props;
  return (
    <div
      ref={ref}
      className={cn(
        'rounded-xl border border-line-default bg-surface-base shadow-soft',
        interactive &&
          'cursor-pointer transition-shadow duration-200 hover:border-line-strong hover:shadow-medium',
        className
      )}
      {...rest}
    />
  );
});
Card.displayName = 'Card';

export default Card;
