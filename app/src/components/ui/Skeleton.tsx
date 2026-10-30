import type { HTMLAttributes } from 'react';

import { cn } from './cn';

export interface SkeletonProps extends HTMLAttributes<HTMLDivElement> {
  /** Fixed size override; default fills the parent block. */
  className?: string;
}

/** Loading placeholder — the atomic building block for content skeletons. */
const Skeleton = (props: SkeletonProps) => {
  const { className, ...rest } = props;
  return (
    <div
      aria-hidden="true"
      className={cn('animate-pulse rounded-md bg-surface-sunken', className)}
      {...rest}
    />
  );
};

export default Skeleton;
