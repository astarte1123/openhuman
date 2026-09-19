import { useT } from '../lib/i18n/I18nContext';

interface ProgressIndicatorProps {
  currentStep: number;
  totalSteps: number;
}

const ProgressIndicator = ({ currentStep, totalSteps }: ProgressIndicatorProps) => {
  const { t } = useT();
  return (
    <div
      role="list"
      aria-label={t('common.progress_indicator.progress_steps')}
      className="flex items-center justify-center space-x-2">
      {Array.from({ length: totalSteps }).map((_, index) => {
        const isCurrent = index === currentStep;
        return (
          <div
            key={index}
            role="listitem"
            aria-label={t('common.progress_indicator.step_of_total', {
              step: index + 1,
              total: totalSteps,
            })}
            aria-current={isCurrent ? 'step' : undefined}
            className={`w-2 h-2 rounded-full transition-colors ${
              isCurrent ? 'bg-stone-800' : 'bg-stone-300'
            }`}
          />
        );
      })}
    </div>
  );
};

export default ProgressIndicator;
