/**
 * useChunkedRecorder — MediaRecorder-based chunked audio recording.
 *
 * Records in CHUNK_MS segments (default 3 min). After each segment,
 * transcribes via OpenAI Whisper and appends to the running transcript.
 * This avoids the 25 MB Whisper limit on long meetings and gives
 * progressive feedback during recording.
 */

import { useCallback, useRef, useState } from 'react';
import { transcribeAudio } from '../api';

const CHUNK_MS = 3 * 60 * 1000; // 3 minutes per segment

export function useChunkedRecorder() {
  const [recording, setRecording] = useState(false);
  const [transcribing, setTranscribing] = useState(false);
  const [transcript, setTranscript] = useState('');
  const [elapsed, setElapsed] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const streamRef = useRef<MediaStream | null>(null);
  const currentMRRef = useRef<MediaRecorder | null>(null);
  const chunkTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const transcriptRef = useRef('');
  const runningRef = useRef(false);
  const startTimeRef = useRef(0);
  const stopResolveRef = useRef<((t: string) => void) | null>(null);
  const allBlobsRef = useRef<Blob[]>([]); // accumulate all audio blobs for diarization

  const appendText = useCallback((chunk: string) => {
    if (!chunk.trim()) return;
    transcriptRef.current = (transcriptRef.current + ' ' + chunk).trim();
    setTranscript(transcriptRef.current);
  }, []);

  const transcribeBlob = useCallback(async (blob: Blob): Promise<string> => {
    if (blob.size < 500) return '';
    const buffer = await blob.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    let binary = '';
    const sz = 8192;
    for (let i = 0; i < bytes.length; i += sz)
      binary += String.fromCharCode(...bytes.subarray(i, i + sz));
    try {
      return await transcribeAudio(btoa(binary), 'webm');
    } catch (e) {
      console.warn('[chunked] segment transcription failed', e);
      return '';
    }
  }, []);

  // Recursively records one segment, transcribes it, then starts the next.
  const recordNextSegment = useCallback(() => {
    if (!runningRef.current || !streamRef.current) {
      // All done — clean up
      if (chunkTimerRef.current) { clearTimeout(chunkTimerRef.current); chunkTimerRef.current = null; }
      streamRef.current?.getTracks().forEach(t => t.stop());
      streamRef.current = null;
      currentMRRef.current = null;
      setRecording(false);
      stopResolveRef.current?.(transcriptRef.current);
      stopResolveRef.current = null;
      return;
    }

    const mr = new MediaRecorder(streamRef.current);
    currentMRRef.current = mr;
    const chunks: Blob[] = [];

    mr.ondataavailable = (e: BlobEvent) => { if (e.data.size > 0) chunks.push(e.data); };

    mr.onstop = async () => {
      if (chunkTimerRef.current) { clearTimeout(chunkTimerRef.current); chunkTimerRef.current = null; }
      const blob = new Blob(chunks, { type: 'audio/webm' });
      allBlobsRef.current.push(blob); // keep for full-audio diarization
      setTranscribing(true);
      const text = await transcribeBlob(blob);
      setTranscribing(false);
      if (text) appendText(text);
      recordNextSegment();
    };

    try {
      mr.start();
    } catch (e) {
      setError(String(e));
      runningRef.current = false;
      recordNextSegment();
      return;
    }

    // Auto-advance to next segment after CHUNK_MS
    chunkTimerRef.current = setTimeout(() => {
      if (runningRef.current && mr.state === 'recording') mr.stop();
    }, CHUNK_MS);
  }, [transcribeBlob, appendText]);

  const getFullBlob = useCallback((): Blob | null => {
    if (allBlobsRef.current.length === 0) return null;
    return new Blob(allBlobsRef.current, { type: 'audio/webm' });
  }, []);

  const start = useCallback(async () => {
    setError(null);
    setTranscript('');
    transcriptRef.current = '';
    allBlobsRef.current = [];
    setElapsed(0);

    try {
      streamRef.current = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch (e) {
      const msg = String(e);
      setError(msg.includes('Permission') || msg.includes('NotAllowed') || msg.includes('NotFound')
        ? 'Microfono non accessibile. Verifica i permessi.'
        : msg);
      return;
    }

    runningRef.current = true;
    startTimeRef.current = Date.now();
    setRecording(true);

    timerRef.current = setInterval(() => {
      setElapsed(Math.floor((Date.now() - startTimeRef.current) / 1000));
    }, 1000);

    recordNextSegment();
  }, [recordNextSegment]);

  const stop = useCallback((): Promise<string> => {
    return new Promise(resolve => {
      if (timerRef.current) { clearInterval(timerRef.current); timerRef.current = null; }
      if (!runningRef.current) {
        resolve(transcriptRef.current);
        return;
      }
      runningRef.current = false;
      stopResolveRef.current = resolve;
      // Stop the current segment immediately — onstop will clean up
      if (currentMRRef.current?.state === 'recording') {
        currentMRRef.current.stop();
      }
    });
  }, []);

  // Cleanup on unmount
  const cleanup = useCallback(() => {
    if (timerRef.current) clearInterval(timerRef.current);
    if (chunkTimerRef.current) clearTimeout(chunkTimerRef.current);
    runningRef.current = false;
    currentMRRef.current?.stop();
    streamRef.current?.getTracks().forEach(t => t.stop());
  }, []);

  return { recording, transcribing, transcript, elapsed, error, start, stop, cleanup, getFullBlob };
}
