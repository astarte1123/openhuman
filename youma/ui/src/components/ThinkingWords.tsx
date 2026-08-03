import { useEffect, useState } from 'react';
import { envGet } from '../api';

// Default phrases — used when no custom phrases are set (YOUMA_THINKING_PHRASES is empty).
export const DEFAULT_THINKING_PHRASES = [
  'interfacing', 'type casting', 'Zod schemas', 'genering', 'type safeting',
  'State management', 'edge case', 'trade-off', 'dependency injection', 'parsing',
  'runtiming', 'compilation', 'destructuring', 'calibrating', 'synthesizing',
  'untangling', 'refactoring', 'abstracting',
];

// Milanese dialect phrases — shown as placeholder in the Custom tab of Settings.
export const MILANESE_THINKING_PHRASES = [
  'Pirling', 'Bauscing', 'Cadreghing', 'Sciuring', 'Sciuraing',
  'Tusing', 'Bagajing', 'Ghe pensi mi', 'Ciumbia', 'Michetting',
  'Mondeghiling', 'Scighering', 'Balabiotting', 'Malnatting', 'Ligering',
  'Traning', 'Barlafüsing', 'Ciappàting', 'Minga', 'Vuning',
  'Dòing', 'Treing', 'Quattering', 'Cinqing', 'Andemming',
  'Vegnì chì fiiiga', "T'el chi", 'Se vedum', 'Bondìng', 'Bonasering',
  'Bonanòtting', 'Te salüding', 'Scüsing', 'Mucheling', 'Buteging',
  'Ròbaing', 'Daneeing', 'Lavoràing', 'Mangiàing', 'Beving',
  'Càing', 'Strading', 'Fioeuing', 'Madering', 'Padering',
  'Pecoling', 'Tòming', 'Ruding', 'Magaràing',
  "Milan l'è on gran Milaning",
  'Va a cà de Misent che te la scurtet',
];

export function ThinkingWords() {
  const [customPhrases, setCustomPhrases] = useState<string[]>([]);

  useEffect(() => {
    envGet('YOUMA_THINKING_PHRASES').then(raw => {
      if (!raw) return;
      const parsed = raw.split(',').map(s => s.trim()).filter(Boolean);
      if (parsed.length > 0) setCustomPhrases(parsed);
    }).catch(() => {});
  }, []);

  const phrases = customPhrases.length > 0 ? customPhrases : DEFAULT_THINKING_PHRASES;

  const [idx, setIdx] = useState(() => Math.floor(Math.random() * DEFAULT_THINKING_PHRASES.length));
  const [charCount, setCharCount] = useState(0);
  const [phase, setPhase] = useState<'typing' | 'hold'>('typing');
  const [cursorOn, setCursorOn] = useState(true);

  const phrase = phrases[idx % phrases.length];

  useEffect(() => {
    const id = setInterval(() => setCursorOn(v => !v), 500);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    if (phase === 'typing') {
      if (charCount < phrase.length) {
        const t = setTimeout(() => setCharCount(c => c + 1), 72);
        return () => clearTimeout(t);
      }
      const t = setTimeout(() => setPhase('hold'), 1800);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => {
      setIdx(prev => {
        const pool = phrases.length;
        if (pool === 1) return prev;
        let next: number;
        do { next = Math.floor(Math.random() * pool); } while (next === prev);
        return next;
      });
      setCharCount(0);
      setPhase('typing');
    }, 350);
    return () => clearTimeout(t);
  }, [phase, charCount, phrase]);

  return (
    <span className="inline-flex items-center gap-1.5 text-stone-400 text-xs select-none">
      <svg className="w-3 h-3 flex-shrink-0 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
        <circle cx="12" cy="12" r="10" strokeOpacity={0.2} />
        <path strokeLinecap="round" d="M12 2a10 10 0 0 1 10 10" />
      </svg>
      <span className="italic">
        {phrase.slice(0, charCount)}
        <span className={cursorOn ? 'opacity-100' : 'opacity-0'}>|</span>
      </span>
    </span>
  );
}
