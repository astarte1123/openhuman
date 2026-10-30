import createDebug from 'debug';
import { useState } from 'react';

import MeshGradient from '../components/MeshGradient';
import OAuthProviderButton from '../components/oauth/OAuthProviderButton';
import { oauthProviderConfigs } from '../components/oauth/providerConfigs';
import RotatingTetrahedronCanvas from '../components/RotatingTetrahedronCanvas';
import Button from '../components/ui/Button';
import { useT } from '../lib/i18n/I18nContext';
import { clearBackendUrlCache } from '../services/backendUrl';
import { clearCoreRpcTokenCache, clearCoreRpcUrlCache } from '../services/coreRpcClient';
import { resetCoreMode } from '../store/coreModeSlice';
import { useDeepLinkAuthState } from '../store/deepLinkAuthState';
import { useAppDispatch } from '../store/hooks';
import { clearAllAppData } from '../utils/clearAllAppData';
import { clearStoredCoreMode, clearStoredCoreToken, storeRpcUrl } from '../utils/configPersistence';

const log = createDebug('app:welcome');

const Welcome = () => {
  const { t } = useT();
  const dispatch = useAppDispatch();
  const { isProcessing, errorMessage, requiresAppDataReset } = useDeepLinkAuthState();

  const [isClearingAppData, setIsClearingAppData] = useState(false);
  const [resetError, setResetError] = useState<string | null>(null);

  const handleClearAppData = async () => {
    setIsClearingAppData(true);
    setResetError(null);
    try {
      // No live session at the Welcome screen — skip the core-side
      // `clearSession` step, just wipe local data and restart.
      await clearAllAppData();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log('clearAllAppData failed: %s', message);
      setResetError(
        message || 'Could not clear app data. Please quit and reopen Aura, then try again.'
      );
      setIsClearingAppData(false);
    }
  };

  const handleSelectRuntime = () => {
    log('[welcome] select-runtime — resetting core mode to return to picker');
    storeRpcUrl('');
    clearStoredCoreToken();
    clearStoredCoreMode();
    clearCoreRpcUrlCache();
    clearCoreRpcTokenCache();
    clearBackendUrlCache();
    dispatch(resetCoreMode());
  };

  return (
    <div className="relative min-h-full overflow-hidden">
      {/* Ambient animated gradient behind the glass card. */}
      <div className="absolute inset-0">
        <MeshGradient />
      </div>
      <div className="relative flex min-h-full flex-col items-center justify-center p-4">
        <div className="w-full max-w-md">
          <div className="rounded-2xl border border-line-default bg-surface-base/80 p-8 shadow-float backdrop-blur-xl animate-fade-up">
            <div className="mb-6 flex justify-center">
              <div className="h-20 w-20">
                <RotatingTetrahedronCanvas />
              </div>
            </div>

            <h1 className="mb-2 text-center font-display text-3xl font-medium tracking-tight text-ink-primary">
              {t('welcome.title')}
            </h1>

            <p className="mb-6 text-sm leading-relaxed text-center text-ink-secondary">
              {t('welcome.subtitle')}
            </p>

            {errorMessage ? (
              <div
                role="alert"
                className="mb-5 rounded-lg border border-coral-200 bg-coral-50 px-3 py-2 text-sm text-coral-700">
                <p>{errorMessage}</p>
                {requiresAppDataReset ? (
                  <div className="mt-3 space-y-2">
                    <button
                      type="button"
                      onClick={handleClearAppData}
                      disabled={isClearingAppData}
                      className="w-full rounded-lg bg-coral-600 px-3 py-2 text-xs font-semibold text-white transition-colors hover:bg-coral-700 disabled:cursor-not-allowed disabled:opacity-60">
                      {isClearingAppData ? (
                        <span className="flex items-center justify-center gap-2">
                          <span className="h-3 w-3 animate-spin rounded-full border border-white border-t-transparent" />
                          Clearing app data...
                        </span>
                      ) : (
                        'Clear app data & restart'
                      )}
                    </button>
                    <p className="text-[11px] leading-4 text-coral-600/80">
                      This wipes locally stored secrets and accounts on this device. Your cloud
                      account is unaffected — you can sign in again right after.
                    </p>
                    {resetError ? (
                      <p className="text-[11px] leading-4 font-medium text-coral-700">
                        {resetError}
                      </p>
                    ) : null}
                  </div>
                ) : null}
              </div>
            ) : null}

            {isProcessing ? (
              <div
                role="status"
                aria-live="polite"
                aria-atomic="true"
                className="mb-5 flex flex-col items-center justify-center gap-3 py-2">
                <div className="w-full space-y-2">
                  <div className="h-4 animate-pulse rounded-md bg-surface-sunken" />
                  <div className="h-4 w-2/3 animate-pulse rounded-md bg-surface-sunken" />
                </div>
                <p className="text-sm font-medium text-ink-secondary">Signing you in...</p>
              </div>
            ) : (
              <>
                {/* Real OAuth: click → system browser → backend → deep link back to app. */}
                <div className="flex flex-col items-stretch gap-2">
                  {oauthProviderConfigs
                    .filter(provider => provider.showOnWelcome)
                    .map(provider => (
                      <OAuthProviderButton
                        key={provider.id}
                        provider={provider}
                        className="w-full"
                      />
                    ))}
                </div>
              </>
            )}
          </div>

          <div className="mt-4 px-2">
            <Button
              variant="secondary"
              size="md"
              onClick={handleSelectRuntime}
              className="w-full py-3">
              {t('welcome.selectRuntime')}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Welcome;
