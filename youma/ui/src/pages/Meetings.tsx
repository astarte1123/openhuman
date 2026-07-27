import { useCallback, useEffect, useRef, useState } from 'react';
import { SUPPORTED_LANGUAGES } from '../i18n/index';
import i18n from '../i18n/index';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { useTranslation } from 'react-i18next';
import {
  meetingsList,
  meetingsGet,
  meetingsDelete,
  meetingsSave,
  meetingsUpdate,
  meetingsUpdateBody,
  meetingsExport,
  meetingsCalendarHint,
  meetingsDiarize,
  meetingsShare,
  settingsGet,
} from '../api';
import type { MeetingMeta, MeetingDetail } from '../api';
import { useChunkedRecorder } from '../hooks/useChunkedRecorder';

// ---------------------------------------------------------------------------
// Web Speech API types
// ---------------------------------------------------------------------------
interface ISREvent extends Event {
  resultIndex: number;
  results: { [i: number]: { [j: number]: { transcript: string } } & { isFinal?: boolean } };
}
interface ISRErrorEvent extends Event { error: string; }
interface ISR extends EventTarget {
  lang: string; interimResults: boolean; continuous: boolean; maxAlternatives: number;
  onresult: ((e: ISREvent) => void) | null;
  onend: (() => void) | null;
  onerror: ((e: ISRErrorEvent) => void) | null;
  start(): void; stop(): void;
}
type SRCtor = new () => ISR;
function getNativeSR(): SRCtor | null {
  const w = window as unknown as Record<string, unknown>;
  return (w['SpeechRecognition'] ?? w['webkitSpeechRecognition'] ?? null) as SRCtor | null;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleString(navigator.language ?? 'it-IT', {
      day: '2-digit', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit',
    });
  } catch {
    return iso.slice(0, 16).replace('T', ' ');
  }
}

