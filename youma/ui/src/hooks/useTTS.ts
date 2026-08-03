/**
 * useTTS — text-to-speech with ElevenLabs, lip-sync amplitude, and fallbacks.
 *
 * Priority chain:
 *   1. ElevenLabs TTS   — highest quality, requires ELEVENLABS_API_KEY.
 *   2. OpenAI TTS       — good quality, requires OPENAI_API_KEY.
 *   3. Native WinRT TTS — free, no key, best system voice (no lip-sync).
 *   4. Browser speechSynthesis — last resort (no lip-sync).
 *
 * Cloud providers (1 & 2) decode MP3 via Web Audio API and drive `mouthAmplitude`
 * (0–1 float, ~60 fps) so the mascot can animate in sync.
 */

import { useCallback, useRef, useState } from 'react';
import { speakElevenLabs, speakNative, speakText } from '../api';

export function useTTS() {
  const [speaking, setSpeaking] = useState(false);
  const [mouthAmplitude, setMouthAmplitude] = useState(0);

  const audioCtxRef  = useRef<AudioContext | null>(null);
  const sourceRef    = useRef<AudioBufferSourceNode | null>(null);
  const analyserRef  = useRef<AnalyserNode | null>(null);
  const rafRef       = useRef<number>(0);
  const activeRef    = useRef(false);

  const stop = useCallback(() => {
    activeRef.current = false;
    cancelAnimationFrame(rafRef.current);
    if (sourceRef.current) {
      try { sourceRef.current.stop(); } catch { /* already stopped */ }
      sourceRef.current = null;
    }
    if (audioCtxRef.current) {
      audioCtxRef.current.close().catch(() => {});
      audioCtxRef.current = null;
    }
    analyserRef.current = null;
    window.speechSynthesis?.cancel();
    setMouthAmplitude(0);
    setSpeaking(false);
  }, []);

  /** Decode a base64 MP3, play it via Web Audio, and drive mouthAmplitude. */
  const playBase64 = useCallback(async (b64: string): Promise<void> => {
    stop();
    activeRef.current = true;

    let audioCtx: AudioContext;
    try {
      audioCtx = new (window.AudioContext ?? (window as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext!)();
    } catch {
      // Web Audio not available — fall back to HTMLAudio without lip-sync
      const audio = new Audio(`data:audio/mp3;base64,${b64}`);
      setSpeaking(true);
      await new Promise<void>(resolve => {
        audio.onended = () => resolve();
        audio.onerror = () => resolve();
        audio.play().catch(() => resolve());
      });
      setSpeaking(false);
      return;
    }

    audioCtxRef.current = audioCtx;

    const bytes = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
    let buffer: AudioBuffer;
    try {
      buffer = await audioCtx.decodeAudioData(bytes.buffer.slice(0));
    } catch {
      audioCtx.close().catch(() => {});
      audioCtxRef.current = null;
      setSpeaking(false);
      return;
    }

    if (!activeRef.current) { audioCtx.close().catch(() => {}); return; }

    const analyser = audioCtx.createAnalyser();
    analyser.fftSize = 256;
    analyserRef.current = analyser;

    const source = audioCtx.createBufferSource();
    source.buffer = buffer;
    sourceRef.current = source;
    source.connect(analyser);
    analyser.connect(audioCtx.destination);

    setSpeaking(true);
    source.start(0);

    // Poll frequency data at ~60 fps to drive lip-sync
    const data = new Uint8Array(analyser.frequencyBinCount);
    const poll = () => {
      if (!analyserRef.current || !activeRef.current) {
        setMouthAmplitude(0);
        return;
      }
      analyserRef.current.getByteFrequencyData(data);
      // Weight low-mid frequencies (speech fundamentals 80–800 Hz)
      const avg = data.slice(0, 20).reduce((a, b) => a + b, 0) / 20 / 255;
      setMouthAmplitude(avg);
      rafRef.current = requestAnimationFrame(poll);
    };
    rafRef.current = requestAnimationFrame(poll);

    await new Promise<void>(resolve => { source.onended = () => resolve(); });
    stop();
  }, [stop]);

  const speak = useCallback(async (text: string) => {
    if (!text.trim()) return;
    stop();

    const plain = text.replace(/[#*`_~[\]()>]/g, '').trim();

    // 1. ElevenLabs TTS
    try {
      const b64 = await speakElevenLabs(plain);
      if (b64) { await playBase64(b64); return; }
    } catch (e) {
      const msg = String(e);
      if (!msg.includes('no_elevenlabs_key')) {
        console.debug('[useTTS] ElevenLabs error:', msg);
      }
    }

    // 2. OpenAI TTS
    try {
      const b64 = await speakText(plain);
      if (b64) { await playBase64(b64); return; }
    } catch (e) {
      const msg = String(e);
      if (!msg.includes('no_openai_key')) {
        console.debug('[useTTS] OpenAI TTS error:', msg);
      }
    }

    // 3. Native WinRT TTS (no lip-sync — native is blocking Rust call)
    try {
      setSpeaking(true);
      await speakNative(plain);
      setSpeaking(false);
      return;
    } catch {
      setSpeaking(false);
    }

    // 4. Browser speechSynthesis
    browserFallback(plain);
  }, [stop, playBase64]);

  function browserFallback(text: string) {
    if (!('speechSynthesis' in window)) return;
    window.speechSynthesis.cancel();
    const utt = new SpeechSynthesisUtterance(text);
    const voices = window.speechSynthesis.getVoices();
    const isFemale = (v: SpeechSynthesisVoice) =>
      /aria|jenny|zira|elsa|hazel|susan|female/i.test(v.name) &&
      !/guy|david|mark|male/i.test(v.name);
    const preferred =
      voices.find(v => isFemale(v) && /natural|neural|enhanced/i.test(v.name)) ??
      voices.find(v => isFemale(v) && v.lang.startsWith(navigator.language.slice(0, 2))) ??
      voices.find(v => isFemale(v)) ??
      voices.find(v => v.lang.startsWith(navigator.language.slice(0, 2))) ??
      null;
    if (preferred) utt.voice = preferred;
    utt.rate = 0.95;
    utt.pitch = 1.1;
    setSpeaking(true);
    utt.onend = () => setSpeaking(false);
    utt.onerror = () => setSpeaking(false);
    window.speechSynthesis.speak(utt);
  }

  return { speaking, speak, stop, mouthAmplitude };
}
