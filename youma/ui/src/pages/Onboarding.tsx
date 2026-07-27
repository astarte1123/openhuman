import { useState } from 'react';
import { setEnvVar, settingsSet, toggleSkill } from '../api';

type Step = 'welcome' | 'runtime' | 'provider' | 'apikey' | 'inference' | 'voice' | 'tools' | 'done';
type Provider = 'anthropic' | 'openai' | 'ollama';
type Runtime = 'cloud' | 'local';
type TtsProvider = 'native' | 'openai' | 'browser';

const PROVIDERS: { id: Provider; name: string; desc: string; keyName: string | null; placeholder: string }[] = [
  { id: 'anthropic', name: 'Anthropic Claude', desc: 'Best reasoning and conversation quality.', keyName: 'ANTHROPIC_API_KEY', placeholder: 'sk-ant-…' },
  { id: 'openai',    name: 'OpenAI GPT',       desc: 'GPT-4o with Whisper voice transcription.', keyName: 'OPENAI_API_KEY',    placeholder: 'sk-…'      },
  { id: 'ollama',    name: 'Ollama (Local)',    desc: 'Run AI models locally. No API key needed.', keyName: null,              placeholder: ''           },
];

const ANTHROPIC_MODELS = ['claude-opus-4-7', 'claude-sonnet-4-6', 'claude-haiku-4-5-20251001', 'claude-3-5-sonnet-20241022'];
const OPENAI_MODELS    = ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-3.5-turbo'];
const OLLAMA_MODELS    = ['llama3', 'mistral', 'gemma2', 'phi3', 'qwen2'];

const CLOUD_STEPS: Step[]  = ['welcome', 'runtime', 'provider', 'apikey', 'done'];
const LOCAL_STEPS: Step[]  = ['welcome', 'runtime', 'provider', 'inference', 'voice', 'tools', 'done'];

const DEFAULT_TOOLS = [
  { id: 'web-search',   label: 'Web Search',    desc: 'Search the web for up-to-date info',  enabled: true  },
  { id: 'memory-notes', label: 'Memory Notes',  desc: 'Remember facts across conversations', enabled: true  },
  { id: 'task-manager', label: 'Task Manager',  desc: 'Manage your todo list via chat',      enabled: true  },
  { id: 'voice-input',  label: 'Voice Input',   desc: 'Speak to Youma with your microphone',  enabled: false },
  { id: 'voice-output', label: 'Voice Output',  desc: 'Youma speaks replies aloud',           enabled: false },
  { id: 'markdown',     label: 'Markdown',      desc: 'Render formatted responses',          enabled: true  },
];

interface Props { onComplete: () => void; }

// ---------------------------------------------------------------------------
// Walkthrough overlay (post-onboarding joyride)
// ---------------------------------------------------------------------------

const WALKTHROUGH_STEPS = [
  { title: 'Home',         desc: 'See your system status and quick links at a glance.',          icon: '🏠' },
  { title: 'Youma',         desc: 'Talk to your AI assistant with text or voice.',                icon: '✨' },
  { title: 'Chat',         desc: 'Full conversation history with markdown rendering.',           icon: '💬' },
  { title: 'Skills',       desc: 'Enable or disable Youma capabilities.',                         icon: '⚡' },
  { title: 'Mind',         desc: 'Memory notes and task manager for persistent context.',        icon: '🧠' },
  // Rewards hidden — pending design decision
  { title: 'Settings',     desc: 'Manage your API keys, voice config, and preferences.',        icon: '⚙️' },
];

