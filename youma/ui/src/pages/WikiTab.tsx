// WikiTab — force-directed knowledge graph (Canvas, no external libs).
// Dark background, Obsidian-style nodes. Pan/zoom/drag. Click node → detail panel.

import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react'; // useState used in NodePanel too
import {
  knowledgeChunkDelete, knowledgeChunkGet, knowledgeChunksGraph, knowledgeRebuild,
  knowledgeTreeNodes, knowledgeVaultPathGet, knowledgeVaultPathSet,
  knowledgeVaultSync, settingsGet,
} from '../api';
import type { KnowledgeChunk, KnowledgeTreeNode } from '../api';

// ── Types ────────────────────────────────────────────────────────────────────

interface GNode {
  id: string; x: number; y: number; vx: number; vy: number;
  r: number; color: string; label: string;
  type: 'root' | 'topic' | 'chunk';
  topicGroup?: string;
  chunkData?: KnowledgeChunk;
  treeData?: KnowledgeTreeNode;
}
interface GEdge { s: string; t: string; sibling?: boolean; }
interface XForm { tx: number; ty: number; scale: number; }

// ── Colors / constants ────────────────────────────────────────────────────────

const BG         = '#0f172a';
const COL_ROOT   = '#ef8c74';
const COL_TOPIC  = '#e07a5f';
const COL_MEM    = '#86bea6';
const COL_COMP   = '#fbbf24';
// COL_CHUNK unused — chunk color derived from source_type via chunkColor()
const EDGE_ROOT  = 'rgba(239,140,116,0.60)';
const EDGE_TOPIC = 'rgba(239,140,116,0.35)';
const COL_MANUAL   = '#4A83DD';
const COL_MESSAGE  = '#a78bfa';
const COL_INBOX    = '#f97316';
const COL_WEB      = '#22d3ee';
const COL_MEETING  = '#ef4444';

// All known source_type values (including backend aliases).
const SOURCE_COLORS: Record<string, string> = {
  manual:    COL_MANUAL,
  file:      COL_MANUAL,
  document:  COL_MANUAL,
  pdf:       COL_MANUAL,
  memory:    COL_MEM,
  note:      COL_MEM,
  composio:  COL_COMP,
  message:   COL_MESSAGE,
  thread:    COL_MESSAGE,
  chat:      COL_MESSAGE,
  inbox:     COL_INBOX,
  email:     COL_INBOX,
  web:       COL_WEB,
  url:       COL_WEB,
  meeting:   COL_MEETING,
};

// Deterministic hue for any source type not in SOURCE_COLORS.
function sourceHue(s: string): string {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) & 0xffffffff;
  return `hsl(${Math.abs(h) % 360},60%,62%)`;
}

function chunkColor(sourceType: string): string {
  return SOURCE_COLORS[sourceType] ?? sourceHue(sourceType);
}

// Return a hex/hsl color with alpha applied for edge drawing.
function edgeColor(nodeColor: string, alpha: number): string {
  if (nodeColor.startsWith('#') && nodeColor.length === 7) {
    const r = parseInt(nodeColor.slice(1, 3), 16);
    const g = parseInt(nodeColor.slice(3, 5), 16);
    const b = parseInt(nodeColor.slice(5, 7), 16);
    return `rgba(${r},${g},${b},${alpha})`;
  }
  if (nodeColor.startsWith('hsl')) {
    return nodeColor.replace('hsl', 'hsla').replace(')', `,${alpha})`);
  }
  return `rgba(148,163,184,${alpha})`;
}

// ── Force simulation ──────────────────────────────────────────────────────────

function tick(nodes: GNode[], edges: GEdge[], cx: number, cy: number) {
  const map = new Map(nodes.map(n => [n.id, n]));

  // Repulsion (skip fixed root)
  for (let i = 0; i < nodes.length; i++) {
    const a = nodes[i]; if (a.type === 'root') continue;
    for (let j = i + 1; j < nodes.length; j++) {
      const b = nodes[j]; if (b.type === 'root') continue;
      const dx = a.x - b.x, dy = a.y - b.y;
      const d2 = Math.max(dx * dx + dy * dy, 1);
      const d  = Math.sqrt(d2);
      const f  = Math.min(5500 / d2, 10);
      const nx = dx / d, ny = dy / d;
      a.vx += nx * f; a.vy += ny * f;
      b.vx -= nx * f; b.vy -= ny * f;
    }
  }

  // Spring attraction along edges
  for (const e of edges) {
    const a = map.get(e.s), b = map.get(e.t);
    if (!a || !b) continue;
    const dx = b.x - a.x, dy = b.y - a.y;
    const d  = Math.sqrt(dx * dx + dy * dy) || 1;
    const rest = (a.type === 'root' || b.type === 'root') ? 155 : 72;
    const f  = 0.04 * (d - rest);
    const nx = dx / d, ny = dy / d;
    if (a.type !== 'root') { a.vx += nx * f; a.vy += ny * f; }
    if (b.type !== 'root') { b.vx -= nx * f; b.vy -= ny * f; }
  }

  // Gravity toward center + integrate
  for (const n of nodes) {
    if (n.type === 'root') continue;
    n.vx += (cx - n.x) * 0.0012;
    n.vy += (cy - n.y) * 0.0012;
    const spd = Math.sqrt(n.vx * n.vx + n.vy * n.vy);
    if (spd > 12) { n.vx = (n.vx / spd) * 12; n.vy = (n.vy / spd) * 12; }
    n.x += n.vx; n.y += n.vy;
    n.vx *= 0.80; n.vy *= 0.80;
  }
}

