import { useEffect, useState } from 'react';

import { BackendMascot } from '../../../features/human/Mascot/backend/BackendMascot';
import type { MascotDetail, MascotSummary } from '../../../features/human/Mascot/backend/types';
import { getMascotPalette, type MascotColor } from '../../../features/human/Mascot/mascotPalette';
import { useT } from '../../../lib/i18n/I18nContext';
import { fetchMascotList, getCachedMascotDetail } from '../../../services/mascotService';
import { useAppDispatch, useAppSelector } from '../../../store/hooks';
import {
  DEFAULT_MASCOT_COLOR,
  selectMascotColor,
  selectSelectedMascotId,
  setMascotColor,
  setSelectedMascotId,
  SUPPORTED_MASCOT_COLORS,
} from '../../../store/mascotSlice';
import SettingsHeader from '../components/SettingsHeader';
import { useSettingsNavigation } from '../hooks/useSettingsNavigation';

interface ColorOption {
  id: MascotColor;
  labelKey: string;
}

const COLOR_OPTIONS: ColorOption[] = [
  { id: 'yellow', labelKey: 'settings.mascot_panel.color_yellow' },
  { id: 'burgundy', labelKey: 'settings.mascot_panel.color_burgundy' },
  { id: 'black', labelKey: 'settings.mascot_panel.color_black' },
  { id: 'navy', labelKey: 'settings.mascot_panel.color_navy' },
  { id: 'green', labelKey: 'settings.mascot_panel.color_green' },
];

