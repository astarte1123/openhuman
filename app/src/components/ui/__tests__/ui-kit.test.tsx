import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import Badge from '../Badge';
import Button from '../Button';
import Card from '../Card';
import { cn } from '../cn';
import { Input, Textarea } from '../Input';
import { ToastProvider, useToast } from '../Toast';

describe('cn', () => {
  it('merges conditional classes', () => {
    const includeB = false;
    expect(cn('a', includeB && 'b', 'c')).toBe('a c');
  });

  it('last conflicting Tailwind utility wins', () => {
    expect(cn('px-2', 'px-4')).toBe('px-4');
  });
});

describe('Button', () => {
  it('renders variant and size classes', () => {
    render(
      <Button variant="secondary" size="lg">
        Save
      </Button>
    );
    const btn = screen.getByRole('button', { name: 'Save' });
    expect(btn).toHaveClass('bg-surface-base');
    expect(btn).toHaveClass('h-11');
  });

  it('loading shows spinner, disables and sets aria-busy', () => {
    render(<Button loading>Send</Button>);
    const btn = screen.getByRole('button', { name: 'Send' });
    expect(btn).toBeDisabled();
    expect(btn).toHaveAttribute('aria-busy', 'true');
  });

  it('forwards click handlers', () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Go</Button>);
    fireEvent.click(screen.getByRole('button', { name: 'Go' }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});

describe('Card', () => {
  it('renders the surface token classes', () => {
    render(<Card data-testid="card">body</Card>);
    const card = screen.getByTestId('card');
    expect(card).toHaveClass('bg-surface-base', 'border-line-default', 'rounded-xl');
  });

  it('interactive flag adds hover affordances', () => {
    render(
      <Card interactive data-testid="card">
        body
      </Card>
    );
    expect(screen.getByTestId('card')).toHaveClass('cursor-pointer');
  });
});

describe('Badge', () => {
  it('renders semantic variant classes', () => {
    render(
      <Badge variant="success" data-testid="badge">
        ok
      </Badge>
    );
    expect(screen.getByTestId('badge')).toHaveClass('bg-sage-50', 'text-sage-700');
  });
});

describe('Input / Textarea', () => {
  it('input renders error text and aria-invalid', () => {
    render(<Input aria-label="name" error="required" />);
    expect(screen.getByRole('textbox', { name: 'name' })).toHaveAttribute('aria-invalid', 'true');
    expect(screen.getByText('required')).toBeInTheDocument();
  });

  it('textarea renders without error slot when clean', () => {
    render(<Textarea aria-label="bio" />);
    expect(screen.getByRole('textbox', { name: 'bio' })).toBeInTheDocument();
    expect(screen.queryByText('required')).not.toBeInTheDocument();
  });
});

describe('Toast', () => {
  function Probe() {
    const { toast } = useToast();
    return (
      <button type="button" onClick={() => toast({ title: 'Saved', variant: 'success' })}>
        fire
      </button>
    );
  }

  it('throws outside the provider', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<Probe />)).toThrow(/ToastProvider/);
    spy.mockRestore();
  });

  it('shows a toast after calling useToast().toast', () => {
    render(
      <ToastProvider>
        <Probe />
      </ToastProvider>
    );
    fireEvent.click(screen.getByRole('button', { name: 'fire' }));
    expect(screen.getByText('Saved')).toBeInTheDocument();
  });
});