// ── Canvas renderer ───────────────────────────────────────────────────────────

function drawGraph(
  ctx: CanvasRenderingContext2D,
  nodes: GNode[], edges: GEdge[],
  map: Map<string, GNode>,
  t: XForm, hov: string | null, sel: string | null,
) {
  const { width: W, height: H } = ctx.canvas;
  ctx.clearRect(0, 0, W, H);
  ctx.fillStyle = BG; ctx.fillRect(0, 0, W, H);

  ctx.save();
  ctx.translate(t.tx, t.ty);
  ctx.scale(t.scale, t.scale);

  // Edges — colored by source type, visible opacity
  for (const e of edges) {
    const a = map.get(e.s), b = map.get(e.t);
    if (!a || !b) continue;

    let stroke: string;
    let lw: number;

    if (a.type === 'root' || b.type === 'root') {
      stroke = EDGE_ROOT; lw = 1.2 / t.scale;
    } else if (a.type === 'topic' && b.type === 'topic') {
      stroke = EDGE_TOPIC; lw = 0.9 / t.scale;
    } else {
      const chunkNode = a.type === 'chunk' ? a : b.type === 'chunk' ? b : null;
      stroke = chunkNode ? edgeColor(chunkNode.color, e.sibling ? 0.60 : 0.50) : EDGE_TOPIC;
      lw = e.sibling ? 1.3 / t.scale : 1.0 / t.scale;
    }

    ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y);
    ctx.strokeStyle = stroke;
    ctx.lineWidth   = lw;
    if (e.sibling) {
      ctx.setLineDash([3 / t.scale, 4 / t.scale]);
    }
    ctx.stroke();
    ctx.setLineDash([]);
  }

  // Nodes
  for (const n of nodes) {
    const isH = n.id === hov, isS = n.id === sel;

    // Glow halo
    if (isH || isS) {
      ctx.beginPath(); ctx.arc(n.x, n.y, n.r + 8 / t.scale, 0, Math.PI * 2);
      ctx.fillStyle = isS ? 'rgba(255,255,255,0.14)' : 'rgba(255,255,255,0.07)';
      ctx.fill();
    }

    ctx.beginPath(); ctx.arc(n.x, n.y, n.r, 0, Math.PI * 2);
    ctx.fillStyle = isS ? '#ffffff' : n.color;
    ctx.fill();

    // Labels: always for root/topic, only on hover for chunks
    if (n.type !== 'chunk' || isH) {
      const fs  = n.type === 'root' ? 13 : n.type === 'topic' ? 11 : 9;
      const off = (n.type === 'root' ? 20 : 16) / t.scale;
      ctx.font      = `${n.type === 'root' ? '600 ' : ''}${fs / t.scale}px Inter, system-ui, sans-serif`;
      ctx.fillStyle = n.type === 'chunk' ? 'rgba(203,213,225,0.9)' : '#f1f5f9';
      ctx.textAlign = 'center';
      const lbl = n.label.length > 26 ? n.label.slice(0, 25) + '…' : n.label;
      ctx.fillText(lbl, n.x, n.y + n.r + off);
    }
  }

  ctx.restore();
}

// ── Utilities ─────────────────────────────────────────────────────────────────

/** Convert topic slug "my-topic" → "My Topic" */
function topicLabel(slug: string): string {
  return slug.split('-').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' ') || slug;
}

// ── Build graph data ──────────────────────────────────────────────────────────

/**
 * Build graph nodes+edges from tree nodes + chunks.
 *
 * If the AI summary tree has not been built yet (no level-1 branch nodes),
 * synthetic topic nodes are created from the unique chunk.topic_group values
 * so the graph always shows meaningful structure even without a Rebuild.
 */