const MascotPanel = () => {
  const { t } = useT();
  const { navigateBack, breadcrumbs } = useSettingsNavigation();
  const dispatch = useAppDispatch();
  const storedColor = useAppSelector(selectMascotColor);
  const selectedMascotId = useAppSelector(selectSelectedMascotId);

  // Backend mascot library (PR tinyhumansai/backend#770). The list endpoint
  // is cheap (no SVG bytes); per-id detail is fetched on demand so the
  // animated preview only pays for the active selection.
  const [backendList, setBackendList] = useState<MascotSummary[] | null>(null);
  const [backendListError, setBackendListError] = useState<string | null>(null);
  const [activeDetail, setActiveDetail] = useState<MascotDetail | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchMascotList()
      .then(list => {
        if (cancelled) return;
        setBackendList(list);
        setBackendListError(null);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        const message =
          err instanceof Error ? err.message : t('settings.mascot_panel.load_library_error');
        setBackendListError(message);
        setBackendList([]);
      });
    return () => {
      cancelled = true;
    };
  }, [t]);

  useEffect(() => {
    if (!selectedMascotId) {
      setActiveDetail(null);
      return;
    }
    let cancelled = false;
    getCachedMascotDetail(selectedMascotId)
      .then(detail => {
        if (cancelled) return;
        setActiveDetail(detail);
        setDetailError(null);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        const message =
          err instanceof Error ? err.message : t('settings.mascot_panel.load_mascot_error');
        setDetailError(message);
        setActiveDetail(null);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedMascotId, t]);

  const handleSelectBackend = (id: string | null) => {
    dispatch(setSelectedMascotId(id));
  };

  // Filter the menu to colors the asset pipeline currently supports — guards
  // against an older persisted value pointing at a variant a future build
  // removed. The selected swatch still highlights iff the stored color is
  // present; otherwise we silently fall back to the default for the preview.
  const available = COLOR_OPTIONS.filter(opt =>
    (SUPPORTED_MASCOT_COLORS as readonly string[]).includes(opt.id)
  );
  const activeColor: MascotColor = (SUPPORTED_MASCOT_COLORS as readonly string[]).includes(
    storedColor
  )
    ? storedColor
    : DEFAULT_MASCOT_COLOR;

  const handleSelect = (color: MascotColor) => {
    if (color === storedColor) return;
    dispatch(setMascotColor(color));
  };

  return (
    <div>
      <SettingsHeader
        title={t('settings.mascot_panel.header_title')}
        showBackButton={true}
        onBack={navigateBack}
        breadcrumbs={breadcrumbs}
      />

      <div className="p-4 space-y-4">
        <div>
          <h3 className="text-xs font-semibold uppercase tracking-wider text-stone-400 mb-2 px-1">
            {t('settings.mascot_panel.color_heading')}
          </h3>
          <div className="bg-white rounded-xl border border-stone-200 overflow-hidden">
            {available.length === 0 ? (
              <p className="p-4 text-sm text-stone-500">
                {t('settings.mascot_panel.no_color_variants')}
              </p>
            ) : (
              <div
                className="grid grid-cols-5 gap-3 p-4"
                role="radiogroup"
                aria-label={t('settings.mascot_panel.color_radiogroup_aria')}>
                {available.map(opt => {
                  const palette = getMascotPalette(opt.id);
                  const selected = opt.id === activeColor;
                  const label = t(opt.labelKey);
                  return (
                    <button
                      key={opt.id}
                      type="button"
                      role="radio"
                      aria-checked={selected}
                      aria-label={label}
                      onClick={() => handleSelect(opt.id)}
                      data-testid={`mascot-color-${opt.id}`}
                      className={`flex flex-col items-center gap-2 rounded-lg p-2 transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-primary-500 ${
                        selected ? 'bg-stone-100' : 'hover:bg-stone-50'
                      }`}>
                      <span
                        className={`w-10 h-10 rounded-full border-2 transition-shadow ${
                          selected ? 'border-primary-500 shadow-soft' : 'border-stone-200'
                        }`}
                        style={{ backgroundColor: palette.bodyFill }}
                      />
                      <span className="text-xs text-stone-700">{label}</span>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
          <p className="text-xs text-stone-500 leading-relaxed px-1 mt-2">
            {t('settings.mascot_panel.color_footer_hint')}
          </p>
        </div>

        <div>
          <h3 className="text-xs font-semibold uppercase tracking-wider text-stone-400 mb-2 px-1">
            {t('settings.mascot_panel.character_heading')}
          </h3>
          <div className="bg-white rounded-xl border border-stone-200 overflow-hidden">
            {backendListError && (
              <p className="p-4 text-sm text-coral-700">
                {t('settings.mascot_panel.library_unavailable', { error: backendListError })}
              </p>
            )}
            {!backendListError && backendList === null && (
              <p className="p-4 text-sm text-stone-500">
                {t('settings.mascot_panel.loading_library')}
              </p>
            )}
            {backendList && backendList.length === 0 && !backendListError && (
              <p className="p-4 text-sm text-stone-500">
                {t('settings.mascot_panel.no_characters')}
              </p>
            )}
            {backendList && backendList.length > 0 && (
              <ul className="divide-y divide-stone-100">
                <li>
                  <button
                    type="button"
                    onClick={() => handleSelectBackend(null)}
                    aria-pressed={selectedMascotId == null}
                    className={`flex w-full items-center justify-between px-4 py-3 text-left text-sm hover:bg-stone-50 ${
                      selectedMascotId == null ? 'bg-stone-50 font-medium' : ''
                    }`}>
                    <span>{t('settings.mascot_panel.local_aura_default')}</span>
                    {selectedMascotId == null && (
                      <span className="text-[10px] uppercase text-primary-600">
                        {t('settings.mascot_panel.active_badge')}
                      </span>
                    )}
                  </button>
                </li>
                {backendList.map(summary => {
                  const active = summary.id === selectedMascotId;
                  return (
                    <li key={summary.id}>
                      <button
                        type="button"
                        onClick={() => handleSelectBackend(summary.id)}
                        aria-pressed={active}
                        data-testid={`backend-mascot-${summary.id}`}
                        className={`flex w-full items-center justify-between px-4 py-3 text-left text-sm hover:bg-stone-50 ${
                          active ? 'bg-stone-50 font-medium' : ''
                        }`}>
                        <span className="flex flex-col">
                          <span>{summary.name}</span>
                          <span className="text-[10px] text-stone-500">
                            {t('settings.mascot_panel.version_states', {
                              version: summary.version,
                              states: summary.states.length,
                            })}
                            {summary.hasVisemes
                              ? t('settings.mascot_panel.visemes_suffix')
                              : ''}
                          </span>
                        </span>
                        {active && (
                          <span className="text-[10px] uppercase text-primary-600">
                            {t('settings.mascot_panel.active_badge')}
                          </span>
                        )}
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>

          {activeDetail && (
            <div className="mt-3 rounded-xl border border-stone-200 bg-stone-50 p-4">
              <p className="text-[11px] font-medium uppercase tracking-wide text-stone-500 mb-2">
                {t('settings.mascot_panel.preview_label', { name: activeDetail.name })}
              </p>
              <div className="flex justify-center">
                <div style={{ width: 160, height: 160 }}>
                  <BackendMascot mascot={activeDetail} />
                </div>
              </div>
            </div>
          )}
          {detailError && <p className="mt-2 text-xs text-coral-700 px-1">{detailError}</p>}
          <p className="text-xs text-stone-500 leading-relaxed px-1 mt-2">
            {t('settings.mascot_panel.character_footer_hint')}
          </p>
        </div>
      </div>
    </div>
  );
};

export default MascotPanel;
