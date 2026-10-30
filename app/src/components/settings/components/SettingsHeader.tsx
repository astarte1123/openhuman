import { ChevronLeft as ChevronLeftIcon, ChevronRight as ChevronRightIcon } from 'lucide-react';

import { useT } from '../../../lib/i18n/I18nContext';
import Icon from '../../ui/Icon';

interface BreadcrumbItem {
  label: string;
  onClick?: () => void;
}

interface SettingsHeaderProps {
  className?: string;
  title?: string;
  showBackButton?: boolean;
  onBack?: () => void;
  breadcrumbs?: BreadcrumbItem[];
}

const SettingsHeader = ({
  className = '',
  title,
  showBackButton = false,
  onBack,
  breadcrumbs,
}: SettingsHeaderProps) => {
  const { t } = useT();

  return (
    <div className={`px-5 pt-5 pb-3 ${className}`}>
      <div className="flex items-center">
        {/* Back button */}
        {showBackButton && onBack && (
          <button
            onClick={onBack}
            className="mr-2 flex h-6 w-6 items-center justify-center rounded-full text-ink-secondary transition-colors duration-fast hover:bg-surface-sunken hover:text-ink-primary"
            aria-label={t('common.back')}>
            <Icon icon={ChevronLeftIcon} size="sm" />
          </button>
        )}

        {/* Breadcrumbs */}
        {breadcrumbs && breadcrumbs.length > 0 && (
          <nav aria-label="Breadcrumb" className="mr-1">
            <ol className="flex items-center gap-1">
              {breadcrumbs.map((crumb, i) => (
                <li key={i} className="flex items-center gap-1">
                  {crumb.onClick ? (
                    <button
                      onClick={crumb.onClick}
                      className="text-xs text-ink-tertiary transition-colors duration-fast hover:text-ink-primary">
                      {crumb.label}
                    </button>
                  ) : (
                    <span className="text-xs text-ink-tertiary">{crumb.label}</span>
                  )}
                  <Icon
                    icon={ChevronRightIcon}
                    size="xs"
                    className="text-line-strong"
                    aria-hidden
                  />
                </li>
              ))}
            </ol>
          </nav>
        )}

        {/* Title */}
        <h2 className="font-display text-sm font-semibold text-ink-primary">
          {title ?? t('nav.settings')}
        </h2>
      </div>
    </div>
  );
};

export default SettingsHeader;
