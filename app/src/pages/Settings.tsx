import {
  Bell as BellIcon,
  Cpu as CpuIcon,
  KeyRound as KeyRoundIcon,
  Link2 as Link2Icon,
  MessagesSquare as MessagesSquareIcon,
  Mic as MicIcon,
  Monitor as MonitorIcon,
  ShieldCheck as ShieldCheckIcon,
  SlidersHorizontal as SlidersHorizontalIcon,
  Users as UsersIcon,
} from 'lucide-react';
import type { ReactNode } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';

import AboutPanel from '../components/settings/panels/AboutPanel';
import AgentChatPanel from '../components/settings/panels/AgentChatPanel';
import AIPanel from '../components/settings/panels/AIPanel';
import AutocompleteDebugPanel from '../components/settings/panels/AutocompleteDebugPanel';
import AutocompletePanel from '../components/settings/panels/AutocompletePanel';
import BillingPanel from '../components/settings/panels/BillingPanel';
import ComposioPanel from '../components/settings/panels/ComposioPanel';
import ComposioTriagePanel from '../components/settings/panels/ComposioTriagePanel';
import ConnectionsPanel from '../components/settings/panels/ConnectionsPanel';
import CronJobsPanel from '../components/settings/panels/CronJobsPanel';
import DeveloperOptionsPanel from '../components/settings/panels/DeveloperOptionsPanel';
import LocalModelDebugPanel from '../components/settings/panels/LocalModelDebugPanel';
import MascotPanel from '../components/settings/panels/MascotPanel';
import MemoryDataPanel from '../components/settings/panels/MemoryDataPanel';
import MemoryDebugPanel from '../components/settings/panels/MemoryDebugPanel';
import MessagingPanel from '../components/settings/panels/MessagingPanel';
import NotificationRoutingPanel from '../components/settings/panels/NotificationRoutingPanel';
import NotificationsPanel from '../components/settings/panels/NotificationsPanel';
import PrivacyPanel from '../components/settings/panels/PrivacyPanel';
import RecoveryPhrasePanel from '../components/settings/panels/RecoveryPhrasePanel';
import ScreenAwarenessDebugPanel from '../components/settings/panels/ScreenAwarenessDebugPanel';
import ScreenIntelligencePanel from '../components/settings/panels/ScreenIntelligencePanel';
import TeamInvitesPanel from '../components/settings/panels/TeamInvitesPanel';
import TeamManagementPanel from '../components/settings/panels/TeamManagementPanel';
import TeamMembersPanel from '../components/settings/panels/TeamMembersPanel';
import TeamPanel from '../components/settings/panels/TeamPanel';
import ToolsPanel from '../components/settings/panels/ToolsPanel';
import VoiceDebugPanel from '../components/settings/panels/VoiceDebugPanel';
import VoicePanel from '../components/settings/panels/VoicePanel';
import WebhooksDebugPanel from '../components/settings/panels/WebhooksDebugPanel';
import SettingsHome from '../components/settings/SettingsHome';
import SettingsSectionPage from '../components/settings/SettingsSectionPage';
import Icon from '../components/ui/Icon';
import { useT } from '../lib/i18n/I18nContext';
import { APP_VERSION } from '../utils/config';
import Intelligence from './Intelligence';
import Webhooks from './Webhooks';

const buildAccountSettingsItems = (t: (key: string) => string) => [
  {
    id: 'recovery-phrase',
    title: t('settings.nav.recovery_phrase'),
    description: t('settings.nav.recovery_phrase.desc'),
    route: 'recovery-phrase',
    icon: <Icon icon={KeyRoundIcon} size="md" />,
  },
  {
    id: 'team',
    title: t('settings.nav.team'),
    description: t('settings.nav.team.desc'),
    route: 'team',
    icon: <Icon icon={UsersIcon} size="md" />,
  },
  {
    id: 'connections',
    title: t('settings.nav.connections'),
    description: t('settings.nav.connections.desc'),
    route: 'connections',
    icon: <Icon icon={Link2Icon} size="md" />,
  },
  {
    id: 'privacy',
    title: t('settings.nav.privacy'),
    description: t('settings.nav.privacy.desc'),
    route: 'privacy',
    icon: <Icon icon={ShieldCheckIcon} size="md" />,
  },
];

const buildFeaturesSettingsItems = (t: (key: string) => string) => [
  {
    id: 'screen-intelligence',
    title: t('settings.nav.screen_intelligence'),
    description: t('settings.nav.screen_intelligence.desc'),
    route: 'screen-intelligence',
    icon: <Icon icon={MonitorIcon} size="md" />,
  },
  // Autocomplete + Voice Dictation hidden per #717 (routes retained for re-enable).
  {
    id: 'messaging',
    title: t('settings.nav.messaging'),
    description: t('settings.nav.messaging.desc'),
    route: 'messaging',
    icon: <Icon icon={MessagesSquareIcon} size="md" />,
  },
  {
    id: 'notifications',
    title: t('settings.nav.notifications'),
    description: t('settings.nav.notifications.desc'),
    route: 'notifications',
    icon: <Icon icon={BellIcon} size="md" />,
  },
  {
    id: 'tools',
    title: t('settings.nav.tools'),
    description: t('settings.nav.tools.desc'),
    route: 'tools',
    icon: <Icon icon={SlidersHorizontalIcon} size="md" />,
  },
];

