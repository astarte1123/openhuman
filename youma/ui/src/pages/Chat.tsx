import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { sanitizeAIContent } from '../utils/sanitize';
import {
  appendMessage,
  createThread,
  deleteThread,
  listMessages,
  listThreads,
  renameThread,
  type Message,
  type Thread,
} from '../api';
import { useTTS } from '../hooks/useTTS';
import { ThinkingWords } from '../components/ThinkingWords';
import { clearInflight, getInflight, setInflight } from '../lib/chatInflight';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface LocalMessage extends Message {
  id: string;
  pending?: boolean;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString([], { month: 'short', day: 'numeric' });
  } catch {
    return '';
  }
}

function uid(): string {
  return Math.random().toString(36).slice(2) + Date.now().toString(36);
}

// ---------------------------------------------------------------------------
// Thread list item
// ---------------------------------------------------------------------------

interface ThreadItemProps {
  thread: Thread;
  active: boolean;
  selected: boolean;
  onSelect: () => void;
  onToggleSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
}

function ThreadItem({ thread, active, selected, onSelect, onToggleSelect, onDelete, onRename }: ThreadItemProps) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(thread.title || t('chat.untitled_thread'));
  const inputRef = useRef<HTMLInputElement>(null);

  function startEdit(e: React.MouseEvent) {
    e.stopPropagation();
    setDraft(thread.title || t('chat.untitled_thread'));
    setEditing(true);
    setTimeout(() => inputRef.current?.select(), 0);
  }

  function commitEdit() {
    setEditing(false);
    const trimmed = draft.trim() || t('chat.untitled_thread');
    if (trimmed !== thread.title) onRename(thread.id, trimmed);
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter') commitEdit();
    if (e.key === 'Escape') setEditing(false);
  }

  return (
    <li>
      <div
        className={`group relative flex items-center gap-1 px-2 py-2 rounded-lg cursor-pointer transition-colors ${
          selected
            ? 'bg-primary-50 border border-primary-200 text-primary-700'
            : active
              ? 'bg-primary-50 text-primary-700 border border-primary-200'
              : 'text-stone-700 hover:bg-stone-100'
        }`}
        onClick={onSelect}
      >
        <input
          type="checkbox"
          checked={selected}
          onChange={e => { e.stopPropagation(); onToggleSelect(thread.id); }}
          onClick={e => e.stopPropagation()}
          className="flex-shrink-0 w-3.5 h-3.5 rounded border-stone-300 text-primary-500 accent-primary-500 mr-0.5"
          aria-label={thread.title || ''}
        />
        {editing ? (
          <input
            ref={inputRef}
            autoFocus
            aria-label={t('chat.thread_title')}
            placeholder={t('chat.thread_title')}
            value={draft}
            onChange={e => setDraft(e.target.value)}
            onBlur={commitEdit}
            onKeyDown={handleKeyDown}
            onClick={e => e.stopPropagation()}
            className="flex-1 text-sm px-1 py-0 rounded border border-primary-300 bg-white focus:outline-none min-w-0"
          />
        ) : (
          <div className="flex-1 min-w-0">
            <div className="text-sm font-medium truncate">
              {thread.title || t('chat.untitled_thread')}
            </div>
            <div className="text-xs text-stone-400 mt-0.5">{formatDate(thread.created_at)}</div>
          </div>
        )}

        {!editing && (
          <div className="hidden group-hover:flex items-center gap-0.5 flex-shrink-0">
            {/* rename */}
            <button
              type="button"
              title={t('chat.rename')}
              onClick={startEdit}
              className="p-1 rounded hover:bg-stone-200 text-stone-400 hover:text-stone-600"
            >
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                  d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
            </button>
            {/* delete */}
            <button
              type="button"
              title={t('chat.delete')}
              onClick={e => { e.stopPropagation(); onDelete(thread.id); }}
              className="p-1 rounded hover:bg-coral-100 text-stone-400 hover:text-coral-600"
            >
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                  d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          </div>
        )}
      </div>
    </li>
  );
}

