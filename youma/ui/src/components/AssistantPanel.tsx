import { useCallback, useEffect, useRef, useState } from 'react';

interface AssistantPanelProps {
  isOpen: boolean;
  left: number;
  top: number;
  width: number;
  maxHeight: number;
  direction: 'down' | 'up';
  onToggleSize: () => void;
  isExpanded: boolean;
  onClose: () => void;
  children: React.ReactNode;
}

export default function AssistantPanel({
  isOpen,
  left,
  top,
  width,
  maxHeight,
  direction,
  onToggleSize,
  isExpanded,
  onClose,
  children,
}: AssistantPanelProps) {
  const [customWidth, setCustomWidth] = useState<number | null>(null);
  const isResizing = useRef(false);
  const resizeStartX = useRef(0);
  const resizeStartWidth = useRef(0);
  // Which side the panel is on relative to mascot (determines resize handle side)
  const effectiveWidth = customWidth ?? width;

  // Reset customWidth when width prop changes significantly (size toggle)
  useEffect(() => {
    setCustomWidth(null);
  }, [width]);

  // Determine if panel is to the right or left of center
  const isRightOfCenter = left > window.innerWidth / 2 - effectiveWidth / 2;

  const handleResizePointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      isResizing.current = true;
      resizeStartX.current = e.clientX;
      resizeStartWidth.current = effectiveWidth;
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    },
    [effectiveWidth],
  );

  const handleResizePointerMove = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    if (!isResizing.current) return;
    const dx = e.clientX - resizeStartX.current;
    // If handle is on the right edge, dragging right increases width
    // If handle is on the left edge, dragging left increases width
    const delta = isRightOfCenter ? -dx : dx;
    const newWidth = Math.max(320, Math.min(resizeStartWidth.current + delta, window.innerWidth * 0.5));
    setCustomWidth(Math.round(newWidth));
  }, [isRightOfCenter]);

  const handleResizePointerUp = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
    isResizing.current = false;
  }, []);

  // Animate: translateY offset based on direction
  const translateYOpen = '0px';
  const translateYClosed = direction === 'down' ? '-8px' : '8px';

  return (
    <div
      data-i
      style={{
        position: 'fixed',
        left,
        top,
        width: effectiveWidth,
        maxHeight,
        pointerEvents: isOpen ? 'auto' : 'none',
        opacity: isOpen ? 1 : 0,
        transform: isOpen
          ? `scale(1) translateY(${translateYOpen})`
          : `scale(0.97) translateY(${translateYClosed})`,
        transition: isOpen
          ? 'opacity 280ms cubic-bezier(0.16,1,0.3,1), transform 280ms cubic-bezier(0.16,1,0.3,1)'
          : 'opacity 180ms ease, transform 180ms ease',
        zIndex: 9994,
        display: 'flex',
        flexDirection: 'column',
        borderRadius: 16,
        overflow: 'hidden',
        background: 'rgb(250 250 249)',
        border: '1px solid rgb(231 229 228)',
        boxShadow: '0 25px 50px -12px rgba(0,0,0,0.25)',
      }}
    >
      {/* Header bar */}
      <div
        className="flex items-center justify-between px-3 bg-white border-b border-stone-100"
        style={{ height: 36, flexShrink: 0 }}
      >
        <span className="text-sm font-semibold text-stone-700 select-none">Youma</span>
        <div className="flex items-center gap-1">
          {/* Expand / shrink toggle */}
          <button
            type="button"
            onClick={onToggleSize}
            className="flex items-center justify-center w-6 h-6 rounded-md text-stone-400 hover:text-stone-600 hover:bg-stone-100 transition-colors"
            aria-label={isExpanded ? 'Riduci pannello' : 'Espandi pannello'}
            title={isExpanded ? 'Riduci' : 'Espandi'}
          >
            {isExpanded ? (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 9L4 4m0 0h5m-5 0v5M15 9l5-5m0 0h-5m5 0v5M9 15l-5 5m0 0h5m-5 0v-5M15 15l5 5m0 0h-5m5 0v-5" />
              </svg>
            ) : (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
              </svg>
            )}
          </button>
          {/* Close button */}
          <button
            type="button"
            onClick={onClose}
            className="flex items-center justify-center w-6 h-6 rounded-md text-stone-400 hover:text-stone-600 hover:bg-stone-100 transition-colors"
            aria-label="Chiudi pannello"
            title="Chiudi"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      {/* Content area */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden" style={{ minHeight: 0 }}>
        {children}
      </div>

      {/* Resize handle — right edge when panel is left-of-center, left edge otherwise */}
      <div
        data-i
        onPointerDown={handleResizePointerDown}
        onPointerMove={handleResizePointerMove}
        onPointerUp={handleResizePointerUp}
        style={{
          position: 'absolute',
          top: 0,
          bottom: 0,
          [isRightOfCenter ? 'left' : 'right']: 0,
          width: 6,
          cursor: 'ew-resize',
          zIndex: 10,
          background: 'transparent',
        }}
        title="Ridimensiona"
      />
    </div>
  );
}