const buildAiSettingsItems = (t: (key: string) => string) => [
  {
    id: 'llm',
    title: t('settings.nav.llm'),
    description: t('settings.nav.llm.desc'),
    route: 'llm',
    icon: <Icon icon={CpuIcon} size="md" />,
  },
  {
    id: 'voice',
    title: t('settings.nav.voice'),
    description: t('settings.nav.voice.desc'),
    route: 'voice',
    icon: <Icon icon={MicIcon} size="md" />,
  },
];

const WrappedSettingsPage = ({ children }: { children: ReactNode }) => {
  return (
    <div className="p-4 pt-6">
      <div className="mx-auto max-w-lg overflow-hidden rounded-2xl border border-line-default bg-surface-base shadow-soft">
        {children}
      </div>
    </div>
  );
};

function wrapSettingsPage(element: ReactNode, footerText: string) {
  return (
    <WrappedSettingsPage>
      {element}
      <div className="border-t border-line-default px-4 py-3 text-center text-[11px] text-ink-tertiary">
        {footerText}
      </div>
    </WrappedSettingsPage>
  );
}

const Settings = () => {
  const { t } = useT();
  const accountSettingsItems = buildAccountSettingsItems(t);
  const featuresSettingsItems = buildFeaturesSettingsItems(t);
  const aiSettingsItems = buildAiSettingsItems(t);
  const footerText = t('settings.page.footer_beta_build', { version: APP_VERSION });
  const wrap = (element: ReactNode) => wrapSettingsPage(element, footerText);
  return (
    <div>
      <Routes>
        <Route index element={wrap(<SettingsHome />)} />
        <Route
          path="account"
          element={wrap(
            <SettingsSectionPage
              title={t('settings.page.account_title')}
              description={t('settings.page.account_description')}
              items={accountSettingsItems}
            />
          )}
        />
        <Route
          path="features"
          element={wrap(
            <SettingsSectionPage
              title={t('settings.page.features_title')}
              description={t('settings.page.features_description')}
              items={featuresSettingsItems}
            />
          )}
        />
        <Route
          path="ai"
          element={wrap(
            <SettingsSectionPage
              title={t('settings.page.ai_title')}
              description={t('settings.page.ai_description')}
              items={aiSettingsItems}
            />
          )}
        />
        {/* Account & Billing leaf panels */}
        <Route path="recovery-phrase" element={wrap(<RecoveryPhrasePanel />)} />
        <Route path="team" element={wrap(<TeamPanel />)} />
        <Route path="team/manage/:teamId" element={wrap(<TeamManagementPanel />)} />
        <Route path="team/manage/:teamId/members" element={wrap(<TeamMembersPanel />)} />
        <Route path="team/manage/:teamId/invites" element={wrap(<TeamInvitesPanel />)} />
        <Route path="team/members" element={wrap(<TeamMembersPanel />)} />
        <Route path="team/invites" element={wrap(<TeamInvitesPanel />)} />
        <Route path="connections" element={wrap(<ConnectionsPanel />)} />
        {/* BillingPanel intentionally uses its own wider layout. */}
        <Route path="billing" element={<BillingPanel />} />
        <Route path="privacy" element={wrap(<PrivacyPanel />)} />
        {/* Features leaf panels */}
        <Route path="screen-intelligence" element={wrap(<ScreenIntelligencePanel />)} />
        <Route path="autocomplete" element={wrap(<AutocompletePanel />)} />
        <Route path="voice" element={wrap(<VoicePanel />)} />
        <Route path="messaging" element={wrap(<MessagingPanel />)} />
        <Route path="notifications" element={wrap(<NotificationsPanel />)} />
        <Route path="mascot" element={wrap(<MascotPanel />)} />
        <Route path="tools" element={wrap(<ToolsPanel />)} />
        {/* Developer Options */}
        <Route path="developer-options" element={wrap(<DeveloperOptionsPanel />)} />
        <Route path="notification-routing" element={wrap(<NotificationRoutingPanel />)} />
        <Route path="llm" element={wrap(<AIPanel />)} />
        <Route path="agent-chat" element={wrap(<AgentChatPanel />)} />
        <Route path="cron-jobs" element={wrap(<CronJobsPanel />)} />
        <Route path="screen-awareness-debug" element={wrap(<ScreenAwarenessDebugPanel />)} />
        <Route path="autocomplete-debug" element={wrap(<AutocompleteDebugPanel />)} />
        <Route path="voice-debug" element={wrap(<VoiceDebugPanel />)} />
        <Route path="local-model-debug" element={wrap(<LocalModelDebugPanel />)} />
        <Route path="webhooks-debug" element={wrap(<WebhooksDebugPanel />)} />
        <Route path="memory-data" element={wrap(<MemoryDataPanel />)} />
        <Route path="memory-debug" element={wrap(<MemoryDebugPanel />)} />
        <Route path="intelligence" element={<Intelligence />} />
        <Route path="webhooks-triggers" element={<Webhooks />} />
        <Route path="composio-triggers" element={wrap(<ComposioTriagePanel />)} />
        <Route path="composio-routing" element={wrap(<ComposioPanel />)} />
        {/* About / updates */}
        <Route path="about" element={wrap(<AboutPanel />)} />
        {/* Fallback */}
        <Route path="*" element={<Navigate to="/settings" replace />} />
      </Routes>
    </div>
  );
};

export default Settings;
