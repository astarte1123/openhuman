import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { YellowMascot } from './YellowMascot';

describe('<YellowMascot />', () => {
  it('renders the mascot image by default with the configured face data attribute', () => {
    const { container } = render(<YellowMascot />);
    const host = container.querySelector('[data-face]') as HTMLElement;
    expect(host).not.toBeNull();
    expect(host.getAttribute('data-face')).toBe('idle');
    expect(container.querySelector('img')).not.toBeNull();
  });

  it.each([
    ['idle', 'idle'],
    ['sleep', 'sleep'],
    ['speaking', 'speaking'],
    ['thinking', 'thinking'],
    ['confused', 'confused'],
  ] as const)('passes %s through to data-face', (face, expected) => {
    const { container } = render(<YellowMascot face={face} />);
    const host = container.querySelector('[data-face]') as HTMLElement;
    expect(host.getAttribute('data-face')).toBe(expected);
  });

  it('renders the sleep face with the mascot image', () => {
    const { container } = render(<YellowMascot face="sleep" />);
    const host = container.querySelector('[data-face="sleep"]') as HTMLElement;
    expect(host).not.toBeNull();
    expect(container.querySelector('img')).not.toBeNull();
  });

  it('forwards a numeric size prop as a pixel width', () => {
    const { container } = render(<YellowMascot size={48} />);
    const host = container.querySelector('[data-face]') as HTMLElement;
    expect(host.style.width).toBe('48px');
  });

  it('renders the same static mascot image regardless of mascotColor', () => {
    const { container: yellow } = render(<YellowMascot />);
    const { container: navy } = render(<YellowMascot mascotColor="navy" />);
    const yellowImg = yellow.querySelector('img');
    const navyImg = navy.querySelector('img');
    expect(yellowImg).not.toBeNull();
    expect(navyImg).not.toBeNull();
    expect(yellowImg?.getAttribute('src')).toBe(navyImg?.getAttribute('src'));
  });
});
