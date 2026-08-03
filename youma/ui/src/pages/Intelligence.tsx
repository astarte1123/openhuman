// Intelligence page — Memory, Tasks, Wiki knowledge graph, Documents.

import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createMemory, createTask, deleteMemory, deleteTask,
  knowledgeIngestFile, knowledgeIngestText, knowledgeRebuild, knowledgeSourceDelete,
  knowledgeSourcesList, listenFileDrop, listMemories, listTasks, updateTask,
} from '../api';
import type { KnowledgeSource, Memory, Task } from '../api';
import WikiTab from './WikiTab';

type Tab = 'memory' | 'tasks' | 'docs' | 'wiki';

function formatDate(iso: string) {
  try { return new Date(iso).toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' }); }
  catch { return ''; }
}

export default function IntelligencePage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>('memory');
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [memories, setMemories] = useState<Memory[]>([]);
  const [memInput, setMemInput] = useState('');
  const [memTags, setMemTags] = useState('');
  const [memSearch, setMemSearch] = useState('');
  const [selectedMemories, setSelectedMemories] = useState<Set<string>>(new Set());
  const [memBulkDeleting, setMemBulkDeleting] = useState(false);
  const memSelectAllRef = useRef<HTMLInputElement>(null);
  const memInputRef = useRef<HTMLTextAreaElement>(null);

  const [tasks, setTasks] = useState<Task[]>([]);
  const [taskInput, setTaskInput] = useState('');

  // ---- Docs state ----
  const [sources, setSources] = useState<KnowledgeSource[]>([]);
  const [docsLoading, setDocsLoading] = useState(false);
  // paste mode
  const [docTitle, setDocTitle] = useState('');
  const [docContent, setDocContent] = useState('');
  const [docTags, setDocTags] = useState('');
  // file path mode
  const [filePath, setFilePath] = useState('');
  const [fileTags, setFileTags] = useState('');
  const [ingestMode, setIngestMode] = useState<'paste' | 'file'>('paste');
  const [ingestBusy, setIngestBusy] = useState(false);
  const ingestBusyRef = useRef(false);

  function setIngestBusySync(val: boolean) {
    ingestBusyRef.current = val;
    setIngestBusy(val);
  }
  const [ingestResult, setIngestResult] = useState('');
  const [rebuildBusy, setRebuildBusy] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);
  const [selectedSources, setSelectedSources] = useState<Set<string>>(new Set());
  const [bulkDeleting, setBulkDeleting] = useState(false);
  const selectAllRef = useRef<HTMLInputElement>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [mems, tsks] = await Promise.all([listMemories(), listTasks()]);
      setMemories(mems);
      setTasks(tsks);
    } catch (e) { setError(String(e)); }
    finally { setLoading(false); }
  }, []);

  const loadSources = useCallback(async () => {
    setDocsLoading(true);
    try { setSources(await knowledgeSourcesList()); }
    catch (e) { setError(String(e)); }
    finally { setDocsLoading(false); }
  }, []);

  useEffect(() => { load(); }, [load]);
  useEffect(() => {
    if (tab === 'docs') { loadSources(); setSelectedSources(new Set()); }
  }, [tab, loadSources]);

  // Sync indeterminate state on select-all checkbox (docs)
  useEffect(() => {
    const el = selectAllRef.current;
    if (!el) return;
    el.indeterminate = selectedSources.size > 0 && selectedSources.size < sources.length;
  }, [selectedSources, sources.length]);

  // Sync indeterminate state on select-all checkbox (memories)
  useEffect(() => {
    const el = memSelectAllRef.current;
    if (!el) return;
    const filtered = memories.filter(m =>
      !memSearch.trim() ||
      m.content.toLowerCase().includes(memSearch.toLowerCase()) ||
      m.tags.toLowerCase().includes(memSearch.toLowerCase())
    );
    el.indeterminate = selectedMemories.size > 0 && selectedMemories.size < filtered.length;
  }, [selectedMemories, memories, memSearch]);

  // Tauri file/folder drag-drop listener (active only on docs tab)
  useEffect(() => {
    if (tab !== 'docs') return;
    let unlisten: (() => void) | undefined;
    listenFileDrop(
      async (paths) => {
        if (ingestBusyRef.current) return;
        setIngestBusySync(true);
        setIngestResult('');
        let total = 0;
        const errMsgs: string[] = [];
        for (const p of paths) {
          try {
            const r = await knowledgeIngestFile({ path: p });
            total += r.chunks;
          } catch (e) {
            errMsgs.push(`${p.split(/[\\/]/).pop() ?? p}: ${String(e)}`);
          }
        }
        const label = paths.length === 1 ? paths[0].split(/[\\/]/).pop() ?? paths[0] : `${paths.length} items`;
        if (errMsgs.length > 0) {
          setIngestResult(`Saved ${total} chunks. Errors: ${errMsgs.slice(0, 3).join(' | ')}`);
          setError(errMsgs.join('\n'));
        } else if (total === 0) {
          setIngestResult(`All files already up to date (0 new chunks from ${label}).`);
        } else {
          setIngestResult(`Saved ${total} chunk${total !== 1 ? 's' : ''} from ${label}.`);
        }
        await loadSources();
        setIngestBusySync(false);
      },
      () => setIsDragOver(true),   // onEnter — OS drag enters window
      () => setIsDragOver(false),  // onLeave — OS drag leaves or drop completes
    ).then(fn => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [tab, loadSources]); // ingestBusy removed from deps — use ref instead

  async function handleIngestPaste() {
    if (!docTitle.trim() || !docContent.trim()) return;
    setIngestBusySync(true); setIngestResult('');
    try {
      const r = await knowledgeIngestText({ title: docTitle.trim(), content: docContent, tags: docTags.trim() });
      setIngestResult(`Saved ${r.chunks} chunk${r.chunks !== 1 ? 's' : ''}.`);
      setDocTitle(''); setDocContent(''); setDocTags('');
      await loadSources();
    } catch (e) { setError(String(e)); }
    finally { setIngestBusySync(false); }
  }

  async function handleIngestFile() {
    if (!filePath.trim()) return;
    setIngestBusySync(true); setIngestResult('');
    try {
      const r = await knowledgeIngestFile({ path: filePath.trim(), tags: fileTags.trim() });
      setIngestResult(`Saved ${r.chunks} chunk${r.chunks !== 1 ? 's' : ''} from file.`);
      setFilePath(''); setFileTags('');
      await loadSources();
    } catch (e) { setError(String(e)); }
    finally { setIngestBusySync(false); }
  }

  async function handleDeleteSource(sourceId: string) {
    try {
      await knowledgeSourceDelete(sourceId);
      setSources(prev => prev.filter(s => s.source_id !== sourceId));
      setSelectedSources(prev => { const n = new Set(prev); n.delete(sourceId); return n; });
    } catch (e) { setError(String(e)); }
  }

  async function handleDeleteSelected() {
    if (selectedSources.size === 0) return;
    setBulkDeleting(true);
    const ids = [...selectedSources];
    const errors: string[] = [];
    for (const id of ids) {
      try { await knowledgeSourceDelete(id); }
      catch (e) { errors.push(String(e)); }
    }
    setSources(prev => prev.filter(s => !ids.includes(s.source_id)));
    setSelectedSources(new Set());
    if (errors.length > 0) setError(errors.join('\n'));
    setBulkDeleting(false);
  }

  function toggleSelectSource(id: string) {
    setSelectedSources(prev => {
      const n = new Set(prev);
      if (n.has(id)) n.delete(id); else n.add(id);
      return n;
    });
  }

  function toggleSelectAll() {
    if (selectedSources.size === sources.length) {
      setSelectedSources(new Set());
    } else {
      setSelectedSources(new Set(sources.map(s => s.source_id)));
    }
  }

  async function handleRebuild() {
    setRebuildBusy(true);
    try { await knowledgeRebuild(); setIngestResult('Rebuild triggered (runs in background).'); }
    catch (e) { setError(String(e)); }
    finally { setRebuildBusy(false); }
  }

  async function handleAddMemory() {
    if (!memInput.trim()) return;
    try {
      const m = await createMemory(memInput.trim(), memTags.trim());
      setMemories(prev => [m, ...prev]);
      setMemInput(''); setMemTags('');
    } catch (e) { setError(String(e)); }
  }

  async function handleDeleteMemory(id: string) {
    try {
      await deleteMemory(id);
      setMemories(prev => prev.filter(m => m.id !== id));
      setSelectedMemories(prev => { const n = new Set(prev); n.delete(id); return n; });
    }
    catch (e) { setError(String(e)); }
  }

  function toggleSelectMemory(id: string) {
    setSelectedMemories(prev => {
      const n = new Set(prev);
      if (n.has(id)) n.delete(id); else n.add(id);
      return n;
    });
  }

  function toggleSelectAllMemories(filtered: Memory[]) {
    if (selectedMemories.size === filtered.length && filtered.length > 0) {
      setSelectedMemories(new Set());
    } else {
      setSelectedMemories(new Set(filtered.map(m => m.id)));
    }
  }

  async function handleDeleteSelectedMemories() {
    if (selectedMemories.size === 0) return;
    setMemBulkDeleting(true);
    const ids = [...selectedMemories];
    const errors: string[] = [];
    for (const id of ids) {
      try { await deleteMemory(id); }
      catch (e) { errors.push(String(e)); }
    }
    setMemories(prev => prev.filter(m => !ids.includes(m.id)));
    setSelectedMemories(new Set());
    if (errors.length > 0) setError(errors.join('\n'));
    setMemBulkDeleting(false);
  }

  async function handleAddTask() {
    if (!taskInput.trim()) return;
    try {
      const t = await createTask(taskInput.trim());
      setTasks(prev => [t, ...prev]);
      setTaskInput('');
    } catch (e) { setError(String(e)); }
  }

  async function handleToggleTask(task: Task) {
    try {
      const updated = await updateTask(task.id, task.status === 'done' ? 'todo' : 'done');
      setTasks(prev => prev.map(t => t.id === updated.id ? updated : t));
    } catch (e) { setError(String(e)); }
  }

  async function handleDeleteTask(id: string) {
    try { await deleteTask(id); setTasks(prev => prev.filter(t => t.id !== id)); }
    catch (e) { setError(String(e)); }
  }

  return (
    <div className="flex h-full min-h-0">

      {/* ---- Left nav sidebar ---- */}
      <aside className={`flex-shrink-0 bg-white border-r border-stone-200 flex flex-col min-h-0 transition-all duration-200 ${sidebarOpen ? 'w-40' : 'w-0 overflow-hidden border-r-0'}`}>
        <div className="px-3 py-2 border-b border-stone-200">
          <span className="text-sm font-semibold text-stone-900">{t('intelligence.title')}</span>
        </div>
        <nav className="p-2 space-y-0.5">
          {(['memory', 'tasks', 'docs', 'wiki'] as const).map(id => (
            <button
              key={id}
              type="button"
              onClick={() => setTab(id)}
              className={`w-full text-left px-2 py-1.5 rounded-lg text-xs transition-colors ${
                tab === id
                  ? 'bg-primary-50 text-primary-700 font-medium'
                  : 'text-stone-600 hover:bg-stone-100'
              }`}
            >
              {t(`intelligence.tab_${id}`)}
            </button>
          ))}
        </nav>
      </aside>

      {/* ---- Right content ---- */}
      <div className="flex-1 flex flex-col min-w-0 min-h-0 overflow-hidden">

        {/* Header with sidebar toggle */}
        <div className="flex-shrink-0 px-3 py-2 border-b border-stone-200 bg-white flex items-center gap-2">
          <button
            type="button"
            onClick={() => setSidebarOpen(v => !v)}
            title={sidebarOpen ? t('chat.hide_threads') : t('chat.show_threads')}
            className="flex-shrink-0 p-1.5 rounded-md text-stone-400 hover:text-stone-700 hover:bg-stone-100 transition-colors"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d={sidebarOpen ? 'M4 6h16M4 12h16M4 18h16' : 'M4 6h16M4 12h10M4 18h16'} />
            </svg>
          </button>
          <span className="text-sm font-semibold text-stone-900">{t(`intelligence.tab_${tab}`)}</span>
        </div>

        {error && (
          <div className="mx-4 mt-3 px-4 py-2 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700 flex justify-between items-center flex-shrink-0">
            <span>{error}</span>
            <button type="button" aria-label={t('intelligence.dismiss')} onClick={() => setError(null)} className="ml-2 text-red-400 hover:text-red-600">
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        )}

        {/* Wiki tab fills remaining height without scroll wrapper */}
        {tab === 'wiki' && (
          <div className="flex-1 min-h-0 p-4 flex flex-col">
            <WikiTab />
          </div>
        )}

        <div className={`flex-1 overflow-y-auto bg-stone-50 p-4 ${tab === 'wiki' ? 'hidden' : ''}`}>

        {/* ---- MEMORY ---- */}
        {tab === 'memory' && (() => {
          const q = memSearch.trim().toLowerCase();
          const filteredMemories = q
            ? memories.filter(m => m.content.toLowerCase().includes(q) || m.tags.toLowerCase().includes(q))
            : memories;
          return loading
            ? <div className="space-y-2">{[1, 2, 3].map(i => <div key={i} className="h-16 bg-stone-200 rounded-xl animate-pulse" />)}</div>
            : (
              <div className="space-y-4">
                {/* Add memory form */}
                <div className="bg-white rounded-xl border border-stone-200 p-4 shadow-sm">
                  <textarea
                    ref={memInputRef}
                    value={memInput}
                    onChange={e => setMemInput(e.target.value)}
                    placeholder={t('intelligence.memory_placeholder')}
                    rows={3}
                    className="w-full resize-none text-sm rounded-lg border border-stone-200 px-3 py-2 focus:border-primary-400 bg-stone-50 placeholder-stone-400 outline-none"
                  />
                  <div className="space-y-2 mt-2">
                    <input
                      value={memTags}
                      onChange={e => setMemTags(e.target.value)}
                      placeholder={t('intelligence.tags_placeholder')}
                      className="w-full text-xs rounded-lg border border-stone-200 px-3 py-1.5 focus:border-primary-400 bg-stone-50 placeholder-stone-400 outline-none"
                    />
                    <div className="flex justify-end">
                      <button
                        type="button"
                        onClick={handleAddMemory}
                        disabled={!memInput.trim()}
                        className="px-4 py-1.5 bg-primary-500 hover:bg-primary-600 disabled:opacity-40 text-white text-sm font-medium rounded-lg transition-colors"
                      >
                        {t('intelligence.save')}
                      </button>
                    </div>
                  </div>
                </div>

                {/* Search field */}
                {memories.length > 0 && (
                  <div className="relative">
                    <svg className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-stone-400 pointer-events-none" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-4.35-4.35M17 11A6 6 0 1 1 5 11a6 6 0 0 1 12 0z" />
                    </svg>
                    <input
                      value={memSearch}
                      onChange={e => { setMemSearch(e.target.value); setSelectedMemories(new Set()); }}
                      placeholder={t('intelligence.search_memories')}
                      className="w-full text-sm rounded-lg border border-stone-200 pl-9 pr-3 py-2 focus:border-primary-400 bg-white placeholder-stone-400 outline-none shadow-sm"
                    />
                  </div>
                )}

                {memories.length === 0 ? (
                  <div className="text-center py-12 text-stone-400">
                    <div className="w-12 h-12 rounded-full bg-stone-100 flex items-center justify-center mx-auto mb-3">
                      <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
                      </svg>
                    </div>
                    <p className="text-sm">{t('intelligence.no_memories')}</p>
                  </div>
                ) : filteredMemories.length === 0 ? (
                  <div className="text-center py-10 text-stone-400">
                    <p className="text-sm">{t('intelligence.no_memories')}</p>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {/* Select-all header + bulk delete bar */}
                    <div className="flex items-center gap-3 px-1">
                      <label className="flex items-center gap-2 cursor-pointer select-none">
                        <input
                          ref={memSelectAllRef}
                          type="checkbox"
                          checked={selectedMemories.size === filteredMemories.length && filteredMemories.length > 0}
                          onChange={() => toggleSelectAllMemories(filteredMemories)}
                          className="w-4 h-4 rounded border-stone-300 text-primary-500 accent-primary-500"
                          aria-label={t('intelligence.select_all_memories')}
                        />
                        <span className="text-xs text-stone-500">
                          {selectedMemories.size === 0
                            ? t('intelligence.memories_count', { count: filteredMemories.length })
                            : t('intelligence.selected', { count: selectedMemories.size })}
                        </span>
                      </label>

                      {selectedMemories.size > 0 && (
                        <button
                          type="button"
                          onClick={handleDeleteSelectedMemories}
                          disabled={memBulkDeleting}
                          className="ml-auto flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-red-50 hover:bg-red-100 text-red-600 border border-red-200 rounded-lg disabled:opacity-40 transition-colors"
                        >
                          {memBulkDeleting ? (
                            <div className="w-3 h-3 border border-red-400 border-t-transparent rounded-full animate-spin" />
                          ) : (
                            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                            </svg>
                          )}
                          {t('intelligence.delete_selected', { count: selectedMemories.size })}
                        </button>
                      )}
                    </div>

                    {filteredMemories.map(m => {
                      const isSelected = selectedMemories.has(m.id);
                      return (
                        <div key={m.id} className={`bg-white rounded-xl border p-4 shadow-sm group transition-colors ${isSelected ? 'border-primary-300 bg-primary-50/40' : 'border-stone-200'}`}>
                          <div className="flex items-start gap-3">
                            <input
                              type="checkbox"
                              checked={isSelected}
                              onChange={() => toggleSelectMemory(m.id)}
                              className="mt-0.5 w-4 h-4 flex-shrink-0 rounded border-stone-300 text-primary-500 accent-primary-500"
                              aria-label={t('intelligence.delete_memory')}
                            />
                            <p className="flex-1 text-sm text-stone-800 leading-relaxed whitespace-pre-wrap">{m.content}</p>
                          </div>
                          <div className="flex items-center justify-between mt-2 pl-7">
                            <div className="flex items-center gap-2 flex-wrap">
                              {m.tags && m.tags.split(',').filter(Boolean).map(tag => (
                                <span key={tag} className="text-xs px-2 py-0.5 bg-primary-50 text-primary-600 rounded-full border border-primary-100">{tag.trim()}</span>
                              ))}
                              <span className="text-xs text-stone-400">{formatDate(m.created_at)}</span>
                            </div>
                            <button
                              type="button"
                              aria-label={t('intelligence.delete_memory')}
                              onClick={() => handleDeleteMemory(m.id)}
                              className="opacity-0 group-hover:opacity-100 transition-opacity text-stone-400 hover:text-red-500 p-1 rounded"
                            >
                              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                              </svg>
                            </button>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
        })()}

        {/* ---- TASKS ---- */}
        {tab === 'tasks' && (
          loading
            ? <div className="space-y-2">{[1, 2, 3].map(i => <div key={i} className="h-12 bg-stone-200 rounded-xl animate-pulse" />)}</div>
            : (
              <div className="space-y-4">
                <div className="bg-white rounded-xl border border-stone-200 p-4 shadow-sm space-y-2">
                  <input
                    value={taskInput}
                    onChange={e => setTaskInput(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && handleAddTask()}
                    placeholder={t('intelligence.task_placeholder')}
                    className="w-full text-sm rounded-lg border border-stone-200 px-3 py-2 focus:border-primary-400 bg-stone-50 placeholder-stone-400 outline-none"
                  />
                  <div className="flex justify-end">
                    <button
                      type="button"
                      onClick={handleAddTask}
                      disabled={!taskInput.trim()}
                      className="px-4 py-2 bg-primary-500 hover:bg-primary-600 disabled:opacity-40 text-white text-sm font-medium rounded-lg transition-colors"
                    >
                      {t('intelligence.add')}
                    </button>
                  </div>
                </div>

                {tasks.length === 0 ? (
                  <div className="text-center py-12 text-stone-400">
                    <div className="w-12 h-12 rounded-full bg-stone-100 flex items-center justify-center mx-auto mb-3">
                      <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4" />
                      </svg>
                    </div>
                    <p className="text-sm">{t('intelligence.no_tasks')}</p>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {tasks.map(task => (
                      <div key={task.id} className={`flex items-center gap-3 bg-white rounded-xl border px-4 py-3 shadow-sm group transition-opacity ${task.status === 'done' ? 'opacity-60' : ''}`}>
                        <button
                          type="button"
                          aria-label={task.status === 'done' ? t('intelligence.mark_todo') : t('intelligence.mark_done')}
                          onClick={() => handleToggleTask(task)}
                          className={`flex-shrink-0 w-5 h-5 rounded-full border-2 flex items-center justify-center transition-colors ${task.status === 'done' ? 'border-primary-500 bg-primary-500' : 'border-stone-300 hover:border-primary-400'}`}
                        >
                          {task.status === 'done' && (
                            <svg className="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                            </svg>
                          )}
                        </button>
                        <span className={`flex-1 text-sm ${task.status === 'done' ? 'line-through text-stone-400' : 'text-stone-800'}`}>{task.title}</span>
                        <span className="text-xs text-stone-400">{formatDate(task.created_at)}</span>
                        <button
                          type="button"
                          aria-label={t('intelligence.delete_task')}
                          onClick={() => handleDeleteTask(task.id)}
                          className="opacity-0 group-hover:opacity-100 transition-opacity text-stone-400 hover:text-red-500 p-1 rounded"
                        >
                          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                          </svg>
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )
        )}

        {/* ---- DOCUMENTS ---- */}
        {tab === 'docs' && (
          <div className="space-y-4">

            {/* Drop zone */}
            <div
              onDragEnter={e => { e.preventDefault(); setIsDragOver(true); }}
              onDragOver={e => { e.preventDefault(); }}
              onDragLeave={e => { if (!e.currentTarget.contains(e.relatedTarget as Node)) setIsDragOver(false); }}
              onDrop={e => { e.preventDefault(); setIsDragOver(false); }}
              className={`relative flex flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed px-6 py-8 text-center transition-colors ${
                isDragOver
                  ? 'border-primary-400 bg-primary-50'
                  : ingestBusy
                    ? 'border-stone-200 bg-stone-50'
                    : 'border-stone-300 bg-white hover:border-primary-300 hover:bg-primary-50/30'
              }`}
            >
              {ingestBusy ? (
                <>
                  <div className="w-8 h-8 border-2 border-primary-400 border-t-transparent rounded-full animate-spin" />
                  <p className="text-sm text-stone-500">{t('intelligence.importing')}</p>
                </>
              ) : isDragOver ? (
                <>
                  <svg className="w-10 h-10 text-primary-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                  </svg>
                  <p className="text-sm font-medium text-primary-600">{t('intelligence.drop_to_import')}</p>
                </>
              ) : (
                <>
                  <svg className="w-10 h-10 text-stone-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                  </svg>
                  <p className="text-sm font-medium text-stone-600">{t('intelligence.drop_files')}</p>
                  <p className="text-xs text-stone-400">{t('intelligence.drop_formats')}</p>
                </>
              )}
            </div>

            {/* Ingest form */}
            <div className="bg-white rounded-xl border border-stone-200 shadow-sm overflow-hidden">
              {/* Mode switcher */}
              <div className="flex border-b border-stone-100">
                {(['paste', 'file'] as const).map(mode => (
                  <button
                    key={mode}
                    type="button"
                    onClick={() => { setIngestMode(mode); setIngestResult(''); }}
                    className={`flex-1 py-2 text-xs font-medium transition-colors ${
                      ingestMode === mode
                        ? 'bg-primary-50 text-primary-600 border-b-2 border-primary-500'
                        : 'text-stone-500 hover:text-stone-700'
                    }`}
                  >
                    {mode === 'paste' ? t('intelligence.paste_text') : t('intelligence.import_file_path')}
                  </button>
                ))}
              </div>

              <div className="p-4 space-y-3">
                {ingestMode === 'paste' ? (
                  <>
                    <input
                      value={docTitle}
                      onChange={e => setDocTitle(e.target.value)}
                      placeholder={t('intelligence.doc_title')}
                      className="w-full text-sm rounded-lg border border-stone-200 px-3 py-2 focus:border-primary-400 bg-stone-50 placeholder-stone-400 outline-none"
                    />
                    <textarea
                      value={docContent}
                      onChange={e => setDocContent(e.target.value)}
                      placeholder={t('intelligence.doc_placeholder')}
                      rows={6}
                      className="w-full text-sm resize-none rounded-lg border border-stone-200 px-3 py-2 focus:border-primary-400 bg-stone-50 placeholder-stone-400 outline-none"
                    />
                    <div className="space-y-2">
                      <input
                        value={docTags}
                        onChange={e => setDocTags(e.target.value)}
                        placeholder={t('intelligence.tags_optional')}
                        className="w-full text-xs rounded-lg border border-stone-200 px-3 py-1.5 focus:border-primary-400 bg-stone-50 placeholder-stone-400 outline-none"
                      />
                      <div className="flex justify-end">
                        <button
                          type="button"
                          onClick={handleIngestPaste}
                          disabled={ingestBusy || !docTitle.trim() || !docContent.trim()}
                          className="px-4 py-1.5 bg-primary-500 hover:bg-primary-600 disabled:opacity-40 text-white text-sm font-medium rounded-lg transition-colors"
                        >
                          {ingestBusy ? t('intelligence.saving') : t('intelligence.save_to_kb')}
                        </button>
                      </div>
                    </div>
                  </>
                ) : (
                  <>
                    <p className="text-xs text-stone-500">
                      File: <code>.pdf</code>, <code>.docx</code>, <code>.xlsx</code>, <code>.ods</code>, <code>.epub</code>, <code>.html</code>, <code>.rtf</code>, <code>.txt</code>, <code>.md</code>, <code>.csv</code>, source code…
                      <span className="ml-1 font-medium text-primary-600">Or paste a folder path to import all files recursively.</span>
                    </p>
                    <input
                      value={filePath}
                      onChange={e => setFilePath(e.target.value)}
                      placeholder="C:\Users\you\Documents\notes.pdf  or  C:\Users\you\Documents\MyFolder"
                      className="w-full text-sm font-mono rounded-lg border border-stone-200 px-3 py-2 focus:border-primary-400 bg-stone-50 placeholder-stone-400 outline-none"
                    />
                    <div className="space-y-2">
                      <input
                        value={fileTags}
                        onChange={e => setFileTags(e.target.value)}
                        placeholder={t('intelligence.tags_optional')}
                        className="w-full text-xs rounded-lg border border-stone-200 px-3 py-1.5 focus:border-primary-400 bg-stone-50 placeholder-stone-400 outline-none"
                      />
                      <div className="flex justify-end">
                        <button
                          type="button"
                          onClick={handleIngestFile}
                          disabled={ingestBusy || !filePath.trim()}
                          className="px-4 py-1.5 bg-primary-500 hover:bg-primary-600 disabled:opacity-40 text-white text-sm font-medium rounded-lg transition-colors"
                        >
                          {ingestBusy ? t('intelligence.importing') : t('intelligence.import_file')}
                        </button>
                      </div>
                    </div>
                  </>
                )}

                {ingestResult && (
                  <p className="text-xs text-sage-700 bg-sage-50 border border-sage-200 rounded-lg px-3 py-2">
                    {ingestResult}
                  </p>
                )}
              </div>
            </div>

            {/* Rebuild KB */}
            <div className="flex items-center justify-between bg-white rounded-xl border border-stone-200 shadow-sm px-4 py-3">
              <div className="min-w-0 mr-3">
                <p className="text-sm font-medium text-stone-800">{t('intelligence.rebuild_title')}</p>
                <p className="text-xs text-stone-500 mt-0.5">{t('intelligence.rebuild_desc')}</p>
              </div>
              <button
                type="button"
                onClick={handleRebuild}
                disabled={rebuildBusy}
                className="ml-4 flex-shrink-0 px-3 py-1.5 text-xs bg-stone-100 hover:bg-stone-200 text-stone-700 font-medium rounded-lg disabled:opacity-40 transition-colors"
              >
                {rebuildBusy ? t('intelligence.rebuild_triggered') : t('intelligence.rebuild_now')}
              </button>
            </div>

            {/* Document list */}
            {docsLoading ? (
              <div className="space-y-2">{[1, 2].map(i => <div key={i} className="h-14 bg-stone-200 rounded-xl animate-pulse" />)}</div>
            ) : sources.length === 0 ? (
              <div className="text-center py-10 text-stone-400">
                <svg className="w-10 h-10 mx-auto mb-2 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                </svg>
                <p className="text-sm">{t('intelligence.no_docs')}</p>
                <p className="text-xs mt-1">{t('intelligence.no_docs_hint')}</p>
              </div>
            ) : (
              <div className="space-y-2">

                {/* Select-all header + bulk delete bar */}
                <div className="flex items-center gap-3 px-1">
                  <label className="flex items-center gap-2 cursor-pointer select-none">
                    <input
                      ref={selectAllRef}
                      type="checkbox"
                      checked={selectedSources.size === sources.length && sources.length > 0}
                      onChange={toggleSelectAll}
                      className="w-4 h-4 rounded border-stone-300 text-primary-500 accent-primary-500"
                      aria-label={t('intelligence.select_all')}
                    />
                    <span className="text-xs text-stone-500">
                      {selectedSources.size === 0
                        ? t('intelligence.docs_count', { count: sources.length })
                        : t('intelligence.selected', { count: selectedSources.size })}
                    </span>
                  </label>

                  {selectedSources.size > 0 && (
                    <button
                      type="button"
                      onClick={handleDeleteSelected}
                      disabled={bulkDeleting}
                      className="ml-auto flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-red-50 hover:bg-red-100 text-red-600 border border-red-200 rounded-lg disabled:opacity-40 transition-colors"
                    >
                      {bulkDeleting ? (
                        <div className="w-3 h-3 border border-red-400 border-t-transparent rounded-full animate-spin" />
                      ) : (
                        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      )}
                      {t('intelligence.delete_selected', { count: selectedSources.size })}
                    </button>
                  )}
                </div>

                {sources.map(src => {
                  const isSelected = selectedSources.has(src.source_id);
                  return (
                    <div
                      key={src.source_id}
                      className={`bg-white rounded-xl border shadow-sm px-4 py-3 flex items-center gap-3 group transition-colors ${
                        isSelected ? 'border-primary-300 bg-primary-50/40' : 'border-stone-200'
                      }`}
                    >
                      <input
                        type="checkbox"
                        checked={isSelected}
                        onChange={() => toggleSelectSource(src.source_id)}
                        className="w-4 h-4 flex-shrink-0 rounded border-stone-300 text-primary-500 accent-primary-500"
                        aria-label={`Select ${src.title}`}
                      />
                      <div className="w-8 h-8 rounded-lg bg-primary-50 flex items-center justify-center flex-shrink-0">
                        <svg className="w-4 h-4 text-primary-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                        </svg>
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium text-stone-800 truncate">{src.title}</p>
                        <p className="text-xs text-stone-500 truncate">
                          {t('intelligence.chunk', { count: src.chunk_count })}
                          {src.topic_group ? ` · ${src.topic_group}` : ''}
                          {src.tags ? ` · ${src.tags}` : ''}
                          {' · '}{formatDate(src.created_at)}
                        </p>
                      </div>
                      <button
                        type="button"
                        aria-label={t('intelligence.delete_document')}
                        onClick={() => handleDeleteSource(src.source_id)}
                        className="opacity-0 group-hover:opacity-100 transition-opacity text-stone-400 hover:text-red-500 p-1 rounded"
                      >
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        </div>
      </div>
    </div>
  );
}
