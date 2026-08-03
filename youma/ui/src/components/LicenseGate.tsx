import { useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { licenseStatus, licenseActivate, licenseStartTrial } from '../api';

// ---------------------------------------------------------------------------
// LicenseGate — blocks app until license is valid.
// Shows spinner while checking, then either passes children through or
// shows the activation / trial-start screen.
// ---------------------------------------------------------------------------

function Spinner() {
  return (
    <div className="fixed inset-0 bg-stone-50 flex items-center justify-center">
      <div className="w-8 h-8 border-2 border-primary-500 border-t-transparent rounded-full animate-spin" />
    </div>
  );
}

function ActivationScreen({ onActivated }: { onActivated: () => void }) {
  const { t } = useTranslation();
  const [key, setKey] = useState('');
  const [serverUrl, setServerUrl] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [activating, setActivating] = useState(false);
  const [startingTrial, setStartingTrial] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [trialAlreadyUsed, setTrialAlreadyUsed] = useState(false);

  useEffect(() => {
    // Check if trial was already started on this machine
    licenseStatus().then(s => {
      if (s.status === 'expired' && s.trial_days_left !== null) {
        setTrialAlreadyUsed(true);
      }
    }).catch(() => {});
  }, []);

  async function handleActivate() {
    const trimmed = key.trim().toUpperCase();
    if (!trimmed) return;
    setActivating(true);
    setError(null);
    try {
      const res = await licenseActivate(trimmed, serverUrl.trim() || undefined);
      if (res.ok) {
        onActivated();
      } else {
        setError(res.error ?? t('license.error_invalid_key'));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setActivating(false);
    }
  }

  async function handleTrial() {
    setStartingTrial(true);
    setError(null);
    try {
      const res = await licenseStartTrial();
      if (res.ok) {
        onActivated();
      } else {
        setError(res.error ?? 'Trial failed');
        setTrialAlreadyUsed(true);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setStartingTrial(false);
    }
  }

  return (
    <div className="fixed inset-0 bg-stone-50 flex items-center justify-center p-6">
      <div className="w-full max-w-sm space-y-6">

        {/* Logo / branding */}
        <div className="text-center">
          <div className="inline-flex items-center justify-center w-14 h-14 rounded-2xl bg-primary-500 text-white text-2xl font-bold mb-3 shadow-lg">
            Y
          </div>
          <h1 className="text-xl font-bold text-stone-900">Youma</h1>
          <p className="text-sm text-stone-500 mt-1">{t('license.activate_desc')}</p>
        </div>

        {/* Key input */}
        <div className="space-y-3">
          <div>
            <label htmlFor="license-key" className="block text-xs font-medium text-stone-700 mb-1">
              {t('license.key_label')}
            </label>
            <input
              id="license-key"
              type="text"
              value={key}
              onChange={e => setKey(e.target.value.toUpperCase())}
              placeholder={t('license.key_placeholder')}
              className="w-full px-4 py-2.5 rounded-xl border border-stone-200 focus:border-primary-400 focus:outline-none bg-white text-stone-900 placeholder-stone-400 text-sm font-mono tracking-wider transition-colors"
              onKeyDown={e => { if (e.key === 'Enter') handleActivate(); }}
            />
          </div>

          {/* Advanced toggle */}
          <button
            type="button"
            onClick={() => setShowAdvanced(v => !v)}
            className="text-xs text-stone-400 hover:text-stone-600 flex items-center gap-1 transition-colors"
          >
            <svg className={`w-3 h-3 transition-transform ${showAdvanced ? 'rotate-90' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
            </svg>
            {t('license.advanced')}
          </button>

          {showAdvanced && (
            <div>
              <label htmlFor="server-url" className="block text-xs font-medium text-stone-700 mb-1">
                {t('license.server_url_label')}
              </label>
              <input
                id="server-url"
                type="text"
                value={serverUrl}
                onChange={e => setServerUrl(e.target.value)}
                placeholder={t('license.server_url_placeholder')}
                className="w-full px-4 py-2.5 rounded-xl border border-stone-200 focus:border-primary-400 focus:outline-none bg-white text-stone-900 placeholder-stone-400 text-sm transition-colors"
              />
            </div>
          )}

          {error && (
            <div className="px-3 py-2 bg-coral-50 border border-coral-200 rounded-lg text-xs text-coral-700">
              {error}
            </div>
          )}

          <button
            type="button"
            onClick={handleActivate}
            disabled={!key.trim() || activating}
            className="w-full px-4 py-2.5 bg-primary-500 hover:bg-primary-600 disabled:opacity-50 disabled:cursor-not-allowed text-white text-sm font-semibold rounded-xl transition-colors"
          >
            {activating ? '…' : t('license.activate')}
          </button>

          {!trialAlreadyUsed && (
            <>
              <div className="flex items-center gap-2">
                <hr className="flex-1 border-stone-200" />
                <span className="text-xs text-stone-400">or</span>
                <hr className="flex-1 border-stone-200" />
              </div>
              <button
                type="button"
                onClick={handleTrial}
                disabled={startingTrial}
                className="w-full px-4 py-2.5 bg-white hover:bg-stone-50 disabled:opacity-50 border border-stone-200 text-stone-700 text-sm font-medium rounded-xl transition-colors"
              >
                {startingTrial ? '…' : t('license.start_trial')}
              </button>
            </>
          )}
        </div>

        <p className="text-center text-xs text-stone-400">
          Youma by Youco Srl
        </p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Gate component
// ---------------------------------------------------------------------------

interface Props {
  children: ReactNode;
}

export default function LicenseGate({ children }: Props) {
  const [ready, setReady] = useState(false);
  const [valid, setValid] = useState(false);

  async function check() {
    try {
      const s = await licenseStatus();
      setValid(s.valid);
    } catch {
      // If we can't read the license at all, allow through (don't block on DB error)
      setValid(true);
    }
    setReady(true);
  }

  useEffect(() => {
    check();
  }, []);

  if (!ready) return <Spinner />;
  if (!valid) return <ActivationScreen onActivated={() => setValid(true)} />;
  return <>{children}</>;
}