function downloadFile(filename: string, content: string) {
  const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url; a.download = filename; a.click();
  URL.revokeObjectURL(url);
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const TEMPLATES = [
  { id: 'general',       labelKey: 'meetings.tpl_general' },
  { id: 'standup',       labelKey: 'meetings.tpl_standup' },
  { id: 'one_on_one',    labelKey: 'meetings.tpl_one_on_one' },
  { id: 'retrospective', labelKey: 'meetings.tpl_retrospective' },
  { id: 'planning',      labelKey: 'meetings.tpl_planning' },
];

const LANGUAGES = SUPPORTED_LANGUAGES;

// ---------------------------------------------------------------------------
// MeetingCard
// ---------------------------------------------------------------------------
function MeetingCard({ meta, onSelect, onDelete, selected, onToggleSelect }: {
  meta: MeetingMeta; onSelect: () => void; onDelete: () => void;
  selected?: boolean; onToggleSelect?: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className={`group relative flex gap-2 px-3 py-2.5 rounded-xl border bg-white hover:shadow-sm transition-all ${selected ? 'border-primary-300 bg-primary-50/40' : 'border-stone-200 hover:border-primary-300'}`}>
      {onToggleSelect && (
        <input
          type="checkbox"
          checked={!!selected}
          onChange={onToggleSelect}
          onClick={e => e.stopPropagation()}
          className="mt-1 flex-shrink-0 w-3.5 h-3.5 rounded border-stone-300 text-primary-500 accent-primary-500"
          aria-label={meta.title || t('meetings.untitled')}
        />
      )}
      <div className="flex-1 min-w-0 flex flex-col gap-1">
        <div className="flex items-start justify-between gap-2">
          <button type="button" className="flex-1 text-left text-sm font-medium text-stone-800 line-clamp-1" onClick={onSelect}>
            {meta.title || t('meetings.untitled')}
          </button>
          <button type="button"
            className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-red-50 hover:text-red-500 text-stone-400 transition-all flex-shrink-0"
            onClick={e => { e.stopPropagation(); onDelete(); }} aria-label={t('meetings.delete')}>
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div className="flex items-center flex-wrap gap-x-2 gap-y-0.5 text-xs text-stone-400">
          <span>{formatDate(meta.date)}</span>
          {meta.duration_secs > 0 && <span>· {formatDuration(meta.duration_secs)}</span>}
          {meta.template && meta.template !== 'general' && (
            <span className="bg-primary-50 text-primary-500 rounded px-1">{meta.template}</span>
          )}
          {meta.tags.slice(0, 2).map(tag => (
            <span key={tag} className="bg-stone-100 text-stone-500 rounded px-1">{tag}</span>
          ))}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// EditMetaPanel
// ---------------------------------------------------------------------------
function EditMetaPanel({ meta, onSave, onCancel }: {
  meta: MeetingMeta;
  onSave: (title: string, tags: string[], attendees: string[]) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [title, setTitle] = useState(meta.title);
  const [tags, setTags] = useState(meta.tags.join(', '));
  const [attendees, setAttendees] = useState(meta.attendees.join(', '));
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    setSaving(true);
    await onSave(
      title,
      tags.split(',').map(s => s.trim()).filter(Boolean),
      attendees.split(',').map(s => s.trim()).filter(Boolean),
    );
    setSaving(false);
  };

  return (
    <div className="flex flex-col gap-2 rounded-xl border border-primary-200 bg-primary-50 p-3">
      <input type="text" className="w-full px-2.5 py-1.5 text-sm rounded-lg border border-stone-200 bg-white focus:outline-none focus:ring-1 focus:ring-primary-300"
        placeholder={t('meetings.title_placeholder')} value={title} onChange={e => setTitle(e.target.value)} />
      <input type="text" className="w-full px-2.5 py-1.5 text-sm rounded-lg border border-stone-200 bg-white focus:outline-none focus:ring-1 focus:ring-primary-300"
        placeholder={t('meetings.attendees_placeholder')} value={attendees} onChange={e => setAttendees(e.target.value)} />
      <input type="text" className="w-full px-2.5 py-1.5 text-sm rounded-lg border border-stone-200 bg-white focus:outline-none focus:ring-1 focus:ring-primary-300"
        placeholder={t('meetings.tags_placeholder')} value={tags} onChange={e => setTags(e.target.value)} />
      <div className="flex gap-2">
        <button type="button"
          className="flex-1 py-1.5 rounded-lg bg-primary-500 hover:bg-primary-600 text-white text-xs font-medium transition-colors disabled:opacity-50"
          onClick={handleSave} disabled={saving}>
          {saving ? t('meetings.saving') : t('meetings.save')}
        </button>
        <button type="button"
          className="flex-1 py-1.5 rounded-lg bg-stone-100 hover:bg-stone-200 text-stone-700 text-xs font-medium transition-colors"
          onClick={onCancel}>{t('meetings.cancel')}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// SetupPanel — pre-recording configuration
// ---------------------------------------------------------------------------
interface SetupState {
  title: string;
  attendees: string;
  template: string;
  lang: string;
  meetLink?: string;
}

function SetupPanel({ initial, onStart, onCancel }: {
  initial: SetupState;
  onStart: (cfg: SetupState) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [cfg, setCfg] = useState<SetupState>(initial);
  const [calLoading, setCalLoading] = useState(false);
  const [calError, setCalError] = useState<string | null>(null);

  const fetchCalendar = async () => {
    setCalLoading(true);
    setCalError(null);
    try {
      const hint = await meetingsCalendarHint();
      if (hint.title === null && hint.attendees.length === 0) {
        setCalError(t('meetings.cal_no_event'));
      } else {
        setCfg(prev => ({
          ...prev,
          title: hint.title ?? prev.title,
          attendees: hint.attendees.length > 0 ? hint.attendees.join(', ') : prev.attendees,
          meetLink: hint.meet_link ?? prev.meetLink,
        }));
      }
    } catch (e) {
      setCalError(t('meetings.cal_error', { error: String(e) }));
    } finally {
      setCalLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-3 py-2">
      <p className="text-xs font-medium text-stone-500 uppercase tracking-wide">{t('meetings.setup_title')}</p>

      {/* Calendar pre-fill */}
      <div className="flex flex-col gap-1">
        <button type="button"
          className="flex items-center gap-2 text-xs text-primary-600 hover:text-primary-800 disabled:opacity-40"
          onClick={fetchCalendar} disabled={calLoading}>
          {calLoading
            ? <span className="w-3 h-3 border border-primary-300 border-t-primary-600 rounded-full animate-spin" />
            : <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                  d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
              </svg>
          }
          {t('meetings.fill_from_calendar')}
        </button>
        {calError && (
          <p className="text-xs text-amber-600">{calError}</p>
        )}
      </div>

      <input type="text"
        className="w-full px-2.5 py-1.5 text-sm rounded-lg border border-stone-200 bg-white focus:outline-none focus:ring-1 focus:ring-primary-300"
        placeholder={t('meetings.title_placeholder')}
        value={cfg.title} onChange={e => setCfg(p => ({ ...p, title: e.target.value }))} />

      <input type="text"
        className="w-full px-2.5 py-1.5 text-sm rounded-lg border border-stone-200 bg-white focus:outline-none focus:ring-1 focus:ring-primary-300"
        placeholder={t('meetings.attendees_placeholder')}
        value={cfg.attendees} onChange={e => setCfg(p => ({ ...p, attendees: e.target.value }))} />

      {/* Template selector */}
      <div>
        <p className="text-xs text-stone-400 mb-1">{t('meetings.template_label')}</p>
        <div className="flex flex-wrap gap-1.5">
          {TEMPLATES.map(tpl => (
            <button key={tpl.id} type="button"
              className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors ${
                cfg.template === tpl.id
                  ? 'bg-primary-500 text-white'
                  : 'bg-stone-100 text-stone-600 hover:bg-stone-200'
              }`}
              onClick={() => setCfg(p => ({ ...p, template: tpl.id }))}>
              {t(tpl.labelKey)}
            </button>
          ))}
        </div>
      </div>

      {/* Language selector */}
      <div>
        <p className="text-xs text-stone-400 mb-1">{t('meetings.language_label')}</p>
        <select
          aria-label={t('meetings.language_label')}
          className="w-full px-2.5 py-1.5 text-sm rounded-lg border border-stone-200 bg-white focus:outline-none focus:ring-1 focus:ring-primary-300"
          value={cfg.lang} onChange={e => setCfg(p => ({ ...p, lang: e.target.value }))}>
          {LANGUAGES.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}
        </select>
      </div>

      {/* C: Google Meet / Teams link */}
      {cfg.meetLink && (
        <a
          href={cfg.meetLink}
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-2 px-3 py-2 rounded-lg bg-green-50 border border-green-200 text-green-700 text-xs font-medium hover:bg-green-100 transition-colors"
        >
          <svg className="w-3.5 h-3.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
              d="M15 10l4.553-2.276A1 1 0 0121 8.723v6.554a1 1 0 01-1.447.894L15 14M3 8a2 2 0 012-2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V8z" />
          </svg>
          {t('meetings.meet_link')}
        </a>
      )}

      <div className="flex gap-2 pt-1">
        <button type="button"
          className="flex-1 flex items-center justify-center gap-2 py-2.5 rounded-xl bg-red-500 hover:bg-red-600 active:scale-95 text-white text-sm font-medium transition-all"
          onClick={() => onStart(cfg)}>
          <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><circle cx="12" cy="12" r="6" /></svg>
          {t('meetings.start_recording')}
        </button>
        <button type="button"
          className="px-4 py-2.5 rounded-xl bg-stone-100 hover:bg-stone-200 text-stone-700 text-sm font-medium transition-colors"
          onClick={onCancel}>{t('meetings.cancel')}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main page
// ---------------------------------------------------------------------------
type View = 'list' | 'setup' | 'recording' | 'saving' | 'detail';

interface SpeakerInsert {
  atLength: number;
  name: string;
}

export default function MeetingsPage() {
  const { t } = useTranslation();

  const [view, setView] = useState<View>('list');
  const [meetings, setMeetings] = useState<MeetingMeta[]>([]);
  const [loadingList, setLoadingList] = useState(false);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [search, setSearch] = useState('');
  const [selectedMeetings, setSelectedMeetings] = useState<Set<string>>(new Set());
  const [meetBulkDeleting, setMeetBulkDeleting] = useState(false);
  const meetSelectAllRef = useRef<HTMLInputElement>(null);
  const [editing, setEditing] = useState(false);
  const [editingBody, setEditingBody] = useState(false);
  const [bodyDraft, setBodyDraft] = useState('');
  const [savingBody, setSavingBody] = useState(false);
  const [savingStatus, setSavingStatus] = useState('');
  const [pageError, setPageError] = useState<string | null>(null);
  const [diarizeEnabled, setDiarizeEnabled] = useState(false);

  // Share state (B)
  const [shareTarget, setShareTarget] = useState<'slack' | 'notion' | 'github' | 'jira' | null>(null);
  const [shareParam, setShareParam] = useState('');
  const [sharing, setSharing] = useState(false);
  const [shareError, setShareError] = useState<string | null>(null);
  const [shareOk, setShareOk] = useState(false);
  const shareDefaults = useRef({ slack: '', github: '', jira: '' });

  // Setup state — defaults loaded from settings
  const [setupCfg, setSetupCfg] = useState<SetupState>({
    title: '', attendees: '', template: 'general',
    lang: i18n.language?.slice(0, 2) ?? 'it',
  });

  useEffect(() => {
    async function loadDefaults() {
      const [tpl, lang, diarize, slack, github, jira] = await Promise.all([
        settingsGet('meetings.default_template'),
        settingsGet('meetings.default_lang'),
        settingsGet('meetings.diarize'),
        settingsGet('meetings.share_slack_channel'),
        settingsGet('meetings.share_github_repo'),
        settingsGet('meetings.share_jira_project'),
      ]);
      setSetupCfg(prev => ({
        ...prev,
        template: tpl ?? prev.template,
        lang: lang ?? prev.lang,
      }));
      if (diarize === '1') setDiarizeEnabled(true);
      shareDefaults.current = {
        slack: slack ?? '',
        github: github ?? '',
        jira: jira ?? '',
      };
    }
    loadDefaults().catch(() => {});
  }, []);

  // Web Speech
  const srRef = useRef<ISR | null>(null);
  const [srTranscript, setSrTranscript] = useState('');
  const srTranscriptRef = useRef('');
  const srFinalRef = useRef('');
  const [srElapsed, setSrElapsed] = useState(0);
  const srTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const srStartRef = useRef(0);
  const stoppingRef = useRef(false);

  // Speaker tagging
  const [speakerCount, setSpeakerCount] = useState(0);
  const speakerNames = useRef<string[]>([]);   // attendee names, populated at recording start
  const speakerInserts = useRef<SpeakerInsert[]>([]);

  // Chunked recorder fallback
  const chunked = useChunkedRecorder();

  const useNativeSR = !!getNativeSR();
  const liveTranscript = useNativeSR ? srTranscript : chunked.transcript;
  const elapsed = useNativeSR ? srElapsed : chunked.elapsed;

  // ---------------------------------------------------------------------------
  // Load list
  // ---------------------------------------------------------------------------
  const loadList = useCallback(async () => {
    setLoadingList(true);
    try { setMeetings(await meetingsList()); }
    catch (e) { console.error('[meetings] load', e); }
    finally { setLoadingList(false); }
  }, []);

  useEffect(() => { loadList(); }, [loadList]);
  useEffect(() => () => { chunked.cleanup(); }, [chunked.cleanup]);

  // ---------------------------------------------------------------------------
  // Recording start — receives setup config
  // ---------------------------------------------------------------------------
  const startRecording = useCallback(async (cfg: SetupState) => {
    setSetupCfg(cfg);
    setPageError(null);
    setSrTranscript('');
    srTranscriptRef.current = '';
    srFinalRef.current = '';
    speakerInserts.current = [];
    // Pre-populate speakers from attendees field.
    const names = cfg.attendees
      .split(',')
      .map(s => s.trim())
      .filter(Boolean);
    speakerNames.current = names;
    setSpeakerCount(names.length);

    if (useNativeSR) {
      const SR = getNativeSR()!;
      const rec = new SR();
      rec.lang = cfg.lang;
      rec.interimResults = true;
      rec.continuous = true;
      rec.maxAlternatives = 1;

      rec.onresult = (e: ISREvent) => {
        const len = Object.keys(e.results).length;
        let interim = '';
        // Start from resultIndex — skip results already processed in previous events.
        for (let i = e.resultIndex; i < len; i++) {
          const r = e.results[i];
          if ((r as unknown as { isFinal?: boolean }).isFinal) {
            srFinalRef.current += r[0].transcript + ' ';
          } else {
            interim = r[0].transcript;
          }
        }
        const combined = (srFinalRef.current + interim).trim();
        srTranscriptRef.current = combined;
        setSrTranscript(combined);
      };
      rec.onend = () => {};
      rec.onerror = (e: ISRErrorEvent) => {
        if (e.error !== 'no-speech') setPageError(`Speech error: ${e.error}`);
      };
      try { rec.start(); } catch (err) { setPageError(String(err)); return; }
      srRef.current = rec;

      srStartRef.current = Date.now();
      setSrElapsed(0);
      srTimerRef.current = setInterval(
        () => setSrElapsed(Math.floor((Date.now() - srStartRef.current) / 1000)), 1000);
    } else {
      await chunked.start();
    }
    setView('recording');
  }, [useNativeSR, chunked]);

  // ---------------------------------------------------------------------------
  // Speaker tag insertion
  // ---------------------------------------------------------------------------
  const addSpeakerTag = useCallback((name: string) => {
    const currentLen = useNativeSR
      ? srTranscriptRef.current.length
      : chunked.transcript.length;
    speakerInserts.current.push({ atLength: currentLen, name });
    // Visually inject into the live transcript
    const tag = `\n\n[${name}]: `;
    if (useNativeSR) {
      srFinalRef.current += tag;
      srTranscriptRef.current = srFinalRef.current.trim();
      setSrTranscript(srTranscriptRef.current);
    }
  }, [useNativeSR, chunked.transcript.length]);

  const addNextSpeaker = useCallback(() => {
    const n = speakerCount + 1;
    setSpeakerCount(n);
    const name = speakerNames.current[speakerCount] ?? `Parlante ${n}`;
    addSpeakerTag(name);
  }, [speakerCount, addSpeakerTag]);

  // ---------------------------------------------------------------------------
  // Recording stop
  // ---------------------------------------------------------------------------
  const stopRecording = useCallback(async () => {
    if (stoppingRef.current) return;
    stoppingRef.current = true;
    const duration = Math.floor((Date.now() - srStartRef.current) / 1000);

    if (srTimerRef.current) { clearInterval(srTimerRef.current); srTimerRef.current = null; }
    srRef.current?.stop(); srRef.current = null;

    setView('saving');
    setSavingStatus(t('meetings.summarizing'));

    let transcript = '';
    if (useNativeSR) {
      transcript = srTranscriptRef.current.trim();
    } else {
      setSavingStatus(t('meetings.transcribing'));
      transcript = await chunked.stop();

      // Diarization: replace plain transcript with speaker-labeled version.
      if (diarizeEnabled) {
        const fullBlob = chunked.getFullBlob();
        if (fullBlob && fullBlob.size > 500) {
          setSavingStatus(t('meetings.diarizing'));
          try {
            const buffer = await fullBlob.arrayBuffer();
            const bytes = new Uint8Array(buffer);
            let binary = '';
            const sz = 8192;
            for (let i = 0; i < bytes.length; i += sz)
              binary += String.fromCharCode(...bytes.subarray(i, i + sz));
            const attendees = setupCfg.attendees.split(',').map(s => s.trim()).filter(Boolean);
            const result = await meetingsDiarize({
              audio_b64: btoa(binary),
              format: 'webm',
              attendees: attendees.length > 0 ? attendees : undefined,
              lang: setupCfg.lang,
            });
            if (result.transcript.trim()) transcript = result.transcript;
          } catch (e) {
            console.warn('[meetings] diarization failed, using plain transcript', e);
          }
        }
      }
    }

    if (!transcript.trim()) {
      stoppingRef.current = false;
      setPageError(t('meetings.empty_transcript'));
      setView('list');
      return;
    }

    setSavingStatus(t('meetings.summarizing'));
    try {
      const title = setupCfg.title.trim() || undefined;
      const attendees = setupCfg.attendees.split(',').map(s => s.trim()).filter(Boolean);
      await meetingsSave({
        transcript,
        title,
        attendees: attendees.length > 0 ? attendees : undefined,
        duration_secs: useNativeSR ? duration : chunked.elapsed,
        template: setupCfg.template,
        lang: setupCfg.lang,
      });
      await loadList();
    } catch (e) {
      setPageError(String(e));
    } finally {
      stoppingRef.current = false;
      // Clear title and attendees for the next recording; keep template and lang.
      setSetupCfg(prev => ({ ...prev, title: '', attendees: '' }));
    }
    setView('list');
  }, [useNativeSR, chunked, t, loadList, setupCfg, diarizeEnabled]);

  // ---------------------------------------------------------------------------
  // Detail
  // ---------------------------------------------------------------------------
  const openDetail = useCallback(async (id: string) => {
    try {
      setDetail(await meetingsGet(id));
      setEditing(false);
      setEditingBody(false);
      setBodyDraft('');
      setShareTarget(null);
      setShareParam('');
      setShareError(null);
      setShareOk(false);
      setView('detail');
    } catch (e) { console.error('[meetings] get', e); }
  }, []);

  const handleDelete = useCallback(async (id: string) => {
    try {
      await meetingsDelete(id);
      setSelectedMeetings(prev => { const n = new Set(prev); n.delete(id); return n; });
      if (detail?.meta.id === id) { setDetail(null); setView('list'); }
      await loadList();
    } catch (e) { console.error('[meetings] delete', e); }
  }, [detail, loadList]);

  const handleSaveEdit = useCallback(async (title: string, tags: string[], attendees: string[]) => {
    if (!detail) return;
    const updated = await meetingsUpdate(detail.meta.id, { title, tags, attendees });
    setDetail(prev => prev ? { ...prev, meta: updated } : prev);
    setEditing(false);
  }, [detail]);

  const handleExport = useCallback(async () => {
    if (!detail) return;
    try {
      const { filename, content } = await meetingsExport(detail.meta.id);
      downloadFile(filename, content);
    } catch (e) { setPageError(String(e)); }
  }, [detail]);

  const handleSaveBody = useCallback(async () => {
    if (!detail) return;
    setSavingBody(true);
    try {
      await meetingsUpdateBody(detail.meta.id, bodyDraft);
      setDetail(prev => prev ? { ...prev, body: bodyDraft } : prev);
      setEditingBody(false);
    } catch (e) { setPageError(String(e)); }
    finally { setSavingBody(false); }
  }, [detail, bodyDraft]);

  const handleShare = useCallback(async (target: 'slack' | 'notion' | 'github' | 'jira', param: string) => {
    if (!detail) return;
    setSharing(true);
    setShareError(null);
    setShareOk(false);
    try {
      await meetingsShare({
        id: detail.meta.id,
        target,
        channel: target === 'slack' ? param : undefined,
        repo: target === 'github' ? param : undefined,
        project: target === 'jira' ? param : undefined,
      });
      setShareOk(true);
      setShareTarget(null);
      setShareParam('');
      setTimeout(() => setShareOk(false), 3000);
    } catch (e) {
      setShareError(String(e));
    } finally {
      setSharing(false);
    }
  }, [detail]);

  // ---------------------------------------------------------------------------
  // Filtered list + select helpers
  // ---------------------------------------------------------------------------
  const filtered = search.trim()
    ? meetings.filter(m =>
        m.title.toLowerCase().includes(search.toLowerCase()) ||
        m.tags.some(tg => tg.toLowerCase().includes(search.toLowerCase())))
    : meetings;

  // Sync indeterminate on select-all checkbox
  useEffect(() => {
    const el = meetSelectAllRef.current;
    if (!el) return;
    el.indeterminate = selectedMeetings.size > 0 && selectedMeetings.size < filtered.length;
  }, [selectedMeetings, filtered.length]);

  function toggleSelectMeeting(id: string) {
    setSelectedMeetings(prev => {
      const n = new Set(prev);
      if (n.has(id)) n.delete(id); else n.add(id);
      return n;
    });
  }

  function toggleSelectAllMeetings() {
    if (selectedMeetings.size === filtered.length && filtered.length > 0) {
      setSelectedMeetings(new Set());
    } else {
      setSelectedMeetings(new Set(filtered.map(m => m.id)));
    }
  }

  async function handleDeleteSelectedMeetings() {
    if (selectedMeetings.size === 0) return;
    setMeetBulkDeleting(true);
    const ids = [...selectedMeetings];
    for (const id of ids) {
      try { await meetingsDelete(id); }
      catch (e) { console.error('[meetings] bulk delete', e); }
    }
    setMeetings(prev => prev.filter(m => !ids.includes(m.id)));
    setSelectedMeetings(new Set());
    if (detail && ids.includes(detail.meta.id)) { setDetail(null); setView('list'); }
    setMeetBulkDeleting(false);
  }

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------
  return (
    <div className="flex flex-col h-full overflow-hidden">

      {/* Header */}
      <div className="flex-shrink-0 px-4 pt-4 pb-2">
        <div className="flex items-center justify-between mb-3">
          <h1 className="text-lg font-semibold text-stone-800">{t('meetings.title')}</h1>
          {(view === 'detail' || view === 'setup') && (
            <button type="button" className="text-xs text-stone-500 hover:text-stone-800"
              onClick={() => { setDetail(null); setEditing(false); setView('list'); }}>
              {t('meetings.back')}
            </button>
          )}
        </div>
        {view === 'list' && (
          <div className="relative">
            <svg className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-stone-400 pointer-events-none"
              fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <input type="text"
              className="w-full pl-8 pr-3 py-1.5 text-sm rounded-lg border border-stone-200 bg-stone-50 focus:outline-none focus:ring-1 focus:ring-primary-300"
              placeholder={t('meetings.search_placeholder')}
              value={search} onChange={e => { setSearch(e.target.value); setSelectedMeetings(new Set()); }} />
          </div>
        )}
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto px-4 pb-4">

        {/* List */}
        {view === 'list' && (
          <div className="flex flex-col gap-2">
            {loadingList
              ? <div className="flex justify-center py-8"><div className="w-5 h-5 border-2 border-primary-400 border-t-transparent rounded-full animate-spin" /></div>
              : filtered.length === 0
                ? <div className="text-center py-10 text-stone-400 text-sm">{search ? t('meetings.no_results') : t('meetings.no_meetings')}</div>
                : (
                  <>
                    {/* Select-all + bulk delete bar */}
                    <div className="flex items-center gap-3 px-1">
                      <label className="flex items-center gap-2 cursor-pointer select-none">
                        <input
                          ref={meetSelectAllRef}
                          type="checkbox"
                          checked={selectedMeetings.size === filtered.length && filtered.length > 0}
                          onChange={toggleSelectAllMeetings}
                          className="w-4 h-4 rounded border-stone-300 text-primary-500 accent-primary-500"
                          aria-label={t('meetings.select_all')}
                        />
                        <span className="text-xs text-stone-500">
                          {selectedMeetings.size === 0
                            ? t('meetings.meetings_count', { count: filtered.length })
                            : t('meetings.selected', { count: selectedMeetings.size })}
                        </span>
                      </label>
                      {selectedMeetings.size > 0 && (
                        <button
                          type="button"
                          onClick={handleDeleteSelectedMeetings}
                          disabled={meetBulkDeleting}
                          className="ml-auto flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-red-50 hover:bg-red-100 text-red-600 border border-red-200 rounded-lg disabled:opacity-40 transition-colors"
                        >
                          {meetBulkDeleting
                            ? <div className="w-3 h-3 border border-red-400 border-t-transparent rounded-full animate-spin" />
                            : <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
                          }
                          {t('meetings.delete_selected', { count: selectedMeetings.size })}
                        </button>
                      )}
                    </div>
                    {filtered.map(m => (
                      <MeetingCard key={m.id} meta={m}
                        onSelect={() => openDetail(m.id)}
                        onDelete={() => handleDelete(m.id)}
                        selected={selectedMeetings.has(m.id)}
                        onToggleSelect={() => toggleSelectMeeting(m.id)} />
                    ))}
                  </>
                )
            }
          </div>
        )}

        {/* Setup */}
        {view === 'setup' && (
          <SetupPanel
            initial={setupCfg}
            onStart={startRecording}
            onCancel={() => setView('list')} />
        )}

        {/* Recording */}
        {view === 'recording' && (
          <div className="flex flex-col items-center gap-4 py-4">
            <div className="flex items-center gap-2">
              <span className="inline-block w-2.5 h-2.5 rounded-full bg-red-500 animate-pulse" />
              <span className="text-2xl font-mono text-stone-800 tabular-nums">{formatDuration(elapsed)}</span>
              {setupCfg.template !== 'general' && (
                <span className="text-xs bg-primary-50 text-primary-600 rounded px-1.5 py-0.5">
                  {t(`meetings.tpl_${setupCfg.template}`)}
                </span>
              )}
            </div>

            {chunked.transcribing && !useNativeSR && (
              <div className="flex items-center gap-1.5 text-xs text-stone-400">
                <div className="w-3 h-3 border border-stone-300 border-t-primary-400 rounded-full animate-spin" />
                {t('meetings.transcribing_segment')}
              </div>
            )}

            {/* Speaker tagging */}
            <div className="w-full flex flex-col gap-1.5">
              <div className="flex items-center gap-1.5 flex-wrap">
              <span className="text-xs text-stone-400">{t('meetings.speakers')}:</span>
              {Array.from({ length: speakerCount }, (_, i) => {
                const name = speakerNames.current[i] ?? `Parlante ${i + 1}`;
                return (
                  <button key={i} type="button"
                    className="px-2 py-0.5 rounded-lg text-xs bg-stone-100 text-stone-600 hover:bg-stone-200 transition-colors"
                    onClick={() => addSpeakerTag(name)}>
                    {name}
                  </button>
                );
              })}
              <button type="button"
                className="px-2 py-0.5 rounded-lg text-xs bg-primary-50 text-primary-600 hover:bg-primary-100 transition-colors"
                onClick={addNextSpeaker}>
                + {t('meetings.new_speaker')}
              </button>
              </div>
              <p className="text-xs text-stone-300 italic">{t('meetings.speaker_tag_hint')}</p>
            </div>

            {setupCfg.meetLink && (
              <a
                href={setupCfg.meetLink}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-1.5 text-xs text-green-600 hover:text-green-800"
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                    d="M15 10l4.553-2.276A1 1 0 0121 8.723v6.554a1 1 0 01-1.447.894L15 14M3 8a2 2 0 012-2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V8z" />
                </svg>
                {t('meetings.meet_link')}
              </a>
            )}

            <p className="text-xs text-stone-400">{t('meetings.recording_hint')}</p>

            {liveTranscript && (
              <div className="w-full max-h-44 overflow-y-auto rounded-xl border border-stone-200 bg-stone-50 p-3 text-sm text-stone-700 leading-relaxed whitespace-pre-wrap">
                {liveTranscript}
              </div>
            )}
          </div>
        )}

        {/* Saving */}
        {view === 'saving' && (
          <div className="flex flex-col items-center gap-3 py-10">
            <div className="w-7 h-7 border-2 border-primary-400 border-t-transparent rounded-full animate-spin" />
            <p className="text-sm text-stone-500">{savingStatus}</p>
          </div>
        )}

        {/* Detail */}
        {view === 'detail' && detail && (
          <div className="flex flex-col gap-3">
            <div className="rounded-xl border border-stone-200 bg-white p-3">
              <div className="flex items-start justify-between gap-2 mb-1">
                <h2 className="font-semibold text-stone-800 flex-1">{detail.meta.title || t('meetings.untitled')}</h2>
                <div className="flex gap-1 flex-shrink-0">
                  <button type="button"
                    className="p-1 rounded hover:bg-stone-100 text-stone-400 hover:text-stone-600 transition-colors"
                    onClick={() => setEditing(e => !e)} aria-label={t('meetings.edit')}>
                    <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                        d="M15.232 5.232l3.536 3.536M9 13l6.414-6.414a2 2 0 012.828 2.828L11.828 15.828a2 2 0 01-1.414.586H8v-2.414a2 2 0 01.586-1.414z" />
                    </svg>
                  </button>
                  <button type="button"
                    className="p-1 rounded hover:bg-stone-100 text-stone-400 hover:text-stone-600 transition-colors"
                    onClick={handleExport} aria-label={t('meetings.export')}>
                    <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                        d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                    </svg>
                  </button>
                  <button type="button"
                    className="p-1 rounded hover:bg-red-50 text-stone-400 hover:text-red-500 transition-colors"
                    onClick={() => handleDelete(detail.meta.id)} aria-label={t('meetings.delete')}>
                    <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                        d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </button>
                </div>
              </div>
              <div className="flex flex-wrap gap-2 text-xs text-stone-400 mb-2">
                <span>{formatDate(detail.meta.date)}</span>
                {detail.meta.duration_secs > 0 && <span>· {formatDuration(detail.meta.duration_secs)}</span>}
                {detail.meta.lang && <span>· {detail.meta.lang}</span>}
                {detail.meta.attendees.length > 0 && <span>· {detail.meta.attendees.join(', ')}</span>}
              </div>
              <div className="flex flex-wrap gap-1">
                {detail.meta.template && detail.meta.template !== 'general' && (
                  <span className="bg-primary-50 text-primary-500 rounded px-1.5 py-0.5 text-xs">{t(`meetings.tpl_${detail.meta.template}`)}</span>
                )}
                {detail.meta.tags.map(tag => (
                  <span key={tag} className="bg-stone-100 text-stone-500 rounded px-1.5 py-0.5 text-xs">{tag}</span>
                ))}
              </div>
            </div>

            {editing && (
              <EditMetaPanel meta={detail.meta} onSave={handleSaveEdit} onCancel={() => setEditing(false)} />
            )}

            {/* B: Share panel */}
            <div className="rounded-xl border border-stone-200 bg-white p-3 flex flex-col gap-2">
              <p className="text-xs font-semibold uppercase tracking-wide text-stone-400">{t('meetings.share_title')}</p>
              <div className="flex flex-wrap gap-1.5">
                {(['slack', 'notion', 'github', 'jira'] as const).map(tgt => (
                  <button key={tgt} type="button"
                    onClick={() => {
                      const next = shareTarget === tgt ? null : tgt;
                      setShareTarget(next);
                      const defaults: Record<string, string> = shareDefaults.current;
                      setShareParam(next ? (defaults[next] ?? '') : '');
                      setShareError(null);
                    }}
                    className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors ${
                      shareTarget === tgt
                        ? 'bg-primary-500 text-white'
                        : 'bg-stone-100 text-stone-600 hover:bg-stone-200'
                    }`}>
                    {t(`meetings.share_${tgt}`)}
                  </button>
                ))}
                {shareOk && <span className="text-xs text-green-600 self-center">{t('meetings.share_ok')}</span>}
              </div>

              {shareTarget && (
                <div className="flex gap-2">
                  {(shareTarget === 'slack' || shareTarget === 'github' || shareTarget === 'jira') && (
                    <input
                      type="text"
                      className="flex-1 px-2.5 py-1.5 text-xs rounded-lg border border-stone-200 bg-stone-50 focus:outline-none focus:ring-1 focus:ring-primary-300"
                      placeholder={t(`meetings.share_${shareTarget}_ph`)}
                      value={shareParam}
                      onChange={e => setShareParam(e.target.value)}
                    />
                  )}
                  <button type="button"
                    disabled={sharing || (shareTarget !== 'notion' && !shareParam.trim())}
                    onClick={() => handleShare(shareTarget, shareParam.trim())}
                    className="px-3 py-1.5 rounded-lg bg-primary-500 hover:bg-primary-600 text-white text-xs font-medium transition-colors disabled:opacity-40">
                    {sharing ? t('meetings.share_sending') : t('meetings.share_send')}
                  </button>
                </div>
              )}
              {shareError && <p className="text-xs text-red-500">{t('meetings.share_err', { error: shareError })}</p>}
            </div>

            <div className="rounded-xl border border-stone-200 bg-white p-3">
              <div className="flex items-center justify-between mb-3">
                <span className="text-xs font-semibold uppercase tracking-wide text-stone-400">{t('meetings.body_section')}</span>
                <div className="flex gap-1.5">
                  <button type="button"
                    className="flex items-center gap-1 px-2.5 py-1 rounded-lg text-xs text-stone-600 bg-stone-100 hover:bg-stone-200 transition-colors"
                    onClick={handleExport}>
                    <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                        d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                    </svg>
                    {t('meetings.download')}
                  </button>
                  {!editingBody && (
                    <button type="button"
                      className="px-2.5 py-1 rounded-lg text-xs text-stone-600 bg-stone-100 hover:bg-stone-200 transition-colors"
                      onClick={() => { setBodyDraft(detail.body); setEditingBody(true); }}>
                      {t('meetings.edit_body')}
                    </button>
                  )}
                </div>
              </div>
              {editingBody ? (
                <div className="flex flex-col gap-2">
                  <textarea
                    aria-label={t('meetings.body_section')}
                    className="w-full h-80 px-3 py-2 text-sm font-mono rounded-lg border border-stone-200 bg-stone-50 focus:outline-none focus:ring-1 focus:ring-primary-300 resize-y"
                    value={bodyDraft}
                    onChange={e => setBodyDraft(e.target.value)}
                  />
                  <div className="flex gap-2 justify-end">
                    <button type="button"
                      className="px-3 py-1.5 rounded-lg text-xs text-stone-600 hover:bg-stone-100 transition-colors"
                      onClick={() => setEditingBody(false)}>
                      {t('meetings.cancel')}
                    </button>
                    <button type="button"
                      disabled={savingBody}
                      className="px-3 py-1.5 rounded-lg bg-primary-500 hover:bg-primary-600 text-white text-xs font-medium transition-colors disabled:opacity-40"
                      onClick={handleSaveBody}>
                      {savingBody ? t('meetings.saving') : t('meetings.save')}
                    </button>
                  </div>
                </div>
              ) : (
                <div className="prose prose-sm prose-stone max-w-none text-sm">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>{detail.body}</ReactMarkdown>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="flex-shrink-0 border-t border-stone-100 px-4 py-3 flex flex-col gap-2">
        {pageError && <p className="text-xs text-red-500 text-center">{pageError}</p>}

        {(view === 'list' || view === 'detail') && (
          <button type="button"
            className="w-full flex items-center justify-center gap-2 py-2.5 rounded-xl bg-red-500 hover:bg-red-600 active:scale-95 text-white text-sm font-medium transition-all shadow-sm"
            onClick={() => setView('setup')}>
            <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><circle cx="12" cy="12" r="6" /></svg>
            {t('meetings.new_recording')}
          </button>
        )}

        {view === 'recording' && (
          <button type="button"
            className="w-full flex items-center justify-center gap-2 py-2.5 rounded-xl bg-stone-700 hover:bg-stone-800 active:scale-95 text-white text-sm font-medium transition-all shadow-sm"
            onClick={stopRecording}>
            <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
              <rect x="6" y="6" width="12" height="12" rx="1" />
            </svg>
            {t('meetings.stop_recording')}
          </button>
        )}
      </div>
    </div>
  );
}
