/**
 * useSTT — smart speech-to-text hook.
 *
 * Strategy:
 *   1. Try native Web Speech API (webkitSpeechRecognition / SpeechRecognition).
 *      Works in WebView2 (Chromium/Edge engine) — free, no rate limits.
 *   2. Fall back to MediaRecorder → base64 → OpenAI Whisper via Tauri command
 *      when the native API is unavailable.
 */

import { useCallback, useRef, useState } from 'react';
import { transcribeAudio } from '../api';

// Web Speech API types (not in default TS lib)
interface ISpeechRecognitionEvent extends Event {
  results: { [i: number]: { [j: number]: { transcript: string } } };
}
interface ISpeechRecognitionErrorEvent extends Event {
  error: string;
}
interface ISpeechRecognition extends EventTarget {
  lang: string;
  interimResults: boolean;
  maxAlternatives: number;
  continuous: boolean;
  onresult: ((e: ISpeechRecognitionEvent) => void) | null;
  onend: (() => void) | null;
  onerror: ((e: ISpeechRecognitionErrorEvent) => void) | null;
  start(): void;
  stop(): void;
}
type SpeechRecognitionCtor = new () => ISpeechRecognition;

function getNativeSR(): SpeechRecognitionCtor | null {
  if (typeof window === 'undefined') return null;
  const w = window as unknown as Record<string, unknown>;
  return (w['SpeechRecognition'] ?? w['webkitSpeechRecognition'] ?? null) as SpeechRecognitionCtor | null;
}

export function useSTT(onResult: (text: string) => void) {
  const [recording, setRecording] = useState(false);
  const [transcribing, setTranscribing] = useState(false);
  const [sttError, setSttError] = useState<string | null>(null);

  // Native SR refs
  const srRef = useRef<ISpeechRecognition | null>(null);
  // MediaRecorder refs (Whisper fallback)
  const mrRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);

  const useNative = !!getNativeSR();

  const toggle = useCallback(async () => {
    setSttError(null);

    // ---- Stop if already recording ----
    if (recording) {
      if (useNative) {
        srRef.current?.stop();
      } else {
        mrRef.current?.stop();
      }
      return;
    }

    // ---- Native Web Speech API ----
    if (useNative) {
      const SR = getNativeSR()!;
      const rec = new SR();
      rec.lang = navigator.language || 'en-US';
      rec.interimResults = false;
      rec.maxAlternatives = 1;
      rec.continuous = false;

      rec.onresult = (e: ISpeechRecognitionEvent) => {
        const text = e.results[0]?.[0]?.transcript ?? '';
        if (text) onResult(text);
      };
      rec.onend = () => setRecording(false);
      rec.onerror = (e: ISpeechRecognitionErrorEvent) => {
        setRecording(false);
        if (e.error !== 'no-speech') setSttError(`Speech error: ${e.error}`);
      };

      try {
        rec.start();
        srRef.current = rec;
        setRecording(true);
      } catch (err) {
        setSttError(String(err));
      }
      return;
    }

    // ---- Whisper fallback via MediaRecorder ----
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const mr = new MediaRecorder(stream);
      chunksRef.current = [];

      mr.ondataavailable = (e: BlobEvent) => {
        if (e.data.size > 0) chunksRef.current.push(e.data);
      };

      mr.onstop = async () => {
        stream.getTracks().forEach(t => t.stop());
        setRecording(false);

        const blob = new Blob(chunksRef.current, { type: 'audio/webm' });
        const buffer = await blob.arrayBuffer();
        const bytes = new Uint8Array(buffer);

        let binary = '';
        const chunkSize = 8192;
        for (let i = 0; i < bytes.length; i += chunkSize) {
          binary += String.fromCharCode(...bytes.subarray(i, i + chunkSize));
        }

        setTranscribing(true);
        try {
          const transcript = await transcribeAudio(btoa(binary), 'webm');
          if (transcript) onResult(transcript);
        } catch (err) {
          setSttError(String(err));
        } finally {
          setTranscribing(false);
        }
      };

      mr.onerror = () => {
        setRecording(false);
        setSttError('Recording error');
      };

      mr.start();
      mrRef.current = mr;
      setRecording(true);
    } catch (err) {
      const msg = String(err);
      if (msg.includes('Permission') || msg.includes('NotAllowed') || msg.includes('NotFound')) {
        setSttError('Microphone access denied. Check browser/system permissions.');
      } else {
        setSttError(msg);
      }
    }
  }, [recording, onResult, useNative]);

  return { recording, transcribing, sttError, toggle, useNative };
}