// ---------------------------------------------------------------------------
// Message bubble with markdown + TTS button
// ---------------------------------------------------------------------------

function MessageBubble({ msg }: { msg: LocalMessage }) {
  const { t } = useTranslation();
  const isUser = msg.role === 'user';
  const { speaking, speak, stop } = useTTS();
  const clean = isUser ? msg.content : sanitizeAIContent(msg.content);
  const plainText = clean.replace(/[#*`_~[\]()]/g, '');
  const [copied, setCopied] = useState(false);

  function handleCopy() {
    void navigator.clipboard.writeText(clean).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }

  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'} group`}>
      {!isUser && !msg.pending && (
        <div className="w-7 h-7 rounded-full bg-primary-100 flex items-center justify-center flex-shrink-0 mr-2 mt-0.5">
          <svg className="w-3.5 h-3.5 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
            <path d="M10 2a8 8 0 100 16A8 8 0 0010 2zm0 14a6 6 0 110-12 6 6 0 010 12z" />
          </svg>
        </div>
      )}
      <div className="flex flex-col items-end gap-1 max-w-[85%] min-w-0">
      <div
        className={msg.pending
          ? 'py-1'
          : `w-full px-4 py-2.5 rounded-2xl text-sm leading-relaxed ${
              isUser
                ? 'bg-primary-500 text-white rounded-br-md'
                : 'bg-white text-stone-900 border border-stone-200 rounded-bl-md shadow-subtle'
            }`}
      >
        {msg.pending ? (
          <ThinkingWords />
        ) : isUser ? (
          <span style={{ whiteSpace: 'pre-wrap' }}>{msg.content}</span>
        ) : (
          <div className="prose prose-sm max-w-none prose-p:my-1 prose-pre:bg-stone-100 prose-pre:text-stone-800 prose-code:bg-stone-100 prose-code:px-1 prose-code:rounded prose-headings:text-stone-900 [overflow-wrap:anywhere]">
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={{ img: () => null }}>{clean}</ReactMarkdown>
          </div>
        )}
      </div>
      {/* Action buttons — visible on hover, only for non-pending assistant messages */}
      {!isUser && !msg.pending && (
        <div className="opacity-0 group-hover:opacity-100 transition-opacity flex items-center gap-0.5 self-start ml-1 mt-0.5">
          {/* Copy button */}
          <button
            type="button"
            onClick={handleCopy}
            title={copied ? t('chat.copied') : t('chat.copy')}
            className={`p-1 rounded-lg transition-colors ${
              copied
                ? 'text-sage-600 bg-sage-50'
                : 'text-stone-400 hover:text-stone-600 hover:bg-stone-100'
            }`}
          >
            {copied ? (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            ) : (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
              </svg>
            )}
          </button>
          {/* TTS button */}
          <button
            type="button"
            onClick={() => speaking ? stop() : speak(plainText)}
            title={speaking ? t('chat.stop_speaking') : t('chat.read_aloud')}
            className={`p-1 rounded-lg transition-colors ${
              speaking
                ? 'bg-primary-100 text-primary-600'
                : 'text-stone-400 hover:text-stone-600 hover:bg-stone-100'
            }`}
          >
            {speaking ? (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 10h6v4H9z" />
              </svg>
            ) : (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15.536 8.464a5 5 0 010 7.072M12 6v12m-3.536-9.536a5 5 0 000 7.072M19.07 4.929a10 10 0 010 14.142" />
              </svg>
            )}
          </button>
        </div>
      )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Chat page
// ---------------------------------------------------------------------------

export default function ChatPage() {
  const { t } = useTranslation();
  const [threads, setThreads] = useState<Thread[]>([]);
  const [activeThreadId, setActiveThreadId] = useState<string | null>(null);
  const [messages, setMessages] = useState<LocalMessage[]>([]);
  const [input, setInput] = useState('');
  const [sending, setSending] = useState(false);
  const [loadingThreads, setLoadingThreads] = useState(true);
  const [creatingThread, setCreatingThread] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [threadSearch, setThreadSearch] = useState('');
  const [selectedThreads, setSelectedThreads] = useState<Set<string>>(new Set());
  const [threadBulkDeleting, setThreadBulkDeleting] = useState(false);
  const threadSelectAllRef = useRef<HTMLInputElement>(null);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // ---- Load threads on mount ----
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const list = await listThreads();
        if (!cancelled) {
          setThreads(list);
          if (list.length > 0) setActiveThreadId(prev => prev ?? list[0].id);
        }
      } catch (err) {
        if (!cancelled) setError(String(err));
      } finally {
        if (!cancelled) setLoadingThreads(false);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // ---- Load messages when thread changes ----
  // If there is an in-flight send for this thread (user navigated away mid-response),
  // restore the snapshot and re-attach to the pending promise instead of hitting the DB.
  useEffect(() => {
    if (!activeThreadId) return;

    const inf = getInflight();
    if (inf && inf.threadId === activeThreadId) {
      const pendingMsg: LocalMessage = { id: uid(), role: 'assistant', content: '', pending: true };
      setMessages([...inf.messagesBefore, inf.userMsg, pendingMsg]);
      setSending(true);
      inf.promise
        .then(result => {
          clearInflight();
          setMessages(prev => [
            ...prev.filter(m => !m.pending),
            { ...result.assistant_message, id: uid() },
          ]);
          setThreads(prev =>
            prev.map(t =>
              t.id === activeThreadId && (!t.title || t.title === '')
                ? { ...t, title: inf.userMsg.content.slice(0, 48) }
                : t,
            ),
          );
        })
        .catch(err => {
          clearInflight();
          setMessages(prev => prev.filter(m => !m.pending));
          setError(String(err));
        })
        .finally(() => setSending(false));
      return;
    }

    let cancelled = false;
    (async () => {
      try {
        const msgs = await listMessages(activeThreadId);
        if (!cancelled) {
          setMessages(msgs.map(m => ({ ...m, id: m.id ?? uid() })));
        }
      } catch { /* non-fatal */ }
    })();
    return () => { cancelled = true; };
  }, [activeThreadId]);

  // ---- Scroll to bottom ----
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // ---- Auto-resize textarea ----
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [input]);

  // ---- Actions ----

  async function handleNewThread() {
    setCreatingThread(true);
    setError(null);
    try {
      const thread = await createThread();
      setThreads(prev => [thread, ...prev]);
      setActiveThreadId(thread.id);
      setMessages([]);
    } catch (err) {
      setError(String(err));
    } finally {
      setCreatingThread(false);
    }
  }

  // Sync indeterminate on select-all checkbox
  useEffect(() => {
    const el = threadSelectAllRef.current;
    if (!el) return;
    const q = threadSearch.trim().toLowerCase();
    const filtered = q ? threads.filter(t => (t.title || '').toLowerCase().includes(q)) : threads;
    el.indeterminate = selectedThreads.size > 0 && selectedThreads.size < filtered.length;
  }, [selectedThreads, threads, threadSearch]);

  function toggleSelectThread(id: string) {
    setSelectedThreads(prev => {
      const n = new Set(prev);
      if (n.has(id)) n.delete(id); else n.add(id);
      return n;
    });
  }

  function toggleSelectAllThreads(filtered: Thread[]) {
    if (selectedThreads.size === filtered.length && filtered.length > 0) {
      setSelectedThreads(new Set());
    } else {
      setSelectedThreads(new Set(filtered.map(t => t.id)));
    }
  }

  async function handleDeleteSelectedThreads() {
    if (selectedThreads.size === 0) return;
    setThreadBulkDeleting(true);
    const ids = [...selectedThreads];
    const errors: string[] = [];
    let newActiveId = activeThreadId;
    for (const id of ids) {
      try { await deleteThread(id); }
      catch (e) { errors.push(String(e)); }
    }
    const remaining = threads.filter(t => !ids.includes(t.id));
    setThreads(remaining);
    setSelectedThreads(new Set());
    if (ids.includes(activeThreadId ?? '')) {
      newActiveId = remaining[0]?.id ?? null;
      setActiveThreadId(newActiveId);
      setMessages([]);
    }
    if (errors.length > 0) setError(errors.join('\n'));
    setThreadBulkDeleting(false);
  }

  async function handleDelete(id: string) {
    try {
      await deleteThread(id);
      setThreads(prev => prev.filter(t => t.id !== id));
      setSelectedThreads(prev => { const n = new Set(prev); n.delete(id); return n; });
      if (activeThreadId === id) {
        const remaining = threads.filter(t => t.id !== id);
        setActiveThreadId(remaining[0]?.id ?? null);
        setMessages([]);
      }
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleRename(id: string, title: string) {
    try {
      await renameThread(id, title);
      setThreads(prev => prev.map(t => t.id === id ? { ...t, title } : t));
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleSend() {
    if (!input.trim() || !activeThreadId || sending) return;

    const userText = input.trim();
    const userMsg: LocalMessage = { id: uid(), role: 'user', content: userText };
    const pendingMsg: LocalMessage = { id: uid(), role: 'assistant', content: '', pending: true };

    // Snapshot messages BEFORE adding userMsg so the inflight tracker can
    // reconstruct the full list on remount.
    const snapshotBefore = messages;

    setMessages(prev => [...prev, userMsg, pendingMsg]);
    setInput('');
    setSending(true);
    setError(null);

    // Register inflight BEFORE await so navigation away doesn't lose the promise.
    const promise = appendMessage(activeThreadId, { role: 'user', content: userText });
    setInflight({ threadId: activeThreadId, messagesBefore: snapshotBefore, userMsg, promise });

    try {
      const result = await promise;
      clearInflight();
      setMessages(prev => {
        const without = prev.filter(m => !m.pending);
        return [
          ...without,
          { ...result.assistant_message, id: uid() },
        ];
      });
      // Update thread title if it was auto-set
      const updatedTitle = userText.slice(0, 48);
      setThreads(prev =>
        prev.map(t =>
          t.id === activeThreadId && (!t.title || t.title === '') ? { ...t, title: updatedTitle } : t
        )
      );
    } catch (err) {
      clearInflight();
      setMessages(prev => prev.filter(m => !m.pending));
      setError(String(err));
    } finally {
      setSending(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  const activeThread = threads.find(t => t.id === activeThreadId);

  return (
    <div className="flex" style={{ height: 'calc(100vh - 72px)' }}>
      {/* ---------------------------------------------------------------- */}
      {/* Sidebar                                                           */}
      {/* ---------------------------------------------------------------- */}
      <aside className={`flex-shrink-0 border-r border-stone-200 bg-white flex flex-col transition-all duration-200 ${sidebarOpen ? 'w-56' : 'w-0 overflow-hidden border-r-0'}`}>
        <div className="flex items-center justify-between px-4 py-3 border-b border-stone-200">
          <span className="text-sm font-semibold text-stone-900">{t('chat.threads')}</span>
          <button
            type="button"
            onClick={handleNewThread}
            disabled={creatingThread}
            title={t('chat.new_thread')}
            className="p-1 rounded-md text-stone-400 hover:text-stone-700 hover:bg-stone-100 transition-colors disabled:opacity-50"
          >
            {creatingThread ? (
              <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8z" />
              </svg>
            ) : (
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
            )}
          </button>
        </div>

        {/* Search */}
        {!loadingThreads && threads.length > 0 && (
          <div className="px-2 py-1.5 border-b border-stone-100">
            <div className="relative">
              <svg className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-stone-400 pointer-events-none" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-4.35-4.35M17 11A6 6 0 1 1 5 11a6 6 0 0 1 12 0z" />
              </svg>
              <input
                value={threadSearch}
                onChange={e => { setThreadSearch(e.target.value); setSelectedThreads(new Set()); }}
                placeholder={t('chat.search_threads')}
                className="w-full text-xs rounded-md border border-stone-200 pl-6 pr-2 py-1 focus:border-primary-400 bg-stone-50 placeholder-stone-400 outline-none"
              />
            </div>
          </div>
        )}

        <div className="flex-1 overflow-y-auto scrollbar-thin">
          {loadingThreads ? (
            <div className="p-4 space-y-2">
              {[1, 2, 3].map(i => (
                <div key={i} className="loading-pulse h-10 rounded-md" />
              ))}
            </div>
          ) : threads.length === 0 ? (
            <div className="p-4 text-center">
              <p className="text-sm text-stone-400">{t('chat.no_threads')}</p>
              <button
                type="button"
                onClick={handleNewThread}
                className="mt-2 text-xs text-primary-500 hover:text-primary-600 font-medium"
              >
                {t('chat.create_one')}
              </button>
            </div>
          ) : (() => {
            const q = threadSearch.trim().toLowerCase();
            const filtered = q ? threads.filter(t => (t.title || '').toLowerCase().includes(q)) : threads;
            return (
              <>
                {/* Select-all + bulk delete bar */}
                {filtered.length > 0 && (
                  <div className="flex items-center gap-2 px-2 py-1 border-b border-stone-100">
                    <label className="flex items-center gap-1.5 cursor-pointer select-none flex-1 min-w-0">
                      <input
                        ref={threadSelectAllRef}
                        type="checkbox"
                        checked={selectedThreads.size === filtered.length && filtered.length > 0}
                        onChange={() => toggleSelectAllThreads(filtered)}
                        className="w-3.5 h-3.5 rounded border-stone-300 text-primary-500 accent-primary-500"
                        aria-label={t('chat.select_all_threads')}
                      />
                      <span className="text-xs text-stone-400 truncate">
                        {selectedThreads.size === 0
                          ? t('chat.threads_count', { count: filtered.length })
                          : t('chat.selected', { count: selectedThreads.size })}
                      </span>
                    </label>
                    {selectedThreads.size > 0 && (
                      <button
                        type="button"
                        onClick={() => handleDeleteSelectedThreads()}
                        disabled={threadBulkDeleting}
                        className="flex-shrink-0 flex items-center gap-1 px-2 py-1 text-xs font-medium bg-red-50 hover:bg-red-100 text-red-600 border border-red-200 rounded-md disabled:opacity-40 transition-colors"
                      >
                        {threadBulkDeleting
                          ? <div className="w-3 h-3 border border-red-400 border-t-transparent rounded-full animate-spin" />
                          : <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
                        }
                        {t('chat.delete_selected', { count: selectedThreads.size })}
                      </button>
                    )}
                  </div>
                )}
                <ul className="p-2 space-y-0.5">
                  {filtered.map(thread => (
                    <ThreadItem
                      key={thread.id}
                      thread={thread}
                      active={thread.id === activeThreadId}
                      selected={selectedThreads.has(thread.id)}
                      onSelect={() => setActiveThreadId(thread.id)}
                      onToggleSelect={toggleSelectThread}
                      onDelete={handleDelete}
                      onRename={handleRename}
                    />
                  ))}
                </ul>
              </>
            );
          })()}
        </div>
      </aside>

      {/* ---------------------------------------------------------------- */}
      {/* Main                                                              */}
      {/* ---------------------------------------------------------------- */}
      <div className="flex-1 flex flex-col bg-stone-50 min-w-0">
        {/* Header */}
        <div className="px-3 py-3 border-b border-stone-200 bg-white flex items-center gap-2">
          {/* Sidebar toggle */}
          <button
            type="button"
            onClick={() => setSidebarOpen(v => !v)}
            title={sidebarOpen ? t('chat.hide_threads') : t('chat.show_threads')}
            className="flex-shrink-0 p-1.5 rounded-md text-stone-400 hover:text-stone-700 hover:bg-stone-100 transition-colors"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d={sidebarOpen
                  ? 'M4 6h16M4 12h16M4 18h16'
                  : 'M4 6h16M4 12h10M4 18h16'} />
            </svg>
          </button>
          <div className="flex-1 min-w-0">
            <h2 className="text-sm font-semibold text-stone-900 truncate">
              {activeThread?.title || (activeThreadId ? t('chat.thread') : t('chat.select_thread'))}
            </h2>
            {activeThread && (
              <p className="text-xs text-stone-400">{formatDate(activeThread.created_at)}</p>
            )}
          </div>
          {/* New thread shortcut when sidebar is hidden */}
          {!sidebarOpen && (
            <button
              type="button"
              onClick={handleNewThread}
              disabled={creatingThread}
              title={t('chat.new_thread')}
              className="flex-shrink-0 p-1.5 rounded-md text-stone-400 hover:text-stone-700 hover:bg-stone-100 transition-colors disabled:opacity-50"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
            </button>
          )}
        </div>

        {/* Error */}
        {error && (
          <div className="mx-4 mt-3 px-4 py-2 bg-coral-50 border border-coral-200 rounded-lg text-sm text-coral-700 flex items-center justify-between">
            <span>{error}</span>
            <button type="button" onClick={() => setError(null)} className="ml-2 text-coral-400 hover:text-coral-600">✕</button>
          </div>
        )}

        {/* Messages */}
        <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4 scrollbar-thin">
          {!activeThreadId ? (
            <div className="flex items-center justify-center h-full">
              <div className="text-center">
                <div className="w-12 h-12 rounded-full bg-stone-200 flex items-center justify-center mx-auto mb-3">
                  <svg className="w-6 h-6 text-stone-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.8}
                      d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
                  </svg>
                </div>
                <p className="text-sm text-stone-400">{t('chat.select_or_create')}</p>
                <button
                  type="button"
                  onClick={handleNewThread}
                  className="mt-3 px-4 py-2 bg-primary-500 hover:bg-primary-600 text-white text-sm font-medium rounded-lg transition-colors"
                >
                  {t('chat.new_thread')}
                </button>
              </div>
            </div>
          ) : messages.length === 0 ? (
            <div className="flex items-center justify-center h-full">
              <p className="text-sm text-stone-400">{t('chat.send_to_start')}</p>
            </div>
          ) : (
            messages.map(msg => <MessageBubble key={msg.id} msg={msg} />)
          )}
          <div ref={messagesEndRef} />
        </div>

        {/* Compose */}
        <div className="border-t border-stone-200 bg-white px-4 pt-3 pb-4">
          <div className="flex items-end gap-2">
            <div className="flex-1 relative">
              <textarea
                ref={textareaRef}
                value={input}
                onChange={e => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                disabled={!activeThreadId || sending}
                placeholder={activeThreadId ? t('chat.type_message') : t('chat.select_first')}
                rows={1}
                className="w-full resize-none px-4 py-2.5 rounded-xl border border-stone-200 focus:border-primary-400 bg-stone-50 text-stone-900 placeholder-stone-400 text-sm leading-relaxed transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                style={{ maxHeight: '160px', overflow: 'auto' }}
              />
            </div>
            <button
              type="button"
              onClick={handleSend}
              disabled={!input.trim() || !activeThreadId || sending}
              className="flex-shrink-0 w-9 h-9 rounded-xl bg-primary-500 hover:bg-primary-600 disabled:opacity-40 disabled:cursor-not-allowed text-white flex items-center justify-center transition-colors"
            >
              {sending ? (
                <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8z" />
                </svg>
              ) : (
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8" />
                </svg>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