function buildGraph(
  trees: KnowledgeTreeNode[], chunks: KnowledgeChunk[], W: number, H: number,
): { nodes: GNode[]; edges: GEdge[]; synthetic: boolean } {
  const cx = W / 2, cy = H / 2;
  const nodes: GNode[] = [];
  const edges: GEdge[] = [];

  nodes.push({
    id: 'root', x: cx, y: cy, vx: 0, vy: 0,
    r: 22, color: COL_ROOT, label: 'Knowledge', type: 'root',
    treeData: trees.find(t => t.id === 'root'),
  });

  // Use AI-built topic nodes when available; otherwise synthesise from chunks.
  const aiTopics = trees.filter(t => t.level === 1);
  const synthetic = aiTopics.length === 0 && chunks.length > 0;

  interface TopicEntry { id: string; label: string; group: string; treeData?: KnowledgeTreeNode }

  const effectiveTopics: TopicEntry[] = synthetic
    ? [...new Set(chunks.map(c => c.topic_group).filter(Boolean))].map(g => ({
        id: `topic:${g}`, label: topicLabel(g), group: g,
      }))
    : aiTopics.map(t => ({
        id: `topic:${t.topic_group}`, label: t.label, group: t.topic_group, treeData: t,
      }));

  effectiveTopics.forEach(({ id, label, group, treeData }, i) => {
    const a = (i / Math.max(effectiveTopics.length, 1)) * Math.PI * 2 - Math.PI / 2;
    nodes.push({
      id,
      x: cx + Math.cos(a) * 145, y: cy + Math.sin(a) * 145,
      vx: 0, vy: 0,
      r: synthetic ? 11 : 13,
      color: synthetic ? 'rgba(224,122,95,0.6)' : COL_TOPIC,
      label, type: 'topic',
      topicGroup: group, treeData,
    });
    edges.push({ s: 'root', t: id });
  });

  // Group chunk ids by topic for sibling edge building
  const topicChunks = new Map<string, string[]>();

  chunks.forEach((c, i) => {
    const pid = `topic:${c.topic_group}`;
    const parent = nodes.find(n => n.id === pid);
    const a = i * 2.3999;
    const d = 60 + Math.random() * 35;
    const nid = `chunk:${c.id}`;
    nodes.push({
      id: nid,
      x: parent ? parent.x + Math.cos(a) * d : cx + (Math.random() - .5) * 260,
      y: parent ? parent.y + Math.sin(a) * d : cy + (Math.random() - .5) * 260,
      vx: 0, vy: 0,
      r: 4 + c.score * 3,
      color: chunkColor(c.source_type),
      label: c.title, type: 'chunk',
      topicGroup: c.topic_group, chunkData: c,
    });
    if (parent) edges.push({ s: pid, t: nid });

    // Track for sibling edges
    const key = c.topic_group || '__orphan__';
    if (!topicChunks.has(key)) topicChunks.set(key, []);
    topicChunks.get(key)!.push(nid);
  });

  // Add dashed sibling edges within each topic cluster (ring, max 6 per topic).
  for (const ids of topicChunks.values()) {
    const limit = Math.min(ids.length, 6);
    for (let i = 0; i < limit - 1; i++) {
      edges.push({ s: ids[i], t: ids[i + 1], sibling: true });
    }
    // Close the ring if 4+ nodes
    if (limit >= 4) edges.push({ s: ids[0], t: ids[limit - 1], sibling: true });
  }

  return { nodes, edges, synthetic };
}

