import { useCallback, useEffect, useRef, useState } from 'react';
import { getCurrentWindow, cursorPosition } from '@tauri-apps/api/window';
import { useDraggableLauncher, MASCOT_SIZE } from '../hooks/useDraggableLauncher';
import { useSmartPanelPosition } from '../hooks/useSmartPanelPosition';
import AssistantPanel from './AssistantPanel';
import DockBar from './DockBar';

const BASE_PANEL_WIDTHS = [0.30, 0.50] as const;
const HIT_POLL_MS = 50;

interface FloatingLauncherProps {
  children: React.ReactNode;
}

// ── Orb SVG ──────────────────────────────────────────────────────────────────

function MascotOrb({ isOpen }: { isOpen: boolean }) {
  const ringSpeed  = isOpen ? '5s'  : '14s';
  const orbitSpeed = isOpen ? '2.8s' : '6s';

  return (
    <svg
      viewBox="0 0 100 100"
      xmlns="http://www.w3.org/2000/svg"
      style={{ width: '100%', height: '100%', overflow: 'visible' }}
      aria-hidden="true"
    >
      <defs>
        {/* Main sphere: bright highlight top-left → primary blue → deep navy */}
        <radialGradient id="orb-sphere" cx="36%" cy="27%" r="70%">
          <stop offset="0%"   stopColor="#f0f7ff" />
          <stop offset="18%"  stopColor="#bfdbfe" />
          <stop offset="55%"  stopColor="#4A83DD" />
          <stop offset="100%" stopColor="#1a3370" />
        </radialGradient>

        {/* Soft outer halo */}
        <radialGradient id="orb-halo" cx="50%" cy="50%" r="50%">
          <stop offset="0%"   stopColor="rgba(74,131,221,0.30)" />
          <stop offset="55%"  stopColor="rgba(74,131,221,0.10)" />
          <stop offset="100%" stopColor="rgba(74,131,221,0)" />
        </radialGradient>

        {/* Glow filter for satellite dot */}
        <filter id="dot-glow" x="-80%" y="-80%" width="260%" height="260%">
          <feGaussianBlur stdDeviation="2" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>

        {/* Subtle inner glow on sphere */}
        <filter id="sphere-glow" x="-10%" y="-10%" width="120%" height="120%">
          <feGaussianBlur stdDeviation="1.5" result="blur" />
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
        stroke="rgba(147,197,253,0.32)"
        strokeWidth="1"
        strokeDasharray="5 4"
        style={{
          transformBox: 'fill-box',
          transformOrigin: 'center',
          animation: `orb-ring-slow ${ringSpeed} linear infinite`,
        }}
      />

      {/* Main sphere */}
      <circle cx="50" cy="50" r="33" fill="url(#orb-sphere)" filter="url(#sphere-glow)" />

      {/* Rim light — gives depth on the edge */}
      <circle
        cx="50" cy="50" r="33"
        fill="none"
        stroke="rgba(147,197,253,0.50)"
        strokeWidth="1.8"
      />

      {/* Primary specular highlight */}
      <ellipse
        cx="39" cy="35" rx="11" ry="7"
        fill="rgba(255,255,255,0.52)"
        style={{ transform: 'rotate(-28deg)', transformBox: 'fill-box', transformOrigin: 'center' }}
      />

      {/* Small secondary specular dot */}
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

      {/* Orbiting satellite — spins on its own rhythm */}
      <g style={{ transformOrigin: '50px 50px', animation: `orb-ring-slow ${orbitSpeed} linear infinite` }}>
        {/* Main satellite dot — bright, glowing */}
        <circle cx="50" cy="13" r="3.8" fill="#93c5fd" filter="url(#dot-glow)" />
        {/* Opposite dim echo dot */}
        <circle cx="50" cy="87" r="2.2" fill="rgba(147,197,253,0.36)" />
      </g>
    </svg>
  );
}

// ── FloatingLauncher ──────────────────────────────────────────────────────────

