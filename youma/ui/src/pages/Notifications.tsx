import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { inboxList, inboxMarkRead, inboxRefresh, inboxSend } from '../api';
import type { InboxMessage } from '../api';

// ---------------------------------------------------------------------------
// Service config
// ---------------------------------------------------------------------------

const SERVICE_LABEL: Record<string, string> = {
  gmail: 'Gmail',
  slack: 'Slack',
  discord: 'Discord',
  hubspot: 'HubSpot',
};

const SERVICE_COLOR: Record<string, string> = {
  gmail:   'bg-[#EA4335] text-white',
  slack:   'bg-[#4A154B] text-white',
  discord: 'bg-[#5865F2] text-white',
  hubspot: 'bg-[#FF7A59] text-white',
};

function serviceLabel(s: string) { return SERVICE_LABEL[s] ?? s; }
function serviceColor(s: string) { return SERVICE_COLOR[s] ?? 'bg-stone-600 text-white'; }

function serviceInitial(s: string) {
  const l = SERVICE_LABEL[s] ?? s;
  return l.slice(0, 2).toUpperCase();
}

// ---------------------------------------------------------------------------
// Compose modal
// ---------------------------------------------------------------------------

interface ComposeState {
  service: string;
  to: string;
  subject: string;
  body: string;
}

