import type { LucideIcon } from 'lucide-react';

export type IconSize = 'xs' | 'sm' | 'md' | 'lg';

const SIZES: Record<IconSize, number> = { xs: 14, sm: 16, md: 18, lg: 24 };

export interface IconProps {
  icon: LucideIcon;
  size?: IconSize;
  className?: string;
  strokeWidth?: number;
  'aria-hidden'?: boolean | 'true' | 'false';
}

/**
 * Single icon entry point over lucide-react (the one icon library).
 * Replaces the hand-drawn inline SVGs scattered through the app — one
 * stroke weight, one viewBox, one size vocabulary.
 */
const Icon = (props: IconProps) => {
  const { icon: Lucide, size = 'md', className, strokeWidth = 1.8, ...rest } = props;
  return <Lucide size={SIZES[size]} strokeWidth={strokeWidth} className={className} {...rest} />;
};

export default Icon;
