import type { ComponentProps } from 'react';

import Button from './Button';
import { cn } from './cn';

type ButtonProps = ComponentProps<typeof Button>;

export interface IconButtonProps extends Omit<ButtonProps, 'children'> {
  /** Required: icon-only buttons must be labelled for screen readers. */
  'aria-label': string;
  icon: NonNullable<ButtonProps['leadingIcon']>;
}

const SQUARE_BY_SIZE: Record<string, string> = {
  xs: 'h-6 w-6 p-0',
  sm: 'h-[30px] w-[30px] p-0',
  md: 'h-9 w-9 p-0',
  lg: 'h-11 w-11 p-0',
};

/** Square icon-only button (toolbar, card headers, composer). */
const IconButton = (props: IconButtonProps) => {
  const { icon, size = 'md', className, variant, ...rest } = props;
  return (
    <Button
      variant={variant ?? 'ghost'}
      size={size}
      leadingIcon={icon}
      className={cn(SQUARE_BY_SIZE[size], className)}
      {...rest}
    />
  );
};

export default IconButton;
