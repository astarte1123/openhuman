import * as RadixSwitch from '@radix-ui/react-switch';
import { forwardRef } from 'react';

import { cn } from './cn';

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  'aria-label': string;
  className?: string;
}

/** Toggle on Radix Switch — replaces the hand-rolled toggles in settings. */
const Switch = forwardRef<HTMLButtonElement, SwitchProps>((props, ref) => {
  const { checked, onCheckedChange, disabled, className, ...rest } = props;
  return (
    <RadixSwitch.Root
      ref={ref}
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={rest['aria-label']}
      className={cn(
        'relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border ' +
          'border-line-default transition-colors duration-200 focus-visible:outline-none ' +
          'focus-visible:ring-2 focus-visible:ring-primary-500/25 disabled:cursor-not-allowed ' +
          'disabled:opacity-50 data-[state=checked]:border-primary-500 data-[state=checked]:bg-primary-500 ' +
          'data-[state=unchecked]:bg-surface-sunken',
        className
      )}>
      <RadixSwitch.Thumb
        className={cn(
          'block h-4 w-4 translate-x-0.5 rounded-full bg-surface-base shadow-subtle ' +
            'transition-transform duration-200 data-[state=checked]:translate-x-[22px]'
        )}
      />
    </RadixSwitch.Root>
  );
});
Switch.displayName = 'Switch';

export default Switch;
