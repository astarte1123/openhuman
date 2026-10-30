/**
 * Voice and dictation commands.
 */
import { callCoreRpc } from '../../services/coreRpcClient';
import { CommandResponse } from './common';
import { ConfigSnapshot } from './config';

export interface VoiceSpeechResult {
  /** Final text — cleaned by LLM post-processing when available. */
  text: string;
  /** Raw whisper output before LLM cleanup. */
  raw_text: string;
  model_id: string;
}

export interface VoiceTtsResult {
  output_path: string;
  voice_id: string;
}

export interface VoiceStatus {
  stt_available: boolean;
  tts_available: boolean;
  stt_model_id: string;
  tts_voice_id: string;
  whisper_binary: string | null;
  piper_binary: string | null;
  stt_model_path: string | null;
  tts_voice_path: string | null;
  /** Whether the whisper model is loaded in-process (low-latency mode). */
  whisper_in_process: boolean;
  /** Whether LLM post-processing is enabled for transcription cleanup. */
  llm_cleanup_enabled: boolean;
  /** Currently selected STT provider ('cloud' or 'whisper'). */
  stt_provider: string;
  /** Currently selected TTS provider ('cloud' or 'piper'). */
  tts_provider: string;
}

export interface VoiceServerStatus {
  state: 'stopped' | 'idle' | 'recording' | 'transcribing';
  hotkey: string;
  activation_mode: 'tap' | 'push';
  transcription_count: number;
  last_error: string | null;
}

export interface VoiceServerSettings {
  auto_start: boolean;
  hotkey: string;
  activation_mode: 'tap' | 'push';
  skip_cleanup: boolean;
  min_duration_secs: number;
  /** RMS energy threshold for silence detection. Recordings below this are
   *  treated as silence and skipped to prevent whisper hallucinations. */
  silence_threshold: number;
  /** Custom vocabulary words to bias whisper toward (names, technical terms). */
  custom_dictionary: string[];
}

export async function auraVoiceStatus(): Promise<VoiceStatus> {
  return await callCoreRpc<VoiceStatus>({ method: 'aura.voice_status', params: {} });
}

export async function auraVoiceServerStatus(): Promise<VoiceServerStatus> {
  return await callCoreRpc<VoiceServerStatus>({ method: 'aura.voice_server_status', params: {} });
}

export async function auraVoiceServerStart(params?: {
  hotkey?: string;
  activation_mode?: 'tap' | 'push';
  skip_cleanup?: boolean;
}): Promise<VoiceServerStatus> {
  return await callCoreRpc<VoiceServerStatus>({
    method: 'aura.voice_server_start',
    params: params ?? {},
  });
}

export async function auraVoiceServerStop(): Promise<VoiceServerStatus> {
  return await callCoreRpc<VoiceServerStatus>({ method: 'aura.voice_server_stop', params: {} });
}

export async function auraGetVoiceServerSettings(): Promise<CommandResponse<VoiceServerSettings>> {
  return await callCoreRpc<CommandResponse<VoiceServerSettings>>({
    method: 'aura.config_get_voice_server_settings',
    params: {},
  });
}

export async function auraUpdateVoiceServerSettings(update: {
  auto_start?: boolean;
  hotkey?: string;
  activation_mode?: 'tap' | 'push';
  skip_cleanup?: boolean;
  min_duration_secs?: number;
  silence_threshold?: number;
  custom_dictionary?: string[];
}): Promise<CommandResponse<ConfigSnapshot>> {
  return await callCoreRpc<CommandResponse<ConfigSnapshot>>({
    method: 'aura.config_update_voice_server_settings',
    params: update,
  });
}

export interface VoiceProvidersUpdate {
  stt_provider?: 'cloud' | 'whisper';
  tts_provider?: 'cloud' | 'piper';
  stt_model?: string;
  tts_voice?: string;
}

export interface VoiceProvidersSnapshot {
  stt_provider: string;
  tts_provider: string;
  stt_model_id: string;
  tts_voice_id: string;
}

/**
 * Persist the STT / TTS provider selection. Maps to the
 * `aura.voice_set_providers` RPC, which validates each value against
 * the supported provider list and rejects unknown ids server-side.
 */
export async function auraVoiceSetProviders(
  update: VoiceProvidersUpdate
): Promise<VoiceProvidersSnapshot> {
  return await callCoreRpc<VoiceProvidersSnapshot>({
    method: 'aura.voice_set_providers',
    params: update,
  });
}

export async function auraVoiceTranscribe(
  audioPath: string,
  context?: string,
  skipCleanup?: boolean
): Promise<VoiceSpeechResult> {
  return await callCoreRpc<VoiceSpeechResult>({
    method: 'aura.voice_transcribe',
    params: { audio_path: audioPath, context, skip_cleanup: skipCleanup },
  });
}

export async function auraVoiceTranscribeBytes(
  audioBytes: number[],
  extension?: string,
  context?: string,
  skipCleanup?: boolean
): Promise<VoiceSpeechResult> {
  return await callCoreRpc<VoiceSpeechResult>({
    method: 'aura.voice_transcribe_bytes',
    params: { audio_bytes: audioBytes, extension, context, skip_cleanup: skipCleanup },
  });
}

export async function auraVoiceTts(text: string, outputPath?: string): Promise<VoiceTtsResult> {
  return await callCoreRpc<VoiceTtsResult>({
    method: 'aura.voice_tts',
    params: { text, output_path: outputPath },
  });
}

/**
 * Notify the overlay of a voice/STT state change from the chat prompt button.
 * Fire-and-forget — errors are logged but never propagated.
 */
export const notifyOverlaySttState = (
  state: 'recording_started' | 'transcription_done' | 'cancelled' | 'error',
  text?: string
): void => {
  void (async () => {
    try {
      await callCoreRpc({ method: 'aura.overlay_stt_notify', params: { state, text } });
    } catch (err: unknown) {
      console.debug('[overlay_stt_notify] fire-and-forget error:', err);
    }
  })();
};
