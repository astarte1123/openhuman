import { useEffect, useState } from 'react';
import { generateInvite, listInvites } from '../api';
import type { InviteCode } from '../api';

function formatDate(iso: string) {
  try { return new Date(iso).toLocaleDateString([], { month: 'short', day: 'numeric' }); }
  catch { return ''; }
}

export default function InvitesPage() {
  const [invites, setInvites] = useState<InviteCode[]>([]);
  const [loading, setLoading] = useState(true);
  const [generating, setGenerating] = useState(false);
  const [copiedCode, setCopiedCode] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listInvites()
      .then(setInvites)
      .catch(e => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  async function handleGenerate() {
    setGenerating(true);
    try {
      const invite = await generateInvite();
      setInvites(prev => [invite, ...prev]);
    } catch (e) { setError(String(e)); }
    finally { setGenerating(false); }
  }

  async function handleCopy(code: string) {
    await navigator.clipboard.writeText(code).catch(() => {});
    setCopiedCode(code);
    setTimeout(() => setCopiedCode(null), 2000);
  }

  return (
    <div className="flex flex-col" style={{ height: 'calc(100vh - 72px)' }}>
      <div className="px-6 py-4 border-b border-stone-200 bg-white flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold text-stone-900">Invites</h1>
          <p className="text-xs text-stone-500 mt-0.5">Generate invite codes to share with friends</p>
        </div>
        <button
          onClick={handleGenerate}
          disabled={generating}
          className="flex items-center gap-2 px-4 py-2 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 text-white text-sm font-medium rounded-lg transition-colors"
        >
          {generating ? (
            <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"/>
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8z"/>
            </svg>
          ) : (
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4"/>
            </svg>
          )}
          Generate
        </button>
      </div>

      {error && (
        <div className="mx-4 mt-3 px-4 py-2 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700 flex justify-between">
          <span>{error}</span>
          <button onClick={() => setError(null)}>✕</button>
        </div>
      )}

      <div className="flex-1 overflow-y-auto bg-stone-50 px-4 py-4">
        {loading ? (
          <div className="space-y-2">{[1,2,3].map(i => <div key={i} className="h-14 bg-stone-200 rounded-xl animate-pulse" />)}</div>
        ) : invites.length === 0 ? (
          <div className="text-center py-16 text-stone-400">
            <div className="text-4xl mb-3">🎟️</div>
            <p className="text-sm font-medium">No invite codes yet.</p>
            <p className="text-xs mt-1">Generate one to share with friends.</p>
          </div>
        ) : (
          <div className="space-y-2">
            {invites.map(invite => (
              <div key={invite.code} className={`bg-white rounded-xl border border-stone-200 px-4 py-3 flex items-center gap-3 shadow-sm ${invite.used ? 'opacity-50' : ''}`}>
                <span className={`font-mono text-sm font-bold tracking-wider ${invite.used ? 'text-stone-400 line-through' : 'text-primary-600'}`}>
                  {invite.code}
                </span>
                <span className="flex-1 text-xs text-stone-400">{formatDate(invite.created_at)}</span>
                {invite.used ? (
                  <span className="text-xs text-stone-400 bg-stone-100 px-2 py-0.5 rounded-full">Used</span>
                ) : (
                  <button
                    onClick={() => handleCopy(invite.code)}
                    className={`text-xs px-3 py-1 rounded-lg font-medium transition-colors ${
                      copiedCode === invite.code ? 'bg-green-100 text-green-700' : 'bg-primary-50 text-primary-600 hover:bg-primary-100'
                    }`}
                  >
                    {copiedCode === invite.code ? 'Copied!' : 'Copy'}
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
