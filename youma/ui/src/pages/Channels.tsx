// Channels page — Composio-powered 118+ third-party integrations.
// Shows available apps from the Composio catalogue and lets users connect/disconnect
// services via one-click OAuth. Connected apps show their last-fetched timestamp.

import { useState, useEffect, useCallback } from 'react';
import {
  type ComposioApp,
  type ComposioConnection,
  composioListApps,
  composioListConnections,
  composioConnect,
  composioPoll,
  composioDisconnect,
} from '../api';

// ---------------------------------------------------------------------------
// Icon — generic plug icon for apps without a logo
// ---------------------------------------------------------------------------

function PlugIcon() {
  return (
    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.8}
        d="M12 9v3m0 0v3m0-3h3m-3 0H9m12 0a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Status badge
// ---------------------------------------------------------------------------

function StatusBadge({ status }: { status: string }) {
  const cfg: Record<string, { label: string; classes: string }> = {
    ACTIVE:    { label: 'Connected', classes: 'bg-sage-100 text-sage-700 border-sage-200' },
    INITIATED: { label: 'Connecting…', classes: 'bg-amber-50 text-amber-600 border-amber-200' },
    FAILED:    { label: 'Failed', classes: 'bg-coral-50 text-coral-600 border-coral-200' },
  };
  const { label, classes } = cfg[status] ?? { label: status, classes: 'bg-stone-100 text-stone-500 border-stone-200' };
  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium border ${classes}`}>
      {label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Single app row
// ---------------------------------------------------------------------------

interface AppRowProps {
  app: ComposioApp;
  connection: ComposioConnection | undefined;
  onConnect: (appName: string) => void;
  onDisconnect: (connectionId: string) => void;
  onPoll: (connectionId: string) => void;
  loading: boolean;
}

function AppRow({ app, connection, onConnect, onDisconnect, onPoll, loading }: AppRowProps) {
  const isConnected = connection?.status === 'ACTIVE';
  const isInitiated = connection?.status === 'INITIATED';

  return (
    <div className="bg-white rounded-xl border border-stone-200 shadow-soft p-4 flex items-center gap-4">
      {/* Logo */}
      <div className="w-11 h-11 rounded-xl bg-stone-50 border border-stone-200 flex items-center justify-center flex-shrink-0 text-stone-400 overflow-hidden">
        {app.logo_url ? (
          <img src={app.logo_url} alt={app.display_name} className="w-7 h-7 object-contain" />
        ) : (
          <PlugIcon />
        )}
      </div>

      {/* Info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-sm font-semibold text-stone-900">{app.display_name}</span>
          {connection && <StatusBadge status={connection.status} />}
        </div>
        <p className="text-xs text-stone-500 mt-0.5 truncate">
          {app.description || app.auth_mode}
        </p>
        {connection?.last_fetched_at && (
          <p className="text-[10px] text-stone-400 mt-0.5">
            Last sync: {new Date(connection.last_fetched_at).toLocaleString()}
          </p>
        )}
      </div>

      {/* Actions */}
      <div className="flex gap-1.5 flex-shrink-0">
        {isInitiated && (
          <button
            type="button"
            onClick={() => onPoll(connection!.id)}
            disabled={loading}
            className="px-3 py-1.5 rounded-lg text-xs font-medium bg-amber-50 hover:bg-amber-100 text-amber-700 border border-amber-200 transition-colors disabled:opacity-50"
          >
            Check
          </button>
        )}
        {isConnected ? (
          <button
            type="button"
            onClick={() => onDisconnect(connection!.id)}
            disabled={loading}
            className="px-3 py-1.5 rounded-lg text-xs font-medium bg-coral-50 hover:bg-coral-100 text-coral-600 border border-coral-200 transition-colors disabled:opacity-50"
          >
            Disconnect
          </button>
        ) : !isInitiated ? (
          <button
            type="button"
            onClick={() => onConnect(app.name)}
            disabled={loading}
            className="px-3 py-1.5 rounded-lg text-xs font-medium bg-primary-500 hover:bg-primary-600 text-white transition-colors disabled:opacity-50"
          >
            Connect
          </button>
        ) : null}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

const POPULAR = ['gmail', 'github', 'notion', 'slack', 'googlecalendar', 'googledrive',
  'linear', 'jira', 'stripe', 'hubspot', 'asana', 'discord', 'dropbox', 'airtable', 'trello'];

export default function ChannelsPage() {
  const [apps, setApps] = useState<ComposioApp[]>([]);
  const [connections, setConnections] = useState<ComposioConnection[]>([]);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [noKey, setNoKey] = useState(false);

  const load = useCallback(async () => {
    try {
      const [appsRes, connsRes] = await Promise.all([
        composioListApps(),
        composioListConnections(),
      ]);
      setNoKey(appsRes.length === 0);
      setApps(appsRes);
      setConnections(connsRes);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  async function handleConnect(appName: string) {
    setLoading(true); setError(null); setNotice(null);
    try {
      await composioConnect(appName);
      setNotice(`Browser opened — complete the OAuth flow then click "Check" to confirm.`);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleDisconnect(connectionId: string) {
    setLoading(true); setError(null);
    try {
      await composioDisconnect(connectionId);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handlePoll(connectionId: string) {
    setLoading(true); setError(null);
    try {
      const res = await composioPoll(connectionId);
      if (res.status === 'ACTIVE') {
        setNotice('Connected successfully.');
      } else {
        setNotice(`Status: ${res.status}. Complete the OAuth flow in the browser, then check again.`);
      }
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  const connMap = new Map(connections.map(c => [c.app_name, c]));

  // Sort: connected first, then popular order, then alphabetical.
  const filtered = apps
    .filter(a =>
      !search ||
      a.display_name.toLowerCase().includes(search.toLowerCase()) ||
      a.name.toLowerCase().includes(search.toLowerCase())
    )
    .sort((a, b) => {
      const aConn = connMap.has(a.name) ? 0 : 1;
      const bConn = connMap.has(b.name) ? 0 : 1;
      if (aConn !== bConn) return aConn - bConn;
      const aIdx = POPULAR.indexOf(a.name);
      const bIdx = POPULAR.indexOf(b.name);
      if (aIdx !== -1 && bIdx !== -1) return aIdx - bIdx;
      if (aIdx !== -1) return -1;
      if (bIdx !== -1) return 1;
      return a.display_name.localeCompare(b.display_name);
    });

  const connectedCount = connections.filter(c => c.status === 'ACTIVE').length;

  return (
    <div className="max-w-lg mx-auto px-4 py-6">
      {/* Header */}
      <div className="mb-4">
        <h1 className="text-xl font-bold text-stone-900 mb-1">Integrations</h1>
        <p className="text-sm text-stone-500">
          {connectedCount > 0
            ? `${connectedCount} service${connectedCount > 1 ? 's' : ''} connected — data auto-fetched every 20 min.`
            : 'Connect Gmail, GitHub, Notion, Slack and 100+ more services.'}
        </p>
      </div>

      {/* No key callout */}
      {noKey && (
        <div className="mb-4 bg-amber-50 border border-amber-200 rounded-xl p-4">
          <p className="text-xs text-amber-800 font-medium mb-1">Composio API key required</p>
          <p className="text-xs text-amber-700">
            Add your <strong>COMPOSIO_API_KEY</strong> in{' '}
            <strong>Settings</strong> to unlock 118+ integrations.
            Get a free key at{' '}
            <span className="font-mono">composio.dev</span>.
          </p>
        </div>
      )}

      {/* Notices */}
      {notice && (
        <div className="mb-4 bg-sage-50 border border-sage-200 rounded-xl p-3 flex items-start gap-2">
          <svg className="w-4 h-4 text-sage-600 mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
          </svg>
          <p className="text-xs text-sage-700">{notice}</p>
          <button type="button" aria-label="Dismiss" onClick={() => setNotice(null)} className="ml-auto text-sage-400 hover:text-sage-600">
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      )}
      {error && (
        <div className="mb-4 bg-coral-50 border border-coral-200 rounded-xl p-3">
          <p className="text-xs text-coral-700">{error}</p>
        </div>
      )}

      {/* Search */}
      {apps.length > 6 && (
        <div className="mb-4 relative">
          <svg className="w-4 h-4 text-stone-400 absolute left-3 top-1/2 -translate-y-1/2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            type="text"
            aria-label="Search integrations"
            placeholder="Search integrations…"
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="w-full pl-9 pr-4 py-2.5 rounded-xl border border-stone-200 bg-white text-sm text-stone-900 placeholder-stone-400 focus:border-primary-400 transition-colors"
          />
        </div>
      )}

      {/* App list */}
      <div className="space-y-2.5">
        {filtered.length === 0 && !noKey && (
          <p className="text-sm text-stone-400 text-center py-8">
            {search ? `No integrations match "${search}"` : 'Loading integrations…'}
          </p>
        )}
        {filtered.map(app => (
          <AppRow
            key={app.name}
            app={app}
            connection={connMap.get(app.name)}
            onConnect={handleConnect}
            onDisconnect={handleDisconnect}
            onPoll={handlePoll}
            loading={loading}
          />
        ))}
      </div>

      {/* Info footer */}
      {!noKey && (
        <div className="mt-6 bg-primary-50 rounded-xl border border-primary-200 p-4">
          <div className="flex items-start gap-3">
            <svg className="w-4 h-4 text-primary-500 mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <p className="text-xs text-primary-700">
              Connections are managed by Composio — Youma never stores your service credentials.
              OAuth flows open in your default browser. Data is fetched every 20 minutes and
              stored as memory notes, giving Youma live context from your tools.
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
