import * as RadixTooltip from '@radix-ui/react-tooltip';
import {
  Blocks as BlocksIcon,
  Brain as BrainIcon,
  Gift as GiftIcon,
  Home as HomeIcon,
  type LucideIcon,
  MessageSquare as MessageSquareIcon,
  Settings as SettingsIcon,
  UserRound as UserRoundIcon,
  Video as VideoIcon,
} from 'lucide-react';
import { useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';

import { useT } from '../lib/i18n/I18nContext';
// [#1123] Commented out — welcome-agent onboarding replaced by Joyride walkthrough
// import { isWelcomeLocked } from '../lib/coreState/store';
import { useCoreState } from '../providers/CoreStateProvider';
import { useAppSelector } from '../store/hooks';
import { selectUnreadCount } from '../store/notificationSlice';
import { isAccountsFullscreen } from '../utils/accountsFullscreen';
import Badge from './ui/Badge';
import Icon from './ui/Icon';

interface TabDef {
  id: string;
  labelKey: string;
  path: string;
  icon: LucideIcon;
}

const TABS: TabDef[] = [
  { id: 'home', labelKey: 'nav.home', path: '/home', icon: HomeIcon },
  { id: 'human', labelKey: 'nav.human', path: '/human', icon: UserRoundIcon },
  { id: 'chat', labelKey: 'nav.chat', path: '/chat', icon: MessageSquareIcon },
  { id: 'meetings', labelKey: 'nav.meetings', path: '/meetings', icon: VideoIcon },
  { id: 'skills', labelKey: 'nav.connections', path: '/skills', icon: BlocksIcon },
  { id: 'intelligence', labelKey: 'nav.memory', path: '/intelligence', icon: BrainIcon },
  // Alerts/Notifications used to be its own bottom tab; moved into
  // Settings › Notifications since it's a low-traffic destination.
  // The /notifications route still exists for deep links.
  { id: 'rewards', labelKey: 'nav.rewards', path: '/rewards', icon: GiftIcon },
  { id: 'settings', labelKey: 'nav.settings', path: '/settings', icon: SettingsIcon },
];

/**
 * Floating pill navigation (UI-kit build). Active tab gets a brand-tinted
 * pill that expands to show its label; inactive tabs show the label on
 * hover/focus. Every tab carries a tooltip and the unread badge lives on
 * the notifications surface via Badge.
 */
const BottomTabBar = () => {
  const { t } = useT();
  const tabs = useMemo(() => TABS.map(tab => ({ ...tab, label: t(tab.labelKey) })), [t]);
  const location = useLocation();
  const navigate = useNavigate();
  const { snapshot } = useCoreState();
  const token = snapshot.sessionToken;
  const [revealed, setRevealed] = useState(false);

  const activeAccountId = useAppSelector(state => state.accounts.activeAccountId);
  const unreadCount = useAppSelector(state => selectUnreadCount(state.notifications.items));

  const hiddenPaths = ['/', '/login'];
  if (
    !token ||
    hiddenPaths.some(path => location.pathname === path || location.pathname.startsWith(`${path}/`))
  ) {
    return null;
  }

  // On /accounts we want as much real estate as possible for the embedded
  // webview — but *only* when a real account (WhatsApp, …) is selected.
  // The Agent entry keeps the tab bar visible so chatting with the agent
  // feels like a normal page. A thin hover strip along the bottom lets
  // the user reveal the bar manually even in fullscreen mode.
  const fullscreen = isAccountsFullscreen(location.pathname, activeAccountId);
  const collapsed = fullscreen && !revealed;

  const isActive = (path: string) => {
    if (path === '/chat') return location.pathname.startsWith('/chat');
    if (path === '/meetings') return location.pathname.startsWith('/meetings');
    if (path === '/settings/cron-jobs') return location.pathname.startsWith('/settings/cron-jobs');
    if (path === '/settings/messaging') return location.pathname.startsWith('/settings/messaging');
    if (path === '/settings')
      return (
        location.pathname === '/settings' ||
        (location.pathname.startsWith('/settings/') &&
          !location.pathname.startsWith('/settings/cron-jobs') &&
          !location.pathname.startsWith('/settings/messaging'))
      );
    if (path === '/home') return location.pathname === '/home';
    return location.pathname === path;
  };

  return (
    <div className="absolute inset-x-0 bottom-0 z-nav">
      {/* Hover strip — only matters when collapsed; provides a 12px bottom
          edge the user can mouse into to reveal the bar again. */}
      {collapsed && (
        <div
          className="pointer-events-auto absolute inset-x-0 bottom-0 h-3"
          onMouseEnter={() => setRevealed(true)}
          aria-hidden
        />
      )}
      <div
        className={`pointer-events-none flex justify-center px-4 pb-4 pt-2 transition-transform duration-base ease-spring ${
          collapsed ? 'translate-y-[calc(100%+8px)]' : 'translate-y-0'
        }`}
        onMouseLeave={() => setRevealed(false)}
        onFocus={() => setRevealed(true)}
        onBlur={e => {
          if (!e.currentTarget.contains(e.relatedTarget as Node)) setRevealed(false);
        }}>
        <RadixTooltip.Provider delayDuration={400}>
          <nav className="pointer-events-auto inline-flex items-center gap-0.5 rounded-full border border-line-default bg-surface-base/85 p-1 shadow-medium backdrop-blur">
            {tabs.map(tab => {
              const active = isActive(tab.path);
              const showBadge = tab.id === 'notifications' && unreadCount > 0;
              // data-walkthrough attributes for the Joyride walkthrough steps.
              const walkthroughAttr: Record<string, string> = {
                chat: 'tab-chat',
                skills: 'tab-skills',
                notifications: 'tab-notifications',
                settings: 'tab-settings',
              };
              const button = (
                <button
                  data-walkthrough={walkthroughAttr[tab.id]}
                  onClick={() => navigate(tab.path)}
                  className={`group relative flex items-center rounded-full px-2.5 py-2 text-sm select-none transition-all duration-base ease-spring active:scale-95 cursor-pointer ${
                    active
                      ? 'bg-primary-50 font-semibold text-primary-700'
                      : 'text-ink-tertiary hover:bg-surface-sunken hover:text-ink-primary'
                  }`}
                  aria-label={
                    tab.id === 'notifications' && unreadCount > 0
                      ? `${tab.label} (${unreadCount} ${t('alerts.unread')})`
                      : tab.label
                  }>
                  <span className="relative inline-flex flex-shrink-0">
                    <Icon icon={tab.icon} size="sm" />
                    {showBadge && (
                      <Badge
                        variant="danger"
                        className="absolute -top-1.5 -right-2 h-3.5 min-w-3.5 justify-center p-0 px-0.5 text-[9px] leading-none font-bold text-ink-inverse">
                        {unreadCount > 9 ? '9+' : unreadCount}
                      </Badge>
                    )}
                  </span>
                  <span
                    className={`overflow-hidden whitespace-nowrap transition-all duration-base ease-spring ${
                      active
                        ? 'ml-2 max-w-40 opacity-100'
                        : 'ml-0 max-w-0 opacity-0 group-hover:ml-2 group-hover:max-w-40 group-hover:opacity-100 group-focus-visible:ml-2 group-focus-visible:max-w-40 group-focus-visible:opacity-100'
                    }`}>
                    {tab.label}
                  </span>
                </button>
              );

              return (
                <RadixTooltip.Root key={tab.id}>
                  <RadixTooltip.Trigger asChild>{button}</RadixTooltip.Trigger>
                  <RadixTooltip.Portal>
                    <RadixTooltip.Content
                      side="top"
                      sideOffset={10}
                      className={
                        'z-toast rounded-md border border-line-default bg-surface-base px-2 py-1 ' +
                        'text-xs text-ink-primary shadow-medium'
                      }>
                      {tab.label}
                      <RadixTooltip.Arrow className="fill-line-default" />
                    </RadixTooltip.Content>
                  </RadixTooltip.Portal>
                </RadixTooltip.Root>
              );
            })}
          </nav>
        </RadixTooltip.Provider>
      </div>
    </div>
  );
};

export default BottomTabBar;
