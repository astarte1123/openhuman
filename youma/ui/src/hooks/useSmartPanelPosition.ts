import { MASCOT_SIZE } from './useDraggableLauncher';

const GAP = 12;
const DOCK_H = 56;
const SCREEN_MARGIN = 8;

export interface SmartPanelPosition {
  panelLeft: number;
  panelTop: number;
  dockLeft: number;
  dockTop: number;
  maxPanelHeight: number;
  direction: 'down' | 'up';
}

export function useSmartPanelPosition(
  pos: { x: number; y: number },
  panelWidth: number,
  _mascotSize: number = MASCOT_SIZE,
): SmartPanelPosition {
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  const spaceRight = vw - (pos.x + MASCOT_SIZE) - GAP;
  const spaceLeft = pos.x - GAP;

  let panelLeft: number;
  if (spaceRight >= panelWidth) {
    panelLeft = pos.x + MASCOT_SIZE + GAP;
  } else if (spaceLeft >= panelWidth) {
    panelLeft = pos.x - GAP - panelWidth;
  } else {
    panelLeft = Math.round((vw - panelWidth) / 2);
  }

  panelLeft = Math.max(SCREEN_MARGIN, Math.min(panelLeft, vw - panelWidth - SCREEN_MARGIN));
  const dockLeft = panelLeft;

  const mascotBottom = pos.y + MASCOT_SIZE;
  const mascotTop = pos.y;
  const spaceBelow = vh - mascotBottom - GAP;

  const direction: 'down' | 'up' = spaceBelow >= 200 ? 'down' : 'up';

  let panelTop: number;
  let dockTop: number;
  let maxPanelHeight: number;

  if (direction === 'down') {
    panelTop = mascotBottom + GAP;
    maxPanelHeight = Math.max(180, vh - panelTop - DOCK_H - GAP - SCREEN_MARGIN);
    dockTop = panelTop + maxPanelHeight + GAP;
    if (dockTop + DOCK_H > vh - SCREEN_MARGIN) {
      dockTop = vh - DOCK_H - SCREEN_MARGIN;
      maxPanelHeight = Math.max(180, dockTop - GAP - panelTop);
    }
  } else {
    dockTop = mascotTop - GAP - DOCK_H;
    if (dockTop < SCREEN_MARGIN) dockTop = SCREEN_MARGIN;
    maxPanelHeight = Math.max(180, dockTop - GAP - SCREEN_MARGIN);
    panelTop = dockTop - GAP - maxPanelHeight;
    if (panelTop < SCREEN_MARGIN) {
      panelTop = SCREEN_MARGIN;
      maxPanelHeight = Math.max(180, dockTop - GAP - panelTop);
    }
  }

  return { panelLeft, panelTop, dockLeft, dockTop, maxPanelHeight, direction };
}
