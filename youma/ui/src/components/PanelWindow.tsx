import { useCallback, useEffect, useRef, useState } from 'react';
import { getCurrentWindow, LogicalSize, LogicalPosition } from '@tauri-apps/api/window';
import { emit, listen } from '@tauri-apps/api/event';
import { HashRouter as Router } from 'react-router-dom';
import { AppGate } from '../App';
import DockBar from './DockBar';

const GAP = 10;
const MARGIN = 8;
const BASE_WIDTHS = [0.30, 0.45] as const;
const HEADER_H = 36;
const DOCK_H = 52;

interface TogglePayload {
  mascotX: number;
  mascotY: number;
  mascotW: number;
  // screenX/screenY: monitor origin in global logical coords (0,0 for primary).
  // screenW: monitor right edge (global logical). screenH: available height.
  screenX?: number;
  screenY?: number;
  screenW: number;
  screenH: number;
  open: boolean;
}

function computeLayout(
  mascotX: number,
  _mascotY: number,
  mascotW: number,
  panelW: number,
  panelH: number,
  screenX: number,
  screenY: number,
  screenW: number,
  availH: number,
) {
  // Horizontal: prefer right of mascot, fall back to left.
  const spaceRight = screenW - mascotX - mascotW - GAP;
  const rawX = spaceRight >= panelW
    ? mascotX + mascotW + GAP
    : mascotX - panelW - GAP;
  const windowX = Math.max(screenX + MARGIN, Math.min(rawX, screenW - panelW - MARGIN));

  // Vertical: panel bottom aligns to work area bottom of the current monitor.
  const windowY = Math.max(screenY + MARGIN, screenY + availH - panelH - MARGIN);

  return { windowX, windowY };
}

export default function PanelWindow() {
  const [, setIsOpen] = useState(false);
  const [sizeIdx, setSizeIdx] = useState(0);
  const lastPayloadRef = useRef<TogglePayload | null>(null);
  const sizeIdxRef = useRef(0);

  const applyLayout = useCallback(async (payload: TogglePayload, idx: number) => {
    const win = getCurrentWindow();
    const { screenX = 0, screenY = 0, screenW, screenH: availH } = payload;

    const panelW = Math.max(320, Math.round((screenW - screenX) * BASE_WIDTHS[idx]));
    // Panel height: fill available work area minus top/bottom margins,
    // capped so there is always breathing room at the top.
    const maxH = availH - MARGIN * 2;
    const panelH = Math.min(Math.round(availH * 0.92), maxH);

    const { windowX, windowY } = computeLayout(
      payload.mascotX,
      payload.mascotY,
      payload.mascotW,
      panelW,
      panelH,
      screenX,
      screenY,
      screenW,
      availH,
    );

    await win.setSize(new LogicalSize(panelW, panelH));
    await win.setPosition(new LogicalPosition(windowX, windowY));
  }, []);

  useEffect(() => {
    const unlistens: Array<() => void> = [];

    listen<TogglePayload>('youma:toggle-panel', async ev => {
      const payload = ev.payload;
      lastPayloadRef.current = payload;
      const win = getCurrentWindow();
      if (payload.open) {
        await applyLayout(payload, sizeIdxRef.current);
        await win.show();
        await win.setFocus();
        setIsOpen(true);
      } else {
        setIsOpen(false);
        await win.hide();
        await emit('youma:panel-closed');
      }
    })
      .then(fn => unlistens.push(fn))
      .catch(() => {});

    return () => unlistens.forEach(fn => fn());
  }, [applyLayout]);

  const close = useCallback(async () => {
    setIsOpen(false);
    await getCurrentWindow().hide();
    await emit('youma:panel-closed');
  }, []);

  const toggleSize = useCallback(async () => {
    const nextIdx = (sizeIdxRef.current + 1) % BASE_WIDTHS.length;
    sizeIdxRef.current = nextIdx;
    setSizeIdx(nextIdx);
    if (lastPayloadRef.current) {
      await applyLayout(lastPayloadRef.current, nextIdx);
    }
  }, [applyLayout]);

  return (
    <Router>
      <div
        style={{
          position: 'fixed',
          inset: 0,
          display: 'flex',
          flexDirection: 'column',
          background: 'rgb(250 250 249)',
          borderRadius: 14,
          overflow: 'hidden',
          border: '1px solid rgb(231 229 228)',
          boxShadow: '0 20px 48px -8px rgba(0,0,0,0.28), 0 0 0 0.5px rgba(0,0,0,0.06)',
        }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between px-3 bg-white border-b border-stone-100"
          style={{ height: HEADER_H, flexShrink: 0 }}
        >
          <span className="text-sm font-semibold text-stone-700 select-none">Youma</span>
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => toggleSize().catch(() => {})}
              className="flex items-center justify-center w-6 h-6 rounded-md text-stone-400 hover:text-stone-600 hover:bg-stone-100 transition-colors"
              aria-label={sizeIdx === 1 ? 'Riduci pannello' : 'Espandi pannello'}
            >
              {sizeIdx === 1 ? (
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                    d="M9 9L4 4m0 0h5m-5 0v5M15 9l5-5m0 0h-5m5 0v5M9 15l-5 5m0 0h5m-5 0v-5M15 15l5 5m0 0h-5m5 0v-5" />
                </svg>
              ) : (
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                    d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
                </svg>
              )}
            </button>
            <button
              type="button"
              onClick={() => close().catch(() => {})}
              className="flex items-center justify-center w-6 h-6 rounded-md text-stone-400 hover:text-stone-600 hover:bg-stone-100 transition-colors"
              aria-label="Chiudi pannello"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                  d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>

        {/* Content — takes remaining height between header and dock */}
        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: 'auto',
            overflowX: 'hidden',
          }}
        >
          <AppGate />
        </div>

        {/* Dock navigation — always pinned at bottom, never cut off */}
        <div style={{ height: DOCK_H, flexShrink: 0 }}>
          <DockBar inline />
        </div>
      </div>
    </Router>
  );
}
