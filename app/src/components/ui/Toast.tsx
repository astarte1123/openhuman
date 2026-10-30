import * as RadixToast from '@radix-ui/react-toast';
import {
  AlertTriangle as AlertTriangleIcon,
  Check as CheckIcon,
  XCircle as XCircleIcon,
} from 'lucide-react';
import { createContext, type ReactNode, useCallback, useContext, useMemo, useState } from 'react';

import { cn } from './cn';
import Icon from './Icon';

export type ToastVariant = 'default' | 'success' | 'warning' | 'danger';

export interface ToastInput {
  title: string;
  description?: string;
  variant?: ToastVariant;
  /** Auto-dismiss milliseconds; 0 disables. Default 5000. */
  duration?: number;
}

interface ToastItem extends ToastInput {
  id: number;
}

interface ToastContextValue {
  toast: (input: ToastInput) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const STYLES: Record<ToastVariant, string> = {
  default: 'border-line-default',
  success: 'border-sage-300',
  warning: 'border-amber-300',
  danger: 'border-coral-300',
};

const ICONS: Record<Exclude<ToastVariant, 'default'>, typeof CheckIcon> = {
  success: CheckIcon,
  warning: AlertTriangleIcon,
  danger: XCircleIcon,
};

/**
 * App-wide toast system on Radix Toast (announcements, swipe/timeout
 * dismiss, focus safe). Mount `<ToastProvider>` once at the app root and
 * call `useToast().toast(...)` anywhere below.
 */
export function ToastProvider(props: { children: ReactNode }) {
  const { children } = props;
  const [items, setItems] = useState<ToastItem[]>([]);

  const toast = useCallback((input: ToastInput) => {
    const id = Date.now() + Math.random();
    setItems(prev => [...prev.slice(-4), { ...input, id }]);
  }, []);

  const dismiss = useCallback((id: number) => {
    setItems(prev => prev.filter(item => item.id !== id));
  }, []);

  const value = useMemo(() => ({ toast }), [toast]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      <RadixToast.Provider swipeDirection="right">
        {items.map(item => {
          const variant = item.variant ?? 'default';
          const IconCmp = variant === 'default' ? null : ICONS[variant];
          return (
            <RadixToast.Root
              key={item.id}
              duration={item.duration ?? 5000}
              onOpenChange={open => {
                if (!open) dismiss(item.id);
              }}
              className={cn(
                'flex items-start gap-2 rounded-lg border bg-surface-base px-3 py-2.5 shadow-medium ' +
                  'data-[swipe=end]:translate-x-[var(--radix-toast-swipe-end-x)] ' +
                  'data-[state=open]:animate-[toast-in_200ms_ease-out] data-[state=closed]:animate-[toast-out_150ms_ease-in]',
                STYLES[variant]
              )}>
              {IconCmp && (
                <Icon
                  icon={IconCmp}
                  size="sm"
                  className={cn(
                    'mt-0.5 shrink-0',
                    variant === 'success' && 'text-sage-600',
                    variant === 'warning' && 'text-amber-600',
                    variant === 'danger' && 'text-coral-600'
                  )}
                />
              )}
              <div className="min-w-0">
                <RadixToast.Title className="text-sm font-medium text-ink-primary">
                  {item.title}
                </RadixToast.Title>
                {item.description && (
                  <RadixToast.Description className="mt-0.5 text-xs text-ink-secondary">
                    {item.description}
                  </RadixToast.Description>
                )}
              </div>
            </RadixToast.Root>
          );
        })}
        <RadixToast.Viewport className="fixed right-4 bottom-20 z-toast flex w-80 flex-col gap-2" />
      </RadixToast.Provider>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error('useToast must be used inside <ToastProvider>');
  }
  return ctx;
}
