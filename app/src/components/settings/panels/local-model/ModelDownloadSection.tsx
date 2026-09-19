import { useT } from '../../../../lib/i18n/I18nContext';
import { statusLabel } from '../../../../utils/localAiHelpers';
import type {
  LocalAiAssetsStatus,
  LocalAiEmbeddingResult,
  LocalAiSpeechResult,
  LocalAiTtsResult,
} from '../../../../utils/tauriCommands';

interface ModelDownloadSectionProps {
  assets: LocalAiAssetsStatus | null;
  assetDownloadBusy: Record<string, boolean>;
  statusTone: (state: string) => string;
  runtimeEnabled: boolean;
  onTriggerAssetDownload: (capability: 'chat' | 'vision' | 'embedding' | 'stt' | 'tts') => void;

  summaryInput: string;
  summaryOutput: string;
  isSummaryLoading: boolean;
  onSetSummaryInput: (value: string) => void;
  onRunSummaryTest: () => void;

  promptInput: string;
  promptOutput: string;
  promptError: string;
  isPromptLoading: boolean;
  promptNoThink: boolean;
  onSetPromptInput: (value: string) => void;
  onSetPromptNoThink: (value: boolean) => void;
  onRunPromptTest: () => void;

  visionPromptInput: string;
  visionImageInput: string;
  visionOutput: string;
  isVisionLoading: boolean;
  onSetVisionPromptInput: (value: string) => void;
  onSetVisionImageInput: (value: string) => void;
  onRunVisionTest: () => void;

  embeddingInput: string;
  embeddingOutput: LocalAiEmbeddingResult | null;
  isEmbeddingLoading: boolean;
  onSetEmbeddingInput: (value: string) => void;
  onRunEmbeddingTest: () => void;

  audioPathInput: string;
  transcribeOutput: LocalAiSpeechResult | null;
  isTranscribeLoading: boolean;
  onSetAudioPathInput: (value: string) => void;
  onRunTranscribeTest: () => void;

  ttsInput: string;
  ttsOutputPath: string;
  ttsOutput: LocalAiTtsResult | null;
  isTtsLoading: boolean;
  onSetTtsInput: (value: string) => void;
  onSetTtsOutputPath: (value: string) => void;
  onRunTtsTest: () => void;
}