function formatRel(iso: string) {
  try {
    const m = Math.floor((Date.now() - new Date(iso).getTime()) / 60000);
    if (m < 2) return 'just now';
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}h ago`;
    return new Date(iso).toLocaleDateString([], { month: 'short', day: 'numeric' });
  } catch { return ''; }
}

function Spinner() {
  return (
    <svg className="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24" aria-hidden="true">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z" />
    </svg>
  );
}

// ── Node detail panel ─────────────────────────────────────────────────────────

function NodePanel({ node, onClose, onDeleteChunk }: {
  node: GNode; onClose: () => void; onDeleteChunk: (id: string) => void;
}) {
  const c = node.chunkData;
  const t = node.treeData;
  const [content, setContent] = useState<string | null>(null);
  const [loadingContent, setLoadingContent] = useState(false);

  // Fetch full content on-demand when a chunk node is opened.
  useEffect(() => {
    if (node.type !== 'chunk' || !c) return;
    setContent(null);
    setLoadingContent(true);
    knowledgeChunkGet(c.id).then(full => {
      setContent(full?.content_md ?? '');
    }).catch(() => setContent('')).finally(() => setLoadingContent(false));
  }, [node.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const bodyText = t?.summary_md ?? content;

  return (
    <div className="absolute bottom-3 right-3 w-72 bg-slate-900/95 backdrop-blur-sm border border-slate-700 rounded-xl shadow-2xl overflow-hidden z-20">
      <div className="flex items-start gap-3 p-4 border-b border-slate-800">
        <span className={`w-2.5 h-2.5 rounded-full flex-shrink-0 mt-1 ${node.type === 'root' ? 'bg-[#ef8c74]' : node.type === 'topic' ? 'bg-[#e07a5f]' : 'bg-slate-400'}`} />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-semibold text-white leading-snug">{node.label}</p>
          <p className="text-[10px] text-slate-400 mt-0.5 capitalize">
            {node.type}
            {c ? ` · ${c.source_type} · ${Math.round(c.score * 100)}%` : ''}
          </p>
        </div>
        <button type="button" aria-label="Close" onClick={onClose} className="text-slate-500 hover:text-slate-300 p-0.5">
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div className="p-4 max-h-44 overflow-y-auto text-xs text-slate-300 leading-relaxed">
        {loadingContent ? (
          <div className="flex items-center gap-2 text-slate-500">
            <div className="w-3 h-3 border border-slate-500 border-t-slate-300 rounded-full animate-spin" />
            Loading…
          </div>
        ) : bodyText ? (
          <pre className="whitespace-pre-wrap font-sans">{bodyText}</pre>
        ) : null}
      </div>

      {c && (
        <div className="px-4 pb-3">
          <button
            type="button"
            onClick={() => onDeleteChunk(c.id)}
            className="flex items-center gap-1.5 text-xs text-red-400 hover:text-red-300 transition-colors"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
            Delete chunk
          </button>
        </div>
      )}
    </div>
  );
}

// ── ForceGraph canvas component ───────────────────────────────────────────────

export interface ForceGraphHandle {
  zoomIn: () => void;
  zoomOut: () => void;
  resetView: () => void;
}

const ForceGraph = forwardRef<ForceGraphHandle, {
  nodes: GNode[]; edges: GEdge[];
  onSelect: (n: GNode | null) => void;
}>(function ForceGraph({ nodes: initNodes, edges: initEdges, onSelect }, ref) {
  const canvasRef  = useRef<HTMLCanvasElement>(null);
  const nodesRef   = useRef<GNode[]>(initNodes);
  const edgesRef   = useRef<GEdge[]>(initEdges);
  const mapRef     = useRef<Map<string, GNode>>(new Map());
  const xformRef   = useRef<XForm>({ tx: 0, ty: 0, scale: 1 });
  const hovRef     = useRef<string | null>(null);
  const selRef     = useRef<string | null>(null);
  const ticksRef   = useRef(0);
  const rafRef     = useRef(0);
  const dragRef    = useRef<{ nodeId: string | null; moved: boolean } | null>(null);
  const autoFittedRef = useRef(false);
  const animRef = useRef<{ from: XForm; to: XForm; startMs: number; durMs: number } | null>(null);

  function computeFit(): XForm | null {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const dpr = window.devicePixelRatio || 1;
    const W = canvas.width / dpr, H = canvas.height / dpr;
    const nodes = nodesRef.current;
    if (nodes.length === 0) return null;
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const n of nodes) {
      x0 = Math.min(x0, n.x - n.r); y0 = Math.min(y0, n.y - n.r);
      x1 = Math.max(x1, n.x + n.r); y1 = Math.max(y1, n.y + n.r);
    }
    const pad = 44;
    const s = Math.min((W - pad * 2) / Math.max(x1 - x0, 1), (H - pad * 2) / Math.max(y1 - y0, 1), 2);
    return { tx: W / 2 - ((x0 + x1) / 2) * s, ty: H / 2 - ((y0 + y1) / 2) * s, scale: s };
  }

  function fitView() {
    const target = computeFit();
    if (!target) return;
    animRef.current = { from: { ...xformRef.current }, to: target, startMs: performance.now(), durMs: 700 };
  }

  useEffect(() => {
    nodesRef.current = initNodes.map(n => ({ ...n }));
    edgesRef.current = initEdges.map(e => ({ ...e }));
    mapRef.current   = new Map(nodesRef.current.map(n => [n.id, n]));
    ticksRef.current = 0;
    selRef.current   = null;
    autoFittedRef.current = false;
    xformRef.current = { tx: 0, ty: 0, scale: 1 };
  }, [initNodes, initEdges]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;

    function resize() {
      if (!canvas) return;
      const r = canvas.getBoundingClientRect();
      canvas.width  = r.width  * dpr;
      canvas.height = r.height * dpr;
    }
    resize();
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);

    const ctx = canvas.getContext('2d')!;

    function frame() {
      const W = canvas!.width / dpr, H = canvas!.height / dpr;
      const cx = W / 2, cy = H / 2;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      if (ticksRef.current < 450) {
        tick(nodesRef.current, edgesRef.current, cx, cy);
        ticksRef.current++;
        mapRef.current = new Map(nodesRef.current.map(n => [n.id, n]));
        if (ticksRef.current === 80 && !autoFittedRef.current) {
          autoFittedRef.current = true;
          fitView();
        }
      }
      // Animate pan/zoom toward target (ease-out cubic)
      const anim = animRef.current;
      if (anim) {
        const t = Math.min((performance.now() - anim.startMs) / anim.durMs, 1);
        const e = 1 - Math.pow(1 - t, 3);
        xformRef.current = {
          tx:    anim.from.tx    + (anim.to.tx    - anim.from.tx)    * e,
          ty:    anim.from.ty    + (anim.to.ty    - anim.from.ty)    * e,
          scale: anim.from.scale + (anim.to.scale - anim.from.scale) * e,
        };
        if (t >= 1) animRef.current = null;
      }
      drawGraph(ctx, nodesRef.current, edgesRef.current, mapRef.current, xformRef.current, hovRef.current, selRef.current);
      rafRef.current = requestAnimationFrame(frame);
    }
    rafRef.current = requestAnimationFrame(frame);

    // Wheel — must be non-passive for preventDefault
    function onWheel(e: WheelEvent) {
      e.preventDefault();
      animRef.current = null;
      const rect = canvas!.getBoundingClientRect();
      const mx = e.clientX - rect.left, my = e.clientY - rect.top;
      const factor = e.deltaY < 0 ? 1.12 : 0.89;
      const t = xformRef.current;
      t.tx = mx - (mx - t.tx) * factor;
      t.ty = my - (my - t.ty) * factor;
      t.scale = Math.max(0.12, Math.min(5, t.scale * factor));
    }
    canvas.addEventListener('wheel', onWheel, { passive: false });

    return () => {
      cancelAnimationFrame(rafRef.current);
      ro.disconnect();
      canvas.removeEventListener('wheel', onWheel);
    };
  }, []);

  useImperativeHandle(ref, () => ({
    zoomIn() {
      const t = xformRef.current;
      const factor = 1.25;
      t.tx = t.tx * factor;
      t.ty = t.ty * factor;
      t.scale = Math.min(5, t.scale * factor);
    },
    zoomOut() {
      const t = xformRef.current;
      const factor = 0.8;
      t.tx = t.tx * factor;
      t.ty = t.ty * factor;
      t.scale = Math.max(0.12, t.scale * factor);
    },
    resetView() {
      fitView();
    },
  }));

  function worldPt(e: React.MouseEvent) {
    const rect = canvasRef.current!.getBoundingClientRect();
    const mx = e.clientX - rect.left, my = e.clientY - rect.top;
    const t = xformRef.current;
    return { wx: (mx - t.tx) / t.scale, wy: (my - t.ty) / t.scale };
  }

  function hit(wx: number, wy: number): GNode | null {
    for (let i = nodesRef.current.length - 1; i >= 0; i--) {
      const n = nodesRef.current[i];
      const dx = wx - n.x, dy = wy - n.y;
      if (dx * dx + dy * dy <= (n.r + 5) * (n.r + 5)) return n;
    }
    return null;
  }

  function onMouseMove(e: React.MouseEvent) {
    const { wx, wy } = worldPt(e);
    if (dragRef.current) {
      dragRef.current.moved = true;
      if (dragRef.current.nodeId) {
        const n = mapRef.current.get(dragRef.current.nodeId);
        if (n) { n.x = wx; n.y = wy; n.vx = 0; n.vy = 0; ticksRef.current = 0; }
      } else {
        xformRef.current.tx += e.movementX;
        xformRef.current.ty += e.movementY;
      }
    } else {
      const h = hit(wx, wy);
      hovRef.current = h?.id ?? null;
      canvasRef.current!.style.cursor = h ? 'pointer' : 'grab';
    }
  }

  function onMouseDown(e: React.MouseEvent) {
    animRef.current = null;
    const { wx, wy } = worldPt(e);
    const h = hit(wx, wy);
    dragRef.current = { nodeId: h?.id ?? null, moved: false };
    canvasRef.current!.style.cursor = 'grabbing';
  }

  function onMouseUp(e: React.MouseEvent) {
    const drag = dragRef.current;
    dragRef.current = null;
    canvasRef.current!.style.cursor = 'grab';
    if (!drag || drag.moved) return;
    const { wx, wy } = worldPt(e);
    const h = hit(wx, wy);
    selRef.current = h?.id ?? null;
    onSelect(h ?? null);
  }

  return (
    <canvas
      ref={canvasRef}
      className="w-full h-full block cursor-grab"
      onMouseMove={onMouseMove}
      onMouseDown={onMouseDown}
      onMouseUp={onMouseUp}
      onMouseLeave={() => { hovRef.current = null; dragRef.current = null; }}
    />
  );
});

// ── Empty state ───────────────────────────────────────────────────────────────

function EmptyState({ rebuilding, onRebuild }: { rebuilding: boolean; onRebuild: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center p-8">
      <div className="w-14 h-14 rounded-2xl bg-slate-800 border border-slate-700 flex items-center justify-center mb-4">
        <svg className="w-7 h-7 text-[#ef8c74]" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.4} d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7" />
        </svg>
      </div>
      <h3 className="text-sm font-semibold text-slate-200 mb-1">Knowledge graph is empty</h3>
      <p className="text-xs text-slate-400 mb-5 leading-relaxed">
        Youma ingests your memories, conversations<br />and connected services, then builds an AI summary tree.
      </p>
      <button
        type="button"
        onClick={onRebuild}
        disabled={rebuilding}
        className="inline-flex items-center gap-2 px-5 py-2.5 bg-[#ef8c74] hover:bg-[#e07a5f] disabled:opacity-50 text-white text-xs font-semibold rounded-xl transition-colors shadow-md"
      >
        {rebuilding ? <Spinner /> : null}
        {rebuilding ? 'Building…' : 'Build Knowledge Graph'}
      </button>
    </div>
  );
}

// ── WikiTab ───────────────────────────────────────────────────────────────────

export default function WikiTab() {
  const [gNodes, setGNodes]           = useState<GNode[]>([]);
  const [gEdges, setGEdges]           = useState<GEdge[]>([]);
  const [syntheticTopics, setSyntheticTopics] = useState(false);
  const [selNode, setSelNode]         = useState<GNode | null>(null);
  const [rebuilding, setRebuilding]   = useState(false);
  const [syncing, setSyncing]         = useState(false);
  const [lastBuild, setLastBuild]     = useState('');
  const [vaultPath, setVaultPath]     = useState('');
  const [vaultFiles, setVaultFiles]   = useState(0);
  const [editPath, setEditPath]       = useState(false);
  const [draftPath, setDraftPath]     = useState('');
  const [loaded, setLoaded]           = useState(false);
  const [error, setError]             = useState<string | null>(null);

  // Raw data stored for filter-reactive graph rebuilds
  const [allChunks, setAllChunks]         = useState<KnowledgeChunk[]>([]);
  const [allTrees, setAllTrees]           = useState<KnowledgeTreeNode[]>([]);
  const [sourceFilter, setSourceFilter]   = useState<Set<string>>(new Set());
  const [availableSources, setAvailableSources] = useState<string[]>([]);

  const graphRef = useRef<ForceGraphHandle>(null);

  const load = useCallback(async () => {
    try {
      const [trees, chunks, lb, vf, vp] = await Promise.all([
        knowledgeTreeNodes(),
        knowledgeChunksGraph(40), // top 40 per topic — all documents always represented
        settingsGet('knowledge_last_build'),
        settingsGet('knowledge_vault_files_count'),
        knowledgeVaultPathGet(),
      ]);
      setAllChunks(chunks);
      setAllTrees(trees);
      setAvailableSources([...new Set(chunks.map(c => c.source_type).filter(Boolean))].sort());
      setLastBuild(lb ?? '');
      setVaultFiles(Number(vf ?? '0'));
      setVaultPath(vp);
      setDraftPath(vp);
    } catch (e) { setError(String(e)); }
    finally { setLoaded(true); }
  }, []);

  // Rebuild graph whenever data or filter changes
  useEffect(() => {
    if (!loaded) return;
    const filtered = sourceFilter.size > 0
      ? allChunks.filter(c => sourceFilter.has(c.source_type))
      : allChunks;
    const { nodes, edges, synthetic } = buildGraph(allTrees, filtered, 800, 480);
    setGNodes(nodes);
    setGEdges(edges);
    setSyntheticTopics(synthetic);
  }, [allChunks, allTrees, sourceFilter, loaded]);

  function toggleFilter(src: string) {
    setSourceFilter(prev => {
      const next = new Set(prev);
      if (next.has(src)) next.delete(src);
      else next.add(src);
      return next;
    });
  }

  useEffect(() => { load(); }, [load]);

  async function handleRebuild() {
    setRebuilding(true);
    try {
      // Snapshot the current build timestamp before triggering.
      const beforeBuild = await settingsGet('knowledge_last_build') ?? '';
      await knowledgeRebuild();
      // Poll until knowledge_last_build advances (set by build_tree on completion).
      // This avoids loading partial data while the backend is mid-rebuild.
      const deadline = Date.now() + 180_000; // 3-min hard cap
      while (Date.now() < deadline) {
        await new Promise<void>(r => setTimeout(r, 2_000));
        const current = await settingsGet('knowledge_last_build') ?? '';
        if (current !== beforeBuild) {
          await load();
          break;
        }
      }
    } catch (e) { setError(String(e)); }
    finally { setRebuilding(false); }
  }

  async function handleSync() {
    setSyncing(true);
    try {
      const count = await knowledgeVaultSync();
      setVaultFiles(count);
    } catch (e) { setError(String(e)); }
    finally { setSyncing(false); }
  }

  async function handleSavePath() {
    try {
      await knowledgeVaultPathSet(draftPath);
      setVaultPath(draftPath);
      setEditPath(false);
    } catch (e) { setError(String(e)); }
  }

  async function handleDeleteChunk(id: string) {
    try {
      await knowledgeChunkDelete(id);
      setSelNode(null);
      setGNodes(prev => prev.filter(n => n.id !== `chunk:${id}`));
      setGEdges(prev => prev.filter(e => e.t !== `chunk:${id}`));
    } catch (e) { setError(String(e)); }
  }

  const hasData = gNodes.length > 1;

  return (
    <div className="flex flex-col gap-3 h-full">

      {/* Controls */}
      <div className="flex flex-col gap-2 bg-white border border-stone-200 rounded-xl px-3 py-2.5 shadow-sm">
        {/* Row 1: actions + vault path */}
        <div className="flex items-center gap-2 flex-wrap">
          <button
            type="button"
            onClick={handleRebuild}
            disabled={rebuilding}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-primary-500 hover:bg-primary-600 disabled:opacity-50 text-white rounded-lg transition-colors"
          >
            {rebuilding ? <Spinner /> : (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            )}
            {rebuilding ? 'Rebuilding…' : 'Rebuild'}
          </button>

          <button
            type="button"
            onClick={handleSync}
            disabled={syncing}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-white border border-stone-200 hover:border-stone-300 disabled:opacity-50 text-stone-700 rounded-lg transition-colors"
          >
            {syncing ? <Spinner /> : (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z" />
              </svg>
            )}
            {syncing ? 'Syncing…' : `Sync Vault${vaultFiles > 0 ? ` (${vaultFiles})` : ''}`}
          </button>

          <div className="flex items-center gap-1.5 flex-1 min-w-0 border-l border-stone-100 pl-3 ml-1">
            <svg className="w-3 h-3 text-stone-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
            {editPath ? (
              <>
                <input
                  aria-label="Vault path"
                  value={draftPath}
                  onChange={e => setDraftPath(e.target.value)}
                  onKeyDown={e => { if (e.key === 'Enter') handleSavePath(); if (e.key === 'Escape') setEditPath(false); }}
                  className="flex-1 font-mono text-[11px] border border-stone-200 rounded px-2 py-0.5 focus:border-primary-400 outline-none min-w-0"
                />
                <button type="button" onClick={handleSavePath} className="text-[11px] text-primary-600 font-medium flex-shrink-0">Save</button>
                <button type="button" onClick={() => setEditPath(false)} className="text-[11px] text-stone-400 flex-shrink-0">Cancel</button>
              </>
            ) : (
              <>
                <span className="flex-1 font-mono text-[11px] text-stone-500 truncate">{vaultPath || '(default)'}</span>
                <button type="button" onClick={() => { setDraftPath(vaultPath); setEditPath(true); }} className="text-[11px] text-stone-400 hover:text-primary-600 flex-shrink-0">Edit</button>
              </>
            )}
          </div>

          {lastBuild && (
            <span className="text-[10px] text-stone-400 flex-shrink-0">Rebuilt {formatRel(lastBuild)}</span>
          )}
        </div>

        {/* Row 2: legend + source filters (shown only when graph has data) */}
        {hasData && (
          <div className="flex items-center gap-2.5 flex-wrap border-t border-stone-100 pt-1.5">
            <span className="flex items-center gap-1 text-[10px] text-stone-500 pointer-events-none">
              <span className="w-2.5 h-2.5 rounded-full inline-block flex-shrink-0" style={{ background: COL_ROOT }} />Root
            </span>
            <span className="flex items-center gap-1 text-[10px] text-stone-500 pointer-events-none">
              <span className="w-2 h-2 rounded-full inline-block flex-shrink-0" style={{ background: COL_TOPIC }} />Topic
            </span>
            {availableSources.length > 0 && (
              <>
                <span className="w-px h-3 bg-stone-200 flex-shrink-0" />
                {availableSources.map(src => {
                  const color = chunkColor(src);
                  const active = sourceFilter.size === 0 || sourceFilter.has(src);
                  return (
                    <button
                      key={src}
                      type="button"
                      onClick={() => toggleFilter(src)}
                      className="flex items-center gap-1 text-[10px] font-medium rounded-md px-1.5 py-0.5 transition-all"
                      style={{
                        color: active ? color : 'rgba(100,116,139,0.5)',
                        background: active ? `${color}18` : 'transparent',
                      }}
                    >
                      <span className="w-1.5 h-1.5 rounded-full flex-shrink-0 transition-all"
                        style={{ background: active ? color : 'rgba(100,116,139,0.35)' }} />
                      {src}
                      {sourceFilter.has(src) && <span className="ml-0.5 text-[9px] opacity-60">✓</span>}
                    </button>
                  );
                })}
                {sourceFilter.size > 0 && (
                  <button
                    type="button"
                    onClick={() => setSourceFilter(new Set())}
                    className="text-[9px] text-stone-400 hover:text-stone-600 transition-colors"
                  >
                    reset
                  </button>
                )}
              </>
            )}
          </div>
        )}
      </div>

      {error && (
        <div className="px-4 py-2 bg-red-50 border border-red-200 rounded-xl text-xs text-red-700 flex justify-between items-center">
          <span>{error}</span>
          <button type="button" aria-label="Dismiss" onClick={() => setError(null)} className="ml-2 text-red-400 hover:text-red-600">
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      )}

      {/* Graph canvas */}
      <div className="relative rounded-2xl overflow-hidden flex-1 min-h-0 bg-[#0f172a]">
        {!loaded ? (
          <div className="flex items-center justify-center h-full">
            <div className="w-6 h-6 border-2 border-slate-700 border-t-slate-400 rounded-full animate-spin" />
          </div>
        ) : !hasData ? (
          <EmptyState rebuilding={rebuilding} onRebuild={handleRebuild} />
        ) : (
          <>
            <ForceGraph ref={graphRef} nodes={gNodes} edges={gEdges} onSelect={setSelNode} />

            {selNode && (
              <NodePanel
                node={selNode}
                onClose={() => setSelNode(null)}
                onDeleteChunk={handleDeleteChunk}
              />
            )}

            {/* Synthetic-topics banner */}
            {syntheticTopics && (
              <div className="absolute bottom-12 left-3 right-3 mx-auto max-w-xs bg-slate-800/90 backdrop-blur-sm border border-slate-600 rounded-xl px-3 py-2 text-center z-10">
                <p className="text-[11px] text-slate-300">
                  Topic derivati dai metadati.{' '}
                  <button
                    type="button"
                    onClick={handleRebuild}
                    disabled={rebuilding}
                    className="text-[#ef8c74] hover:text-[#e07a5f] font-medium underline underline-offset-2"
                  >
                    Rebuild
                  </button>
                  {' '}per sommari AI.
                </p>
              </div>
            )}


            {/* Zoom controls */}
            <div className="absolute bottom-3 right-3 flex flex-col gap-1 z-10">
              <button type="button" aria-label="Zoom in"
                onClick={() => graphRef.current?.zoomIn()}
                className="w-8 h-8 bg-slate-800/90 border border-slate-600 text-slate-300 hover:text-white rounded-lg flex items-center justify-center text-lg font-bold leading-none">
                +
              </button>
              <button type="button" aria-label="Zoom out"
                onClick={() => graphRef.current?.zoomOut()}
                className="w-8 h-8 bg-slate-800/90 border border-slate-600 text-slate-300 hover:text-white rounded-lg flex items-center justify-center text-lg font-bold leading-none">
                −
              </button>
              <button type="button" aria-label="Reset view"
                onClick={() => graphRef.current?.resetView()}
                className="w-8 h-8 bg-slate-800/90 border border-slate-600 text-slate-300 hover:text-white rounded-lg flex items-center justify-center">
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
                </svg>
              </button>
            </div>

            {/* Zoom/interaction hint */}
            <div className="absolute bottom-12 right-3 text-[10px] text-slate-600 pointer-events-none z-10">
              Scroll · Drag · Click
            </div>
          </>
        )}
      </div>
    </div>
  );
}
