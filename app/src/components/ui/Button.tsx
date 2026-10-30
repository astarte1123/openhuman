import { type ButtonHTMLAttributes, forwardRef, type ReactNode } from 'react';

import { cn } from './cn';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  leadingIcon?: ReactNode;
  trailingIcon?: ReactNode;
  /** Shows a spinner, sets aria-busy and disables the button. */
  loading?: boolean;
}

const BASE =
  'inline-flex items-center justify-center gap-2 font-medium select-none ' +
  'transition-all duration-fast ease-standard ' +
  'active:scale-[0.98] ' +
  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500/25 ' +
  'focus-visible:ring-offset-2 disabled:opacity-40 disabled:pointer-events-none';

const VARIANTS: Record<ButtonVariant, string> = {
  primary: 'bg-primary-500 text-white hover:bg-primary-600 active:bg-primary-700',
  secondary: 'bg-surface-base text-ink-primary border border-line-default hover:bg-surface-raised',
  ghost: 'bg-transparent text-ink-secondary hover:bg-surface-sunken hover:text-ink-primary',
  danger: 'bg-transparent text-coral-600 border border-coral-300/50 hover:bg-coral-50',
};

const SIZES: Record<ButtonSize, string> = {
  xs: 'h-6 px-2 text-xs rounded-sm',
  sm: 'h-[30px] px-3 text-sm rounded-md',
  md: 'h-9 px-4 text-sm rounded-lg',
  lg: 'h-11 px-5 text-base rounded-lg',
  xl: 'h-14 px-7 text-base rounded-xl font-medium',
};

function Spinner() {
  return (
    <svg className="h-3.5 w-3.5 animate-spin" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
      />
    </svg>
  );
}

const Button = forwardRef<HTMLButtonElement, ButtonProps>((props, ref) => {
  const {
    variant = 'primary',
    size = 'md',
    leadingIcon,
    trailingIcon,
    loading = false,
    className,
    type,
    disabled,
    children,
    ...rest
  } = props;

  return (
    <button
      ref={ref}
      type={type ?? 'button'}
      className={cn(BASE, VARIANTS[variant], SIZES[size], className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}>
      {loading ? <Spinner /> : leadingIcon}
      {children}
      {!loading && trailingIcon}
    </button>
  );
});
Button.displayName = 'Button';

export default Button;
