import * as RadixDialog from '@radix-ui/react-dialog';
import { X as XIcon } from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from './cn';
import Icon from './Icon';

export interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
  /** Footer slot — keep to one row of actions. */
  footer?: ReactNode;
  className?: string;
}

/**
 * Modal on Radix Dialog — focus trap, Esc/outside dismiss, scrim and
 * portal included. Replaces the hand-rolled createPortal modals.
 */
const Dialog = (props: DialogProps) => {
  const { open, onOpenChange, title, description, children, footer, className } = props;
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="fixed inset-0 z-overlay bg-[var(--surface-overlay)]" />
        <div className="fixed inset-0 z-modal flex items-center justify-center p-4">
          <RadixDialog.Content
            className={cn(
              'w-full max-w-md rounded-xl border border-line-default bg-surface-base p-5 ' +
                'shadow-float focus:outline-none',
              className
            )}>
            <div className="flex items-start justify-between gap-4">
              <div>
                <RadixDialog.Title className="font-display text-lg font-medium text-ink-primary">
                  {title}
                </RadixDialog.Title>
                {description && (
                  <RadixDialog.Description className="mt-1 text-sm text-ink-secondary">
                    {description}
                  </RadixDialog.Description>
                )}
              </div>
              <RadixDialog.Close
                aria-label="Close"
                className="rounded-md p-1 text-ink-tertiary transition-colors hover:bg-surface-sunken hover:text-ink-primary">
                <Icon icon={XIcon} size="sm" />
              </RadixDialog.Close>
            </div>
            <div className="mt-4">{children}</div>
            {footer && <div className="mt-5 flex justify-end gap-2">{footer}</div>}
          </RadixDialog.Content>
        </div>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
};

export default Dialog;
