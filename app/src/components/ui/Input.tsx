import { forwardRef, type InputHTMLAttributes, type TextareaHTMLAttributes } from 'react';

import { cn } from './cn';

const CONTROL_BASE =
  'w-full rounded-lg border border-line-default bg-surface-sunken px-3 text-sm text-ink-primary ' +
  'placeholder:text-ink-tertiary transition-colors duration-150 ' +
  'focus:outline-none focus:border-primary-400 focus:ring-2 focus:ring-primary-500/15 ' +
  'disabled:opacity-50 disabled:pointer-events-none aria-[invalid=true]:border-coral-400';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Rendered below the control in coral when set. */
  error?: string;
}

const Input = forwardRef<HTMLInputElement, InputProps>((props, ref) => {
  const { error, className, ...rest } = props;
  return (
    <div className="w-full">
      <input
        ref={ref}
        aria-invalid={error ? true : undefined}
        className={cn(CONTROL_BASE, 'h-9', className)}
        {...rest}
      />
      {error && <p className="mt-1 text-xs text-coral-600">{error}</p>}
    </div>
  );
});
Input.displayName = 'Input';

export interface TextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  error?: string;
}

const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>((props, ref) => {
  const { error, className, ...rest } = props;
  return (
    <div className="w-full">
      <textarea
        ref={ref}
        aria-invalid={error ? true : undefined}
        className={cn(CONTROL_BASE, 'py-2', className)}
        {...rest}
      />
      {error && <p className="mt-1 text-xs text-coral-600">{error}</p>}
    </div>
  );
});
Textarea.displayName = 'Textarea';

export { Input, Textarea };