function ComposeModal({
  initial,
  onClose,
  onSent,
}: {
  initial: Partial<ComposeState>;
  onClose: () => void;
  onSent: () => void;
}) {
  const { t } = useTranslation();
  const [form, setForm] = useState<ComposeState>({
    service: initial.service ?? 'gmail',
    to: initial.to ?? '',
    subject: initial.subject ?? '',
    body: initial.body ?? '',
  });
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');

  const services = Object.keys(SERVICE_LABEL);

  async function handleSend() {
    if (!form.to.trim() || !form.body.trim()) {
      setError(t('inbox.recipient_required'));
      return;
    }
    setSending(true);
    setError('');
    try {
      await inboxSend({
        service: form.service,
        to: form.to,
        subject: form.subject,
        body: form.body,
      });
      onSent();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm">
      <div className="bg-white rounded-2xl shadow-xl w-full max-w-lg mx-4 p-5 flex flex-col gap-4">
        <div className="flex items-center justify-between">
          <h2 className="font-semibold text-stone-900">{t('inbox.new_message')}</h2>
          <button type="button" onClick={onClose} aria-label={t('inbox.close')} className="text-stone-400 hover:text-stone-700">
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <select
          value={form.service}
          onChange={e => setForm(f => ({ ...f, service: e.target.value }))}
          className="w-full border border-stone-200 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary-400"
          aria-label={t('inbox.service')}
        >
          {services.map(s => (
            <option key={s} value={s}>{serviceLabel(s)}</option>
          ))}
        </select>

        <input
          type="text"
          placeholder={t('inbox.to_placeholder')}
          value={form.to}
          onChange={e => setForm(f => ({ ...f, to: e.target.value }))}
          className="w-full border border-stone-200 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary-400"
        />

        {(form.service === 'gmail' || form.service === 'hubspot') && (
          <input
            type="text"
            placeholder={t('inbox.subject_placeholder')}
            value={form.subject}
            onChange={e => setForm(f => ({ ...f, subject: e.target.value }))}
            className="w-full border border-stone-200 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary-400"
          />
        )}

        <textarea
          placeholder={t('inbox.message_placeholder')}
          value={form.body}
          onChange={e => setForm(f => ({ ...f, body: e.target.value }))}
          rows={5}
          className="w-full border border-stone-200 rounded-lg px-3 py-2 text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary-400"
        />

        {error && <p className="text-xs text-coral-600">{error}</p>}

        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-sm text-stone-600 hover:text-stone-900 rounded-lg hover:bg-stone-100"
          >
            {t('inbox.cancel')}
          </button>
          <button
            type="button"
            onClick={handleSend}
            disabled={sending}
            className="px-4 py-2 text-sm bg-primary-500 text-white rounded-lg hover:bg-primary-600 disabled:opacity-50"
          >
            {sending ? t('inbox.sending') : t('inbox.send')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Message detail panel
// ---------------------------------------------------------------------------

function MessageDetail({
  msg,
  onClose,
  onReply,
}: {
  msg: InboxMessage;
  onClose: () => void;
  onReply: (initial: Partial<ComposeState>) => void;
}) {
  const { t } = useTranslation();
  const ts = new Date(msg.received_at).toLocaleString(undefined, {
    month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
  });

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 px-4 py-3 border-b border-stone-100">
        <button type="button" onClick={onClose} aria-label={t('inbox.back_to_inbox')} className="text-stone-400 hover:text-stone-700 mr-1">
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <span className={`inline-flex items-center justify-center w-6 h-6 rounded text-[10px] font-bold flex-shrink-0 ${serviceColor(msg.service)}`}>
          {serviceInitial(msg.service)}
        </span>
        <span className="text-xs text-stone-500 font-medium">{serviceLabel(msg.service)}</span>
      </div>

      <div className="flex-1 overflow-y-auto px-5 py-4">
        <h3 className="text-base font-semibold text-stone-900 mb-1 leading-snug">
          {msg.subject || t('inbox.no_subject')}
        </h3>
        <div className="flex items-center gap-2 mb-4">
          <span className="text-xs text-stone-500">{msg.sender || t('inbox.unknown_sender')}</span>
          <span className="text-stone-300">·</span>
          <span className="text-xs text-stone-400">{ts}</span>
        </div>
        <div className="text-sm text-stone-700 leading-relaxed whitespace-pre-wrap break-words">
          {msg.body || <em className="text-stone-400">{t('inbox.no_content')}</em>}
        </div>
      </div>

      <div className="border-t border-stone-100 px-4 py-3">
        <button
          type="button"
          onClick={() => onReply({ service: msg.service, to: msg.sender, subject: `Re: ${msg.subject}` })}
          className="flex items-center gap-2 px-3 py-1.5 text-sm bg-primary-500 text-white rounded-lg hover:bg-primary-600"
        >
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6" />
          </svg>
          {t('inbox.reply')}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main inbox page
// ---------------------------------------------------------------------------

export default function InboxPage() {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<InboxMessage[]>([]);
  const [unread, setUnread] = useState(0);
  const [activeService, setActiveService] = useState<string | null>(null);
  const [selected, setSelected] = useState<InboxMessage | null>(null);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [compose, setCompose] = useState<Partial<ComposeState> | null>(null);
  const [error, setError] = useState('');
  const loadedRef = useRef(false);

  const load = useCallback(async (service?: string) => {
    setLoading(true);
    setError('');
    try {
      const res = await inboxList({ service: service ?? undefined, limit: 100 });
      setMessages(res.result);
      setUnread(res.unread);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!loadedRef.current) {
      loadedRef.current = true;
      load(activeService ?? undefined);
    }
  }, [load, activeService]);

  async function handleRefresh() {
    setRefreshing(true);
    setError('');
    try {
      await inboxRefresh();
      await load(activeService ?? undefined);
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshing(false);
    }
  }

  async function handleSelect(msg: InboxMessage) {
    setSelected(msg);
    if (!msg.read) {
      try {
        await inboxMarkRead(msg.id);
        setMessages(prev => prev.map(m => m.id === msg.id ? { ...m, read: true } : m));
        setUnread(prev => Math.max(0, prev - 1));
      } catch { /* non-fatal */ }
    }
  }

  function handleServiceFilter(svc: string | null) {
    setActiveService(svc);
    setSelected(null);
    loadedRef.current = false;
    load(svc ?? undefined);
  }

  const services = Array.from(new Set(messages.map(m => m.service)));

  const relativeTs = (iso: string) => {
    const d = new Date(iso);
    const diffMs = Date.now() - d.getTime();
    if (diffMs < 60_000) return t('inbox.just_now');
    if (diffMs < 3_600_000) return t('inbox.minutes_ago', { count: Math.floor(diffMs / 60_000) });
    if (diffMs < 86_400_000) return t('inbox.hours_ago', { count: Math.floor(diffMs / 3_600_000) });
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  };

  return (
    <div className="flex h-full min-h-0">
      {/* ---- Left: message list ---- */}
      <div className="w-[42%] flex-shrink-0 flex flex-col border-r border-stone-200 bg-white">

        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-stone-100">
          <div className="flex items-center gap-2">
            <h1 className="text-base font-semibold text-stone-900">{t('inbox.title')}</h1>
            {unread > 0 && (
              <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-semibold bg-primary-100 text-primary-700">
                {unread}
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleRefresh}
              disabled={refreshing}
              title={t('inbox.refresh')}
              className="p-1.5 rounded-lg text-stone-400 hover:text-stone-700 hover:bg-stone-100 disabled:opacity-40"
            >
              <svg className={`w-4 h-4 ${refreshing ? 'animate-spin' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            </button>
            <button
              type="button"
              onClick={() => setCompose({})}
              title={t('inbox.compose')}
              className="p-1.5 rounded-lg bg-primary-500 text-white hover:bg-primary-600"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
            </button>
          </div>
        </div>

        {/* Service filter chips */}
        {services.length > 1 && (
          <div className="flex gap-1.5 px-4 py-2 border-b border-stone-100 overflow-x-auto">
            <button
              type="button"
              onClick={() => handleServiceFilter(null)}
              className={`flex-shrink-0 px-2.5 py-1 rounded-full text-xs font-medium transition-colors ${
                !activeService ? 'bg-primary-500 text-white' : 'bg-stone-100 text-stone-600 hover:bg-stone-200'
              }`}
            >
              {t('inbox.all')}
            </button>
            {services.map(svc => (
              <button
                type="button"
                key={svc}
                onClick={() => handleServiceFilter(svc)}
                className={`flex-shrink-0 px-2.5 py-1 rounded-full text-xs font-medium transition-colors ${
                  activeService === svc ? 'bg-primary-500 text-white' : 'bg-stone-100 text-stone-600 hover:bg-stone-200'
                }`}
              >
                {serviceLabel(svc)}
              </button>
            ))}
          </div>
        )}

        {/* List */}
        <div className="flex-1 overflow-y-auto">
          {error && (
            <div className="px-4 py-3 text-xs text-amber-700 bg-amber-50 border-b border-amber-100">{error}</div>
          )}

          {loading ? (
            <div className="flex items-center justify-center h-32">
              <div className="w-5 h-5 border-2 border-primary-400 border-t-transparent rounded-full animate-spin" />
            </div>
          ) : messages.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-48 text-center px-6">
              <div className="w-10 h-10 rounded-full bg-stone-100 flex items-center justify-center mb-2">
                <svg className="w-5 h-5 text-stone-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.8} d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
                </svg>
              </div>
              <p className="text-xs text-stone-400 leading-relaxed">
                {t('inbox.no_messages')}<br />
                {t('inbox.no_messages_hint')}
              </p>
            </div>
          ) : (
            messages.map(msg => (
              <button
                type="button"
                key={msg.id}
                onClick={() => handleSelect(msg)}
                className={`w-full text-left px-4 py-3 border-b border-stone-100 hover:bg-stone-50 transition-colors ${
                  selected?.id === msg.id ? 'bg-primary-50 border-l-2 border-l-primary-500' : ''
                }`}
              >
                <div className="flex items-start gap-2.5">
                  <span className={`inline-flex items-center justify-center w-6 h-6 rounded text-[10px] font-bold flex-shrink-0 mt-0.5 ${serviceColor(msg.service)}`}>
                    {serviceInitial(msg.service)}
                  </span>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center justify-between gap-1 mb-0.5">
                      <span className={`text-xs font-semibold truncate ${msg.read ? 'text-stone-600' : 'text-stone-900'}`}>
                        {msg.sender || t('inbox.unknown')}
                      </span>
                      <span className="text-[10px] text-stone-400 whitespace-nowrap flex-shrink-0">{relativeTs(msg.received_at)}</span>
                    </div>
                    <p className={`text-xs truncate ${msg.read ? 'text-stone-400' : 'text-stone-700 font-medium'}`}>
                      {msg.subject || msg.body.slice(0, 60) || t('inbox.empty')}
                    </p>
                    <p className="text-[10px] text-stone-400 truncate mt-0.5">
                      {msg.body.slice(0, 80)}
                    </p>
                  </div>
                  {!msg.read && (
                    <div className="w-1.5 h-1.5 rounded-full bg-primary-500 mt-1.5 flex-shrink-0" />
                  )}
                </div>
              </button>
            ))
          )}
        </div>
      </div>

      {/* ---- Right: detail panel ---- */}
      <div className="flex-1 flex flex-col min-w-0 bg-stone-50">
        {selected ? (
          <MessageDetail
            msg={selected}
            onClose={() => setSelected(null)}
            onReply={initial => setCompose(initial)}
          />
        ) : (
          <div className="flex flex-col items-center justify-center h-full text-center px-8 text-stone-400">
            <svg className="w-10 h-10 mb-3 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
            </svg>
            <p className="text-sm">{t('inbox.select_message')}</p>
            <p className="text-xs mt-1 opacity-70">{t('inbox.encrypted_desc')}</p>
          </div>
        )}
      </div>

      {/* Compose modal */}
      {compose !== null && (
        <ComposeModal
          initial={compose}
          onClose={() => setCompose(null)}
          onSent={() => load(activeService ?? undefined)}
        />
      )}
    </div>
  );
}
