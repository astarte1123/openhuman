import { useCallback, useEffect, useRef, useState } from 'react';

export const MASCOT_SIZE = 80;
const POS_KEY = 'youma_launcher_pos';

function clamp(v: number, lo: number, hi: number) {
  return Math.max(lo, Math.min(hi, v));
}

function loadPos(): { x: number; y: number } {
  try {
    const raw = localStorage.getItem(POS_KEY);
    if (raw) {
      const p = JSON.parse(raw);
      if (typeof p.x === 'number' && typeof p.y === 'number')
        return {
          x: clamp(p.x, 0, window.innerWidth - MASCOT_SIZE),
          y: clamp(p.y, 0, window.innerHeight - MASCOT_SIZE),
        };
    }
  } catch {}
  return { x: 32, y: window.innerHeight - MASCOT_SIZE - 48 };
}

export function useDraggableLauncher() {
  const [pos, setPos] = useState(loadPos);
  const [isDragging, setIsDragging] = useState(false);
  const hasDragged = useRef(false);
  const startRef = useRef<{ mx: number; my: number; px: number; py: number } | null>(null);

  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLElement>) => {
      e.preventDefault();
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      hasDragged.current = false;
      startRef.current = { mx: e.clientX, my: e.clientY, px: pos.x, py: pos.y };
      setIsDragging(true);
    },
    [pos.x, pos.y],
  );

  const onPointerMove = useCallback((e: React.PointerEvent<HTMLElement>) => {
    if (!startRef.current) return;
    const dx = e.clientX - startRef.current.mx;
    const dy = e.clientY - startRef.current.my;
    if (Math.abs(dx) + Math.abs(dy) > 4) hasDragged.current = true;
    setPos({
      x: clamp(startRef.current.px + dx, 0, window.innerWidth - MASCOT_SIZE),
      y: clamp(startRef.current.py + dy, 0, window.innerHeight - MASCOT_SIZE),
    });
  }, []);

  const onPointerUp = useCallback((e: React.PointerEvent<HTMLElement>) => {
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
    startRef.current = null;
    setIsDragging(false);
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem(POS_KEY, JSON.stringify(pos));
    } catch {}
  }, [pos]);

  useEffect(() => {
    const h = () =>
      setPos(p => ({
        x: clamp(p.x, 0, window.innerWidth - MASCOT_SIZE),
        y: clamp(p.y, 0, window.innerHeight - MASCOT_SIZE),
      }));
    window.addEventListener('resize', h);
    return () => window.removeEventListener('resize', h);
  }, []);

  return { pos, isDragging, hasDragged, onPointerDown, onPointerMove, onPointerUp };
}
