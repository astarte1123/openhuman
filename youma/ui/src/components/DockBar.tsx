import { useLocation, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

interface DockBarProps {
  isOpen?: boolean;
  left?: number;
  top?: number;
  width?: number;
  inline?: boolean;
}

export default function DockBar({ isOpen, left, top, width, inline }: DockBarProps) {
  const location = useLocation();
  const navigate = useNavigate();
  const { t } = useTranslation();

  const TABS = [
    {
      id: 'home',
      label: t('nav.home'),
      path: '/home',
      icon: (
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-4 0a2 2 0 01-2-2v-4a2 2 0 012-2h2a2 2 0 012 2v4a2 2 0 01-2 2h-2z"
          />
        </svg>
      ),
    },
    {
      id: 'human',
      label: t('nav.youma'),
      path: '/human',
      icon: (
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
          />
        </svg>
      ),
    },
    {
      id: 'chat',
      label: t('nav.chat'),
      path: '/chat',
      icon: (
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
          />
        </svg>
      ),
    },
    {
      id: 'meetings',
      label: t('nav.meetings'),
      path: '/meetings',
      icon: (
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.8}
            d="M17 20h5v-2a4 4 0 00-5-3.87M9 20H4v-2a4 4 0 015-3.87m6-4.13a4 4 0 10-8 0 4 4 0 008 0zm6 0a3 3 0 11-6 0 3 3 0 016 0zm-14 0a3 3 0 11-6 0 3 3 0 016 0z" />
        </svg>
      ),
    },
    {
      id: 'intelligence',
      label: t('nav.mind'),
      path: '/intelligence',
      icon: (
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M12 5C12 5 9.5 3 7 4.5C4.5 6 4 9 5.5 11C4 12 3.5 14 4.5 15.5C5.5 17 7.5 17 9 16.5C9 18 10 19 12 19C14 19 15 18 15 16.5C16.5 17 18.5 17 19.5 15.5C20.5 14 20 12 18.5 11C20 9 19.5 6 17 4.5C14.5 3 12 5 12 5Z"
          />
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M12 5V19M9 9C9 9 10.5 10 12 9M15 9C15 9 13.5 10 12 9M9 13C9 13 10.5 14 12 13M15 13C15 13 13.5 14 12 13"
          />
        </svg>
      ),
    },
    {
      id: 'settings',
      label: t('nav.settings'),
      path: '/settings',
      icon: (
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
          />
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
          />
        </svg>
      ),
    },
  ];

  const isActive = (path: string) => location.pathname.startsWith(path);

  const tabButtons = TABS.map(tab => {
    const active = isActive(tab.path);
    return (
      <button
        type="button"
        key={tab.id}
        onClick={() => navigate(tab.path)}
        className={`group relative flex items-center gap-2 px-3 py-1.5 rounded-xl text-sm font-medium transition-all duration-200 whitespace-nowrap flex-shrink-0 ${
          active
            ? 'bg-primary-50 text-primary-600'
            : 'text-stone-500 hover:bg-stone-100 hover:text-stone-700'
        }`}
        aria-label={tab.label}
      >
        <span className="flex-shrink-0">{tab.icon}</span>
        <span
          className={`overflow-hidden transition-[max-width,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] ${
            active
              ? 'max-w-[120px] opacity-100'
              : 'max-w-0 opacity-0 group-hover:max-w-[120px] group-hover:opacity-100'
          }`}
        >
          {tab.label}
        </span>
      </button>
    );
  });

  if (inline) {
    return (
      <nav
        className="flex items-center gap-1 border-t border-stone-200 bg-white/90 backdrop-blur-md px-2 py-1.5 overflow-x-auto scrollbar-hide"
        style={{ height: 56, flexShrink: 0 }}
      >
        {tabButtons}
      </nav>
    );
  }

  return (
    <div
      data-i
      style={{
        position: 'fixed',
        left,
        top,
        width,
        pointerEvents: isOpen ? 'auto' : 'none',
        opacity: isOpen ? 1 : 0,
        transform: isOpen ? 'translateY(0)' : 'translateY(8px)',
        transition: isOpen
          ? 'opacity 220ms cubic-bezier(0.16,1,0.3,1) 30ms, transform 220ms cubic-bezier(0.16,1,0.3,1) 30ms'
          : 'opacity 180ms ease, transform 180ms ease',
        zIndex: 9995,
      }}
    >
      <nav
        className="flex items-center gap-1 rounded-2xl border border-stone-200 bg-white/90 backdrop-blur-md shadow-lg px-2 py-1.5 overflow-x-auto scrollbar-hide"
        style={{ height: 56 }}
      >
        {tabButtons}
      </nav>
    </div>
  );
}