function WalkthroughOverlay({ onFinish }: { onFinish: () => void }) {
  const [idx, setIdx] = useState(0);
  const step = WALKTHROUGH_STEPS[idx];
  const isLast = idx === WALKTHROUGH_STEPS.length - 1;

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center pb-24 bg-black/30 backdrop-blur-sm">
      <div className="w-full max-w-sm mx-4 bg-white rounded-2xl border border-stone-200 shadow-xl p-6 animate-slide-up">
        <div className="flex items-center gap-3 mb-3">
          <span className="text-3xl">{step.icon}</span>
          <div>
            <div className="font-semibold text-stone-900">{step.title}</div>
            <div className="text-xs text-stone-500">{step.desc}</div>
          </div>
        </div>
        <div className="flex items-center gap-2 mb-4">
          {WALKTHROUGH_STEPS.map((_, i) => (
            <div key={i} className={`flex-1 h-1 rounded-full transition-colors ${i <= idx ? 'bg-primary-500' : 'bg-stone-200'}`} />
          ))}
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={onFinish}
            className="flex-1 py-2 text-sm text-stone-500 hover:text-stone-700 transition-colors"
          >
            Skip tour
          </button>
          <button
            type="button"
            onClick={() => isLast ? onFinish() : setIdx(i => i + 1)}
            className="flex-[2] py-2 bg-primary-500 hover:bg-primary-600 text-white text-sm font-semibold rounded-xl transition-colors"
          >
            {isLast ? 'Get started' : 'Next →'}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main onboarding
// ---------------------------------------------------------------------------

export default function OnboardingPage({ onComplete }: Props) {
  const [step, setStep]           = useState<Step>('welcome');
  const [runtime, setRuntime]     = useState<Runtime>('cloud');
  const [provider, setProvider]   = useState<Provider>('anthropic');
  const [apiKey, setApiKey]       = useState('');
  const [model, setModel]         = useState('');
  const [ttsProvider, setTts]     = useState<TtsProvider>('native');
  const [sttLang, setSttLang]     = useState('en-US');
  const [tools, setTools]         = useState(DEFAULT_TOOLS);
  const [saving, setSaving]       = useState(false);
  const [error, setError]         = useState<string | null>(null);
  const [showWalkthrough, setShowWalkthrough] = useState(false);

  const sel   = PROVIDERS.find(p => p.id === provider)!;
  const steps = runtime === 'cloud' ? CLOUD_STEPS : LOCAL_STEPS;
  const stepIdx = steps.indexOf(step);

  const modelList = provider === 'anthropic' ? ANTHROPIC_MODELS
    : provider === 'openai' ? OPENAI_MODELS
    : OLLAMA_MODELS;

  function goNext() {
    const next = steps[stepIdx + 1];
    if (next) setStep(next);
  }

  function goPrev() {
    const prev = steps[stepIdx - 1];
    if (prev) setStep(prev);
  }

  async function handleSaveKey() {
    if (sel.keyName && !apiKey.trim()) { setError('Enter your API key.'); return; }
    setSaving(true);
    setError(null);
    try {
      if (sel.keyName) await setEnvVar(sel.keyName, apiKey.trim());
      await setEnvVar('AI_PROVIDER', provider);
      if (runtime === 'cloud') {
        await settingsSet('onboarding_complete', '1');
        setStep('done');
        setTimeout(() => setShowWalkthrough(true), 800);
      } else {
        goNext();
      }
    } catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  }

  async function handleSaveInference() {
    setSaving(true);
    try {
      if (model) await setEnvVar('AI_MODEL', model);
      goNext();
    } catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  }

  async function handleSaveVoice() {
    setSaving(true);
    try {
      await setEnvVar('TTS_PROVIDER', ttsProvider);
      await setEnvVar('STT_LANG', sttLang);
      goNext();
    } catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  }

  async function handleSaveTools() {
    setSaving(true);
    try {
      for (const tool of tools) {
        await toggleSkill(tool.id, tool.enabled).catch(() => {});
      }
      await settingsSet('onboarding_complete', '1');
      setStep('done');
      setTimeout(() => setShowWalkthrough(true), 800);
    } catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  }

  function toggleTool(id: string) {
    setTools(prev => prev.map(t => t.id === id ? { ...t, enabled: !t.enabled } : t));
  }

  return (
    <>
      <div className="fixed inset-0 bg-stone-50 flex items-center justify-center z-40 p-4 app-dotted-canvas">
        <div className="w-full max-w-md">
          {/* Progress dots */}
          <div className="flex justify-center gap-2 mb-8">
            {steps.map((s, i) => (
              <div
                key={s}
                className={`w-2 h-2 rounded-full transition-all duration-300 ${
                  s === step ? 'bg-primary-500 w-5' : i < stepIdx ? 'bg-primary-300' : 'bg-stone-300'
                }`}
              />
            ))}
          </div>

          {/* ── Welcome ── */}
          {step === 'welcome' && (
            <div className="bg-white rounded-2xl border border-stone-200 p-8 shadow-lg text-center">
              <div className="flex items-center justify-center mx-auto mb-4">
                <img src="/mascot.png" alt="Youma" width={120} height={120} className="w-30 h-30 object-contain drop-shadow-lg mascot-float" draggable={false} />
              </div>
              <h1 className="text-2xl font-bold text-stone-900 mb-2">Welcome to Youma</h1>
              <p className="text-stone-500 text-sm mb-6">Your personal AI assistant. Let's get you set up in a few quick steps.</p>
              <button type="button" onClick={goNext} className="w-full py-3 bg-primary-500 hover:bg-primary-600 text-white font-semibold rounded-xl transition-colors">
                Get Started
              </button>
            </div>
          )}

          {/* ── Runtime choice ── */}
          {step === 'runtime' && (
            <div className="bg-white rounded-2xl border border-stone-200 p-8 shadow-lg">
              <h2 className="text-xl font-bold text-stone-900 mb-1">Choose your setup</h2>
              <p className="text-stone-500 text-sm mb-5">Quick setup or full custom configuration?</p>
              <div className="space-y-3 mb-6">
                {([
                  { id: 'cloud' as Runtime, label: 'Quick setup', icon: '☁️', desc: 'Pick a provider, enter your API key, and start chatting.' },
                  { id: 'local' as Runtime, label: 'Full configuration', icon: '🔧', desc: 'Configure inference, voice, and tools for a tailored experience.' },
                ] as const).map(opt => (
                  <button
                    key={opt.id}
                    type="button"
                    onClick={() => setRuntime(opt.id)}
                    className={`w-full text-left px-4 py-3 rounded-xl border-2 transition-colors ${runtime === opt.id ? 'border-primary-500 bg-primary-50' : 'border-stone-200 hover:border-stone-300'}`}
                  >
                    <p className="font-medium text-stone-900 text-sm">{opt.icon} {opt.label}</p>
                    <p className="text-xs text-stone-500 mt-0.5">{opt.desc}</p>
                  </button>
                ))}
              </div>
              <div className="flex gap-3">
                <button type="button" onClick={goPrev} className="flex-1 py-3 border border-stone-200 text-stone-700 font-medium rounded-xl hover:bg-stone-50 transition-colors text-sm">Back</button>
                <button type="button" onClick={goNext} className="flex-[2] py-3 bg-primary-500 hover:bg-primary-600 text-white font-semibold rounded-xl transition-colors">Continue</button>
              </div>
            </div>
          )}

          {/* ── Provider ── */}
          {step === 'provider' && (
            <div className="bg-white rounded-2xl border border-stone-200 p-8 shadow-lg">
              <h2 className="text-xl font-bold text-stone-900 mb-1">Choose AI Provider</h2>
              <p className="text-stone-500 text-sm mb-5">Select which AI backend to use. You can change this later in Settings.</p>
              <div className="space-y-3 mb-6">
                {PROVIDERS.map(p => (
                  <button key={p.id} type="button" onClick={() => { setProvider(p.id); setModel(''); }}
                    className={`w-full text-left px-4 py-3 rounded-xl border-2 transition-colors ${provider === p.id ? 'border-primary-500 bg-primary-50' : 'border-stone-200 hover:border-stone-300'}`}
                  >
                    <p className="font-medium text-stone-900 text-sm">{p.name}</p>
                    <p className="text-xs text-stone-500 mt-0.5">{p.desc}</p>
                  </button>
                ))}
              </div>
              <div className="flex gap-3">
                <button type="button" onClick={goPrev} className="flex-1 py-3 border border-stone-200 text-stone-700 font-medium rounded-xl hover:bg-stone-50 transition-colors text-sm">Back</button>
                <button type="button" onClick={goNext} className="flex-[2] py-3 bg-primary-500 hover:bg-primary-600 text-white font-semibold rounded-xl transition-colors">Continue</button>
              </div>
            </div>
          )}

          {/* ── API Key ── */}
          {step === 'apikey' && (
            <div className="bg-white rounded-2xl border border-stone-200 p-8 shadow-lg">
              <h2 className="text-xl font-bold text-stone-900 mb-1">{sel.keyName ? 'Enter API Key' : 'Ollama Setup'}</h2>
              <p className="text-stone-500 text-sm mb-5">
                {sel.keyName
                  ? `Paste your ${sel.name} API key. Stored locally on your device.`
                  : 'Ollama runs locally. Make sure Ollama is running at http://localhost:11434'}
              </p>
              {error && <div className="mb-4 px-3 py-2 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700">{error}</div>}
              {sel.keyName ? (
                <input
                  type="password"
                  value={apiKey}
                  onChange={e => setApiKey(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && handleSaveKey()}
                  placeholder={sel.placeholder}
                  autoFocus
                  className="w-full mb-4 px-4 py-3 rounded-xl border border-stone-200 focus:border-primary-400 text-sm outline-none bg-stone-50 font-mono"
                />
              ) : (
                <div className="mb-4 px-4 py-3 bg-stone-50 rounded-xl border border-stone-200 text-sm text-stone-600 font-mono">
                  http://localhost:11434
                </div>
              )}
              <div className="flex gap-3">
                <button type="button" onClick={goPrev} className="flex-1 py-3 border border-stone-200 text-stone-700 font-medium rounded-xl hover:bg-stone-50 transition-colors text-sm">Back</button>
                <button
                  type="button"
                  onClick={handleSaveKey}
                  disabled={saving || (!!sel.keyName && !apiKey.trim())}
                  className="flex-[2] py-3 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 text-white font-semibold rounded-xl transition-colors"
                >
                  {saving ? 'Saving…' : 'Continue'}
                </button>
              </div>
            </div>
          )}

          {/* ── Inference config (local path only) ── */}
          {step === 'inference' && (
            <div className="bg-white rounded-2xl border border-stone-200 p-8 shadow-lg">
              <h2 className="text-xl font-bold text-stone-900 mb-1">Inference Settings</h2>
              <p className="text-stone-500 text-sm mb-5">Choose the model to use. You can change this anytime in Settings.</p>
              {error && <div className="mb-4 px-3 py-2 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700">{error}</div>}
              <div className="mb-5">
                <label className="block text-xs font-medium text-stone-600 mb-2">Model</label>
                <select
                  value={model}
                  onChange={e => setModel(e.target.value)}
                  title="AI model selection"
                  className="w-full px-4 py-3 rounded-xl border border-stone-200 bg-stone-50 text-sm text-stone-900 outline-none focus:border-primary-400"
                >
                  <option value="">Default (recommended)</option>
                  {modelList.map(m => <option key={m} value={m}>{m}</option>)}
                </select>
              </div>
              {provider === 'ollama' && (
                <div className="mb-5 px-4 py-3 bg-amber-50 border border-amber-200 rounded-xl text-xs text-amber-700">
                  Make sure Ollama is running: <span className="font-mono">ollama serve</span>
                </div>
              )}
              <div className="flex gap-3">
                <button type="button" onClick={goPrev} className="flex-1 py-3 border border-stone-200 text-stone-700 font-medium rounded-xl hover:bg-stone-50 transition-colors text-sm">Back</button>
                <button
                  type="button"
                  onClick={handleSaveInference}
                  disabled={saving}
                  className="flex-[2] py-3 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 text-white font-semibold rounded-xl transition-colors"
                >
                  {saving ? 'Saving…' : 'Continue'}
                </button>
              </div>
            </div>
          )}

          {/* ── Voice config (local path only) ── */}
          {step === 'voice' && (
            <div className="bg-white rounded-2xl border border-stone-200 p-8 shadow-lg">
              <h2 className="text-xl font-bold text-stone-900 mb-1">Voice Settings</h2>
              <p className="text-stone-500 text-sm mb-5">Configure how Youma speaks and listens.</p>
              {error && <div className="mb-4 px-3 py-2 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700">{error}</div>}

              <div className="mb-4">
                <label className="block text-xs font-medium text-stone-600 mb-2">Text-to-Speech (voice output)</label>
                <div className="space-y-2">
                  {([
                    { id: 'native'  as TtsProvider, label: 'System voice',  desc: 'WinRT / macOS AVSpeech / espeak-ng — free, offline' },
                    { id: 'openai'  as TtsProvider, label: 'OpenAI TTS',    desc: 'High-quality neural voice (requires OpenAI key)'     },
                    { id: 'browser' as TtsProvider, label: 'Browser voice', desc: 'Web Speech API — always available, basic quality'    },
                  ] as const).map(opt => (
                    <button
                      key={opt.id}
                      type="button"
                      onClick={() => setTts(opt.id)}
                      className={`w-full text-left px-3 py-2.5 rounded-xl border-2 transition-colors ${ttsProvider === opt.id ? 'border-primary-500 bg-primary-50' : 'border-stone-200 hover:border-stone-300'}`}
                    >
                      <p className="font-medium text-stone-900 text-sm">{opt.label}</p>
                      <p className="text-xs text-stone-500">{opt.desc}</p>
                    </button>
                  ))}
                </div>
              </div>

              <div className="mb-5">
                <label className="block text-xs font-medium text-stone-600 mb-2">Speech recognition language</label>
                <select
                  value={sttLang}
                  onChange={e => setSttLang(e.target.value)}
                  title="Speech recognition language"
                  className="w-full px-4 py-3 rounded-xl border border-stone-200 bg-stone-50 text-sm text-stone-900 outline-none focus:border-primary-400"
                >
                  {[
                    ['en-US', 'English (US)'],
                    ['en-GB', 'English (UK)'],
                    ['it-IT', 'Italian'],
                    ['es-ES', 'Spanish'],
                    ['fr-FR', 'French'],
                    ['de-DE', 'German'],
                    ['pt-BR', 'Portuguese (Brazil)'],
                    ['ja-JP', 'Japanese'],
                    ['zh-CN', 'Chinese (Simplified)'],
                  ].map(([code, label]) => <option key={code} value={code}>{label}</option>)}
                </select>
              </div>

              <div className="flex gap-3">
                <button type="button" onClick={goPrev} className="flex-1 py-3 border border-stone-200 text-stone-700 font-medium rounded-xl hover:bg-stone-50 transition-colors text-sm">Back</button>
                <button
                  type="button"
                  onClick={handleSaveVoice}
                  disabled={saving}
                  className="flex-[2] py-3 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 text-white font-semibold rounded-xl transition-colors"
                >
                  {saving ? 'Saving…' : 'Continue'}
                </button>
              </div>
            </div>
          )}

          {/* ── Tools config (local path only) ── */}
          {step === 'tools' && (
            <div className="bg-white rounded-2xl border border-stone-200 p-8 shadow-lg">
              <h2 className="text-xl font-bold text-stone-900 mb-1">Enable Skills</h2>
              <p className="text-stone-500 text-sm mb-5">Choose which capabilities to activate. You can change these anytime in Skills.</p>
              {error && <div className="mb-4 px-3 py-2 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700">{error}</div>}
              <div className="space-y-2 mb-6">
                {tools.map(tool => (
                  <div key={tool.id} className="flex items-center justify-between px-3 py-2.5 rounded-xl border border-stone-200 hover:border-stone-300 transition-colors">
                    <div>
                      <div className="text-sm font-medium text-stone-900">{tool.label}</div>
                      <div className="text-xs text-stone-500">{tool.desc}</div>
                    </div>
                    <button
                      type="button"
                      onClick={() => toggleTool(tool.id)}
                      aria-label={`${tool.enabled ? 'Disable' : 'Enable'} ${tool.label}`}
                      className={`relative inline-flex h-5 w-9 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ${tool.enabled ? 'bg-primary-500' : 'bg-stone-300'}`}
                    >
                      <span className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm transform transition-transform duration-200 ${tool.enabled ? 'translate-x-4' : 'translate-x-0'}`} />
                    </button>
                  </div>
                ))}
              </div>
              <div className="flex gap-3">
                <button type="button" onClick={goPrev} className="flex-1 py-3 border border-stone-200 text-stone-700 font-medium rounded-xl hover:bg-stone-50 transition-colors text-sm">Back</button>
                <button
                  type="button"
                  onClick={handleSaveTools}
                  disabled={saving}
                  className="flex-[2] py-3 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 text-white font-semibold rounded-xl transition-colors"
                >
                  {saving ? 'Saving…' : "Let's go!"}
                </button>
              </div>
            </div>
          )}

          {/* ── Done ── */}
          {step === 'done' && (
            <div className="bg-white rounded-2xl border border-stone-200 p-8 shadow-lg text-center">
              <div className="w-20 h-20 rounded-full bg-green-50 border-2 border-green-200 flex items-center justify-center mx-auto mb-4 text-4xl">✅</div>
              <h2 className="text-2xl font-bold text-stone-900 mb-2">You're all set!</h2>
              <p className="text-stone-500 text-sm">Taking you to Youma…</p>
            </div>
          )}
        </div>
      </div>

      {/* Post-onboarding walkthrough */}
      {showWalkthrough && (
        <WalkthroughOverlay onFinish={() => { setShowWalkthrough(false); onComplete(); }} />
      )}
    </>
  );
}
