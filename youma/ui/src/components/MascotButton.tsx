import { useCallback, useEffect, useRef, useState } from 'react';
import { currentMonitor, getCurrentWindow, LogicalPosition } from '@tauri-apps/api/window';
import { emit, listen } from '@tauri-apps/api/event';

const WIN_W = 32;
const WIN_H = 32;
const DRAG_THRESHOLD = 6;
const INITIAL_X = 100;
const POS_KEY = 'youma_mascot_pos_v2';

function taskbarPos(): { x: number; y: number } {
  const taskbarH = window.screen.height - window.screen.availHeight;
  return {
    x: INITIAL_X,
    y: window.screen.availHeight + Math.floor(Math.max(0, taskbarH - WIN_H) / 2),
  };
}

function loadSavedPos(): { x: number; y: number } | null {
  try {
    const raw = localStorage.getItem(POS_KEY);
    if (raw) {
      const p = JSON.parse(raw) as unknown;
      if (p && typeof p === 'object' && 'x' in p && 'y' in p &&
          typeof (p as { x: unknown }).x === 'number' &&
          typeof (p as { y: unknown }).y === 'number') {
        return { x: (p as { x: number }).x, y: (p as { y: number }).y };
      }
    }
  } catch {}
  return null;
}

export default function MascotButton() {
  const isOpenRef = useRef(false);
  const [isOpen, _setIsOpen] = useState(false);
  const [isPressed, setIsPressed] = useState(false);
  const [isSpinning, setIsSpinning] = useState(false);
  const [isBouncing, setIsBouncing] = useState(false);
  const prevIsOpenRef = useRef(false);

  const setIsOpen = useCallback((v: boolean) => {
    isOpenRef.current = v;
    _setIsOpen(v);
  }, []);

  const hasDraggedRef = useRef(false);
  const pointerStartRef = useRef<{ x: number; y: number } | null>(null);

  // Bounce on panel open
  useEffect(() => {
    if (!isOpen || prevIsOpenRef.current) {
      prevIsOpenRef.current = isOpen;
      return;
    }
    prevIsOpenRef.current = isOpen;
    setIsBouncing(true);
    const t = setTimeout(() => setIsBouncing(false), 760);
    return () => clearTimeout(t);
  }, [isOpen]);

  useEffect(() => {
    const win = getCurrentWindow();
    win.setIgnoreCursorEvents(false).catch(() => {});

    // Restore saved position (survives monitor switches); only fall back to
    // taskbar default on very first launch (no saved pos yet).
    const saved = loadSavedPos();
    if (saved) {
      win.setPosition(new LogicalPosition(saved.x, saved.y)).catch(() => {});
    } else {
      const { x, y } = taskbarPos();
      win.setPosition(new LogicalPosition(x, y)).catch(() => {});
    }

    const unlistens: Array<() => void> = [];

    // Save position whenever the window moves (native drag via startDragging).
    listen('tauri://move', async () => {
      try {
        const [pos, sf] = await Promise.all([win.outerPosition(), win.scaleFactor()]);
        localStorage.setItem(POS_KEY, JSON.stringify({ x: pos.x / sf, y: pos.y / sf }));
      } catch {}
    }).then(fn => unlistens.push(fn)).catch(() => {});

    listen('youma:panel-closed', () => setIsOpen(false))
      .then(fn => unlistens.push(fn))
      .catch(() => {});
    return () => unlistens.forEach(fn => fn());
  }, [setIsOpen]);

  const toggle = useCallback(async () => {
    try {
      const win = getCurrentWindow();
      const [outer, sf, monitor] = await Promise.all([
        win.outerPosition(),
        win.scaleFactor(),
        currentMonitor(),
      ]);
      const newOpen = !isOpenRef.current;
      setIsOpen(newOpen);

      // Use the monitor the mascot is actually on for correct panel sizing/placement.
      // Fall back to primary-screen values for safety.
      let screenX = 0;
      let screenY = 0;
      let screenW = window.screen.width;
      let screenH = window.screen.availHeight;

      if (monitor) {
        const msf = monitor.scaleFactor;
        screenX = Math.round(monitor.position.x / msf);
        screenY = Math.round(monitor.position.y / msf);
        const monW = Math.round(monitor.size.width / msf);
        const monH = Math.round(monitor.size.height / msf);
        // Estimate taskbar: on the primary monitor use actual value; 48px on others.
        const taskbarH = screenX === 0 && screenY === 0
          ? (window.screen.height - window.screen.availHeight || 48)
          : 48;
        screenW = screenX + monW;
        screenH = monH - taskbarH;
      }

      await emit('youma:toggle-panel', {
        mascotX: outer.x / sf,
        mascotY: outer.y / sf,
        mascotW: WIN_W,
        screenX,
        screenY,
        screenW,
        screenH,
        open: newOpen,
      });
    } catch (err) {
      console.error('[MascotButton] toggle', err);
    }
  }, [setIsOpen]);

  const onPointerDown = useCallback((e: React.PointerEvent<Element>) => {
    e.preventDefault();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    hasDraggedRef.current = false;
    pointerStartRef.current = { x: e.clientX, y: e.clientY };
    setIsPressed(true);
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent<Element>) => {
    if (!pointerStartRef.current || hasDraggedRef.current) return;
    const dx = e.clientX - pointerStartRef.current.x;
    const dy = e.clientY - pointerStartRef.current.y;
    if (Math.abs(dx) + Math.abs(dy) > DRAG_THRESHOLD) {
      hasDraggedRef.current = true;
      setIsPressed(false);
      getCurrentWindow().startDragging().catch(() => {});
    }
  }, []);

  const onPointerUp = useCallback((e: React.PointerEvent<Element>) => {
    (e.currentTarget as Element).releasePointerCapture(e.pointerId);
    pointerStartRef.current = null;
    setIsPressed(false);
  }, []);

  const handleClick = useCallback(() => {
    if (hasDraggedRef.current) {
      hasDraggedRef.current = false;
      return;
    }
    setIsSpinning(true);
    setTimeout(() => setIsSpinning(false), 660);
    toggle().catch(() => {});
  }, [toggle]);

  // Animation priority: pressed overrides all via direct transform;
  // spin > bounce > idle-open > closed (no anim)
  const animation = isPressed
    ? 'none'
    : isSpinning
      ? 'orb-click-spin 660ms cubic-bezier(0.34,1.56,0.64,1) forwards'
      : isBouncing
        ? 'orb-bounce-open 760ms cubic-bezier(0.34,1.56,0.64,1) forwards'
        : isOpen
          ? 'orb-idle-pulse 2.8s ease-in-out infinite'
          : 'none';

  const glowFilter = isOpen
    ? 'drop-shadow(0 0 5px rgba(74,131,221,0.9)) drop-shadow(0 0 12px rgba(74,131,221,0.5))'
    : 'drop-shadow(0 2px 6px rgba(0,0,0,0.35)) drop-shadow(0 0 4px rgba(74,131,221,0.3))';

  // Ring & satellite rotation speed: faster when panel is open
  const ringDuration  = isOpen ? '5s'   : '14s';
  const orbitDuration = isOpen ? '2.8s' : '6s';

  return (
    <svg
      viewBox="0 0 100 100"
      xmlns="http://www.w3.org/2000/svg"
      style={{
        position: 'fixed',
        inset: 0,
        width: '100%',
        height: '100%',
        display: 'block',
        cursor: hasDraggedRef.current ? 'grabbing' : 'grab',
        userSelect: 'none',
        overflow: 'visible',
        animation,
        filter: glowFilter,
        transform: isPressed ? 'scale(0.82)' : undefined,
        transition: isPressed
          ? 'transform 80ms ease-in, filter 100ms ease'
          : 'filter 300ms ease',
        willChange: 'transform, filter',
      }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onClick={handleClick}
      role="button"
      aria-label={isOpen ? 'Chiudi Youma' : 'Apri Youma'}
      tabIndex={0}
      onKeyDown={e => {
        if (e.key === 'Enter' || e.key === ' ') toggle().catch(() => {});
      }}
    >
      <defs>
        {/* Main sphere: light highlight top-left → primary blue → deep navy */}
        <radialGradient id="orb-sphere" cx="36%" cy="27%" r="70%">
          <stop offset="0%"   stopColor="#f0f7ff" />
          <stop offset="18%"  stopColor="#bfdbfe" />
          <stop offset="55%"  stopColor="#4A83DD" />
          <stop offset="100%" stopColor="#1a3370" />
        </radialGradient>

        {/* Soft outer halo */}
        <radialGradient id="orb-halo" cx="50%" cy="50%" r="50%">
          <stop offset="0%"   stopColor="rgba(74,131,221,0.28)" />
          <stop offset="55%"  stopColor="rgba(74,131,221,0.08)" />
          <stop offset="100%" stopColor="rgba(74,131,221,0)" />
        </radialGradient>

        {/* Glow filter for satellite dot */}
        <filter id="dot-glow" x="-100%" y="-100%" width="300%" height="300%">
          <feGaussianBlur stdDeviation="2.5" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      {/* Outer diffuse halo */}
      <circle cx="50" cy="50" r="49" fill="url(#orb-halo)" />

      {/* Rotating dashed orbit ring */}
      <circle
        cx="50" cy="50" r="44"
        fill="none"
        stroke="rgba(147,197,253,0.35)"
        strokeWidth="1"
        strokeDasharray="5 4"
        style={{
          transformBox: 'fill-box',
          transformOrigin: 'center',
          animation: `orb-ring-slow ${ringDuration} linear infinite`,
        }}
      />

      {/* Main sphere */}
      <circle cx="50" cy="50" r="33" fill="url(#orb-sphere)" />

      {/* Rim light */}
      <circle cx="50" cy="50" r="33" fill="none" stroke="rgba(147,197,253,0.50)" strokeWidth="1.8" />

      {/* Primary specular highlight */}
      <ellipse
        cx="39" cy="35" rx="11" ry="7"
        fill="rgba(255,255,255,0.52)"
        style={{ transform: 'rotate(-28deg)', transformBox: 'fill-box', transformOrigin: 'center' }}
      />

      {/* Secondary specular dot */}
      <circle cx="32" cy="30" r="4" fill="rgba(255,255,255,0.20)" />

      {/* Micro glint */}
      <circle cx="28" cy="27" r="1.8" fill="rgba(255,255,255,0.55)" />

      {/* Tilted orbit trace */}
      <ellipse
        cx="50" cy="50" rx="39" ry="12"
        fill="none"
        stroke="rgba(147,197,253,0.18)"
        strokeWidth="1"
        style={{ transform: 'rotate(-22deg)', transformBox: 'fill-box', transformOrigin: 'center' }}
      />

      {/* Orbiting satellite */}
      <g style={{ transformOrigin: '50px 50px', animation: `orb-ring-slow ${orbitDuration} linear infinite` }}>
        <circle cx="50" cy="13" r="3.8" fill="#93c5fd" filter="url(#dot-glow)" />
        <circle cx="50" cy="87" r="2.2" fill="rgba(147,197,253,0.36)" />
      </g>
    </svg>
  );
}
