import * as RadixTooltip from '@radix-ui/react-tooltip';
import type { ReactNode } from 'react';

/**
 * Tooltip on Radix Tooltip — hover/focus label for icon buttons and
 * truncated values. Wrap the app root once with TooltipProvider (done in
 * AppShell when the kit is adopted app-wide).
 */
export interface TooltipProps {
  content: ReactNode;
  children: ReactNode;
  side?: 'top' | 'right' | 'bottom' | 'left';
}

const Tooltip = (props: TooltipProps) => {
  const { content, children, side = 'top' } = props;
  return (
    <RadixTooltip.Root>
      <RadixTooltip.Trigger asChild>{children}</RadixTooltip.Trigger>
      <RadixTooltip.Portal>
        <RadixTooltip.Content
          side={side}
          sideOffset={6}
          className={
            'z-toast max-w-xs rounded-md border border-line-default bg-surface-base px-2 py-1 ' +
            'text-xs text-ink-primary shadow-medium'
          }>
          {content}
          <RadixTooltip.Arrow className="fill-line-default" />
        </RadixTooltip.Content>
      </RadixTooltip.Portal>
    </RadixTooltip.Root>
  );
};

export default Tooltip;
