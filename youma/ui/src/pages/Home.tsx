import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { healthSnapshot, settingsGet, type HealthSnapshot } from '../api';

export default function HomePage() {
  const navigate = useNavigate();
  const { t } = useTranslation();

  // Typewriter effect
  const variants = useMemo(() => [
    t('home.welcome_0'),
    t('home.welcome_1'),
    t('home.welcome_2'),
  ], [t]);
  const [variantIndex, setVariantIndex] = useState(0);
  const [typed, setTyped] = useState('');
  const [isDeleting, setIsDeleting] = useState(false);

  // Reset typewriter when language changes (variants reference changes with new t)
  useEffect(() => {
    setTyped('');
    setIsDeleting(false);
    setVariantIndex(0);
  }, [variants]);

  // Custom logo from settings
  const [customLogo, setCustomLogo] = useState<string | null>(null);

  useEffect(() => {
    settingsGet('app.custom_logo').then(v => { if (v) setCustomLogo(v); }).catch(() => {});
  }, []);

  // Health snapshot from core
  const [health, setHealth] = useState<HealthSnapshot | null>(null);
  const [healthLoading, setHealthLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const snap = await healthSnapshot();
        if (!cancelled) setHealth(snap);
      } catch {
        if (!cancelled) setHealth({ status: 'unreachable', version: '—' });
      } finally {
        if (!cancelled) setHealthLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // Typewriter loop
  useEffect(() => {
    const activeVariant = variants[variantIndex] ?? '';
    const isFullyTyped = typed === activeVariant;
    const isFullyDeleted = typed.length === 0;

    const delay = isDeleting
      ? 36
      : isFullyTyped
        ? 1400
        : typed.length === 0
          ? 250
          : 55;

    const id = window.setTimeout(() => {
      if (!isDeleting) {
        if (isFullyTyped) {
          setIsDeleting(true);
          return;
        }
        setTyped(activeVariant.slice(0, typed.length + 1));
        return;
      }
      if (!isFullyDeleted) {
        setTyped(activeVariant.slice(0, typed.length - 1));
        return;
      }
      setIsDeleting(false);
      setVariantIndex(i => (i + 1) % variants.length);
    }, delay);

    return () => window.clearTimeout(id);
  }, [isDeleting, typed, variantIndex, variants]);

  const statusColor =
    health?.status === 'ok'
      ? 'text-sage-600'
      : health?.status === 'unreachable'
        ? 'text-coral-500'
        : 'text-amber-600';

  return (
    <div className="min-h-full flex flex-col items-center justify-center p-4">
      <div className="max-w-md w-full">
        {/* Main card */}
        <div className="bg-white rounded-2xl shadow-soft border border-stone-200 p-6 animate-fade-up">
          {/* Custom logo */}
          {customLogo && (
            <div className="flex justify-center mb-4 h-20">
              <img
                src={customLogo}
                alt="Logo"
                className="h-full w-auto max-w-[240px] object-contain"
              />
            </div>
          )}

          {/* Version / identity row */}
          <div className="flex items-center justify-center mb-4">
            <span className="text-xs text-center text-stone-400">{t('home.version')}</span>
          </div>

          {/* Typewriter title */}
          <h1 className="min-h-[3.5rem] text-3xl font-bold text-stone-900 text-center">
            {typed}
            <span aria-hidden="true" className="ml-0.5 inline-block text-primary-500 animate-pulse">
              |
            </span>
          </h1>

          {/* Core status pill */}
          <div className="flex justify-center mb-3 mt-3">
            {healthLoading ? (
              <span className="inline-flex items-center px-3 py-1 rounded-full text-xs font-medium bg-stone-100 text-stone-500 animate-pulse">
                {t('home.connecting')}
              </span>
            ) : (
              <span
                className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium ${
                  health?.status === 'ok'
                    ? 'bg-sage-100 text-sage-700'
                    : health?.status === 'unreachable'
                      ? 'bg-coral-100 text-coral-700'
                      : 'bg-amber-100 text-amber-700'
                }`}
              >
                <span
                  className={`w-1.5 h-1.5 rounded-full ${
                    health?.status === 'ok'
                      ? 'bg-sage-500'
                      : health?.status === 'unreachable'
                        ? 'bg-coral-500'
                        : 'bg-amber-500'
                  }`}
                />
                {health?.status === 'ok'
                  ? t('home.core_online')
                  : health?.status === 'unreachable'
                    ? t('home.core_unreachable')
                    : health?.status ?? 'Unknown'}
              </span>
            )}
          </div>

          {/* Status copy */}
          <p className={`text-sm text-center mb-4 leading-relaxed ${healthLoading ? 'text-stone-400' : statusColor}`}>
            {healthLoading
              ? t('home.checking_health')
              : health?.status === 'ok'
                ? t('home.all_systems')
                : t('home.core_not_responding')}
          </p>

          {/* CTA */}
          <button
            type="button"
            onClick={() => navigate('/chat')}
            className="w-full py-3 bg-primary-500 hover:bg-primary-600 text-white font-medium rounded-xl transition-colors duration-200"
          >
            {t('home.start_chatting')}
          </button>
        </div>

        {/* Quick links card */}
        <div className="mt-3 bg-white rounded-2xl shadow-soft border border-stone-200 p-4">
          <div className="text-[11px] uppercase tracking-wide text-stone-400 mb-2">{t('home.quick_links')}</div>
          <div className="divide-y divide-stone-100">
            {[
              { title: t('home.configure_keys'), desc: t('home.configure_keys_desc'), path: '/settings' },
              { title: t('home.skills'),         desc: t('home.skills_desc'),         path: '/skills'   },
              { title: t('home.intelligence'),   desc: t('home.intelligence_desc'),   path: '/intelligence' },
              // Rewards hidden — pending design decision
            ].map(item => (
              <button
                type="button"
                key={item.path}
                onClick={() => navigate(item.path)}
                className="w-full flex items-center justify-between py-2.5 text-left hover:bg-stone-50 rounded-md px-2 -mx-2 transition-colors"
              >
                <div>
                  <div className="text-sm font-medium text-stone-900">{item.title}</div>
                  <div className="text-xs text-stone-500">{item.desc}</div>
                </div>
                <svg
                  className="w-4 h-4 text-stone-400 flex-shrink-0"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                </svg>
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