export default function FloatingLauncher({ children }: FloatingLauncherProps) {
  const [isOpen, setIsOpen]       = useState(false);
  const [sizeIdx, setSizeIdx]     = useState(0);
  const [isSpinning, setIsSpinning] = useState(false);
  const [isBouncing, setIsBouncing] = useState(false);
  const [viewport, setViewport]   = useState({ w: window.innerWidth, h: window.innerHeight });
  const prevIsOpenRef             = useRef(false);

  const { pos, isDragging, hasDragged, onPointerDown, onPointerMove, onPointerUp } =
    useDraggableLauncher();

  useEffect(() => {
    const h = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener('resize', h);
    return () => window.removeEventListener('resize', h);
  }, []);

  // Bounce when panel opens
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

  const panelWidth = Math.max(
    320,
    Math.min(Math.round(viewport.w * BASE_PANEL_WIDTHS[sizeIdx]), viewport.w - 24),
  );
  const layout = useSmartPanelPosition(pos, panelWidth, MASCOT_SIZE);

  // Cursor hit-test polling (keeps setIgnoreCursorEvents in sync)
  useEffect(() => {
    const win = getCurrentWindow();
    let isIgnoring = true;
    win.setIgnoreCursorEvents(true).catch(() => {});

    let winPhysX = 0, winPhysY = 0;
    let sf = window.devicePixelRatio || 1;

    Promise.all([win.outerPosition(), win.scaleFactor()])
      .then(([outerPos, s]) => { winPhysX = outerPos.x; winPhysY = outerPos.y; sf = s; })
      .catch(() => {});

    const tick = async () => {
      try {
        const cursor = await cursorPosition();
        const localX = (cursor.x - winPhysX) / sf;
        const localY = (cursor.y - winPhysY) / sf;
        const el = document.elementFromPoint(localX, localY);
        const over = !!el?.closest('[data-i]');
        if (over === isIgnoring) {
          isIgnoring = !over;
          await win.setIgnoreCursorEvents(!over);
        }
      } catch {}
    };

    const interval = setInterval(tick, HIT_POLL_MS);
    return () => {
      clearInterval(interval);
      win.setIgnoreCursorEvents(false).catch(() => {});
    };
  }, []);

  const toggle = useCallback(() => {
    if (!hasDragged.current) {
      setIsSpinning(true);
      setTimeout(() => setIsSpinning(false), 660);
      setIsOpen(v => !v);
    }
  }, [hasDragged]);

  // Animation priority: spin > bounce > idle-open > none
  const orbAnimation = isSpinning
    ? 'orb-click-spin 660ms cubic-bezier(0.34,1.56,0.64,1) forwards'
    : isBouncing
      ? 'orb-bounce-open 760ms cubic-bezier(0.34,1.56,0.64,1) forwards'
      : isOpen
        ? 'orb-idle-pulse 2.8s ease-in-out infinite'
        : 'none';

  const orbFilter = isDragging
    ? 'drop-shadow(0 4px 14px rgba(0,0,0,0.32))'
    : isOpen
      ? 'drop-shadow(0 0 9px rgba(74,131,221,0.75)) drop-shadow(0 0 20px rgba(74,131,221,0.40))'
      : 'drop-shadow(0 4px 16px rgba(0,0,0,0.22)) drop-shadow(0 0 6px rgba(74,131,221,0.28))';

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        pointerEvents: 'none',
        zIndex: 9990,
        overflow: 'hidden',
      }}
    >
      <AssistantPanel
        isOpen={isOpen}
        left={layout.panelLeft}
        top={layout.panelTop}
        width={panelWidth}
        maxHeight={layout.maxPanelHeight}
        direction={layout.direction}
        isExpanded={sizeIdx === 1}
        onToggleSize={() => setSizeIdx(i => (i + 1) % BASE_PANEL_WIDTHS.length)}
        onClose={() => setIsOpen(false)}
      >
        {children}
      </AssistantPanel>

      <DockBar
        isOpen={isOpen}
        left={layout.dockLeft}
        top={layout.dockTop}
        width={panelWidth}
      />

      {/* Mascot orb button */}
      <div
        data-i
        style={{
          position: 'fixed',
          left: pos.x,
          top: pos.y,
          width: MASCOT_SIZE,
          height: MASCOT_SIZE,
          pointerEvents: 'auto',
          cursor: isDragging ? 'grabbing' : 'grab',
          zIndex: 9999,
          userSelect: 'none',
          animation: orbAnimation,
          filter: orbFilter,
          transition: isDragging ? 'none' : 'filter 300ms ease',
          willChange: 'transform, filter',
        }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onClick={toggle}
        role="button"
        aria-label={isOpen ? 'Chiudi Youma' : 'Apri Youma'}
        tabIndex={0}
        onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') toggle(); }}
      >
        <MascotOrb isOpen={isOpen} />
      </div>
    </div>
  );
}
