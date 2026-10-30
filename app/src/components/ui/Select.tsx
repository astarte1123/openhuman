import * as RadixSelect from '@radix-ui/react-select';
import { ChevronDown as ChevronDownIcon } from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from './cn';
import Icon from './Icon';

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps {
  value: string;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  /** Placeholder when no value is selected. */
  placeholder?: string;
  disabled?: boolean;
  'aria-label': string;
  className?: string;
}

/**
 * Dropdown on Radix Select — keyboard navigable, focus managed, portal'd.
 * Replaces native <select> (which cannot be styled consistently) and the
 * hand-rolled dropdowns.
 */
const Select = (props: SelectProps) => {
  const { value, onValueChange, options, placeholder, disabled, className, ...rest } = props;
  const current = options.find(o => o.value === value);

  return (
    <RadixSelect.Root value={value} onValueChange={onValueChange} disabled={disabled}>
      <RadixSelect.Trigger
        aria-label={rest['aria-label']}
        className={cn(
          'inline-flex h-9 w-full items-center justify-between gap-2 rounded-lg border border-line-default ' +
            'bg-surface-sunken px-3 text-sm text-ink-primary transition-colors duration-150 ' +
            'focus:outline-none focus:border-primary-400 focus:ring-2 focus:ring-primary-500/15 ' +
            'disabled:opacity-50 data-[placeholder]:text-ink-tertiary [&>span]:truncate',
          className
        )}>
        <RadixSelect.Value placeholder={placeholder}>{current?.label}</RadixSelect.Value>
        <RadixSelect.Icon>
          <Icon icon={ChevronDownIcon} size="sm" className="text-ink-tertiary" />
        </RadixSelect.Icon>
      </RadixSelect.Trigger>
      <RadixSelect.Portal>
        <RadixSelect.Content
          position="popper"
          sideOffset={4}
          className={cn(
            'z-modal min-w-[var(--radix-select-trigger-width)] overflow-hidden rounded-lg border ' +
              'border-line-default bg-surface-base shadow-medium'
          )}>
          <RadixSelect.Viewport className="p-1">
            {options.map(option => (
              <RadixSelect.Item
                key={option.value}
                value={option.value}
                className={cn(
                  'flex cursor-pointer items-center rounded-md px-2 py-1.5 text-sm text-ink-primary ' +
                    'outline-none select-none data-[highlighted]:bg-surface-sunken data-[state=checked]:font-medium'
                )}>
                <RadixSelect.ItemText>{option.label as ReactNode}</RadixSelect.ItemText>
              </RadixSelect.Item>
            ))}
          </RadixSelect.Viewport>
        </RadixSelect.Content>
      </RadixSelect.Portal>
    </RadixSelect.Root>
  );
};

export default Select;
