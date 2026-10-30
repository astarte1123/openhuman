import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

/**
 * Merge conditional class names, deduplicating conflicting Tailwind
 * utilities (last wins). The standard composition tool across the UI kit.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