const ModelDownloadSection = ({
  assets,
  assetDownloadBusy,
  statusTone,
  runtimeEnabled,
  onTriggerAssetDownload,
  summaryInput,
  summaryOutput,
  isSummaryLoading,
  onSetSummaryInput,
  onRunSummaryTest,
  promptInput,
  promptOutput,
  promptError,
  isPromptLoading,
  promptNoThink,
  onSetPromptInput,
  onSetPromptNoThink,
  onRunPromptTest,
  visionPromptInput,
  visionImageInput,
  visionOutput,
  isVisionLoading,
  onSetVisionPromptInput,
  onSetVisionImageInput,
  onRunVisionTest,
  embeddingInput,
  embeddingOutput,
  isEmbeddingLoading,
  onSetEmbeddingInput,
  onRunEmbeddingTest,
  audioPathInput,
  transcribeOutput,
  isTranscribeLoading,
  onSetAudioPathInput,
  onRunTranscribeTest,
  ttsInput,
  ttsOutputPath,
  ttsOutput,
  isTtsLoading,
  onSetTtsInput,
  onSetTtsOutputPath,
  onRunTtsTest,
}: ModelDownloadSectionProps) => {
  const { t } = useT();
  return (
    <>
      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.model_download.capability_assets_title')}
        </h3>
        <div className="bg-stone-50 rounded-lg border border-stone-200 p-4 space-y-3">
          <div className="text-xs text-stone-500">
            {t('settings.model_download.quantization_preference', {
              quantization: assets?.quantization ?? 'q4',
            })}
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
            {[
              { label: 'Chat', key: 'chat' as const, item: assets?.chat },
              { label: 'Vision', key: 'vision' as const, item: assets?.vision },
              { label: 'Embedding', key: 'embedding' as const, item: assets?.embedding },
              { label: 'STT', key: 'stt' as const, item: assets?.stt },
              { label: 'TTS', key: 'tts' as const, item: assets?.tts },
            ].map(({ label, key, item }) => (
              <div key={String(label)} className="rounded-md border border-stone-200 p-2">
                <div className="text-stone-500 text-xs uppercase tracking-wide">{label}</div>
                <div className="text-stone-800 mt-1 break-all">{item?.id ?? 'n/a'}</div>
                <div className={`text-xs mt-1 ${statusTone(item?.state ?? 'idle')}`}>
                  {statusLabel(item?.state ?? 'idle')}
                </div>
                {item?.path && (
                  <div className="text-[10px] text-stone-500 mt-1 break-all">{item.path}</div>
                )}
                {item?.provider === 'ollama' || item?.provider === 'lm_studio' ? (
                  <div className="mt-2 text-[10px] text-stone-500">
                    {t('settings.model_download.manage_in_external_runtime')}
                  </div>
                ) : (
                  <button
                    onClick={() => onTriggerAssetDownload(key)}
                    disabled={!runtimeEnabled || assetDownloadBusy[key]}
                    className="mt-2 px-2 py-1 text-[10px] rounded border border-stone-200 hover:border-stone-300 disabled:opacity-60 text-stone-600">
                    {assetDownloadBusy[key]
                      ? t('settings.model_download.downloading')
                      : t('settings.model_download.download')}
                  </button>
                )}
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.model_download.test_summarization_title')}
        </h3>
        <div className="bg-stone-50 rounded-lg border border-stone-200 p-4 space-y-3">
          <textarea
            value={summaryInput}
            onChange={e => onSetSummaryInput(e.target.value)}
            placeholder={t('settings.model_download.summary_input_placeholder')}
            className="w-full min-h-28 rounded-md bg-white border border-stone-200 px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:outline-none focus:ring-1 focus:ring-primary-500"
          />
          <div className="flex items-center justify-between">
            <div className="text-xs text-stone-500">
              {t('settings.model_download.calls_inference_summarize')}
            </div>
            <button
              onClick={onRunSummaryTest}
              disabled={!runtimeEnabled || isSummaryLoading || !summaryInput.trim()}
              className="px-3 py-1.5 text-xs rounded-md bg-emerald-600 hover:bg-emerald-700 disabled:opacity-60 text-white">
              {isSummaryLoading
                ? t('settings.model_download.running')
                : t('settings.model_download.run_summary_test')}
            </button>
          </div>
          {summaryOutput && (
            <pre className="whitespace-pre-wrap rounded-md bg-stone-50 border border-stone-200 p-3 text-xs text-stone-700">
              {summaryOutput}
            </pre>
          )}
        </div>
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.model_download.test_custom_prompt_title')}
        </h3>
        <div className="bg-stone-50 rounded-lg border border-stone-200 p-4 space-y-3">
          <textarea
            value={promptInput}
            onChange={e => onSetPromptInput(e.target.value)}
            placeholder={t('settings.model_download.prompt_input_placeholder')}
            className="w-full min-h-28 rounded-md bg-white border border-stone-200 px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:outline-none focus:ring-1 focus:ring-primary-500"
          />
          <div className="flex flex-wrap items-center justify-between gap-2">
            <label className="flex items-center gap-2 text-xs text-stone-700">
              <input
                type="checkbox"
                checked={promptNoThink}
                onChange={e => onSetPromptNoThink(e.target.checked)}
                className="h-3.5 w-3.5 rounded border-stone-300 bg-white text-primary-500 focus:ring-primary-500"
              />
              {t('settings.model_download.no_think_mode')}
            </label>
            <button
              onClick={onRunPromptTest}
              disabled={!runtimeEnabled || isPromptLoading || !promptInput.trim()}
              className="px-3 py-1.5 text-xs rounded-md bg-primary-600 hover:bg-primary-700 disabled:opacity-60 text-white">
              {isPromptLoading
                ? t('settings.model_download.running')
                : t('settings.model_download.run_prompt_test')}
            </button>
          </div>
          {isPromptLoading && (
            <div className="flex items-center gap-2 text-xs text-primary-600">
              <div className="h-3 w-3 rounded-full border-2 border-blue-400 border-t-transparent animate-spin" />
              {t('settings.model_download.running_prompt')}
            </div>
          )}
          {promptError && (
            <div className="rounded-md bg-red-50 border border-red-300 p-3 text-xs text-red-600">
              {promptError}
            </div>
          )}
          {promptOutput && (
            <pre className="whitespace-pre-wrap rounded-md bg-stone-50 border border-stone-200 p-3 text-xs text-stone-700 max-h-64 overflow-auto">
              {promptOutput}
            </pre>
          )}
        </div>
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.model_download.test_vision_prompt_title')}
        </h3>
        <div className="bg-stone-50 rounded-lg border border-stone-200 p-4 space-y-3">
          <textarea
            value={visionPromptInput}
            onChange={e => onSetVisionPromptInput(e.target.value)}
            placeholder={t('settings.model_download.vision_prompt_placeholder')}
            className="w-full min-h-20 rounded-md bg-white border border-stone-200 px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:outline-none focus:ring-1 focus:ring-primary-400"
          />
          <textarea
            value={visionImageInput}
            onChange={e => onSetVisionImageInput(e.target.value)}
            placeholder={t('settings.model_download.vision_image_placeholder')}
            className="w-full min-h-20 rounded-md bg-white border border-stone-200 px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:outline-none focus:ring-1 focus:ring-primary-400"
          />
          <button
            onClick={onRunVisionTest}
            disabled={
              !runtimeEnabled ||
              isVisionLoading ||
              !visionPromptInput.trim() ||
              !visionImageInput.trim()
            }
            className="px-3 py-1.5 text-xs rounded-md bg-indigo-600 hover:bg-indigo-700 disabled:opacity-60 text-white">
            {isVisionLoading
              ? t('settings.model_download.running')
              : t('settings.model_download.run_vision_test')}
          </button>
          {visionOutput && (
            <pre className="whitespace-pre-wrap rounded-md bg-stone-50 border border-stone-200 p-3 text-xs text-stone-700">
              {visionOutput}
            </pre>
          )}
        </div>
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.model_download.test_embeddings_title')}
        </h3>
        <div className="bg-stone-50 rounded-lg border border-stone-200 p-4 space-y-3">
          <textarea
            value={embeddingInput}
            onChange={e => onSetEmbeddingInput(e.target.value)}
            placeholder={t('settings.model_download.embedding_input_placeholder')}
            className="w-full min-h-20 rounded-md bg-white border border-stone-200 px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:outline-none focus:ring-1 focus:ring-primary-400"
          />
          <button
            onClick={onRunEmbeddingTest}
            disabled={!runtimeEnabled || isEmbeddingLoading || !embeddingInput.trim()}
            className="px-3 py-1.5 text-xs rounded-md bg-teal-600 hover:bg-teal-700 disabled:opacity-60 text-white">
            {isEmbeddingLoading
              ? t('settings.model_download.running')
              : t('settings.model_download.run_embedding_test')}
          </button>
          {embeddingOutput && (
            <div className="rounded-md bg-stone-50 border border-stone-200 p-3 text-xs text-stone-700 space-y-1">
              <div>
                {t('settings.model_download.embedding_model_line', {
                  modelId: embeddingOutput.model_id,
                })}
              </div>
              <div>
                {t('settings.model_download.embedding_dimensions_line', {
                  dimensions: embeddingOutput.dimensions,
                })}
              </div>
              <div>
                {t('settings.model_download.embedding_vectors_line', {
                  count: embeddingOutput.vectors.length,
                })}
              </div>
            </div>
          )}
        </div>
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.model_download.test_stt_title')}
        </h3>
        <div className="bg-stone-50 rounded-lg border border-stone-200 p-4 space-y-3">
          <input
            value={audioPathInput}
            onChange={e => onSetAudioPathInput(e.target.value)}
            placeholder={t('settings.model_download.audio_path_placeholder')}
            className="w-full rounded-md bg-white border border-stone-200 px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:outline-none focus:ring-1 focus:ring-primary-400"
          />
          <button
            onClick={onRunTranscribeTest}
            disabled={!runtimeEnabled || isTranscribeLoading || !audioPathInput.trim()}
            className="px-3 py-1.5 text-xs rounded-md bg-purple-600 hover:bg-purple-700 disabled:opacity-60 text-white">
            {isTranscribeLoading
              ? t('settings.model_download.running')
              : t('settings.model_download.run_transcription_test')}
          </button>
          {transcribeOutput && (
            <div className="rounded-md bg-stone-50 border border-stone-200 p-3 text-xs text-stone-700 space-y-2">
              <div>
                {t('settings.model_download.embedding_model_line', {
                  modelId: transcribeOutput.model_id,
                })}
              </div>
              <div>
                <span className="text-stone-400">
                  {t('settings.model_download.transcript_label')}
                </span>
                <pre className="whitespace-pre-wrap mt-1">{transcribeOutput.text}</pre>
              </div>
            </div>
          )}
        </div>
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-stone-900">
          {t('settings.model_download.test_tts_title')}
        </h3>
        <div className="bg-stone-50 rounded-lg border border-stone-200 p-4 space-y-3">
          <textarea
            value={ttsInput}
            onChange={e => onSetTtsInput(e.target.value)}
            placeholder={t('settings.model_download.tts_input_placeholder')}
            className="w-full min-h-20 rounded-md bg-white border border-stone-200 px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:outline-none focus:ring-1 focus:ring-primary-400"
          />
          <input
            value={ttsOutputPath}
            onChange={e => onSetTtsOutputPath(e.target.value)}
            placeholder={t('settings.model_download.tts_output_path_placeholder')}
            className="w-full rounded-md bg-white border border-stone-200 px-3 py-2 text-sm text-stone-900 placeholder:text-stone-400 focus:outline-none focus:ring-1 focus:ring-primary-400"
          />
          <button
            onClick={onRunTtsTest}
            disabled={!runtimeEnabled || isTtsLoading || !ttsInput.trim()}
            className="px-3 py-1.5 text-xs rounded-md bg-rose-600 hover:bg-rose-700 disabled:opacity-60 text-white">
            {isTtsLoading
              ? t('settings.model_download.running')
              : t('settings.model_download.run_tts_test')}
          </button>
          {ttsOutput && (
            <div className="rounded-md bg-stone-50 border border-stone-200 p-3 text-xs text-stone-700 space-y-1">
              <div>
                {t('settings.model_download.tts_voice_line', { voiceId: ttsOutput.voice_id })}
              </div>
              <div className="break-all">
                {t('settings.model_download.tts_output_line', {
                  outputPath: ttsOutput.output_path,
                })}
              </div>
            </div>
          )}
        </div>
      </section>
    </>
  );
};

export default ModelDownloadSection;
