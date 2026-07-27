import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { useTranslation } from 'react-i18next';
import { appendMessage, createThread, listThreads } from '../api';
import type { Message, Thread } from '../api';
import { useSTT } from '../hooks/useSTT';
import { useTTS } from '../hooks/useTTS';
import { sanitizeAIContent } from '../utils/sanitize';
import { ThinkingWords } from '../components/ThinkingWords';

interface LocalMessage extends Message {
  id: string;
  pending?: boolean;
}

function uid() {
  return Math.random().toString(36).slice(2) + Date.now().toString(36);
}

// ---------------------------------------------------------------------------
// Mascot — scales to fit its container, never overflows
// ---------------------------------------------------------------------------

function Mascot({
  talking,
  listening,
  mouthAmplitude,
}: {
  talking: boolean;
  listening: boolean;
  mouthAmplitude: number;
}) {
  const { t } = useTranslation();
  const label = listening ? t('human.listening') : talking ? t('human.thinking') : 'Youma';
  const glowRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const el = glowRef.current;
    if (!el) return;
    const opacity = talking ? Math.min(mouthAmplitude * 2.5, 0.7) : listening ? 0.35 : 0;
    const blur = talking ? 8 + mouthAmplitude * 32 : listening ? 16 : 0;
    el.style.setProperty('--glow-opacity', String(opacity));
    el.style.setProperty('--glow-blur', `${blur}px`);
  });

  return (
    <div className="flex flex-col items-center justify-center w-full h-full select-none overflow-hidden">
      <div
        className={`relative flex items-center justify-center w-full flex-1 min-h-0 transition-all duration-500 ${
          listening ? 'scale-110' : talking ? 'scale-105' : 'scale-100'
        }`}
      >
        <img
          src="/mascot.png"
          alt={label}
          draggable={false}
          className={`mascot-img object-contain drop-shadow-lg ${listening ? 'mascot-float-fast' : 'mascot-float'}`}
        />
        <div
          ref={glowRef}
          className={`absolute inset-0 rounded-full pointer-events-none mascot-glow${
            talking || listening ? '' : ' opacity-0'
          }`}
        />
      </div>
      <p className="mt-1 text-xs font-medium text-stone-500 tracking-wide flex-shrink-0">{label}</p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Human page
// ---------------------------------------------------------------------------

export default function HumanPage() {
  const { t } = useTranslation();
  const [thread, setThread] = useState<Thread | null>(null);
  const [messages, setMessages] = useState<LocalMessage[]>([]);
  const [sending, setSending] = useState(false);
  const [speakReplies, setSpeakReplies] = useState(true);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const { speaking: ttsActive, speak, stop: stopTTS, mouthAmplitude } = useTTS();

  useEffect(() => {
    (async () => {
      try {
        const threads = await listThreads();
        const existing = threads.find(t => t.title === '__human__') ?? threads[0] ?? null;
        if (existing) {
          setThread(existing);
        } else {
          const newThread = await createThread();
          setThread(newThread);
        }
      } catch { /* ignore */ }
    })();
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  async function sendMessage(text: string) {
    if (!text.trim() || !thread || sending) return;
    const userMsg: LocalMessage = { id: uid(), role: 'user', content: text };
    const pendingMsg: LocalMessage = { id: uid(), role: 'assistant', content: '', pending: true };
    setMessages(prev => [...prev, userMsg, pendingMsg]);
    setSending(true);
    try {
      const result = await appendMessage(thread.id, { role: 'user', content: text });
      const reply = result.assistant_message.content;
      setMessages(prev => [
        ...prev.filter(m => !m.pending),
        { ...result.assistant_message, id: uid() },
      ]);
      if (speakReplies && reply) speak(sanitizeAIContent(reply));
    } catch { /* ignore */ }
    finally { setSending(false); }
  }

  const handleVoiceResult = useCallback((text: string) => {
    sendMessage(text);
  }, [thread, sending, speakReplies]); // eslint-disable-line react-hooks/exhaustive-deps

  const { recording, transcribing, sttError, toggle: toggleSTT } = useSTT(handleVoiceResult);
  const isListening = recording || transcribing;

  // Mic button scales between 40px (narrow) and 56px (wide) using container query approach
  const micSize = 'w-12 h-12';

  return (
    // human-page-height = height: 100% — fills the flex-1 content area from PanelWindow
    <div className="flex human-page-height min-h-0">

      {/* ---- Left: Mascot panel ---- */}
      <div className="flex flex-col items-center justify-between w-[42%] flex-shrink-0 bg-stone-50 border-r border-stone-200 px-2 py-3 min-h-0 overflow-hidden">

        {/* Speak replies toggle */}
        <div className="w-full flex items-center gap-1.5 flex-shrink-0">
          <label className="flex items-center gap-1.5 cursor-pointer select-none w-full">
            <input
              type="checkbox"
              checked={speakReplies}
              onChange={e => setSpeakReplies(e.target.checked)}
              className="w-3.5 h-3.5 accent-primary-500 cursor-pointer flex-shrink-0"
            />
            <span className="text-xs text-stone-600 font-medium leading-tight truncate">
              {t('human.speak_replies')}
            </span>
          </label>
        </div>

        {/* Mascot — flex-1 fills remaining vertical space */}
        <div className="flex-1 flex items-center justify-center w-full min-h-0 py-2">
          <Mascot talking={ttsActive} listening={isListening} mouthAmplitude={mouthAmplitude} />
        </div>

        {/* Mic + stop buttons */}
        <div className="w-full flex flex-col items-center gap-1.5 flex-shrink-0 pb-1">
          {sttError && (
            <p className="text-xs text-amber-600 text-center px-1 leading-tight">⚠ {sttError}</p>
          )}
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={toggleSTT}
              disabled={transcribing || !thread}
              className={`${micSize} rounded-full flex items-center justify-center shadow-md transition-all ${
                recording
                  ? 'bg-coral-500 animate-pulse scale-110'
                  : transcribing
                    ? 'bg-amber-400 cursor-not-allowed'
                    : 'bg-primary-500 hover:bg-primary-600 hover:scale-105'
              } disabled:opacity-50`}
            >
              {transcribing ? (
                <svg className="w-5 h-5 text-white animate-spin" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8z" />
                </svg>
              ) : (
                <svg className="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                    d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
                </svg>
              )}
            </button>
            {ttsActive && (
              <button
                type="button"
                onClick={stopTTS}
                title={t('human.stop_speaking')}
                className="w-8 h-8 rounded-full bg-stone-200 hover:bg-coral-100 hover:text-coral-600 flex items-center justify-center shadow transition-all"
              >
                <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
                  <rect x="5" y="5" width="14" height="14" rx="2" />
                </svg>
              </button>
            )}
          </div>
          <p className="text-xs text-stone-500 font-medium leading-tight text-center">
            {recording
              ? t('human.tap_to_stop')
              : transcribing
                ? t('human.transcribing')
                : ttsActive
                  ? t('human.speaking')
                  : t('human.tap_and_speak')}
          </p>
        </div>
      </div>

      {/* ---- Right: Chat panel ---- */}
      <div className="flex-1 flex flex-col min-w-0 min-h-0 bg-white overflow-hidden">

        {/* Messages — scrollable, thin scrollbar */}
        <div className="flex-1 overflow-y-auto overflow-x-hidden px-3 py-3 space-y-3 scrollbar-thin min-h-0">
          {messages.length === 0 ? (
            <div className="flex items-center justify-center h-full">
              <p className="text-xs text-stone-400 text-center px-2">{t('human.tap_to_start')}</p>
            </div>
          ) : (
            messages.map(msg => (
              <div
                key={msg.id}
                className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                <div
                  className={`max-w-[90%] px-3 py-2 rounded-2xl text-xs leading-relaxed ${
                    msg.pending
                      ? 'py-1'
                      : msg.role === 'user'
                        ? 'bg-primary-500 text-white rounded-br-md'
                        : 'bg-stone-100 text-stone-900 rounded-bl-md'
                  }`}
                >
                  {msg.pending ? (
                    <ThinkingWords />
                  ) : msg.role === 'user' ? (
                    <span>{msg.content}</span>
                  ) : (
                    <div className="prose prose-xs max-w-none prose-p:my-0.5">
                      <ReactMarkdown
                        remarkPlugins={[remarkGfm]}
                        components={{ img: () => null }}
                      >
                        {sanitizeAIContent(msg.content)}
                      </ReactMarkdown>
                    </div>
                  )}
                </div>
              </div>
            ))
          )}
          <div ref={messagesEndRef} />
        </div>

        {/* Stats bar — compact */}
        <div className="border-t border-stone-100 px-3 py-1.5 flex items-center gap-3 flex-shrink-0">
          <div className="flex items-center gap-1 text-xs text-stone-400">
            <span className="font-medium">5h</span>
            <div className="w-10 h-0.5 bg-stone-200 rounded-full overflow-hidden">
              <div className="h-full bg-stone-300 rounded-full w-0" />
            </div>
            <span>0%</span>
          </div>
          <div className="flex items-center gap-1 text-xs text-stone-400">
            <span className="font-medium">7d</span>
            <div className="w-10 h-0.5 bg-stone-200 rounded-full overflow-hidden">
              <div className="h-full bg-stone-300 rounded-full w-0" />
            </div>
            <span>0%</span>
          </div>
        </div>
      </div>
    </div>
  );
}
