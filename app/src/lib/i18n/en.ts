import type { TranslationMap } from './types';

const en: TranslationMap = {
  // Navigation
  'nav.home': 'Home',
  'nav.human': 'Human',
  'nav.chat': 'Chat',
  'nav.connections': 'Connections',
  'nav.memory': 'Intelligence',
  'nav.alerts': 'Alerts',
  'nav.rewards': 'Rewards',
  'nav.settings': 'Settings',
  'nav.meetings': 'Meetings',

  // Common
  'common.cancel': 'Cancel',
  'common.save': 'Save',
  'common.confirm': 'Confirm',
  'common.delete': 'Delete',
  'common.edit': 'Edit',
  'common.create': 'Create',
  'common.search': 'Search',
  'common.loading': 'loading…',
  'common.error': 'Error',
  'common.success': 'Success',
  'common.back': 'Back',
  'common.next': 'Next',
  'common.finish': 'Finish',
  'common.close': 'Close',
  'common.enabled': 'Enabled',
  'common.disabled': 'Disabled',
  'common.on': 'On',
  'common.off': 'Off',
  'common.yes': 'Yes',
  'common.no': 'No',
  'common.ok': 'Got it',
  'common.retry': 'Retry',
  'common.copy': 'Copy',
  'common.copied': 'Copied!',
  'common.learnMore': 'Learn more',
  'common.seeAll': 'View',
  'common.dismiss': 'Dismiss',
  'common.clear': 'Clear',
  'common.reset': 'Reset',
  'common.refresh': 'Refresh',
  'common.export': 'Export',
  'common.import': 'Import',
  'common.upload': 'Upload',
  'common.download': 'Download',
  'common.add': 'Add',
  'common.remove': 'Remove',
  'common.showMore': 'Show more',
  'common.showLess': 'Show less',
  'common.submit': 'Submit',
  'common.continue': 'Continue',

  // Settings Home
  'settings.general': 'General',
  'settings.featuresAndAI': 'Features & AI',
  'settings.billingAndRewards': 'Billing & Rewards',
  'settings.support': 'Support',
  'settings.advanced': 'Advanced',
  'settings.dangerZone': 'Danger Zone',
  'settings.account': 'Account',
  'settings.accountDesc': 'Recovery phrase, team, connections, and privacy',
  'settings.notifications': 'Notifications',
  'settings.notificationsDesc': 'Do Not Disturb and per-account notification controls',
  'settings.features': 'Features',
  'settings.featuresDesc': 'Screen awareness, messaging, and tools',
  'settings.aiModels': 'AI & Models',
  'settings.aiModelsDesc': 'Local AI model setup, downloads, and LLM provider',
  'settings.ai': 'AI Configuration',
  'settings.aiDesc': 'Cloud providers, local Ollama models, and per-workload routing',
  'settings.billingUsage': 'Billing & Usage',
  'settings.billingUsageDesc': 'Subscription plan, credits, and payment methods',
  'settings.rewards': 'Rewards',
  'settings.rewardsDesc': 'Referrals, coupons, and earned credits',
  'settings.restartTour': 'Restart Tour',
  'settings.restartTourDesc': 'Replay the product walkthrough from the beginning',
  'settings.about': 'About',
  'settings.aboutDesc': 'App version and software updates',
  'settings.developerOptions': 'Advanced',
  'settings.developerOptionsDesc':
    'AI configuration, messaging channels, tools, diagnostics, and debug panels',
  'settings.clearAppData': 'Clear App Data',
  'settings.clearAppDataDesc': 'Sign out and permanently clear all local app data',
  'settings.logOut': 'Log out',
  'settings.logOutDesc': 'Sign out of your account',
  'settings.language': 'Language',
  'settings.languageDesc': 'Display language for the app interface',
  'settings.alerts': 'Alerts',
  'settings.alertsDesc': 'View recent alerts and activity in your inbox',

  // Settings: Account
  'settings.account.recoveryPhrase': 'Recovery Phrase',
  'settings.account.recoveryPhraseDesc': 'View and back up your account recovery phrase',
  'settings.account.team': 'Team',
  'settings.account.teamDesc': 'Manage team members and permissions',
  'settings.account.connections': 'Connections',
  'settings.account.connectionsDesc': 'Manage linked accounts and services',
  'settings.account.privacy': 'Privacy',
  'settings.account.privacyDesc': 'Control what data leaves your computer',

  // Settings: Notifications
  'settings.notifications.doNotDisturb': 'Do Not Disturb',
  'settings.notifications.doNotDisturbDesc': 'Pause all notifications for a set period',
  'settings.notifications.channelControls': 'Per-Channel Controls',
  'settings.notifications.channelControlsDesc':
    'Configure notification preferences for each channel',

  // Settings: Features
  'settings.features.screenAwareness': 'Screen Awareness',
  'settings.features.screenAwarenessDesc': 'Let the assistant see your active window',
  'settings.features.messaging': 'Messaging',
  'settings.features.messagingDesc': 'Channel and messaging integration settings',
  'settings.features.tools': 'Tools',
  'settings.features.toolsDesc': 'Manage connected tools and integrations',

  // Settings: AI & Models
  'settings.ai.localSetup': 'Local AI Setup',
  'settings.ai.localSetupDesc': 'Download and configure local AI models',
  'settings.ai.llmProvider': 'LLM Provider',
  'settings.ai.llmProviderDesc': 'Choose and configure your AI provider',

  // Clear App Data modal
  'clearData.title': 'Clear App Data',
  'clearData.warning': 'This will sign you out and permanently delete local app data including:',
  'clearData.bulletSettings': 'App settings and conversations',
  'clearData.bulletCache': 'All local integration cache data',
  'clearData.bulletWorkspace': 'Workspace data',
  'clearData.bulletOther': 'All other local data',
  'clearData.irreversible': 'This action cannot be undone.',
  'clearData.clearing': 'Clearing App Data...',
  'clearData.failed': 'Failed to clear data and logout. Please try again.',
  'clearData.failedLogout': 'Failed to log out. Please try again.',
  'clearData.failedPersist': 'Failed to clear persisted app state. Please try again.',

  // Welcome page
  'welcome.title': 'Welcome to Aura',
  'welcome.subtitle':
    'Your personal AI super intelligence. Private, simple and extremely powerful.',
  'welcome.connectPrompt': 'Configure RPC URL (Advanced)',
  'welcome.selectRuntime': 'Select a Runtime',
  'welcome.urlPlaceholder': 'http://localhost:8089',
  'welcome.invalidUrl': 'Please enter a valid HTTP or HTTPS URL',
  'welcome.connecting': 'Testing',
  'welcome.connect': 'Test',

  // Home page
  'home.greeting': 'Good morning',
  'home.greetingAfternoon': 'Good afternoon',
  'home.greetingEvening': 'Good evening',
  'home.askAssistant': 'Ask your assistant anything...',
  'home.statusOk':
    'Your device is connected. Keep the app running to keep the connection alive. Message your agent with the button below.',
  'home.statusBackendOnly': 'Reconnecting to backend… your agent will be available again shortly.',
  'home.statusCoreUnreachable':
    "Local core sidecar isn't responding. The Aura background process may have crashed or failed to start.",
  'home.statusInternetOffline':
    'Your device is offline right now. Check your network or restart the app to reconnect.',
  'home.restartCore': 'Restart Core',
  'home.restartingCore': 'Restarting core…',

  // Chat / Conversations
  'chat.newThread': 'New thread',
  'chat.typeMessage': 'Type a message...',
  'chat.send': 'Send message',
  'chat.thinking': 'Thinking...',
  'chat.noMessages': 'No messages yet',
  'chat.startConversation': 'Start a conversation',
  'chat.regenerate': 'Regenerate',
  'chat.copyResponse': 'Copy response',
  'chat.citations': 'Citations',
  'chat.toolUsed': 'Tool used',

  // Skills / Connections
  'scope.legacy': 'Legacy',
  'scope.user': 'User',
  'scope.project': 'Project',
  'skills.title': 'Connections',
  'skills.search': 'Search connections...',
  'skills.noResults': 'No connections found',
  'skills.connect': 'Connect',
  'skills.disconnect': 'Disconnect',
  'skills.configure': 'Manage',
  'skills.connected': 'Connected',
  'skills.available': 'Available',
  'skills.addAccount': 'Add Account',
  'skills.channels': 'Channels',
  'skills.integrations': 'Integrations',

  // Intelligence / Memory
  'memory.title': 'Memory',
  'memory.search': 'Search memories...',
  'memory.noResults': 'No memories found',
  'memory.empty': 'No memories yet. Memories are created automatically as you interact.',
  'memory.tab.memory': 'Memory',
  'memory.tab.subconscious': 'Subconscious',
  'memory.tab.dreams': 'Dreams',
  'memory.tab.calls': 'Calls',
  'memory.tab.settings': 'Settings',
  'memory.analyzeNow': 'Analyze Now',

  // Notifications / Alerts
  'alerts.title': 'Alerts',
  'alerts.empty': 'No alerts yet',
  'alerts.markAllRead': 'Mark all as read',
  'alerts.unread': 'unread',

  // Rewards
  'rewards.title': 'Rewards',
  'rewards.referrals': 'Referrals',
  'rewards.coupons': 'Redeem',
  'rewards.credits': 'Credits',
  'rewards.referralCode': 'Your referral code',
  'rewards.copyCode': 'Copy code',
  'rewards.share': 'Share',

  // Onboarding
  'onboarding.welcome': "Hi. I'm Aura.",
  'onboarding.welcomeDesc':
    'Your super-intelligent AI assistant that runs on your computer. Private, simple, and extremely powerful.',
  'onboarding.context': 'Context Gathering',
  'onboarding.contextDesc': 'Connect the tools and services you use every day.',
  'onboarding.localAI': 'Local AI',
  'onboarding.localAIDesc': 'Set up a local AI model that runs on your machine.',
  'onboarding.chatProvider': 'Chat Provider',
  'onboarding.chatProviderDesc': 'Choose how you want to interact with your assistant.',
  'onboarding.referral': 'Referral',
  'onboarding.referralDesc': 'Apply a referral code if you have one.',
  'onboarding.finish': 'Finish Setup',
  'onboarding.finishDesc': "You're all set! Start using Aura.",
  'onboarding.skip': 'Skip',
  'onboarding.getStarted': 'Get Started',

  // Onboarding: runtime-choice step (Cloud vs Custom)
  'onboarding.runtimeChoice.title': 'How would you like to run Aura?',
  'onboarding.runtimeChoice.subtitle':
    'Pick the setup that fits you best. You can change this later in Settings.',
  'onboarding.runtimeChoice.cloud.title': 'Simple',
  'onboarding.runtimeChoice.cloud.tagline': 'Let Aura manage everything for you.',
  'onboarding.runtimeChoice.cloud.f1': 'Built-in security',
  'onboarding.runtimeChoice.cloud.f2': 'Token compression to stretch your usage further',
  'onboarding.runtimeChoice.cloud.f3': 'One subscription, every model included',
  'onboarding.runtimeChoice.cloud.f4': 'No API keys to manage',
  'onboarding.runtimeChoice.cloud.f5': 'Simple to set up',
  'onboarding.runtimeChoice.custom.title': 'Run Custom',
  'onboarding.runtimeChoice.custom.tagline':
    "Bring your own keys. Full control of what you're using.",
  'onboarding.runtimeChoice.custom.f1': "You'll need API keys for almost everything",
  'onboarding.runtimeChoice.custom.f2': 'Reuses services you already pay for',
  'onboarding.runtimeChoice.custom.f3': 'Can be free if you run everything locally',
  'onboarding.runtimeChoice.custom.f4': 'More setup, more knobs',
  'onboarding.runtimeChoice.custom.f5': 'Best for power users and developers',
  'onboarding.runtimeChoice.cloud.creditHighlight': '$1 free credit to try it out',
  'onboarding.runtimeChoice.continueCloud': 'Continue with Simple',
  'onboarding.runtimeChoice.continueCustom': 'Continue with Custom',
  'onboarding.runtimeChoice.recommended': 'Recommended',

  // Onboarding: API keys step (only when Custom is picked)
  'onboarding.apiKeys.title': "Let's Add Your API Keys",
  'onboarding.apiKeys.subtitle':
    'You can paste them now or skip and add them later in Settings › AI. Keys are stored on this device, encrypted at rest.',
  'onboarding.apiKeys.openaiLabel': 'OpenAI API key',
  'onboarding.apiKeys.openaiPlaceholder': 'sk-...',
  'onboarding.apiKeys.anthropicLabel': 'Anthropic API key',
  'onboarding.apiKeys.anthropicPlaceholder': 'sk-ant-...',
  'onboarding.apiKeys.saveError': "Couldn't save that key. Please double-check it and try again.",
  'onboarding.apiKeys.skipForNow': 'Skip for now',
  'onboarding.apiKeys.continue': 'Save and continue',
  'onboarding.apiKeys.saving': 'Saving…',

  // Onboarding: Custom wizard (Inference / Voice / OAuth / Search / Memory)
  'onboarding.custom.stepperInference': 'Inference',
  'onboarding.custom.stepperVoice': 'Voice',
  'onboarding.custom.stepperOAuth': 'OAuth',
  'onboarding.custom.stepperSearch': 'Search',
  'onboarding.custom.stepperMemory': 'Memory',
  'onboarding.custom.stepCounter': 'Step {n} of {total}',
  'onboarding.custom.defaultTitle': 'Default',
  'onboarding.custom.defaultSubtitle': 'Let Aura manage it for you.',
  'onboarding.custom.configureTitle': 'Configure',
  'onboarding.custom.configureSubtitle': "I'll pick what to use.",
  'onboarding.custom.progressAriaLabel': 'Onboarding progress',
  'onboarding.custom.continue': 'Continue',
  'onboarding.custom.back': 'Back',
  'onboarding.custom.finish': 'Finish Setup',
  'onboarding.custom.configureLater':
    "You can finish wiring this up after onboarding. We'll drop you on the matching Settings page once you're done.",
  'onboarding.custom.openSettings': 'Open in Settings',

  // Onboarding: Custom > Inference (text)
  'onboarding.custom.inference.title': 'Inference (Text)',
  'onboarding.custom.inference.subtitle':
    'Which language model should answer your questions and run your agents?',
  'onboarding.custom.inference.defaultDesc':
    'Aura routes every workload to a sensible default model. No keys, no setup.',
  'onboarding.custom.inference.configureDesc':
    'Bring your own OpenAI or Anthropic key. We use it for every text-based workload.',

  // Onboarding: Custom > Voice
  'onboarding.custom.voice.title': 'Voice',
  'onboarding.custom.voice.subtitle': 'Speech-to-text and text-to-speech for voice mode.',
  'onboarding.custom.voice.defaultDesc':
    'Aura ships with managed STT/TTS that just works. Nothing to wire up.',
  'onboarding.custom.voice.configureDesc':
    'Use your own ElevenLabs / OpenAI Whisper / etc. Configure in Settings › Voice.',

  // Onboarding: Custom > OAuth (Composio)
  'onboarding.custom.oauth.title': 'Connections (OAuth)',
  'onboarding.custom.oauth.subtitle':
    'Gmail, Slack, Notion, and other connected services that need OAuth.',
  'onboarding.custom.oauth.defaultDesc':
    'Aura runs a managed Composio workspace. One click to connect each service later.',
  'onboarding.custom.oauth.configureDesc':
    'Bring your own Composio account / API key. Configure in Settings › Connections.',

  // Onboarding: Custom > Search
  'onboarding.custom.search.title': 'Web Search',
  'onboarding.custom.search.subtitle': 'How Aura searches the web on your behalf.',
  'onboarding.custom.search.defaultDesc':
    'Aura uses a managed search backend. No keys needed.',
  'onboarding.custom.search.configureDesc':
    'Bring your own search provider key (Tavily, Brave, etc.). Configure in Settings › Tools.',

  // Onboarding: Custom > Memory
  'onboarding.custom.memory.title': 'Memory',
  'onboarding.custom.memory.subtitle':
    'How Aura remembers your context, preferences, and prior conversations.',
  'onboarding.custom.memory.defaultDesc':
    'Aura manages memory storage and retrieval automatically. Nothing to set up.',
  'onboarding.custom.memory.configureDesc':
    'Inspect, export, or wipe memory yourself. Configure in Settings › Memory.',

  // Accounts
  'accounts.addAccount': 'Add Account',
  'accounts.manageAccounts': 'Manage Accounts',
  'accounts.noAccounts': 'No accounts connected',
  'accounts.connectAccount': 'Connect an account to get started',
  'accounts.agent': 'Agent',
  'accounts.respondQueue': 'Respond Queue',
  'accounts.disconnect': 'Disconnect',
  'accounts.disconnectConfirm': 'Are you sure you want to disconnect this account?',
  'accounts.searchAccounts': 'Search accounts...',

  // Channels
  'channels.title': 'Channels',
  'channels.configure': 'Configure Channel',
  'channels.setup': 'Setup',
  'channels.noChannels': 'No channels configured',
  'channels.addChannel': 'Add Channel',
  'channels.status.connected': 'Connected',
  'channels.status.disconnected': 'Disconnected',
  'channels.status.error': 'Error',
  'channels.status.configuring': 'Configuring',
  'channels.defaultMessaging': 'Default Messaging Channel',

  // Webhooks
  'webhooks.title': 'Webhooks',
  'webhooks.create': 'Create Webhook',
  'webhooks.noWebhooks': 'No webhooks configured',
  'webhooks.url': 'URL',
  'webhooks.secret': 'Secret',
  'webhooks.events': 'Events',
  'webhooks.archiveDirectory': 'Archive Directory',
  'webhooks.todayFile': "Today's File",

  // Invites
  'invites.title': 'Invites',
  'invites.create': 'Create Invite',
  'invites.noInvites': 'No pending invites',
  'invites.code': 'Invite Code',
  'invites.copyLink': 'Copy Link',

  // Developer Options
  'devOptions.title': 'Advanced',
  'devOptions.diagnostics': 'Diagnostics',
  'devOptions.diagnosticsDesc': 'System health, logs, and performance metrics',
  'devOptions.debugPanels': 'Debug Panels',
  'devOptions.debugPanelsDesc': 'Feature flags, state inspection, and debugging tools',
  'devOptions.webhooks': 'Webhooks',
  'devOptions.webhooksDesc': 'Configure and test webhook integrations',
  'devOptions.memoryInspection': 'Memory Inspection',
  'devOptions.memoryInspectionDesc': 'Browse, query, and manage memory entries',

  // Voice / Dictation
  'voice.pushToTalk': 'Push to Talk',
  'voice.recording': 'Recording...',
  'voice.processing': 'Processing...',
  'voice.languageHint': 'Language',

  // Misc
  'misc.rehydrating': 'Loading your data...',
  'misc.checkingServices': 'Checking services...',
  'misc.serviceUnavailable': 'Service Unavailable',
  'misc.somethingWentWrong': 'Something went wrong',
  'misc.tryAgainLater': 'Please try again later.',
  'misc.restartApp': 'Restart App',
  'misc.updateAvailable': 'Update Available',
  'misc.updateNow': 'Update Now',
  'misc.updateLater': 'Later',
  'misc.downloading': 'Downloading...',
  'misc.installing': 'Installing...',
  'misc.beta':
    'Aura is in early beta. Feel free to share feedback or report any bugs you run into — every report helps us ship faster.',
  'misc.betaFeedback': 'Send feedback',

  // Mnemonic / Recovery
  'mnemonic.title': 'Recovery Phrase',
  'mnemonic.warning': 'Write down these words in order and store them somewhere safe.',
  'mnemonic.copyWarning':
    'Never share your recovery phrase. Anyone with these words can access your account.',
  'mnemonic.copied': 'Recovery phrase copied to clipboard',
  'mnemonic.reveal': 'Reveal phrase',
  'mnemonic.hidden': 'Recovery phrase is hidden',

  // What Leaves My Computer
  'privacy.title': 'Privacy & Security',
  'privacy.description': 'Transparency report of data sent to external services.',
  'privacy.empty': 'No external data transfers detected.',
  'privacy.whatLeavesComputer': 'What leaves your computer',
  'privacy.loading': 'Loading privacy details...',
  'privacy.loadError': 'Could not load the live privacy list. Analytics controls below still work.',
  'privacy.noCapabilities': 'No capabilities currently disclose data movement.',
  'privacy.sentTo': 'Sent to',
  'privacy.leavesDevice': 'Leaves device',
  'privacy.staysLocal': 'Stays local',
  'privacy.anonymizedAnalytics': 'Anonymized Analytics',
  'privacy.shareAnonymizedData': 'Share Anonymized Usage Data',
  'privacy.shareAnonymizedDataDesc':
    'Help improve Aura by sharing anonymous crash reports and usage analytics. All data is fully anonymized — no personal data, messages, wallet keys, or session information is ever collected.',
  'privacy.meetingFollowUps': 'Meeting follow-ups',
  'privacy.autoHandoffMeet': 'Auto-handoff Google Meet transcripts to the orchestrator',
  'privacy.autoHandoffMeetDesc':
    "When a Google Meet call ends, Aura's orchestrator can read the transcript and may take actions like drafting messages, scheduling follow-ups, or posting summaries to your connected Slack workspace. Off by default.",
  'privacy.analyticsDisclaimer':
    'All analytics and bug reports are fully anonymized. When enabled, we collect only crash information, device type, and the file location of errors. We never access your messages, session data, wallet keys, API keys, or any personally identifiable information. You can change this setting at any time.',

  // Settings: About
  'settings.about.version': 'Version',
  'settings.about.updateAvailable': 'is available',
  'settings.about.softwareUpdates': 'Software updates',
  'settings.about.lastChecked': 'Last checked',
  'settings.about.checking': 'Checking...',
  'settings.about.checkForUpdates': 'Check for updates',
  'settings.about.releases': 'Releases',
  'settings.about.releasesDesc': 'Browse release notes and earlier builds on GitHub.',
  'settings.about.openReleases': 'Open GitHub releases',

  // Settings: AI
  'settings.ai.overview': 'AI System Overview',
  'settings.ai.configStatus': 'Configuration Status',
  'settings.ai.fallbackMode': 'Fallback Mode',
  'settings.ai.loadedFromRuntime': 'Loaded from Runtime',
  'settings.ai.loadingDuration': 'Loading Duration',
  'settings.ai.localRuntime': 'Local Model Runtime',
  'settings.ai.openManager': 'Open Manager',
  'settings.ai.retryDownload': 'Retry Download',
  'settings.ai.state': 'State',
  'settings.ai.targetModel': 'Target Model',
  'settings.ai.download': 'Download',
  'settings.ai.localModelUnavailable': 'Local model status unavailable.',
  'settings.ai.soulConfig': 'SOUL Persona Configuration',
  'settings.ai.refreshing': 'Refreshing...',
  'settings.ai.refreshSoul': 'Refresh SOUL',
  'settings.ai.loadingSoul': 'Loading SOUL configuration...',
  'settings.ai.identity': 'Identity',
  'settings.ai.personality': 'Personality',
  'settings.ai.safetyRules': 'Safety Rules',
  'settings.ai.source': 'Source',
  'settings.ai.loaded': 'Loaded',
  'settings.ai.toolsConfig': 'TOOLS Configuration',
  'settings.ai.refreshTools': 'Refresh TOOLS',
  'settings.ai.toolsAvailable': 'Tools Available',
  'settings.ai.tools': 'tools',
  'settings.ai.activeSkills': 'Active Skills',
  'settings.ai.skills': 'skills',
  'settings.ai.skillsOverview': 'Skills Overview',
  'settings.ai.refreshingAll': 'Refreshing All...',
  'settings.ai.refreshAll': 'Refresh All AI Configuration',

  // Settings: Notifications
  'settings.notifications.suppressAll': 'Suppress all notifications',
  'settings.notifications.suppressAllDesc':
    'Block all OS notification toasts from embedded apps regardless of focus state.',
  'settings.notifications.toggleDnd': 'Toggle Do Not Disturb',
  'settings.notifications.categories': 'Categories',
  'settings.notifications.categoryFooter':
    'Disabling a category stops new notifications of that type from appearing in the notification center. Existing notifications remain until cleared.',

  // Settings: Billing
  'settings.billing.movedToWeb': 'Billing moved to the web',
  'settings.billing.openDashboard': 'Open billing dashboard',
  'settings.billing.movedToWebDesc':
    'Subscription changes, payment methods, credits, and invoices are now managed at TinyHumans on the web.',
  'settings.billing.backToSettings': 'Back to settings',
  'settings.billing.openingBrowser': 'Opening your browser...',
  'settings.billing.browserNotOpen': 'If your browser did not open, use the button above.',
  'settings.billing.browserOpenFailed':
    'The browser could not be opened automatically. Use the button above.',

  // Settings: Tools
  'settings.tools.chooseCapabilities':
    'Choose which capabilities Aura can use on your behalf.',
  'settings.tools.saveChanges': 'Save Changes',
  'settings.tools.preferencesSaved': 'Preferences saved',
  'settings.tools.saveFailed': 'Failed to save preferences. Try again.',

  // Settings: Screen Awareness
  'settings.screenAwareness.mode': 'Mode',
  'settings.screenAwareness.allExceptBlacklist': 'All Except Blacklist',
  'settings.screenAwareness.whitelistOnly': 'Whitelist Only',
  'settings.screenAwareness.screenMonitoring': 'Screen Monitoring',
  'settings.screenAwareness.saveSettings': 'Save Settings',
  'settings.screenAwareness.session': 'Session',
  'settings.screenAwareness.status': 'Status',
  'settings.screenAwareness.active': 'Active',
  'settings.screenAwareness.stopped': 'Stopped',
  'settings.screenAwareness.remaining': 'Remaining',
  'settings.screenAwareness.startSession': 'Start Session',
  'settings.screenAwareness.stopSession': 'Stop Session',
  'settings.screenAwareness.analyzeNow': 'Analyze Now',
  'settings.screenAwareness.macosOnly':
    'Screen Awareness desktop capture and permission controls are currently supported on macOS only.',

  // Connections
  'connections.comingSoon': 'Coming soon',
  'connections.setUp': 'Set up',
  'connections.configured': 'Configured',
  'connections.unavailable': 'Unavailable',
  'connections.checking': 'Checking…',
  'connections.walletConfigured':
    'Local EVM, BTC, Solana, and Tron identities are configured from your recovery phrase.',
  'connections.walletReady':
    'Set up local EVM, BTC, Solana, and Tron identities from one recovery phrase.',
  'connections.walletError':
    'Could not check wallet status. Tap to retry from the Recovery Phrase panel.',
  'connections.walletChecking': 'Checking wallet status...',
  'connections.walletIdentities': 'Wallet identities',
  'connections.walletDerived':
    'Derived locally from your recovery phrase and stored as safe metadata only.',
  'connections.privacySecurity': 'Privacy & Security',
  'connections.privacySecurityDesc':
    'All data and credentials are stored locally with zero-data retention policy. Your information is encrypted and never shared with third parties.',

  // Channels
  'channels.status.connecting': 'Connecting',
  'channels.status.notConfigured': 'Not configured',
  'channels.noActiveRoute': 'No active route',
  'channels.activeRoute': 'Active route',
  'channels.loadingDefinitions': 'Loading channel definitions...',
  'channels.channelConnections': 'Channel Connections',
  'channels.configureAuthModes': 'Configure auth modes for each messaging channel.',
  'channels.configNotAvailable': 'Configuration for',
  'channels.channel': 'channel',

  // Dev Options
  'devOptions.coreModeNotSet': 'Core mode: not set',
  'devOptions.coreModeNotSetDesc':
    "The boot-check picker hasn't been confirmed yet. Use Switch mode on the picker to choose Local or Cloud.",
  'devOptions.local': 'Local',
  'devOptions.embeddedCoreSidecar': 'Embedded core sidecar',
  'devOptions.sidecarSpawned': 'Spawned in-process by the Tauri shell on app launch.',
  'devOptions.cloud': 'Cloud',
  'devOptions.remoteCoreRpc': 'Remote core RPC',
  'devOptions.token': 'Token',
  'devOptions.tokenNotSet': 'not set — RPC will 401',
  'devOptions.triggerSentryTest': 'Trigger Sentry Test (staging)',
  'devOptions.triggerSentryTestDesc':
    'Fires a tagged error to verify the Sentry pipeline. Issue #1072 — remove after verification.',
  'devOptions.sendTestEvent': 'Send test event',
  'devOptions.sending': 'Sending…',
  'devOptions.eventSent': 'Event sent',
  'devOptions.failed': 'Failed',
  'devOptions.appLogs': 'App logs',
  'devOptions.appLogsDesc':
    'Open the folder containing rolling daily log files. Attach the most recent file when reporting an issue.',
  'devOptions.openLogsFolder': 'Open logs folder',

  // Mnemonic
  'mnemonic.phraseSaved': 'Recovery phrase saved',
  'mnemonic.walletReady': 'Multi-chain wallet identities are ready. Returning to settings...',
  'mnemonic.writeDownWords': 'Write down these',
  'mnemonic.wordsInOrder':
    'words in order and store them somewhere safe. This phrase secures your local encryption key and your EVM, BTC, Solana, and Tron wallet identities.',
  'mnemonic.cannotRecover':
    'This phrase can never be recovered if lost and should stay fully local to your device.',
  'mnemonic.copyToClipboard': 'Copy to Clipboard',
  'mnemonic.alreadyHavePhrase': 'I already have a recovery phrase',
  'mnemonic.consentSaved': 'I saved this phrase and consent to using it for local wallet setup',
  'mnemonic.enterPhraseToRestore':
    'Enter your recovery phrase below to restore your local wallet identities, or paste the full phrase into any field (12 words for new backups; 24-word phrases from older versions still work).',
  'mnemonic.words': 'Words',
  'mnemonic.validPhrase': 'Valid recovery phrase',
  'mnemonic.generateNewPhrase': 'Generate a new recovery phrase instead',
  'mnemonic.securingData': 'Securing Your Data...',
  'mnemonic.saveRecoveryPhrase': 'Save Recovery Phrase',
  'mnemonic.userNotLoaded': 'User not loaded. Please sign in again or refresh the page.',
  'mnemonic.invalidPhrase': 'Invalid recovery phrase. Please check your words and try again.',
  'mnemonic.somethingWentWrong': 'Something went wrong. Please try again.',

  // Team
  'team.failedToCreate': 'Failed to create team',
  'team.invalidInviteCode': 'Invalid or expired invite code',
  'team.failedToSwitch': 'Failed to switch team',
  'team.failedToLeave': 'Failed to leave team',
  'team.role.owner': 'Owner',
  'team.role.admin': 'Admin',
  'team.role.billingManager': 'Billing Manager',
  'team.role.member': 'Member',
  'team.active': 'Active',
  'team.personalTeam': 'Personal team',
  'team.manageTeam': 'Manage Team',
  'team.switching': 'Switching...',
  'team.switch': 'Switch',
  'team.leaving': 'Leaving...',
  'team.leave': 'Leave',
  'team.yourTeams': 'Your Teams',
  'team.createNewTeam': 'Create New Team',
  'team.teamName': 'Team name',
  'team.creating': 'Creating...',
  'team.joinExistingTeam': 'Join Existing Team',
  'team.inviteCode': 'Invite code',
  'team.joining': 'Joining...',
  'team.join': 'Join',
  'team.leaveTeam': 'Leave Team',
  'team.confirmLeave': 'Are you sure you want to leave',
  'team.leaveWarning':
    "You will lose access to the team and all team resources. You'll need a new invite to rejoin.",
  'team.management': 'Team Management',
  'team.notFound': 'Team not found',
  'team.accessDenied': 'Access denied',
  'team.members': 'Members',

  // Voice
  'voice.title': 'Voice Dictation',
  'voice.settings': 'Voice Settings',
  'voice.settingsDesc': 'Hold the hotkey to dictate and insert text into the active field.',
  'voice.hotkey': 'Hotkey',
  'voice.activationMode': 'Activation Mode',
  'voice.tapToToggle': 'Tap to toggle',
  'voice.writingStyle': 'Writing Style',
  'voice.verbatimTranscription': 'Verbatim transcription',
  'voice.naturalCleanup': 'Natural cleanup',
  'voice.autoStart': 'Start voice server automatically with the core',
  'voice.customDictionary': 'Custom Dictionary',
  'voice.customDictionaryDesc':
    'Add names, technical terms, and domain words to improve recognition accuracy.',
  'voice.addWord': 'Add a word...',
  'voice.sttDisabled':
    'Voice dictation is disabled until the local STT model is downloaded and ready.',
  'voice.openLocalAiModel': 'Open Local AI Model',
  'voice.serverRestarted': 'Voice server restarted with the new settings.',
  'voice.settingsSaved': 'Voice settings saved.',
  'voice.serverStarted': 'Voice server started.',
  'voice.serverStopped': 'Voice server stopped.',
  'voice.saveVoiceSettings': 'Save Voice Settings',
  'voice.startVoiceServer': 'Start Voice Server',
  'voice.stopVoiceServer': 'Stop Voice Server',
  'voice.debugTitle': 'Voice Debug',

  // Autocomplete
  'autocomplete.title': 'Autocomplete',
  'autocomplete.settings': 'Settings',
  'autocomplete.acceptWithTab': 'Accept With Tab',
  'autocomplete.stylePreset': 'Style Preset',
  'autocomplete.style.balanced': 'Balanced',
  'autocomplete.style.concise': 'Concise',
  'autocomplete.style.formal': 'Formal',
  'autocomplete.style.casual': 'Casual',
  'autocomplete.style.custom': 'Custom',
  'autocomplete.disabledApps': 'Disabled Apps (one bundle/app token per line)',
  'autocomplete.saveSettings': 'Save Settings',
  'autocomplete.saving': 'Saving…',
  'autocomplete.runtime': 'Runtime',
  'autocomplete.running': 'Running',
  'autocomplete.start': 'Start',
  'autocomplete.stop': 'Stop',
  'autocomplete.settingsSaved': 'Autocomplete settings saved.',
  'autocomplete.started': 'Autocomplete started.',
  'autocomplete.didNotStart': 'Autocomplete did not start. Check if it is enabled.',
  'autocomplete.stopped': 'Autocomplete stopped.',
  'autocomplete.advancedSettings': 'Advanced settings',
  'autocomplete.debugTitle': 'Autocomplete Debug',

  // Chat
  'chat.agentChat': 'Agent Chat',
  'chat.overrides': 'Overrides',
  'chat.model': 'Model',
  'chat.temperature': 'Temperature',
  'chat.conversation': 'Conversation',
  'chat.startAgentConversation': 'Start a conversation with the agent.',
  'chat.you': 'You',
  'chat.agent': 'Agent',
  'chat.askAgent': 'Ask the agent anything...',
  'chat.sendMessage': 'Send Message',

  // Composio
  'composio.triageTitle': 'Integration Triggers',
  'composio.triageDesc':
    'When active, each incoming Composio trigger runs through an AI triage step that classifies the event and may kick off automated actions — one local LLM turn per trigger. Disable globally or per integration if you prefer manual review. If the environment variable',
  'composio.disableAllTriage': 'Disable AI triage for all triggers',
  'composio.triggersStillRecorded': 'Triggers are still recorded to history — no LLM turn is run.',
  'composio.disableSpecificIntegrations': 'Disable AI triage for specific integrations',
  'composio.settingsSaved': 'Settings saved',
  'composio.saveFailed': 'Failed to save. Try again.',

  // Cron
  'cron.title': 'Cron Jobs',
  'cron.scheduledJobs': 'Scheduled Jobs',
  'cron.manageCronJobs': 'Manage cron jobs from the core scheduler.',
  'cron.refreshCronJobs': 'Refresh Cron Jobs',

  // Local Model
  'localModel.modelStatus': 'Model Status',
  'localModel.downloadModels': 'Download Models',
  'localModel.usage': 'Usage',
  'localModel.usageDesc':
    'Choose which subsystems run on the local model. Anything off uses the cloud.',
  'localModel.enableRuntime': 'Enable local AI runtime',
  'localModel.enableRuntimeDesc':
    'Master switch. Off by default — Ollama stays idle. When on, the tree summarizer, screen intelligence, and autocomplete always use the local model.',
  'localModel.advancedSettings': 'Advanced settings',
  'localModel.debugTitle': 'Local Model Debug',

  // Screen Awareness
  'screenAwareness.debugTitle': 'Screen Awareness Debug',

  // Memory
  'memory.debugTitle': 'Memory Debug',

  // Webhooks
  'webhooks.debugTitle': 'Webhooks Debug',

  // Notifications
  'notifications.routingTitle': 'Notification Routing',

  // Common (additional)
  'common.reload': 'Reload',
  'common.skip': 'Skip',
  'common.disable': 'Disable',
  'common.enable': 'Enable',

  // Chat (additional)
  'chat.safetyTimeout':
    'No response from the agent after 2 minutes. Try again or check your connection.',
  'chat.filter.all': 'All',
  'chat.filter.work': 'Work',
  'chat.filter.briefing': 'Briefing',
  'chat.filter.notification': 'Notification',
  'chat.filter.workers': 'Workers',
  'chat.selectThread': 'Select a thread',
  'chat.threads': 'Threads',
  'chat.noThreads': 'No threads yet',
  'chat.noLabelThreads': 'No "{label}" threads',
  'chat.noWorkerThreads': 'No worker threads yet',
  'chat.deleteThread': 'Delete thread',
  'chat.deleteThreadConfirm': 'Are you sure you want to delete "{title}"?',
  'chat.untitledThread': 'Untitled thread',
  'chat.hideSidebar': 'Hide sidebar',
  'chat.showSidebar': 'Show sidebar',
  'chat.newThreadShortcut': 'New thread (/new)',
  'chat.new': 'New',
  'chat.failedToLoadMessages': 'Failed to load messages',
  'chat.thinkingIteration': 'Thinking... ({n})',
  'chat.thinkingDots': 'Thinking...',
  'chat.approachingLimit': 'Approaching usage limit',
  'chat.approachingLimitMsg': 'You have used {pct}% of your available quota.',
  'chat.upgrade': 'Upgrade',
  'chat.weeklyLimitHit': "You've hit your weekly limit.",
  'chat.resets': 'Resets',
  'chat.topUpToContinue': 'Top up to continue.',
  'chat.budgetComplete': 'Your included budget is complete. Add credits or upgrade to continue.',
  'chat.rateLimitReached': '10-hour rate limit reached.',
  'chat.topUp': 'Top Up',
  'chat.fiveHourLimit': '5-hour limit',
  'chat.weeklyLimit': 'Weekly limit',
  'chat.left': 'left',
  'chat.setup': 'Set up',
  'chat.switchToText': 'Switch to text',
  'chat.transcribing': 'Transcribing...',
  'chat.stopAndSend': 'Stop and send',
  'chat.startTalking': 'Start talking',
  'chat.playingVoiceReply': 'Playing voice reply',
  'chat.voiceHint': 'Use the mic to speak',
  'chat.micUnavailable': 'Microphone unavailable',
  'chat.turn': 'turn',
  'chat.turns': 'turns',
  'chat.openWorkerThread': 'Open worker thread',

  // Memory (additional)
  'memory.searchAria': 'Search memory',
  'memory.searchPlaceholder': 'Search memory entries...',
  'memory.sourceFilter.all': 'All sources',
  'memory.sourceFilter.email': 'Email',
  'memory.sourceFilter.calendar': 'Calendar',
  'memory.sourceFilter.telegram': 'Telegram',
  'memory.sourceFilter.aiInsight': 'AI Insight',
  'memory.sourceFilter.system': 'System',
  'memory.sourceFilter.trading': 'Trading',
  'memory.sourceFilter.security': 'Security',
  'memory.ingestionActivity': 'Ingestion Activity',
  'memory.events': 'events',
  'memory.event': 'event',
  'memory.overTheLast': 'over the last',
  'memory.months': 'months',
  'memory.peak': 'Peak',
  'memory.perDay': '/day',
  'memory.less': 'Less',
  'memory.more': 'More',
  'memory.on': 'on',
  'memory.loading': 'Loading Memory',
  'memory.fetching': 'Fetching your memory entries...',
  'memory.analyzing': 'Analyzing Memory',
  'memory.analyzingHint': 'Processing your memories to extract insights...',
  'memory.noMatches': 'No Matches Found',
  'memory.noMatchesHint': 'Try changing your search terms or filters.',
  'memory.allCaughtUp': 'All Caught Up',
  'memory.allCaughtUpHint': 'No new memory entries to process.',
  'memory.noAnalysis': 'No Analysis Yet',
  'memory.noAnalysisHint': 'Run an analysis to discover patterns in your memories.',
  'memory.emptyHint': 'Start interacting to create your first memories.',

  // Mic
  'mic.unavailable': 'Microphone is not available',
  'mic.permissionDenied': 'Microphone permission denied',
  'mic.failedToStartRecorder': 'Failed to start recorder',
  'mic.transcribing': 'Transcribing...',
  'mic.tapToSend': 'Tap to send',
  'mic.waitingForAgent': 'Waiting for agent...',
  'mic.tapAndSpeak': 'Tap and speak',
  'mic.stopRecording': 'Stop recording and send',
  'mic.startRecording': 'Start recording',

  // Token
  'token.usageLimitReached': 'Usage limit reached',
  'token.approachingLimit': 'Approaching limit',
  'token.planClickForDetails': 'plan - click for details',
  'token.sessionTokens': 'In: {in} | Out: {out} | Turns: {turns}',
  'token.limit': 'Limit Reached',

  // Catalog
  'catalog.noCapabilityBinding': 'No capability binding',
  'catalog.downloadFailed': 'Download failed',
  'catalog.active': 'Active',
  'catalog.installed': 'Installed',
  'catalog.notDownloaded': 'Not downloaded',
  'catalog.inUse': 'In Use',
  'catalog.use': 'Use',
  'catalog.deleteModel': 'Delete model',
  'catalog.download': 'Download',

  // Navigator
  'navigator.recent': 'Recent',
  'navigator.today': 'Today',
  'navigator.thisWeek': 'This Week',
  'navigator.sources': 'Sources',
  'navigator.email': 'Email',
  'navigator.slack': 'Slack',
  'navigator.chat': 'Chat',
  'navigator.documents': 'Documents',
  'navigator.people': 'People',
  'navigator.topics': 'Topics',

  // Dreams
  'dreams.description':
    'Dreams are AI-generated reflections that synthesize patterns from your memories.',
  'dreams.comingSoon': 'Coming soon',

  // Assignment
  'assignment.memoryLlm': 'Memory LLM',
  'assignment.memoryLlmAria': 'Memory LLM selection',
  'assignment.embedder': 'Embedder',
  'assignment.loaded': 'Loaded',
  'assignment.notDownloaded': 'Not downloaded',
  'assignment.usedForExtractSummarise': 'Used for extraction and summarization',

  // Insights
  'insights.knownFacts': 'Known Facts',
  'insights.preferences': 'Preferences',
  'insights.relationships': 'Relationships',
  'insights.skills': 'Skills',
  'insights.opinions': 'Opinions',
  'insights.other': 'Other',
  'insights.title': 'Insights',
  'insights.empty': 'No insights yet. Insights are generated as your memory grows.',
  'insights.description': 'Based on {count} relations in your memory graph.',
  'insights.items': 'items',
  'insights.more': 'more',

  // Calls
  'calls.joiningCall': 'Joining call',
  'calls.meetWindowOpening': 'The Meet window is opening...',
  'calls.failedToStart': 'Failed to start Meet call',
  'calls.couldNotStart': 'Could not start call',
  'calls.failedToClose': 'Failed to close call',
  'calls.couldNotClose': 'Could not close call',
  'calls.joinMeet': 'Join a Google Meet call',
  'calls.joinMeetDescription': 'Enter a Google Meet link to join.',
  'calls.meetLink': 'Meet Link',
  'calls.displayName': 'Display Name',
  'calls.openingMeet': 'Opening Meet...',
  'calls.joinCall': 'Join Call',
  'calls.activeCalls': 'Active Calls',
  'calls.leave': 'Leave',

  // Workspace
  'workspace.wipeConfirm': 'Are you sure you want to wipe all memory? This cannot be undone.',
  'workspace.resetTreeConfirm': 'Are you sure you want to rebuild the memory tree?',
  'workspace.wipeTitle': 'Wipe Memory',
  'workspace.resetting': 'Resetting...',
  'workspace.resetMemory': 'Reset Memory',
  'workspace.resetTreeTitle': 'Rebuild Memory Tree',
  'workspace.rebuilding': 'Rebuilding...',
  'workspace.resetMemoryTree': 'Reset Memory Tree',
  'workspace.building': 'Building...',
  'workspace.buildSummaryTrees': 'Build Summary Trees',
  'workspace.viewVault': 'View Vault',
  'workspace.graphLoadFailed': 'Failed to load memory graph',
  'workspace.loadingGraph': 'Loading memory graph...',
  'workspace.graphViewMode': 'Memory graph view mode',
  'workspace.trees': 'Trees',
  'workspace.contacts': 'Contacts',

  // Graph
  'graph.noContactMentions': 'No contact mentions',
  'graph.noMemory': 'No memory',
  'graph.source': 'Source',
  'graph.topic': 'Topic',
  'graph.global': 'Global',
  'graph.document': 'Document',
  'graph.contact': 'Contact',
  'graph.nodes': 'nodes',
  'graph.parentChild': 'parent-child',
  'graph.documentContact': 'document-contact',
  'graph.link': 'link',
  'graph.links': 'links',
  'graph.children': 'children',
  'graph.clickToOpenObsidian': 'Click to open in Obsidian',
  'graph.person': 'Person',

  // Modal
  'modal.dontShowAgain': "Don't show similar suggestions",

  // Reflections
  'reflections.loading': 'Loading reflections...',
  'reflections.empty': 'No reflections yet',
  'reflections.title': 'Reflections',
  'reflections.proposedAction': 'Proposed Action',
  'reflections.act': 'Act',
  'reflections.dismiss': 'Dismiss',

  // WhatsApp
  'whatsapp.chatsSynced': 'chats synced',
  'whatsapp.chatSynced': 'chat synced',

  // Sync
  'sync.active': 'Active',
  'sync.recent': 'Recent',
  'sync.idle': 'Idle',
  'sync.memorySources': 'Memory Sources',
  'sync.noConnectedSources': 'No connected sources',
  'sync.chunks': 'chunks',
  'sync.lastChunk': 'Last chunk:',
  'sync.pending': 'pending',
  'sync.processed': 'processed',
  'sync.syncing': 'Syncing…',
  'sync.sync': 'Sync',
  'sync.failedToLoad': 'Failed to load sync status',
  'sync.noContent': 'No content has been synced into memory yet. Connect an integration to start.',

  // Backend
  'backend.aiBackend': 'AI Backend',
  'backend.cloud': 'Cloud',
  'backend.recommended': 'Recommended',
  'backend.cloudDescription':
    'Fast, powerful models hosted on our servers. Ready to use immediately.',
  'backend.privacyNote': 'No personal data, messages, or keys are ever sent to our servers.',
  'backend.local': 'Local',
  'backend.advanced': 'Advanced',
  'backend.localDescription':
    'Run models on your own machine using Ollama. Full privacy, requires setup.',
  'backend.ramRecommended': '16GB+ RAM recommended',

  // Subconscious
  'subconscious.tasks': 'tasks',
  'subconscious.ticks': 'ticks',
  'subconscious.last': 'Last',
  'subconscious.failed': 'failed',
  'subconscious.tickInterval': 'Tick Interval',
  'subconscious.runNow': 'Run Now',
  'subconscious.approvalNeeded': 'Approval Needed',
  'subconscious.requiresApproval': 'Requires approval',
  'subconscious.fixInConnections': 'Fix in Connections',
  'subconscious.goAhead': 'Go Ahead',
  'subconscious.activeTasks': 'Active Tasks',
  'subconscious.noActiveTasks': 'No active tasks',
  'subconscious.default': 'Default',
  'subconscious.addTaskPlaceholder': 'Add a new task...',
  'subconscious.activityLog': 'Activity Log',
  'subconscious.noActivity': 'No activity yet',
  'subconscious.decision.nothingNew': 'Nothing new',
  'subconscious.decision.completed': 'Completed',
  'subconscious.decision.evaluating': 'Evaluating',
  'subconscious.decision.waitingApproval': 'Waiting for approval',
  'subconscious.decision.failed': 'Failed',
  'subconscious.decision.cancelled': 'Cancelled',
  'subconscious.decision.skipped': 'Skipped',

  // Actionable
  'actionable.complete': 'Complete',
  'actionable.dismiss': 'Dismiss',
  'actionable.snooze': 'Snooze',
  'actionable.new': 'New',

  // Stats
  'stats.storage': 'Storage',
  'stats.files': 'files',
  'stats.documents': 'Documents',
  'stats.today': 'today',
  'stats.namespaces': 'Namespaces',
  'stats.relations': 'Relations',
  'stats.firstMemory': 'First Memory',
  'stats.latest': 'Latest',
  'stats.sessions': 'Sessions',
  'stats.tokens': 'tokens',

  // Boot Check Gate
  'bootCheck.invalidUrl': 'Please enter a runtime URL.',
  'bootCheck.urlMustStartWith': 'The URL needs to start with http:// or https://',
  'bootCheck.validUrlRequired':
    "That doesn't look like a valid URL (try https://core.example.com/rpc)",
  'bootCheck.tokenRequired': "We'll need an auth token to connect.",
  'bootCheck.chooseCoreMode': 'Select a Runtime',
  'bootCheck.connectToCore': 'Connect to Your Runtime',
  'bootCheck.desktopDescription': 'Aura needs a runtime to think. Pick where it should live.',
  'bootCheck.webDescription':
    'On the web, Aura connects to a runtime you control. Drop in its URL and auth token below, or grab the desktop app to run one right on your machine.',
  'bootCheck.preferDesktop': 'Rather keep everything on your own device?',
  'bootCheck.downloadDesktop': 'Get the Desktop App',
  'bootCheck.localRecommended': 'Run Locally (Recommended)',
  'bootCheck.localDescription':
    'Runs right here on your computer. Fastest, fully private, nothing to set up.',
  'bootCheck.cloudMode': 'Run on the Cloud (Complex)',
  'bootCheck.cloudDescription':
    "Connect to a runtime you're hosting elsewhere. Stays online 24×7 so you don't need to keep this device running.",
  'bootCheck.coreRpcUrl': 'Runtime URL',
  'bootCheck.rpcUrlPlaceholder': 'https://core.example.com/rpc',
  'bootCheck.authToken': 'Auth Token',
  'bootCheck.bearerTokenPlaceholder': 'The bearer token from your remote runtime',
  'bootCheck.storedLocally': 'Kept on this device only. Sent as ',
  'bootCheck.testing': 'Testing…',
  'bootCheck.testConnection': 'Test Connection',
  'bootCheck.connectedOk': "Connected. You're good to go.",
  'bootCheck.authFailed': "That token didn't work. Double-check it and try again.",
  'bootCheck.unreachablePrefix': "Couldn't reach it:",
  'bootCheck.checkingCore': 'Waking up your runtime…',
  'bootCheck.cannotReach': "Can't Reach the Runtime",
  'bootCheck.cannotReachDesc': "We couldn't connect to your runtime. Want to try a different one?",
  'bootCheck.switchMode': 'Pick a Different Runtime',
  'bootCheck.quit': 'Quit',
  'bootCheck.legacyDetected': 'Legacy Background Runtime Detected',
  'bootCheck.legacyDescription':
    'A separately-installed Aura daemon is already running on this device. We need to clear it out before the built-in runtime can take over.',
  'bootCheck.removing': 'Removing…',
  'bootCheck.removeContinue': 'Remove and Continue',
  'bootCheck.localNeedsRestart': 'Local Runtime Needs a Restart',
  'bootCheck.localNeedsRestartDesc':
    'Your local runtime is on a different version than this app. A quick restart will get them back in sync.',
  'bootCheck.restarting': 'Restarting…',
  'bootCheck.restartCore': 'Restart Runtime',
  'bootCheck.cloudNeedsUpdate': 'Cloud Runtime Needs an Update',
  'bootCheck.cloudNeedsUpdateDesc':
    'Your cloud runtime is on a different version than this app. Run the updater to bring them back in sync.',
  'bootCheck.updating': 'Updating…',
  'bootCheck.updateCloudCore': 'Update Cloud Runtime',
  'bootCheck.versionCheckFailed': 'Runtime Version Check Failed',
  'bootCheck.versionCheckFailedDesc':
    "Your runtime is up but isn't reporting its version. It may be outdated. Restart or update it to continue.",
  'bootCheck.working': 'Working…',
  'bootCheck.restartUpdateCore': 'Restart / Update Runtime',
  'bootCheck.unexpectedError': 'Unexpected Boot-Check Error',
  'bootCheck.actionFailed': 'Something went wrong. Please try again.',

  // Notifications: category labels & timestamps
  'notifications.justNow': 'just now',
  'notifications.minAgo': '{n}m ago',
  'notifications.hrAgo': '{n}h ago',
  'notifications.dayAgo': '{n}d ago',
  'notifications.category.messages': 'Messages',
  'notifications.category.agents': 'Agents',
  'notifications.category.skills': 'Skills',
  'notifications.category.system': 'System',
  'notifications.category.meetings': 'Meetings',
  'notifications.category.reminders': 'Reminders',
  'notifications.category.important': 'Important',

  // About / Updates: status summary phrases
  'about.update.status.checking': 'Checking...',
  'about.update.status.available': 'v{version} available',
  'about.update.status.availableNoVersion': 'Update available',
  'about.update.status.downloading': 'Downloading...',
  'about.update.status.readyToInstall': 'v{version} ready to install',
  'about.update.status.readyToInstallNoVersion':
    'A new version is downloaded and ready. Restart to apply.',
  'about.update.status.installing': 'Installing...',
  'about.update.status.restarting': 'Restarting...',
  'about.update.status.upToDate': 'You are running the latest version.',
  'about.update.status.error': 'Update check failed',
  'about.update.status.default': 'Check for updates',

  // Welcome: connection error messages
  'welcome.connectionFailed': 'Connection failed: {status} {statusText}',
  'welcome.connectionFailedMsg': 'Connection failed: {message}',

  // Chat: Agent chat panel description
  'chat.agentChatDesc': 'Open a direct chat session with the agent.',

  // Channels: active route interpolated value
  'channels.activeRouteValue': '{channel} via {authMode}',

  // Privacy: data kind labels for What Leaves My Computer
  'privacy.dataKind.messages': 'Messages',
  'privacy.dataKind.agents': 'Agents',
  'privacy.dataKind.skills': 'Skills',
  'privacy.dataKind.system': 'System',
  'privacy.dataKind.meetings': 'Meetings',
  'privacy.dataKind.reminders': 'Reminders',
  'privacy.dataKind.important': 'Important',

  // Onboarding: supplementary keys
  'onboarding.enableLocalAI': 'Enable Local AI',
  'onboarding.skills.status.available': 'Available',
  'onboarding.skills.status.connected': 'Connected',
  'onboarding.skills.status.connecting': 'Connecting',
  'onboarding.skills.status.error': 'Error',
  'onboarding.skills.status.unavailable': 'Unavailable',

  // Composio: miscellaneous
  'composio.statusUnavailable': 'Status unavailable',
  'composio.authExpired': 'Auth expired',
  'composio.reconnect': 'Reconnect',
  'composio.envVarOverrides': 'is set, it overrides this setting.',

  // Memory: day-of-week labels for heatmap
  'memory.day.sun': 'Sun',
  'memory.day.mon': 'Mon',
  'memory.day.tue': 'Tue',
  'memory.day.wed': 'Wed',
  'memory.day.thu': 'Thu',
  'memory.day.fri': 'Fri',
  'memory.day.sat': 'Sat',

  // Memory: ingestion status labels
  'memory.ingesting': 'Ingesting',
  'memory.ingestionQueued': 'Queued',
  'memory.ingestingTitle': 'Ingesting {title}',

  // Mic: error messages
  'mic.noAudioCaptured': 'No audio captured',
  'mic.noSpeechDetected': 'No speech detected',
  'mic.failedToStopRecording': 'Failed to stop recording: {message}',
  'mic.transcriptionFailed': 'Transcription failed: {message}',

  // Reflections: kind labels
  'reflections.kind.retrospective': 'Retrospective',
  'reflections.kind.derivedFact': 'Derived Fact',
  'reflections.kind.moodInsight': 'Mood Insight',
  'reflections.kind.relationshipInsight': 'Relationship Insight',

  // Graph: tooltip keys
  'graph.tooltip.summary': 'Summary',
  'graph.tooltip.contact': 'Contact',

  // Local Model: usage labels
  'localModel.usage.never': 'Never',
  'localModel.usage.mediumLoad': 'Medium load',
  'localModel.usage.lowLoad': 'Low load',
  'localModel.usage.idleMode': 'Idle mode',
  'localModel.rebootstrapComplete': 'Model re-bootstrap complete.',
  'localModel.modelsVerified': 'Local models verified.',

  // Meetings
  'meetings.title': 'Meetings',
  'meetings.subtitle': 'Record and revisit your conversations',
  'meetings.newRecording': 'New recording',
  'meetings.empty': 'No meetings yet',
  'meetings.status.recording': 'Recording',
  'meetings.status.completed': 'Completed',
  'meetings.status.error': 'Error',
  'meetings.searchPlaceholder': 'Search transcripts…',
  'meetings.searchNoResults': 'No matching transcripts',

  'meetings.recordModal.title': 'Start a new recording',
  'meetings.recordModal.microphone': 'Microphone',
  'meetings.recordModal.systemAudio': 'System audio (optional)',
  'meetings.recordModal.none': 'None',
  'meetings.recordModal.noDevices': 'No microphone found',
  'meetings.recordModal.titleLabel': 'Title (optional)',
  'meetings.recordModal.titlePlaceholder': 'Untitled meeting',
  'meetings.recordModal.start': 'Start',
  'meetings.recordModal.starting': 'Starting…',

  'meetings.detail.notFound': 'Meeting not found',
  'meetings.detail.deleteMeeting': 'Delete meeting',
  'meetings.detail.deleteTitle': 'Delete meeting?',
  'meetings.detail.deleteConfirm': 'This will permanently delete "{title}" and its transcript.',
  'meetings.detail.recordingInProgress': 'Recording in progress…',
  'meetings.detail.stopRecording': 'Stop recording',
  'meetings.detail.stopping': 'Stopping…',
  'meetings.detail.transcript': 'Transcript',
  'meetings.detail.noTranscript': 'No transcript yet',
  'meetings.detail.summary': 'Summary',
  'meetings.detail.noSummary': 'No summary yet',
  'meetings.detail.generateSummary': 'Generate summary',
  'meetings.detail.regenerateSummary': 'Regenerate summary',
  'meetings.detail.summarizing': 'Summarizing…',
  'meetings.detail.recordFirst': 'Record something first',
  'meetings.detail.slides': 'Slides',
  'meetings.detail.noSlides': 'No slides captured yet',
  'meetings.detail.slideAt': 'Slide at {time}',
  'meetings.detail.slideTextShowMore': 'Show more',
  'meetings.detail.slideTextShowLess': 'Show less',
  'meetings.detail.export': 'Export',
  'meetings.detail.exportAs': 'Export {format}',
  'meetings.detail.exporting': 'Exporting…',
  'meetings.detail.exportSuccess': 'Exported to:',

  // rewards.referrals
  'rewards.referrals.status_completed': 'Completed',
  'rewards.referrals.status_expired': 'Expired',
  'rewards.referrals.status_joined': 'Joined',
  'rewards.referrals.load_stats_error': 'Could not load referral stats',
  'rewards.referrals.copied': 'Copied',
  'rewards.referrals.copy_failed': 'Copy failed',
  'rewards.referrals.share_intro': 'Join me on Aura.',
  'rewards.referrals.share_code_line': 'Referral code: {code}',
  'rewards.referrals.share_download_line': 'Download Aura: {url}',
  'rewards.referrals.apply_error': 'Could not apply referral code',
  'rewards.referrals.heading': 'Invite friends, earn credits',
  'rewards.referrals.description':
    "Share your referral code. When a friend subscribes to a monthly plan, you both get $5 in account credit. Self-referrals and duplicate rewards are blocked on the server.",
  'rewards.referrals.loading': 'Loading referral program…',
  'rewards.referrals.retry': 'Retry',
  'rewards.referrals.your_code_label': 'Your code',
  'rewards.referrals.total_earned_label': 'Total earned',
  'rewards.referrals.pending_referrals_label': 'Pending referrals',
  'rewards.referrals.completed_stat_label': 'Completed',
  'rewards.referrals.copy_code_button': 'Copy code',
  'rewards.referrals.share_button': 'Share',
  'rewards.referrals.apply_heading': 'Have a referral code?',
  'rewards.referrals.apply_description':
    "Enter a friend's referral code. You're eligible if you haven't subscribed yet — once you subscribe, you'll both get $5 in credit.",
  'rewards.referrals.code_placeholder': 'Referral code',
  'rewards.referrals.applying': 'Applying…',
  'rewards.referrals.apply_button': 'Apply',
  'rewards.referrals.linked_program_prefix': "You're linked to a referral program",
  'rewards.referrals.linked_program_code_suffix': ' (code {code})',
  'rewards.referrals.activity_heading': 'Referral activity',
  'rewards.referrals.no_referrals': 'No referrals yet. Share your code to get started.',
  'rewards.referrals.table_referred_user': 'Referred user',
  'rewards.referrals.table_status': 'Status',
  'rewards.referrals.table_reward': 'Reward',
  'rewards.referrals.table_updated': 'Updated',

  // rewards.community
  'rewards.community.discord_waiting_sync': 'Waiting for backend sync',
  'rewards.community.discord_joined': 'Joined the server',
  'rewards.community.discord_linked_not_in_server': 'Linked, but not in server',
  'rewards.community.discord_not_linked': 'Not linked',
  'rewards.community.discord_status_unavailable': 'Membership status unavailable',
  'rewards.community.hero_heading': 'Earn Rewards & Discord Roles',
  'rewards.community.hero_description':
    'Unlock exclusive channels, supporter badges, and backend-synced rewards by connecting your Discord account.',
  'rewards.community.connect_discord_button': 'Connect Discord',
  'rewards.community.join_discord_button': 'Join Discord',
  'rewards.community.sync_error':
    'Rewards sync is unavailable right now. The page is showing connection guidance without claiming new unlocks. Details: {error}',
  'rewards.community.retrying': 'Retrying…',
  'rewards.community.try_again': 'Try again',
  'rewards.community.your_progress_heading': 'Your Progress',
  'rewards.community.loading_rewards': 'Loading rewards…',
  'rewards.community.achievements_unlocked': '{unlocked} of {total} achievements unlocked',
  'rewards.community.roles_rewards_heading': 'Roles & Rewards',
  'rewards.community.unlocked_label': 'Unlocked',
  'rewards.community.locked_label': 'Locked',
  'rewards.community.sync_pending_heading': 'Rewards sync pending',
  'rewards.community.sync_pending_description':
    'The backend did not return achievement data yet. Join Discord and connect your account now, then refresh this page once sync is available again.',
  'rewards.community.discord_server_label': 'Discord server',
  'rewards.community.current_streak_label': 'Current streak',
  'rewards.community.streak_days_value': '{days} days',
  'rewards.community.cumulative_tokens_label': 'Cumulative tokens',
  'rewards.community.unknown_value': 'Unknown',

  // rewards.coupon
  'rewards.coupon.pending_status': 'Pending',
  'rewards.coupon.status_applied': 'Applied',
  'rewards.coupon.status_pending_action': 'Pending action',
  'rewards.coupon.status_redeemed': 'Redeemed',
  'rewards.coupon.success_pending':
    '{code} accepted. {amount} will unlock after the required action is completed.',
  'rewards.coupon.success_redeemed': '{code} redeemed. {amount} was added to your credits.',
  'rewards.coupon.load_error': 'Could not load reward codes right now.',
  'rewards.coupon.submit_error': 'Could not apply that reward code.',
  'rewards.coupon.heading': 'Redeem a coupon code',
  'rewards.coupon.description':
    'Redeem promo or coupon codes here. Successful redemptions refresh your credits immediately, and pending rewards stay visible in your history.',
  'rewards.coupon.promo_credits_label': 'Promo credits',
  'rewards.coupon.redeemed_codes_label': 'Redeemed codes',
  'rewards.coupon.code_placeholder': 'Coupon code',
  'rewards.coupon.redeeming': 'Redeeming...',
  'rewards.coupon.redeem_button': 'Redeem Code',
  'rewards.coupon.retry': 'Retry',
  'rewards.coupon.recent_redemptions_heading': 'Recent redemptions',
  'rewards.coupon.refresh_button': 'Refresh',
  'rewards.coupon.loading_history': 'Loading reward history…',
  'rewards.coupon.no_redemptions': 'No reward codes redeemed yet.',
  'rewards.coupon.table_code': 'Code',
  'rewards.coupon.table_reward': 'Reward',
  'rewards.coupon.table_status': 'Status',
  'rewards.coupon.table_redeemed': 'Redeemed',

  // webhooks.composio_history
  'webhooks.composio_history.title': 'ComposeIO Trigger History',
  'webhooks.composio_history.empty': 'No ComposeIO triggers have been captured yet.',
  'webhooks.composio_history.metadata_id': 'Metadata ID',
  'webhooks.composio_history.metadata_uuid': 'Metadata UUID',
  'webhooks.composio_history.payload': 'Payload',

  // webhooks.tunnel_list
  'webhooks.tunnel_list.title': 'Webhook Tunnels',
  'webhooks.tunnel_list.loading': 'Loading...',
  'webhooks.tunnel_list.refresh': 'Refresh',
  'webhooks.tunnel_list.new_tunnel': '+ New Tunnel',
  'webhooks.tunnel_list.name_placeholder': 'Tunnel name (e.g. telegram-bot)',
  'webhooks.tunnel_list.description_placeholder': 'Description (optional)',
  'webhooks.tunnel_list.cancel': 'Cancel',
  'webhooks.tunnel_list.creating': 'Creating...',
  'webhooks.tunnel_list.create': 'Create',
  'webhooks.tunnel_list.dismiss': 'Dismiss',
  'webhooks.tunnel_list.empty': 'No tunnels yet. Create one to receive webhook events.',
  'webhooks.tunnel_list.create_failed': 'Failed to create tunnel',
  'webhooks.tunnel_list.delete_failed': 'Failed to delete tunnel',
  'webhooks.tunnel_list.toggle_echo_failed': 'Failed to toggle echo',
  'webhooks.tunnel_list.status_active': 'Active',
  'webhooks.tunnel_list.status_inactive': 'Inactive',
  'webhooks.tunnel_list.echo_badge': 'Echo',
  'webhooks.tunnel_list.resolving_url': 'Resolving backend URL…',
  'webhooks.tunnel_list.copied': 'Copied!',
  'webhooks.tunnel_list.copy': 'Copy',
  'webhooks.tunnel_list.remove_echo': 'Remove Echo',
  'webhooks.tunnel_list.enable_echo': 'Enable Echo',
  'webhooks.tunnel_list.busy': '...',
  'webhooks.tunnel_list.delete': 'Delete',

  // webhooks.activity
  'webhooks.activity.title': 'Recent Activity',
  'webhooks.activity.empty':
    'No webhook activity yet. Events will appear here when webhooks are received.',

  // commands.palette
  'commands.palette.default_group': 'Actions',
  'commands.palette.aria_label': 'Command palette',
  'commands.palette.title': 'Command palette',
  'commands.palette.description':
    'Search and run commands. Use arrow keys to navigate, Enter to select, Escape to close.',
  'commands.palette.placeholder': 'Type a command or search…',
  'commands.palette.search_aria_label': 'Search commands',
  'commands.palette.no_results': 'No results.',
  'commands.palette.footer_hint': 'Press ? for all shortcuts',

  // overlay
  'overlay.orb_title': 'Drag to move · Double-click to reset position',

  // skills.autocomplete_setup
  'skills.autocomplete_setup.enable_error': 'Failed to enable autocomplete',
  'skills.autocomplete_setup.title': 'Text Auto-Complete',
  'skills.autocomplete_setup.subtitle_enable': 'Enable inline completions',
  'skills.autocomplete_setup.subtitle_success': 'Ready to go',
  'skills.autocomplete_setup.description':
    'Text Auto-Complete suggests inline completions as you type across any app. Suggestions appear as an overlay you can accept with Tab.',
  'skills.autocomplete_setup.platform_unsupported': 'Auto-complete is not supported on this platform.',
  'skills.autocomplete_setup.style_preset_label': 'Style preset',
  'skills.autocomplete_setup.style_preset_value': 'Balanced (configurable later)',
  'skills.autocomplete_setup.accept_key_label': 'Accept key',
  'skills.autocomplete_setup.accept_key_value': 'Tab',
  'skills.autocomplete_setup.debounce_label': 'Debounce',
  'skills.autocomplete_setup.enabling': 'Enabling...',
  'skills.autocomplete_setup.enable_button': 'Enable Auto-Complete',
  'skills.autocomplete_setup.success_title': 'Auto-Complete is Active',
  'skills.autocomplete_setup.success_description':
    'Start typing in any app and suggestions will appear as an inline overlay. Press Tab to accept.',
  'skills.autocomplete_setup.customize_settings': 'Customize Settings',
  'skills.autocomplete_setup.done': 'Done',

  // skills.create_modal
  'skills.create_modal.title': 'New skill',
  'skills.create_modal.subtitle_prefix': 'Scaffolds a',
  'skills.create_modal.subtitle_suffix': 'with the supplied frontmatter.',
  'skills.create_modal.close_aria': 'Close',
  'skills.create_modal.name_label': 'Name',
  'skills.create_modal.name_placeholder': 'e.g. Trade Journal',
  'skills.create_modal.slug_label': 'Slug:',
  'skills.create_modal.description_label': 'Description',
  'skills.create_modal.description_placeholder': 'What does this skill do?',
  'skills.create_modal.scope_label': 'Scope',
  'skills.create_modal.scope_hint_user':
    'Written to ~/.aura/skills/<slug>/SKILL.md — available across all workspaces.',
  'skills.create_modal.scope_hint_project':
    'Written to <workspace>/.aura/skills/<slug>/SKILL.md — requires workspace trust.',
  'skills.create_modal.license_label': 'License',
  'skills.create_modal.license_placeholder': 'MIT',
  'skills.create_modal.author_label': 'Author',
  'skills.create_modal.author_placeholder': 'Your name',
  'skills.create_modal.tags_label': 'Tags',
  'skills.create_modal.comma_separated': '(comma-separated)',
  'skills.create_modal.tags_placeholder': 'trading, research',
  'skills.create_modal.allowed_tools_label': 'Allowed tools',
  'skills.create_modal.allowed_tools_placeholder': 'node_exec, fetch',
  'skills.create_modal.allowed_tools_hint_prefix': 'Rendered into the SKILL.md frontmatter as',
  'skills.create_modal.error_title': 'Could not create skill',
  'skills.create_modal.cancel': 'Cancel',
  'skills.create_modal.creating': 'Creating…',
  'skills.create_modal.create_button': 'Create skill',

  // skills.install_dialog
  'skills.install_dialog.error_url_rejected_title': 'URL rejected',
  'skills.install_dialog.error_url_rejected_hint':
    'Only public HTTPS URLs are allowed. Private, loopback, and metadata hosts are blocked.',
  'skills.install_dialog.error_url_form_title': 'URL form not supported',
  'skills.install_dialog.error_url_form_hint':
    'Only direct `.md` links work. For GitHub, link to a file (github.com/owner/repo/blob/…/SKILL.md) — tree and repo roots are not installed.',
  'skills.install_dialog.error_too_large_title': 'SKILL.md too large',
  'skills.install_dialog.error_too_large_hint':
    'The SKILL.md must be under 1 MiB. Split bundled resources into `references/` or `scripts/` files instead of inlining them.',
  'skills.install_dialog.error_timeout_title': 'Fetch timed out',
  'skills.install_dialog.error_timeout_hint':
    'The remote host did not respond in time. Try again or raise the timeout (1–600 s).',
  'skills.install_dialog.error_fetch_failed_title': 'Fetch failed',
  'skills.install_dialog.error_fetch_failed_hint':
    'The request did not complete successfully. Check the URL points at a reachable public file, and that the host returned a 2xx response.',
  'skills.install_dialog.error_parse_title': 'SKILL.md did not parse',
  'skills.install_dialog.error_parse_hint':
    'The frontmatter must be valid YAML with non-empty `name` and `description` fields, terminated by `---`.',
  'skills.install_dialog.error_already_installed_title': 'Skill already installed',
  'skills.install_dialog.error_already_installed_hint':
    "A skill with this slug already exists in the workspace. Remove it first or change the frontmatter `metadata.id` / `name`.",
  'skills.install_dialog.error_write_failed_title': 'Could not write SKILL.md',
  'skills.install_dialog.error_write_failed_hint':
    'The workspace skills directory was not writable. Check filesystem permissions for `<workspace>/.aura/skills/`.',
  'skills.install_dialog.error_generic_title': 'Could not install skill',
  'skills.install_dialog.error_generic_hint':
    'The backend returned an error. The raw message is shown below.',
  'skills.install_dialog.title': 'Install skill from URL',
  'skills.install_dialog.subtitle_prefix': 'Fetches a single',
  'skills.install_dialog.subtitle_middle': 'over HTTPS and installs it under',
  'skills.install_dialog.subtitle_suffix': '. HTTPS only; private and loopback hosts are blocked.',
  'skills.install_dialog.close_aria': 'Close',
  'skills.install_dialog.url_label': 'Skill URL',
  'skills.install_dialog.url_invalid_prefix': 'URL must be a well-formed',
  'skills.install_dialog.url_invalid_suffix': 'link.',
  'skills.install_dialog.url_hint_prefix': 'Direct link to a',
  'skills.install_dialog.url_hint_middle': 'file.',
  'skills.install_dialog.url_hint_suffix': 'URLs auto-rewrite to',
  'skills.install_dialog.timeout_label': 'Timeout',
  'skills.install_dialog.timeout_unit_hint': '(seconds, optional)',
  'skills.install_dialog.timeout_invalid': 'Must be an integer between 1 and 600.',
  'skills.install_dialog.timeout_hint':
    'Defaults to 60 seconds. Values outside 1–600 are clamped server-side.',
  'skills.install_dialog.fetching_prefix': 'Fetching',
  'skills.install_dialog.fetching_suffix': '… this can take up to the timeout you configured.',
  'skills.install_dialog.success_title': 'Install complete',
  'skills.install_dialog.discovered_one': 'Discovered {count} new skill.',
  'skills.install_dialog.discovered_many': 'Discovered {count} new skills.',
  'skills.install_dialog.no_new_skills':
    'Skill installed, but no new skill ids appeared — the catalog may already contain a skill with the same slug.',
  'skills.install_dialog.fetch_log': 'Fetch log',
  'skills.install_dialog.parse_warnings': 'Parse warnings',
  'skills.install_dialog.raw_error': 'Raw error',
  'skills.install_dialog.done': 'Done',
  'skills.install_dialog.cancel': 'Cancel',
  'skills.install_dialog.installing': 'Installing…',
  'skills.install_dialog.install_button': 'Install',

  // skills.meeting_bots_card
  'skills.meeting_bots_card.platform_gmeet': 'Google Meet',
  'skills.meeting_bots_card.platform_zoom': 'Zoom',
  'skills.meeting_bots_card.platform_teams': 'Microsoft Teams',
  'skills.meeting_bots_card.banner_title': 'Send Aura to a meeting',
  'skills.meeting_bots_card.new_badge': 'New',
  'skills.meeting_bots_card.banner_description':
    'Drop a Google Meet link and Aura joins as a guest, talks, listens, and waves back.',
  'skills.meeting_bots_card.coming_soon_error': '{platform} support is coming soon.',
  'skills.meeting_bots_card.toast_joining_title': 'Aura is joining the meeting',
  'skills.meeting_bots_card.toast_joining_message':
    'It should appear as a participant in a few seconds.',
  'skills.meeting_bots_card.toast_busy_title': 'Aura is busy',
  'skills.meeting_bots_card.toast_failed_title': 'Could not start Aura',
  'skills.meeting_bots_card.default_error_message': 'Failed to start Aura.',
  'skills.meeting_bots_card.modal_aria_label': 'Send Aura to a meeting',
  'skills.meeting_bots_card.close_aria': 'Close',
  'skills.meeting_bots_card.modal_title': 'Send Aura to a meeting',
  'skills.meeting_bots_card.modal_description':
    'Aura joins as an anonymous guest, streams its video into the call, and replies via the agent.',
  'skills.meeting_bots_card.soon_suffix': '· soon',
  'skills.meeting_bots_card.meeting_link_label': 'Meeting link',
  'skills.meeting_bots_card.display_name_label': 'Display name',
  'skills.meeting_bots_card.cancel': 'Cancel',
  'skills.meeting_bots_card.coming_soon_button': '{platform} coming soon',
  'skills.meeting_bots_card.starting': 'Starting…',
  'skills.meeting_bots_card.send_to_button': 'Send to {platform}',

  // skills.screen_intelligence_setup
  'skills.screen_intelligence_setup.granted_badge': 'Granted',
  'skills.screen_intelligence_setup.opening': 'Opening...',
  'skills.screen_intelligence_setup.grant': 'Grant',
  'skills.screen_intelligence_setup.enable_error': 'Failed to enable Screen Intelligence',
  'skills.screen_intelligence_setup.title': 'Screen Intelligence',
  'skills.screen_intelligence_setup.close_aria': 'Close',
  'skills.screen_intelligence_setup.macos_only':
    'Screen Intelligence is currently available on macOS only.',
  'skills.screen_intelligence_setup.close_button': 'Close',
  'skills.screen_intelligence_setup.subtitle_permissions': 'Grant permissions',
  'skills.screen_intelligence_setup.subtitle_enable': 'Enable the skill',
  'skills.screen_intelligence_setup.subtitle_success': 'Ready to go',
  'skills.screen_intelligence_setup.permissions_description':
    'Screen Intelligence needs macOS permissions to capture your screen and provide context to your agent.',
  'skills.screen_intelligence_setup.permission_screen_recording': 'Screen Recording',
  'skills.screen_intelligence_setup.permission_accessibility': 'Accessibility',
  'skills.screen_intelligence_setup.permission_input_monitoring': 'Input Monitoring',
  'skills.screen_intelligence_setup.denied_hint':
    'After granting permissions in System Settings, click below to restart and pick up the changes.',
  'skills.screen_intelligence_setup.applies_privacy_to': 'macOS applies privacy to:',
  'skills.screen_intelligence_setup.restarting': 'Restarting...',
  'skills.screen_intelligence_setup.restart_and_refresh': 'Restart & Refresh',
  'skills.screen_intelligence_setup.refresh_status': 'Refresh Status',
  'skills.screen_intelligence_setup.all_permissions_granted': 'All permissions granted',
  'skills.screen_intelligence_setup.enable_description':
    "Enable Screen Intelligence to continuously capture what's on your screen and feed useful context into your agent's memory.",
  'skills.screen_intelligence_setup.capture_mode_label': 'Capture mode',
  'skills.screen_intelligence_setup.capture_mode_value': 'All windows (configurable later)',
  'skills.screen_intelligence_setup.vision_model_label': 'Vision model',
  'skills.screen_intelligence_setup.vision_model_value': 'Enabled',
  'skills.screen_intelligence_setup.panic_hotkey_label': 'Panic hotkey',
  'skills.screen_intelligence_setup.enabling': 'Enabling...',
  'skills.screen_intelligence_setup.enable_button': 'Enable Screen Intelligence',
  'skills.screen_intelligence_setup.success_title': 'Screen Intelligence is Enabled',
  'skills.screen_intelligence_setup.success_description':
    "Screen Intelligence is now enabled. Start a session from the settings panel to begin capturing screen context for your agent.",
  'skills.screen_intelligence_setup.advanced_settings': 'Advanced Settings',
  'skills.screen_intelligence_setup.done': 'Done',

  // skills.card
  'skills.card.more_actions': 'More actions',

  // skills.detail_drawer
  'skills.detail_drawer.scope_legacy': 'Legacy',
  'skills.detail_drawer.scope_user': 'User',
  'skills.detail_drawer.scope_project': 'Project',
  'skills.detail_drawer.close_aria': 'Close skill details',
  'skills.detail_drawer.author_label': 'Author',
  'skills.detail_drawer.tags_label': 'Tags',
  'skills.detail_drawer.allowed_tools_label': 'Allowed tools',
  'skills.detail_drawer.location_label': 'Location',
  'skills.detail_drawer.warnings_label': 'Warnings',
  'skills.detail_drawer.resources_heading': 'Bundled resources ({count})',
  'skills.detail_drawer.no_resources': 'No bundled resources.',

  // skills.resource_preview
  'skills.resource_preview.close_aria': 'Close preview',
  'skills.resource_preview.loading_aria': 'Loading',
  'skills.resource_preview.loading_text': 'Loading preview…',
  'skills.resource_preview.preview_failed': 'Preview failed',

  // skills.resource_tree
  'skills.resource_tree.group_scripts': 'Scripts',
  'skills.resource_tree.group_references': 'References',
  'skills.resource_tree.group_assets': 'Assets',
  'skills.resource_tree.group_other': 'Other',
  'skills.resource_tree.no_resources': 'No bundled resources.',

  // skills.uninstall_confirm
  'skills.uninstall_confirm.title': 'Uninstall {name}?',
  'skills.uninstall_confirm.description':
    'This permanently deletes the skill directory and all its bundled resources. The agent will stop seeing it at the next turn.',
  'skills.uninstall_confirm.error_heading': 'Could not uninstall',
  'skills.uninstall_confirm.error_message': "Couldn't uninstall skill: {message}",
  'skills.uninstall_confirm.cancel_button': 'Cancel',
  'skills.uninstall_confirm.uninstalling_button': 'Uninstalling…',
  'skills.uninstall_confirm.uninstall_button': 'Uninstall',

  // skills.voice_setup
  'skills.voice_setup.title': 'Voice Intelligence',
  'skills.voice_setup.subtitle_setup': 'Model download required',
  'skills.voice_setup.subtitle_enable': 'Start voice server',
  'skills.voice_setup.subtitle_success': 'Ready to go',
  'skills.voice_setup.stt_not_ready_heading': 'Speech-to-text model not ready',
  'skills.voice_setup.stt_not_ready_body':
    'Voice Intelligence requires a local Whisper model for transcription. Download it from the Local Model settings.',
  'skills.voice_setup.stt_download_hint':
    'Once the STT model is downloaded, you can return here to enable voice dictation and voice-driven AI chat.',
  'skills.voice_setup.download_stt_button': 'Download STT Model',
  'skills.voice_setup.cancel_button': 'Cancel',
  'skills.voice_setup.stt_ready_label': 'Speech-to-text model ready',
  'skills.voice_setup.enable_hint':
    'Start the voice server to use dictation and voice-driven chat. Press the hotkey to toggle recording.',
  'skills.voice_setup.hotkey_label': 'Hotkey',
  'skills.voice_setup.activation_label': 'Activation',
  'skills.voice_setup.activation_push': 'Push-to-talk',
  'skills.voice_setup.activation_tap': 'Tap to toggle',
  'skills.voice_setup.start_server_error_fallback': 'Failed to start voice server',
  'skills.voice_setup.starting_button': 'Starting...',
  'skills.voice_setup.start_server_button': 'Start Voice Server',
  'skills.voice_setup.success_heading': 'Voice Intelligence is Active',
  'skills.voice_setup.success_desc_prefix': 'Press',
  'skills.voice_setup.success_desc_suffix':
    'to start dictating. Your voice will be transcribed and sent to your agent.',
  'skills.voice_setup.customize_settings_button': 'Customize Settings',
  'skills.voice_setup.done_button': 'Done',

  // settings.model_download
  'settings.model_download.capability_assets_title': 'Capability Assets',
  'settings.model_download.quantization_preference': 'Quantization preference: {quantization}',
  'settings.model_download.manage_in_external_runtime': 'Manage this model in your external runtime.',
  'settings.model_download.downloading': 'Downloading...',
  'settings.model_download.download': 'Download',
  'settings.model_download.test_summarization_title': 'Test Summarization',
  'settings.model_download.summary_input_placeholder': 'Paste text to summarize with the local model...',
  'settings.model_download.calls_inference_summarize': 'Calls `aura.inference_summarize` via Rust core',
  'settings.model_download.running': 'Running...',
  'settings.model_download.run_summary_test': 'Run Summary Test',
  'settings.model_download.test_custom_prompt_title': 'Test Custom Prompt',
  'settings.model_download.prompt_input_placeholder': 'Type any prompt and run it against the local model...',
  'settings.model_download.no_think_mode': 'No-think mode',
  'settings.model_download.run_prompt_test': 'Run Prompt Test',
  'settings.model_download.running_prompt': 'Running prompt against local model...',
  'settings.model_download.test_vision_prompt_title': 'Test Vision Prompt',
  'settings.model_download.vision_prompt_placeholder': 'Enter a prompt for the vision model...',
  'settings.model_download.vision_image_placeholder':
    'One image reference per line (data URI, URL, or local path marker)',
  'settings.model_download.run_vision_test': 'Run Vision Test',
  'settings.model_download.test_embeddings_title': 'Test Embeddings',
  'settings.model_download.embedding_input_placeholder': 'One input string per line...',
  'settings.model_download.run_embedding_test': 'Run Embedding Test',
  'settings.model_download.embedding_model_line': 'Model: {modelId}',
  'settings.model_download.embedding_dimensions_line': 'Dimensions: {dimensions}',
  'settings.model_download.embedding_vectors_line': 'Vectors: {count}',
  'settings.model_download.test_stt_title': 'Test Voice Input (STT)',
  'settings.model_download.audio_path_placeholder': 'Absolute path to audio file',
  'settings.model_download.run_transcription_test': 'Run Transcription Test',
  'settings.model_download.transcript_label': 'Transcript:',
  'settings.model_download.test_tts_title': 'Test Voice Output (TTS)',
  'settings.model_download.tts_input_placeholder': 'Enter text to synthesize...',
  'settings.model_download.tts_output_path_placeholder': 'Optional output WAV path',
  'settings.model_download.run_tts_test': 'Run TTS Test',
  'settings.model_download.tts_voice_line': 'Voice: {voiceId}',
  'settings.model_download.tts_output_line': 'Output: {outputPath}',

  // settings.composio_panel
  'settings.composio_panel.title': 'Composio',
  'settings.composio_panel.loading': 'Loading…',
  'settings.composio_panel.intro_prefix':
    "Composio powers integrations with Gmail, Notion, Slack, GitHub, and 1000+ other apps. By default Aura manages it for you, so you don't need to bring an API key. If you prefer to use your own Composio account directly, switch to",
  'settings.composio_panel.direct_word': 'Direct',
  'settings.composio_panel.intro_suffix': 'mode and paste your key below.',
  'settings.composio_panel.routing_mode_legend': 'Routing mode',
  'settings.composio_panel.managed_aria_label': 'Managed (Aura handles it for you)',
  'settings.composio_panel.managed_title': 'Managed (Aura handles it for you)',
  'settings.composio_panel.managed_description':
    'Default. Aura manages the Composio connection for you. Trigger webhooks (real-time Gmail / Slack events) work out of the box.',
  'settings.composio_panel.direct_aria_label': 'Direct (bring your own API key)',
  'settings.composio_panel.direct_title': 'Direct (bring your own API key)',
  'settings.composio_panel.direct_description_prefix': 'Calls go to',
  'settings.composio_panel.direct_description_middle':
    'directly. Sovereign / offline-friendly. Tool execution works synchronously; real-time trigger webhooks are',
  'settings.composio_panel.not_yet_routed': 'not yet routed',
  'settings.composio_panel.direct_description_suffix': 'in direct mode (follow-up issue).',
  'settings.composio_panel.api_key_label': 'Composio API key',
  'settings.composio_panel.api_key_help_prefix': 'Find your key at',
  'settings.composio_panel.api_key_help_suffix':
    'The key is stored encrypted on this device and is never sent to Aura.',
  'settings.composio_panel.api_key_placeholder_stored':
    '••••••••••••  (key stored — paste a new key to replace)',
  'settings.composio_panel.api_key_placeholder_empty': 'ck_live_xxxxxxxxxxxxxxxx',
  'settings.composio_panel.api_key_stored_notice': 'A Composio API key is currently stored on this device.',
  'settings.composio_panel.confirm_title': '⚠️ Switching to Direct mode',
  'settings.composio_panel.confirm_integrations_prefix':
    'Your existing integrations (Gmail, Slack, GitHub, etc. linked through Aura)',
  'settings.composio_panel.wont_be_visible': "won't be visible",
  'settings.composio_panel.confirm_integrations_suffix':
    '— they live in the Aura-managed Composio tenant.',
  'settings.composio_panel.confirm_needs_title': "You'll need:",
  'settings.composio_panel.confirm_need_account_prefix': 'An account at',
  'settings.composio_panel.confirm_need_account_suffix': 'with an API key',
  'settings.composio_panel.confirm_need_relink': 'To re-link each integration through your personal Composio account',
  'settings.composio_panel.confirm_need_triggers_note':
    "Note: Composio triggers (real-time webhooks) don't fire in Direct mode yet — only synchronous tool calls",
  'settings.composio_panel.cancel': 'Cancel',
  'settings.composio_panel.switching': 'Switching…',
  'settings.composio_panel.confirm_switch_button': 'I understand, switch to Direct',
  'settings.composio_panel.saving': 'Saving…',
  'settings.composio_panel.save': 'Save',
  'settings.composio_panel.settings_saved': 'Settings saved',
  'settings.composio_panel.switched_to_backend': 'Switched to Backend mode',
  'settings.composio_panel.save_error': 'Failed to save. Direct mode requires a non-empty API key.',

  // settings.model_status
  'settings.model_status.ollama_unavailable_title': 'Ollama runtime unavailable',
  'settings.model_status.ollama_unavailable_description':
    'Aura now treats Ollama as an external inference runtime. Start your own Ollama server, pull the models you want, and point workload routing at it.',
  'settings.model_status.ollama_docs_link': 'Ollama docs',
  'settings.model_status.runtime_status_title': 'Runtime Status',
  'settings.model_status.refresh': 'Refresh',
  'settings.model_status.state_label': 'State',
  'settings.model_status.unavailable': 'Unavailable',
  'settings.model_status.progress_label': 'Progress:',
  'settings.model_status.installing_ollama_runtime': 'Installing Ollama runtime...',
  'settings.model_status.downloading_size_unknown': 'Downloading (size unknown)',
  'settings.model_status.eta': 'ETA {eta}',
  'settings.model_status.provider_label': 'Provider',
  'settings.model_status.model_label': 'Model',
  'settings.model_status.backend_label': 'Backend',
  'settings.model_status.last_latency_label': 'Last Latency',
  'settings.model_status.generation_tps_label': 'Generation TPS',
  'settings.model_status.artifact': 'Artifact: {path}',
  'settings.model_status.hide_error_details': 'Hide error details',
  'settings.model_status.show_error_details': 'Show error details',
  'settings.model_status.connect_only_prefix':
    'Aura only connects to an already-running Ollama-compatible endpoint. See',
  'settings.model_status.connect_only_suffix':
    'for setup instructions, then retry after your runtime is reachable.',
  'settings.model_status.ollama_diagnostics_title': 'Ollama Diagnostics',
  'settings.model_status.checking': 'Checking...',
  'settings.model_status.run_diagnostics': 'Run Diagnostics',
  'settings.model_status.run_diagnostics_hint':
    'Click "Run Diagnostics" to verify your external Ollama endpoint is reachable and has the expected models.',
  'settings.model_status.checking_ollama_endpoint': 'Checking Ollama endpoint and models...',
  'settings.model_status.all_checks_passed': 'All checks passed',
  'settings.model_status.issues_found': '{count} issue(s) found',
  'settings.model_status.server_label': 'Server',
  'settings.model_status.running': 'Running',
  'settings.model_status.not_running': 'Not running',
  'settings.model_status.binary_label': 'Binary',
  'settings.model_status.external_process': 'External process',
  'settings.model_status.not_found': 'Not found',
  'settings.model_status.running_via_external_process': 'Running via external process',
  'settings.model_status.installed_models_heading': 'Installed Models ({count})',
  'settings.model_status.expected_models_heading': 'Expected Models',
  'settings.model_status.chat_model_line': 'Chat: {model}',
  'settings.model_status.embedding_model_line': 'Embedding: {model}',
  'settings.model_status.vision_model_line': 'Vision: {model}',
  'settings.model_status.issues_heading': 'Issues',
  'settings.model_status.manage_ollama_footer':
    'Manage the Ollama process and model pulls outside Aura, then rerun diagnostics.',

  // settings.device_capability
  'settings.device_capability.model_tier_title': 'Model Tier',
  'settings.device_capability.loading_device_info': 'Loading device info and presets…',
  'settings.device_capability.could_not_load_presets': 'Could not load presets: {error}',
  'settings.device_capability.ram_label': 'RAM',
  'settings.device_capability.cpu_label': 'CPU',
  'settings.device_capability.cores_count': '{count} cores',
  'settings.device_capability.gpu_label': 'GPU',
  'settings.device_capability.detected': 'Detected',
  'settings.device_capability.not_detected': 'Not detected',
  'settings.device_capability.installing_ollama': 'Installing Ollama',
  'settings.device_capability.downloading_models_suffix': ' (downloading models)',
  'settings.device_capability.downloading_ollama_setup':
    'Downloading the OllamaSetup installer (~2 GB) and unpacking it. This can take a minute on first install.',
  'settings.device_capability.ollama_install_failed': 'Ollama install failed',
  'settings.device_capability.installer_exited_message':
    'The installer exited before Ollama was usable. Click retry to try again, or install manually from ollama.com.',
  'settings.device_capability.retrying': 'Retrying…',
  'settings.device_capability.retry_install': 'Retry install',
  'settings.device_capability.install_manually': 'Install manually',
  'settings.device_capability.run_ollama_first': 'Run Ollama first.',
  'settings.device_capability.run_ollama_first_description':
    'Local tiers depend on an externally managed Ollama endpoint. Start it yourself, pull the models you want, and keep using "Disabled (cloud fallback)" until the runtime is reachable.',
  'settings.device_capability.ollama_docs': 'Ollama docs',
  'settings.device_capability.disabled_tier_label': 'Disabled',
  'settings.device_capability.active_badge': 'Active',
  'settings.device_capability.recommended_badge': 'Recommended',
  'settings.device_capability.zero_gb': '0 GB',
  'settings.device_capability.disabled_tier_description':
    'Fallback to the cloud summarizer model. No local download or Ollama install required.',
  'settings.device_capability.needs_ollama_tier_title': 'Run Ollama first to use this tier',
  'settings.device_capability.applying_badge': 'Applying…',
  'settings.device_capability.needs_ollama_badge': 'Needs Ollama',
  'settings.device_capability.approx_download_gb': '~{gb} GB',
  'settings.device_capability.tier_summary_line':
    'Chat: {chatModel} · Vision: {visionModel} · Target RAM: {targetRam} GB',
  'settings.device_capability.custom_model_ids_notice':
    'You are using custom model IDs that do not match any built-in preset.',
  'settings.device_capability.local_ai_disabled_notice': 'Local AI disabled — using cloud fallback.',
  'settings.device_capability.applied_tier_with_model': 'Applied {tier} tier: {model}',
  'settings.device_capability.applied_tier': 'Applied {tier} tier',

  // intelligence.screen_debug_panel
  'intelligence.screen_debug_panel.debug_diagnostics_title': 'Debug & Diagnostics',
  'intelligence.screen_debug_panel.refresh': 'Refresh',
  'intelligence.screen_debug_panel.permissions_heading': 'Permissions',
  'intelligence.screen_debug_panel.platform_not_supported': 'Platform not supported',
  'intelligence.screen_debug_panel.permission_screen': 'Screen',
  'intelligence.screen_debug_panel.permission_accessibility': 'Accessibility',
  'intelligence.screen_debug_panel.permission_input': 'Input',
  'intelligence.screen_debug_panel.session_heading': 'Session',
  'intelligence.screen_debug_panel.active_label': 'Active',
  'intelligence.screen_debug_panel.yes': 'Yes',
  'intelligence.screen_debug_panel.no': 'No',
  'intelligence.screen_debug_panel.frames_label': 'Frames',
  'intelligence.screen_debug_panel.vision_state_label': 'Vision State',
  'intelligence.screen_debug_panel.vision_queue_label': 'Vision Queue',
  'intelligence.screen_debug_panel.last_app_label': 'Last App',
  'intelligence.screen_debug_panel.capture_test_heading': 'Capture Test',
  'intelligence.screen_debug_panel.capturing': 'Capturing...',
  'intelligence.screen_debug_panel.test_capture': 'Test Capture',
  'intelligence.screen_debug_panel.status_label': 'Status',
  'intelligence.screen_debug_panel.success': 'Success',
  'intelligence.screen_debug_panel.failed': 'Failed',
  'intelligence.screen_debug_panel.mode_label': 'Mode',
  'intelligence.screen_debug_panel.time_label': 'Time',
  'intelligence.screen_debug_panel.size_label': 'Size',
  'intelligence.screen_debug_panel.app_label': 'App',
  'intelligence.screen_debug_panel.unknown': 'Unknown',
  'intelligence.screen_debug_panel.bounds_label': 'Bounds',
  'intelligence.screen_debug_panel.recent_vision_summaries_heading': 'Recent Vision Summaries',

  // settings.completion_style
  'settings.completion_style.settings_title': 'Settings',
  'settings.completion_style.enabled_label': 'Enabled',
  'settings.completion_style.accept_with_tab_label': 'Accept With Tab',
  'settings.completion_style.debounce_label': 'Debounce (ms)',
  'settings.completion_style.max_chars_label': 'Max Chars',
  'settings.completion_style.overlay_ttl_label': 'Overlay TTL (ms)',
  'settings.completion_style.style_preset_label': 'Style Preset',
  'settings.completion_style.preset_balanced': 'Balanced',
  'settings.completion_style.preset_concise': 'Concise',
  'settings.completion_style.preset_formal': 'Formal',
  'settings.completion_style.preset_casual': 'Casual',
  'settings.completion_style.preset_custom': 'Custom',
  'settings.completion_style.style_instructions_label': 'Style Instructions',
  'settings.completion_style.style_examples_label': 'Style Examples (one per line)',
  'settings.completion_style.disabled_apps_label': 'Disabled Apps (one bundle/app token per line)',
  'settings.completion_style.saving': 'Saving…',
  'settings.completion_style.save_autocomplete_settings': 'Save Autocomplete Settings',
  'settings.completion_style.personalization_history_title': 'Personalization History',
  'settings.completion_style.clearing': 'Clearing…',
  'settings.completion_style.clear_history': 'Clear History',
  'settings.completion_style.loading': 'Loading…',
  'settings.completion_style.no_history_yet':
    'No accepted completions yet. Accept suggestions with Tab to start personalising.',
  'settings.completion_style.history_count_stored':
    '{count, plural, one{{count} accepted completion} other{{count} accepted completions}} stored — used to personalise future suggestions.',

  // settings.background_loop_controls
  'settings.background_loop_controls.load_failed': 'Load failed',
  'settings.background_loop_controls.cadence_minutes': '{minutes} min',
  'settings.background_loop_controls.cadence_queue': 'queue',
  'settings.background_loop_controls.loop_heartbeat_planner_name': 'Heartbeat planner',
  'settings.background_loop_controls.loop_heartbeat_planner_work':
    'Runs proactive collectors: cron reminders, calendar meetings, relevant notifications.',
  'settings.background_loop_controls.loop_heartbeat_planner_risk_on':
    '{calendarPlannerCallsPerTick} Composio read call(s)/tick; {calendarConnectionsSkipped} calendar link(s) over cap skipped.',
  'settings.background_loop_controls.loop_heartbeat_planner_risk_off':
    'Calendar collector off; planner reads only local enabled categories.',
  'settings.background_loop_controls.loop_subconscious_tick_name': 'Subconscious tick',
  'settings.background_loop_controls.loop_subconscious_tick_work':
    'Evaluates subconscious tasks/reflections through kind=subconscious_tick.',
  'settings.background_loop_controls.loop_subconscious_tick_risk_on':
    '{calls} model call(s)/week at current interval.',
  'settings.background_loop_controls.loop_subconscious_tick_risk_off':
    'Inference off; no scheduled subconscious model calls.',
  'settings.background_loop_controls.loop_memory_workers_name': 'Memory tree workers',
  'settings.background_loop_controls.loop_memory_workers_work':
    'Extracts chunks, seals branches, runs daily digests, routes topics.',
  'settings.background_loop_controls.loop_memory_workers_risk':
    '{workers} workers poll every {pollSeconds}s; LLM calls only when queue has extract/seal/digest/topic jobs.',
  'settings.background_loop_controls.loop_reflection_rebuild_name': 'Reflection rebuild',
  'settings.background_loop_controls.loop_reflection_rebuild_work':
    'Refreshes reflection state after memory activity.',
  'settings.background_loop_controls.loop_reflection_rebuild_risk':
    '{wakeups} wakeups/week; LLM work only when rebuild needs reflection.',
  'settings.background_loop_controls.loop_composio_sync_name': 'Composio sync',
  'settings.background_loop_controls.loop_composio_sync_route': 'Integration APIs',
  'settings.background_loop_controls.loop_composio_sync_work': 'Polls connected tools when provider sync is due.',
  'settings.background_loop_controls.loop_composio_sync_risk':
    '{wakeups} wakeups/week; scans {connections} active connection(s).',
  'settings.background_loop_controls.background_loops_title': 'Background loops',
  'settings.background_loop_controls.background_loops_description':
    'See what runs without a chat message, pause heartbeat work, and inspect recent credit ledger rows.',
  'settings.background_loop_controls.heartbeat_controls_title': 'Heartbeat controls',
  'settings.background_loop_controls.heartbeat_controls_description':
    'Defaults off. Enabling starts the loop; disabling aborts the running task.',
  'settings.background_loop_controls.refresh': 'Refresh',
  'settings.background_loop_controls.heartbeat_loop_label': 'Heartbeat loop',
  'settings.background_loop_controls.heartbeat_loop_description':
    'Master scheduler for planner + optional subconscious inference.',
  'settings.background_loop_controls.subconscious_inference_label': 'Subconscious inference',
  'settings.background_loop_controls.subconscious_inference_description':
    'Runs model-backed task/reflection evaluation on heartbeat ticks.',
  'settings.background_loop_controls.calendar_meeting_checks_label': 'Calendar meeting checks',
  'settings.background_loop_controls.calendar_meeting_checks_description':
    'Calls calendar event list for active Google Calendar connections.',
  'settings.background_loop_controls.calendar_cap_label': 'Calendar cap',
  'settings.background_loop_controls.conn_per_tick_option': '{count} conn/tick',
  'settings.background_loop_controls.meeting_lookahead_label': 'Meeting lookahead',
  'settings.background_loop_controls.reminder_lookahead_label': 'Reminder lookahead',
  'settings.background_loop_controls.cron_reminder_checks_label': 'Cron reminder checks',
  'settings.background_loop_controls.cron_reminder_checks_description':
    'Scans enabled cron jobs for reminder-like upcoming items.',
  'settings.background_loop_controls.relevant_notification_checks_label': 'Relevant notification checks',
  'settings.background_loop_controls.relevant_notification_checks_description':
    'Promotes urgent local notifications into proactive alerts.',
  'settings.background_loop_controls.external_delivery_label': 'External delivery',
  'settings.background_loop_controls.external_delivery_description':
    'Lets heartbeat alerts send proactive messages to external channels.',
  'settings.background_loop_controls.interval_label': 'Interval',
  'settings.background_loop_controls.running': 'Running...',
  'settings.background_loop_controls.planner_tick_now': 'Planner tick now',
  'settings.background_loop_controls.planner_summary':
    'Planner: {sourceEvents} source events, {deliveriesSent} sent, {deliveriesSkippedDedup} deduped.',
  'settings.background_loop_controls.loading_heartbeat_controls': 'Loading heartbeat controls...',
  'settings.background_loop_controls.heartbeat_controls_unavailable': 'Heartbeat controls unavailable.',
  'settings.background_loop_controls.loop_map_heading': 'Loop map',
  'settings.background_loop_controls.loop_on': 'on',
  'settings.background_loop_controls.loop_off': 'off',
  'settings.background_loop_controls.route_prefix': 'route: {route}',
  'settings.background_loop_controls.recent_usage_ledger_title': 'Recent usage ledger',
  'settings.background_loop_controls.recent_usage_ledger_description':
    'Backend rows expose action/time today; source tags need backend support.',
  'settings.background_loop_controls.reload': 'Reload',
  'settings.background_loop_controls.week_budget_label': 'Week budget',
  'settings.background_loop_controls.resets_detail': 'resets {dateTime}',
  'settings.background_loop_controls.week_remaining_label': 'Week remaining',
  'settings.background_loop_controls.used_detail': '{amount} used',
  'settings.background_loop_controls.five_hour_window_label': '5h window',
  'settings.background_loop_controls.avg_spend_row_label': 'Avg spend row',
  'settings.background_loop_controls.recent_spend_rows_detail': '{count} recent spend rows',
  'settings.background_loop_controls.bg_api_reads_label': 'Bg API reads',
  'settings.background_loop_controls.per_week': '{value}/week',
  'settings.background_loop_controls.planner_plus_sync_detail': '{planner} planner + {sync} sync',
  'settings.background_loop_controls.bg_wakeups_label': 'Bg wakeups',
  'settings.background_loop_controls.memory_polls_detail': '{value} memory polls',
  'settings.background_loop_controls.budget_math_heading': 'Budget math',
  'settings.background_loop_controls.rows_left_label': 'Rows left',
  'settings.background_loop_controls.rows_left_formula': 'remaining / avg row = {remaining} / {avgRow}',
  'settings.background_loop_controls.need_recent_spend_rows': 'Need recent spend rows to estimate.',
  'settings.background_loop_controls.rows_per_budget_label': 'Rows per full week budget',
  'settings.background_loop_controls.rows_per_budget_formula':
    'cycle budget / avg row = {cycleBudget} / {avgRow}',
  'settings.background_loop_controls.sample_burn_rate_label': 'Sample burn rate',
  'settings.background_loop_controls.per_hour': '{amount}/hr',
  'settings.background_loop_controls.sample_burn_rate_detail': '{rowsPerHour} rows/hr across {sampleHours}h sample',
  'settings.background_loop_controls.need_two_spend_rows': 'Need timestamps from at least two spend rows.',
  'settings.background_loop_controls.projected_empty_label': 'Projected empty',
  'settings.background_loop_controls.projected_empty_detail': '{hours}h after latest spend at recent burn rate',
  'settings.background_loop_controls.no_projection': 'No projection without recent hourly spend.',
  'settings.background_loop_controls.api_reads_per_dollar_label': 'API reads per $ remaining',
  'settings.background_loop_controls.reads_per_dollar': '{value} reads/$',
  'settings.background_loop_controls.api_reads_per_dollar_formula':
    'background API reads/week / remaining = {reads} / {remaining}',
  'settings.background_loop_controls.need_usage_response': 'Need usage response to estimate.',
  'settings.background_loop_controls.loop_call_budget_heading': 'Loop call budget',
  'settings.background_loop_controls.heartbeat_ticks_label': 'Heartbeat ticks',
  'settings.background_loop_controls.heartbeat_ticks_detail': '10080 min/week / {intervalMinutes} min interval',
  'settings.background_loop_controls.calendar_planner_calls_label': 'Calendar planner calls',
  'settings.background_loop_controls.calendar_planner_calls_detail':
    'ticks * (1 list_connections + {calendarConnectionsPolled} GOOGLECALENDAR_EVENTS_LIST)',
  'settings.background_loop_controls.meeting_collector_disabled': 'Meeting collector disabled.',
  'settings.background_loop_controls.calendar_fanout_cap_label': 'Calendar fanout cap',
  'settings.background_loop_controls.calendar_fanout_cap_value': '{polled}/{active} conn/tick',
  'settings.background_loop_controls.calendar_fanout_cap_detail':
    'max_calendar_connections_per_tick = {maxCalendarConnectionsPerTick}; skipped now = {calendarConnectionsSkipped}',
  'settings.background_loop_controls.subconscious_model_calls_label': 'Subconscious model calls',
  'settings.background_loop_controls.subconscious_model_calls_detail_on':
    'one kind=subconscious_tick model call per heartbeat tick',
  'settings.background_loop_controls.heartbeat_inference_disabled': 'Heartbeat inference disabled.',
  'settings.background_loop_controls.composio_sync_scans_label': 'Composio sync scans',
  'settings.background_loop_controls.composio_sync_scans_detail':
    '{connections} active integration connection(s) scanned every {intervalMinutes} min',
  'settings.background_loop_controls.total_bg_api_read_budget_label': 'Total bg API read budget',
  'settings.background_loop_controls.total_bg_api_read_budget_detail':
    'calendar planner reads + periodic integration scans; excludes user-initiated chat tools',
  'settings.background_loop_controls.memory_worker_polls_label': 'Memory worker polls',
  'settings.background_loop_controls.per_week_max': '{value}/week max',
  'settings.background_loop_controls.memory_worker_polls_detail':
    '{workers} workers * {pollSeconds}s poll; LLM calls only for queued jobs',
  'settings.background_loop_controls.latest_spend': 'Latest spend: {amount} at {dateTime} ({action})',
  'settings.background_loop_controls.top_actions_heading': 'Top actions',
  'settings.background_loop_controls.count_over_amount': '{count} / {amount}',
  'settings.background_loop_controls.no_spend_rows_loaded': 'No spend rows loaded.',
  'settings.background_loop_controls.top_hours_heading': 'Top hours',
  'settings.background_loop_controls.no_hourly_spend_yet': 'No hourly spend yet.',

  // settings.you_worker_provider_dialog
  'settings.you_worker_provider_dialog.connect_youworker_aria_label': 'Connect YouWorker',
  'settings.you_worker_provider_dialog.edit_youworker_title': 'Edit YouWorker',
  'settings.you_worker_provider_dialog.connect_youworker_title': 'Connect YouWorker',
  'settings.you_worker_provider_dialog.intro_prefix':
    "Params come from your youworker.ai deployment's integration handoff doc — the client secret is encrypted at rest in",
  'settings.you_worker_provider_dialog.display_label_label': 'Display label',
  'settings.you_worker_provider_dialog.transport_label': 'Transport',
  'settings.you_worker_provider_dialog.transport_webhook_option': 'Webhook (needs a public outbound endpoint)',
  'settings.you_worker_provider_dialog.transport_websocket_option':
    'WebSocket (reply on the same connection, no setup)',
  'settings.you_worker_provider_dialog.transport_hint':
    "WebSocket needs no outbound HMAC secret or public URL — youworker.ai replies on the connection this app opens. Webhook scales better once youworker.ai has your outbound endpoint registered.",
  'settings.you_worker_provider_dialog.yw_base_label': 'YouWorker base URL (yw_base)',
  'settings.you_worker_provider_dialog.autentika_url_label': 'Autentika URL',
  'settings.you_worker_provider_dialog.persona_label': 'Persona',
  'settings.you_worker_provider_dialog.client_id_label': 'Client ID',
  'settings.you_worker_provider_dialog.subject_uuid_label': 'Subject UUID',
  'settings.you_worker_provider_dialog.client_secret_label': 'Client secret',
  'settings.you_worker_provider_dialog.clear_stored_secret': 'Clear stored secret',
  'settings.you_worker_provider_dialog.client_secret_placeholder_existing': 'Leave blank to keep existing secret',
  'settings.you_worker_provider_dialog.outbound_hmac_secret_label': 'Outbound webhook HMAC secret',
  'settings.you_worker_provider_dialog.outbound_hmac_hint':
    "Verifies youworker's async reply webhook. Optional to save now, but chat calls will time out without it once youworker.ai can reach this app's outbound endpoint.",
  'settings.you_worker_provider_dialog.cancel': 'Cancel',
  'settings.you_worker_provider_dialog.masked_key_configured': '•••• configured',
  'settings.you_worker_provider_dialog.masked_key_not_configured': 'Not configured',
  'settings.you_worker_provider_dialog.saving': 'Saving…',
  'settings.you_worker_provider_dialog.save_changes': 'Save changes',
  'settings.you_worker_provider_dialog.connect': 'Connect',

  // composio.connect_modal
  'composio.connect_modal.atlassian_subdomain_hint_middle': 'for',
  'composio.connect_modal.atlassian_subdomain_hint_prefix': 'Enter the short subdomain only — e.g.',
  'composio.connect_modal.atlassian_subdomain_hint_suffix': '. Do not paste the full URL.',
  'composio.connect_modal.atlassian_subdomain_invalid':
    'Enter the short subdomain only (e.g. "acme"), not the full URL. It should contain only letters, numbers, and hyphens.',
  'composio.connect_modal.atlassian_subdomain_label': 'Atlassian subdomain',
  'composio.connect_modal.atlassian_subdomain_placeholder': 'your-subdomain',
  'composio.connect_modal.atlassian_subdomain_required': 'Please enter your Atlassian subdomain to continue.',
  'composio.connect_modal.cancel': 'Cancel',
  'composio.connect_modal.close': 'Close',
  'composio.connect_modal.close_button': 'Close',
  'composio.connect_modal.connect_button': 'Connect {name}',
  'composio.connect_modal.connected_status': '{name} is connected.',
  'composio.connect_modal.connection_failed': 'Connection failed (status: {status}).',
  'composio.connect_modal.disconnect': 'Disconnect',
  'composio.connect_modal.disconnect_failed': 'Disconnect failed: {message}',
  'composio.connect_modal.disconnecting': 'Disconnecting…',
  'composio.connect_modal.dismiss': 'Dismiss',
  'composio.connect_modal.expired_body':
    'Reconnect to re-enable {name} tools. Aura will keep this integration unavailable until you refresh OAuth access.',
  'composio.connect_modal.expired_title': '{name} authorization expired',
  'composio.connect_modal.generic_error': 'Something went wrong.',
  'composio.connect_modal.header_connect': 'Connect {name}',
  'composio.connect_modal.header_manage': 'Manage {name}',
  'composio.connect_modal.header_reconnect': 'Reconnect {name}',
  'composio.connect_modal.idle_intro':
    "Connect your {name} account. We'll open a browser window, you approve access there, and this app will detect the connection automatically.",
  'composio.connect_modal.needs_additional_configuration':
    'This connection requires additional configuration. Please contact support for assistance.',
  'composio.connect_modal.needs_subdomain_intro_middle': 'for',
  'composio.connect_modal.needs_subdomain_intro_prefix':
    'To connect {name}, enter your Atlassian subdomain (e.g.',
  'composio.connect_modal.needs_subdomain_intro_suffix': ') and try again.',
  'composio.connect_modal.permission_intro_prefix': '{name} can expose',
  'composio.connect_modal.permission_intro_suffix':
    ". After you connect, Aura's own agent permissions are controlled below as read, write, and admin toggles.",
  'composio.connect_modal.permissions_default_hint': 'Read + Write enabled by default',
  'composio.connect_modal.permissions_heading': 'Permissions',
  'composio.connect_modal.poll_timeout':
    'Timed out waiting for OAuth to complete. Please retry or check that the browser finished the flow.',
  'composio.connect_modal.reconnect_button': 'Reconnect {name}',
  'composio.connect_modal.reopen_browser': 'Reopen browser',
  'composio.connect_modal.requesting_connect_url': 'Requesting connect URL…',
  'composio.connect_modal.retry_connection': 'Retry connection',
  'composio.connect_modal.scope_admin_hint': 'Allow destructive or permission-changing actions (delete, share, etc.).',
  'composio.connect_modal.scope_admin_label': 'Admin',
  'composio.connect_modal.scope_load_failed': "Couldn't load scope preferences: {message}",
  'composio.connect_modal.scope_read_hint': 'Allow listing, fetching, searching (e.g. read emails / pages).',
  'composio.connect_modal.scope_read_label': 'Read',
  'composio.connect_modal.scope_save_failed': "Couldn't save {key} scope: {message}",
  'composio.connect_modal.scope_toggle_aria_disable': 'Disable {label} scope',
  'composio.connect_modal.scope_toggle_aria_enable': 'Enable {label} scope',
  'composio.connect_modal.scope_write_hint': 'Allow sending, creating, updating (e.g. send emails, create pages).',
  'composio.connect_modal.scope_write_label': 'Write',
  'composio.connect_modal.waba_id_hint_middle': 'then',
  'composio.connect_modal.waba_id_hint_prefix': 'Find it via',
  'composio.connect_modal.waba_id_hint_suffix': 'using your Meta access token.',
  'composio.connect_modal.waba_id_label': 'WhatsApp Business Account ID (WABA ID)',
  'composio.connect_modal.waba_id_placeholder': 'e.g. 123456789012345',
  'composio.connect_modal.waba_id_required':
    'Please enter your WhatsApp Business Account ID (WABA ID) to continue.',
  'composio.connect_modal.waiting_hint':
    'Complete the sign-in in your browser. This window will update when the connection is active.',
  'composio.connect_modal.waiting_oauth': 'Waiting for {name} OAuth to complete…',

  // conversations.task_kanban
  'conversations.task_kanban.column_blocked': 'Blocked',
  'conversations.task_kanban.column_done': 'Done',
  'conversations.task_kanban.column_in_progress': 'In progress',
  'conversations.task_kanban.column_todo': 'To do',
  'conversations.task_kanban.move_left': 'Move left',
  'conversations.task_kanban.move_right': 'Move right',
  'conversations.task_kanban.tasks_heading': 'Tasks',

  // settings.ai_panel
  'settings.ai_panel.background_group_label': 'Background',
  'settings.ai_panel.chat_group_label': 'Chat',
  'settings.ai_panel.default_resolves_prefix': 'Default resolves to',
  'settings.ai_panel.default_resolves_suffix': '.',
  'settings.ai_panel.header_title': 'LLM',
  'settings.ai_panel.loading': 'Loading…',
  'settings.ai_panel.local_runtime_connect_aria': 'Connect {label}',
  'settings.ai_panel.local_runtime_disconnect_aria': 'Disconnect {label}',
  'settings.ai_panel.providers_heading': 'LLM Providers',
  'settings.ai_panel.providers_subtitle':
    "Connect the language-model backends you want Aura to use. Toggle a provider on to add its key; toggle off to disconnect.",
  'settings.ai_panel.routing_heading': 'Routing',
  'settings.ai_panel.routing_subtitle':
    'Pick how each workload is served. Default uses Aura; Custom lets you point a workload at a specific provider and model.',

  // settings.auto_recharge
  'settings.auto_recharge.add_card': '+ Add card on Stripe',
  'settings.auto_recharge.amount_label': 'Add this amount',
  'settings.auto_recharge.cancel': 'Cancel',
  'settings.auto_recharge.confirm': 'Confirm',
  'settings.auto_recharge.default_badge': 'Default',
  'settings.auto_recharge.enable_subtitle': 'Automatically top up when your balance runs low',
  'settings.auto_recharge.enable_title': 'Enable Auto-Recharge',
  'settings.auto_recharge.expires': 'Expires {month}/{year}',
  'settings.auto_recharge.last_recharge_failed': 'Last recharge failed: {error}',
  'settings.auto_recharge.last_recharged': 'Last recharged {date}',
  'settings.auto_recharge.no_cards': 'No saved cards. Add one to enable auto-recharge.',
  'settings.auto_recharge.payment_methods_title': 'Payment Methods',
  'settings.auto_recharge.recharge_in_progress': 'Recharge in progress',
  'settings.auto_recharge.remove': 'Remove',
  'settings.auto_recharge.save_settings': 'Save Settings',
  'settings.auto_recharge.saving': 'Saving…',
  'settings.auto_recharge.set_default': 'Set default',
  'settings.auto_recharge.threshold_label': 'Recharge when balance drops below',
  'settings.auto_recharge.toggle_aria_label': 'Toggle auto-recharge',
  'settings.auto_recharge.validation_hint': 'Recharge amount should be greater than the trigger threshold.',
  'settings.auto_recharge.weekly_limit_label': 'Weekly spending limit',
  'settings.auto_recharge.weekly_spent': '${spent} of ${limit} used this week',

  // settings.cloud_provider_editor
  'settings.cloud_provider_editor.add_provider': 'Add provider',
  'settings.cloud_provider_editor.add_title': 'Add cloud provider',
  'settings.cloud_provider_editor.api_key_label': 'API key',
  'settings.cloud_provider_editor.api_key_placeholder_existing': 'Leave blank to keep existing key',
  'settings.cloud_provider_editor.api_key_placeholder_new': 'sk-...',
  'settings.cloud_provider_editor.cancel': 'Cancel',
  'settings.cloud_provider_editor.clear_stored_key': 'Clear stored key',
  'settings.cloud_provider_editor.display_label_label': 'Display label',
  'settings.cloud_provider_editor.display_label_placeholder': 'My Provider',
  'settings.cloud_provider_editor.edit_title': 'Edit {label}',
  'settings.cloud_provider_editor.encrypted_hint_prefix': 'API keys are encrypted at rest in',
  'settings.cloud_provider_editor.encrypted_hint_suffix': '.',
  'settings.cloud_provider_editor.endpoint_label': 'Endpoint',
  'settings.cloud_provider_editor.endpoint_placeholder': 'https://api.example.com/v1',
  'settings.cloud_provider_editor.provider_slug_label': 'Provider slug',
  'settings.cloud_provider_editor.save_changes': 'Save changes',
  'settings.cloud_provider_editor.saving': 'Saving…',

  // settings.custom_routing_dialog
  'settings.custom_routing_dialog.aria_label': 'Custom routing for {workload}',
  'settings.custom_routing_dialog.cancel': 'Cancel',
  'settings.custom_routing_dialog.close': 'Close',
  'settings.custom_routing_dialog.local_ollama': 'Local (Ollama)',
  'settings.custom_routing_dialog.model_label': 'Model',
  'settings.custom_routing_dialog.model_placeholder_generic': 'model-id',
  'settings.custom_routing_dialog.model_placeholder_slug': '{slug} model id',
  'settings.custom_routing_dialog.no_providers':
    'No custom providers are set up yet. Add a cloud provider key above, or enable the local Ollama runtime, then come back to pick one.',
  'settings.custom_routing_dialog.provider_label': 'Provider',
  'settings.custom_routing_dialog.save': 'Save',
  'settings.custom_routing_dialog.title': 'Custom routing',
  'settings.custom_routing_dialog.try_suggestion': 'Not sure? Try:',

  // settings.mascot_panel
  'settings.mascot_panel.active_badge': 'Active',
  'settings.mascot_panel.character_footer_hint':
    'Characters come from the server-side library and animate via the same tween and viseme pipeline as the meeting bot video stream.',
  'settings.mascot_panel.character_heading': 'Character',
  'settings.mascot_panel.color_black': 'Black',
  'settings.mascot_panel.color_burgundy': 'Burgundy',
  'settings.mascot_panel.color_footer_hint':
    'The selected color is applied everywhere Aura appears and is remembered across restarts.',
  'settings.mascot_panel.color_green': 'Green',
  'settings.mascot_panel.color_heading': 'Color',
  'settings.mascot_panel.color_navy': 'Navy',
  'settings.mascot_panel.color_radiogroup_aria': 'Aura color',
  'settings.mascot_panel.color_yellow': 'Yellow',
  'settings.mascot_panel.header_title': 'Aura',
  'settings.mascot_panel.library_unavailable': 'Aura library unavailable: {error}',
  'settings.mascot_panel.load_library_error': 'Could not load mascot library.',
  'settings.mascot_panel.load_mascot_error': 'Could not load mascot.',
  'settings.mascot_panel.loading_library': 'Loading Aura library…',
  'settings.mascot_panel.local_aura_default': 'Local Aura (default)',
  'settings.mascot_panel.no_characters':
    'No Aura characters are available yet. Local color variants above are still active.',
  'settings.mascot_panel.no_color_variants': 'No Aura color variants are available in this build.',
  'settings.mascot_panel.preview_label': 'Preview · {name}',
  'settings.mascot_panel.version_states': 'v{version} · {states} states',
  'settings.mascot_panel.visemes_suffix': ' · visemes',

  // settings.pay_as_you_go
  'settings.pay_as_you_go.available_label': 'Available',
  'settings.pay_as_you_go.balance_subtitle':
    'You can top up your credits if you ever exhaust your monthly budget or hit rate limits. Credits are consumed after any included subscription budget is exhausted.',
  'settings.pay_as_you_go.balance_title': 'Your Credit Balance',
  'settings.pay_as_you_go.charge_custom_amount': 'Charge custom amount',
  'settings.pay_as_you_go.custom_amount_label': 'Custom amount',
  'settings.pay_as_you_go.custom_amount_placeholder': 'Enter amount',
  'settings.pay_as_you_go.opening': 'Opening…',
  'settings.pay_as_you_go.promotional_credits_label': 'Promotional Credits',
  'settings.pay_as_you_go.topup_amount_subtitle':
    'Choose one of the preset amounts above or enter your own charge amount.',
  'settings.pay_as_you_go.topup_amount_title': 'Choose a Top-up Amount',
  'settings.pay_as_you_go.topup_balance_label': 'Top-up Balance',
  'settings.pay_as_you_go.topup_credits_label': 'Top Up Credits',
  'settings.pay_as_you_go.unable_to_load': 'Unable to load balance.',

  // settings.page
  'settings.page.footer_beta_build': 'Beta build - v{version}',
  'settings.page.account_title': 'Account',
  'settings.page.account_description': 'Recovery phrase, team, connections, and privacy settings.',
  'settings.page.features_title': 'Features',
  'settings.page.features_description': 'Screen awareness, messaging, and tools.',
  'settings.page.ai_title': 'AI',
  'settings.page.ai_description': 'Language model providers, local Ollama, and voice (STT / TTS).',

  // composio.trigger_toggles
  'composio.trigger_toggles.heading': 'Triggers',
  'composio.trigger_toggles.loading': 'Loading…',
  'composio.trigger_toggles.empty_state': 'No triggers are currently available for {toolkitName}.',
  'composio.trigger_toggles.subtitle': 'Listen for events from {toolkitName}',
  'composio.trigger_toggles.needs_configuration': 'Needs configuration',
  'composio.trigger_toggles.action_enable': 'Enable',
  'composio.trigger_toggles.action_disable': 'Disable',
  'composio.trigger_toggles.aria_label': '{action} {triggerName}',
  'composio.trigger_toggles.aria_label_repo': '{action} {triggerName} for {owner}/{repo}',
  'composio.trigger_toggles.action_failed': '{action} failed for {triggerName}: {message}',
  'composio.trigger_toggles.load_error': "Couldn't load triggers: {message}",

  // local_ai.download_snackbar
  'local_ai.download_snackbar.expand_aria': 'Expand download progress',
  'local_ai.download_snackbar.collapse_aria': 'Collapse download progress',
  'local_ai.download_snackbar.dismiss_aria': 'Dismiss download notification',
  'local_ai.download_snackbar.installing': 'Installing...',
  'local_ai.download_snackbar.preparing': 'Preparing...',

  // settings.cron_job_list
  'settings.cron_job_list.heading': 'Core Cron Jobs',
  'settings.cron_job_list.subheading': 'Jobs persisted in the Aura core scheduler database.',
  'settings.cron_job_list.loading': 'Loading cron jobs...',
  'settings.cron_job_list.empty': 'No core cron jobs found.',
  'settings.cron_job_list.status_enabled': 'Enabled',
  'settings.cron_job_list.status_paused': 'Paused',
  'settings.cron_job_list.schedule_label': 'Schedule:',
  'settings.cron_job_list.schedule_every': 'every {ms}ms',
  'settings.cron_job_list.schedule_at': 'at {at}',
  'settings.cron_job_list.next_run_label': 'Next run:',
  'settings.cron_job_list.last_status_label': 'Last status:',
  'settings.cron_job_list.saving': 'Saving…',
  'settings.cron_job_list.pause': 'Pause',
  'settings.cron_job_list.resume': 'Resume',
  'settings.cron_job_list.running': 'Running…',
  'settings.cron_job_list.run_now': 'Run Now',
  'settings.cron_job_list.loading_ellipsis': 'Loading…',
  'settings.cron_job_list.view_runs': 'View Runs',
  'settings.cron_job_list.removing': 'Removing…',
  'settings.cron_job_list.remove': 'Remove',
  'settings.cron_job_list.recent_runs': 'Recent Runs',
  'settings.cron_job_list.run_at': 'at',

  // settings.app_filter
  'settings.app_filter.yes': 'yes',
  'settings.app_filter.no': 'no',
  'settings.app_filter.unknown': 'unknown',
  'settings.app_filter.not_available': 'n/a',
  'settings.app_filter.none': 'none',
  'settings.app_filter.runtime_heading': 'Runtime',
  'settings.app_filter.platform_supported': 'Platform supported: {value}',
  'settings.app_filter.enabled': 'Enabled: {value}',
  'settings.app_filter.running': 'Running: {value}',
  'settings.app_filter.phase': 'Phase: {value}',
  'settings.app_filter.debounce': 'Debounce: {value}ms',
  'settings.app_filter.model': 'Model: {value}',
  'settings.app_filter.app': 'App: {value}',
  'settings.app_filter.last_error': 'Last error: {value}',
  'settings.app_filter.current_suggestion': 'Current suggestion: {value}',
  'settings.app_filter.refreshing': 'Refreshing…',
  'settings.app_filter.refresh_status': 'Refresh Status',
  'settings.app_filter.start': 'Start',
  'settings.app_filter.stop': 'Stop',
  'settings.app_filter.test_heading': 'Test',
  'settings.app_filter.context_override_label': 'Context Override (optional)',
  'settings.app_filter.get_suggestion': 'Get Suggestion',
  'settings.app_filter.accept_suggestion': 'Accept Suggestion',
  'settings.app_filter.debug_focus': 'Debug Focus',
  'settings.app_filter.live_logs_heading': 'Live Logs',
  'settings.app_filter.clear': 'Clear',
  'settings.app_filter.no_logs': 'No logs yet.',

  // settings.inference_budget
  'settings.inference_budget.heading': 'Inference Budget',
  'settings.inference_budget.loading': 'Loading…',
  'settings.inference_budget.remaining': '${remaining} / ${budget} remaining',
  'settings.inference_budget.no_recurring_plan': 'No recurring plan budget',
  'settings.inference_budget.ten_hour_cap': '10-hour cap: ${limit} / ${cap}',
  'settings.inference_budget.cycle_ends': 'Cycle ends {date}',
  'settings.inference_budget.exhausted_message':
    'Included subscription usage is exhausted. Top up credits to continue using AI features without waiting for the next cycle.',
  'settings.inference_budget.no_budget_message':
    'Your current plan does not include a recurring weekly inference budget. Usage is paid from available credits instead.',
  'settings.inference_budget.unable_to_load': 'Unable to load usage data',

  // aura_link_modal
  'aura_link_modal.close_aria': 'Close',
  'aura_link_modal.close_button': 'Close',
  'aura_link_modal.title_notifications': 'Allow notifications',
  'aura_link_modal.title_billing': 'Billing & credits',
  'aura_link_modal.title_messaging': 'Connect a chat channel',
  'aura_link_modal.title_discord': 'Join the community',
  'aura_link_modal.title_accounts_setup': 'Connect your apps',
  'aura_link_modal.loading_channel_setup': 'Loading channel setup…',
  'aura_link_modal.telegram_unavailable':
    "Telegram channel definition isn't available right now. Try again from Settings → Messaging.",
  'aura_link_modal.notifications_desktop_only':
    'Native notifications are only available in the desktop app (run `pnpm dev:app`).',
  'aura_link_modal.notifications_permission_off':
    'Notification permission is off. Enable Aura in System Settings → Notifications, then retry.',
  'aura_link_modal.notification_title': 'Aura is good to go',
  'aura_link_modal.notification_body': 'You will get pings here when something needs your attention.',
  'aura_link_modal.notification_send_failed':
    'Aura could not trigger a system notification. Check OS notification settings and retry.',
  'aura_link_modal.notifications_intro':
    'Aura uses native notifications so it can ping you when something needs your attention, even when the chat window is hidden.',
  'aura_link_modal.notifications_blocked_line1': 'Notifications are currently blocked.',
  'aura_link_modal.notifications_blocked_line2': '1. Open System Settings → Notifications → Aura',
  'aura_link_modal.notifications_blocked_line3': '2. Turn on Allow Notifications',
  'aura_link_modal.notifications_blocked_line4': '3. Return here and tap Retry test notification',
  'aura_link_modal.notifications_first_step':
    'First step: tap Send test notification and allow permission in the macOS prompt.',
  'aura_link_modal.asking_os': 'Asking your OS…',
  'aura_link_modal.retry_test_notification': 'Retry test notification',
  'aura_link_modal.send_test_notification': 'Send test notification',
  'aura_link_modal.notification_sent_success':
    'Test notification sent. If you didn’t receive it, go to System Settings → Notifications → Aura, turn on Allow Notifications, and set Banner Style to Persistent.',
  'aura_link_modal.notification_send_error': "Couldn't send: {error}",
  'aura_link_modal.trial_credit_label': 'Trial credit',
  'aura_link_modal.trial_credit_amount': '$1.00',
  'aura_link_modal.trial_credit_hint':
    'More than enough to play around. Top up or pick a plan when you want real usage.',
  'aura_link_modal.open_dashboard': 'Open dashboard in browser',
  'aura_link_modal.stay_on_trial': 'Stay on trial',
  'aura_link_modal.discord_intro':
    "Hop into our Discord and link your Aura account. You'll get exclusive early access to features, free credits to play with, a great community to nerd out with, and yes, free merch.",
  'aura_link_modal.discord_perk_feature_access': 'Exclusive feature access',
  'aura_link_modal.discord_perk_free_credits': 'Free credits for active members',
  'aura_link_modal.discord_perk_community': 'Community of builders and operators',
  'aura_link_modal.discord_perk_merch': 'Free merch when you stick around',
  'aura_link_modal.open_discord_invite': 'Open Discord invite',
  'aura_link_modal.maybe_later': 'Maybe later',
  'aura_link_modal.account_status_connected': 'Connected',
  'aura_link_modal.account_status_loading': 'Loading…',
  'aura_link_modal.account_status_needs_sign_in': 'Needs sign-in',
  'aura_link_modal.account_status_timed_out': 'Timed out',
  'aura_link_modal.account_status_error': 'Error',
  'aura_link_modal.account_status_closed': 'Closed',
  'aura_link_modal.accounts_setup_intro':
    "Pick the chat apps and inboxes you already use. We'll add them as built-in webviews here so you can ditch six browser tabs and stick with one app, and the agent can listen in across all of them in the background.",
  'aura_link_modal.disconnect_provider': 'Disconnect {label}',
  'aura_link_modal.connect_provider': 'Connect {label}',
  'aura_link_modal.accounts_setup_privacy_hint':
    "Toggling on adds a private webview. You'll sign in the first time you open it — credentials stay on your device.",
  'aura_link_modal.continue_with_sign_in': 'Continue with {label} sign-in',
  'aura_link_modal.done_default': 'Done',
  'aura_link_modal.skip_default': 'Skip for now',

  // upsell.global_banner
  'upsell.global_banner.limit_reached_title': "You've reached your usage limit",
  'upsell.global_banner.limit_reached_message': 'Upgrade your plan or top up credits to continue',
  'upsell.global_banner.upgrade_cta': 'Upgrade',
  'upsell.global_banner.near_limit_title': 'Approaching usage limit',
  'upsell.global_banner.near_limit_message':
    "You've used {pct}% of your usage limit. Upgrade for higher limits.",

  // channels.discord_config
  'channels.discord_config.field_required': '{field} is required',
  'channels.discord_config.link_token_expired': 'Link token expired. Please try again.',
  'channels.discord_config.restart_required': 'Channel saved. Restart the app to activate it.',
  'channels.discord_config.copied': 'Copied!',
  'channels.discord_config.copy': 'Copy',
  'channels.discord_config.link_instructions_prefix': 'In Discord, send ',
  'channels.discord_config.link_instructions_suffix': 'to the Aura bot. Token expires in 5 minutes.',
  'channels.discord_config.link_token_warning': 'Save this command — this token is shown only once.',
  'channels.discord_config.account_linked': 'Your Discord account is linked.',
  'channels.discord_config.disconnect': 'Disconnect',
  'channels.discord_config.connect': 'Connect',

  // onboarding.context_gathering
  'onboarding.context_gathering.title': 'Context Gathering',
  'onboarding.context_gathering.error_message':
    "We couldn't build your full profile right now, but that's okay — you can continue and your profile will build over time.",
  'onboarding.context_gathering.continue_to_chat': 'Continue to chat',
  'onboarding.context_gathering.building_profile': 'Building your profile...',
  'onboarding.context_gathering.connecting_integrations':
    'Connecting your integrations and building your personal context.',

  // notifications.center
  'notifications.center.load_failed': 'Failed to load notifications',
  'notifications.center.title': 'Notifications',
  'notifications.center.mark_all_read': 'Mark all read',
  'notifications.center.all_filter': 'All',
  'notifications.center.loading': 'Loading…',
  'notifications.center.empty_title': 'No notifications yet',
  'notifications.center.empty_subtitle': 'Notifications from your connected accounts will appear here.',

  // settings.memory_window_control
  'settings.memory_window_control.minimal_label': 'Minimal',
  'settings.memory_window_control.minimal_badge': 'Cheapest',
  'settings.memory_window_control.minimal_hint':
    'Smallest memory window. Cheapest, fastest, least continuity between runs.',
  'settings.memory_window_control.balanced_label': 'Balanced',
  'settings.memory_window_control.balanced_badge': 'Recommended',
  'settings.memory_window_control.balanced_hint':
    'Sensible default — good continuity without burning extra tokens on every run.',
  'settings.memory_window_control.extended_label': 'Extended',
  'settings.memory_window_control.extended_badge': 'More context',
  'settings.memory_window_control.extended_hint':
    'More long-term memory injected into each run. Higher token cost per turn.',
  'settings.memory_window_control.maximum_label': 'Maximum',
  'settings.memory_window_control.maximum_badge': 'Highest cost',
  'settings.memory_window_control.maximum_hint':
    'The largest safe window. Best continuity, meaningfully higher token bill on every run.',
  'settings.memory_window_control.load_failed': 'Failed to load memory settings',
  'settings.memory_window_control.save_failed': 'Failed to save memory window',
  'settings.memory_window_control.heading': 'Long-term memory window',
  'settings.memory_window_control.description':
    'How much remembered context Aura injects into every new agent run. Larger windows feel more aware of past conversations but use more tokens — and cost more — on every run.',

  // home.banners
  'home.banners.promo_credits': 'You have {amount} of promotional credits.',
  'home.banners.promo_credits_prefix': "Give Aura a spin, and when you're ready for more,",
  'home.banners.get_subscription': 'get a subscription',
  'home.banners.promo_credits_suffix': 'and get 10x more usage.',
  'home.banners.dismiss_early_bird': 'Dismiss early bird banner',
  'home.banners.early_bird_title': 'The first 1,000 users get 60% off.',
  'home.banners.early_bird_discount_prefix': 'Use discount code',
  'home.banners.early_bird_discount_middle': 'on your',
  'home.banners.first_subscription': 'first subscription.',
  'home.banners.join_discord': 'Join Our Discord',
  'home.banners.discord_description':
    'Get updates, free merch, credits, report bugs, and be part of the Aura community.',

  // intelligence.memory_chunk_detail
  'intelligence.memory_chunk_detail.copy_chunk_id': 'Copy chunk id',
  'intelligence.memory_chunk_detail.chunk_label': 'chunk {id}',
  'intelligence.memory_chunk_detail.copied': 'copied',
  'intelligence.memory_chunk_detail.embedding_present': 'bge-m3 1024dim',
  'intelligence.memory_chunk_detail.embedding_absent': 'no embedding',

  // accounts.add_account_modal
  'accounts.add_account_modal.title': 'Add account',
  'accounts.add_account_modal.close': 'Close',
  'accounts.add_account_modal.all_connected': "You've connected every supported app.",

  // accounts.respond_queue_panel
  'accounts.respond_queue_panel.unknown_time': 'unknown time',
  'accounts.respond_queue_panel.just_now': 'just now',
  'accounts.respond_queue_panel.minutes_ago': '{mins}m ago',
  'accounts.respond_queue_panel.hours_ago': '{hours}h ago',
  'accounts.respond_queue_panel.days_ago': '{days}d ago',
  'accounts.respond_queue_panel.title': 'Respond queue',
  'accounts.respond_queue_panel.pending_count': '{count} pending',
  'accounts.respond_queue_panel.refresh': 'Refresh',
  'accounts.respond_queue_panel.loading': 'Loading queue…',
  'accounts.respond_queue_panel.load_failed': 'Failed to load respond queue',
  'accounts.respond_queue_panel.empty': 'No pending provider events yet.',

  // upsell.banner / chat.runtime_provider / daemon / channels.selector / intelligence memory chunk / common / accounts.webview_host
  'upsell.banner.dismiss': 'Dismiss',
  'chat.runtime_provider.agent_error_message':
    'Something went wrong. Please try again.\nThis error has been reported. You can also report it on Discord.\n<aura-link path="community/discord">Report on Discord</aura-link>',
  'daemon.service_blocking_gate.retry_failed': 'Retry failed. Download the latest app build and try again.',
  'daemon.service_blocking_gate.title': 'Aura core is unavailable',
  'daemon.service_blocking_gate.description':
    'We could not recover the local core service after multiple attempts. This can happen after a critical crash or binary mismatch.',
  'daemon.service_blocking_gate.download_hint': 'Download the latest app build to restore a healthy core runtime.',
  'daemon.service_blocking_gate.retrying': 'Retrying...',
  'daemon.service_blocking_gate.retry_core': 'Retry Core',
  'daemon.service_blocking_gate.download_latest': 'Download Latest Version',
  'channels.selector.active_route_value': '{channel} via {authMode}',
  'channels.selector.no_active_route': 'No active route',
  'channels.selector.title': 'Channels',
  'channels.selector.active_route_label': 'Active route:',
  'intelligence.memory_chunk_letterhead.from': 'from',
  'intelligence.memory_chunk_letterhead.to': 'to',
  'intelligence.memory_chunk_mentioned.heading': 'm e n t i o n e d',
  'intelligence.memory_chunk_mentioned.chunk_count': '{count} chunk{count, plural, one{} other{s}}',
  'common.progress_indicator.progress_steps': 'Progress steps',
  'common.progress_indicator.step_of_total': 'Step {step} of {total}',
  'accounts.webview_host.almost_ready': 'Almost ready...',
  'accounts.webview_host.restoring_session': 'Restoring session...',
  'accounts.webview_host.webview_host_for_account': 'webview host for account {accountId}',
  'accounts.webview_host.loading_account': 'Loading account',
  'accounts.webview_host.loading_provider': 'Loading {providerName}...',
  'accounts.webview_host.webview_load_timeout': 'Webview load timeout',
  'accounts.webview_host.taking_longer_than_expected': '{providerName} is taking longer than expected.',
  'accounts.webview_host.timeout_retry_hint':
    'The embedded app may still be starting up. Retry to reload it without signing in again.',
  'accounts.webview_host.retry_loading': 'Retry loading',

  // channels.discord_channel_picker
  'channels.discord_channel_picker.server_channel_selection': 'Server & Channel Selection',
  'channels.discord_channel_picker.server_label': 'Server',
  'channels.discord_channel_picker.loading_servers': 'Loading servers...',
  'channels.discord_channel_picker.no_servers_found': 'No servers found',
  'channels.discord_channel_picker.select_a_server': 'Select a server',
  'channels.discord_channel_picker.no_servers_hint':
    'The bot is not in any servers. Invite it using the Discord Developer Portal.',
  'channels.discord_channel_picker.channel_label': 'Channel',
  'channels.discord_channel_picker.loading_channels': 'Loading channels...',
  'channels.discord_channel_picker.no_text_channels_found': 'No text channels found',
  'channels.discord_channel_picker.select_a_channel': 'Select a channel',
  'channels.discord_channel_picker.category_label': 'Category {categoryId}',
  'channels.discord_channel_picker.checking_permissions': 'Checking permissions...',
  'channels.discord_channel_picker.all_permissions_ok':
    'Bot has all required permissions in this channel.',
  'channels.discord_channel_picker.missing_permissions_label': 'Missing permissions: ',

  // app_update_prompt
  'app_update_prompt.default_error': 'Update failed. See logs for details.',
  'app_update_prompt.dismiss_aria_label': 'Dismiss update notification',
  'app_update_prompt.version_ready': 'Version {version} is ready to install.',
  'app_update_prompt.new_version_ready': 'A new version is ready to install.',
  'app_update_prompt.currently_on': 'Currently on {version}.',
  'app_update_prompt.restart_notice':
    'Restarting will close any open conversations briefly. The new build launches automatically.',
  'app_update_prompt.restart_now': 'Restart now',
  'app_update_prompt.later': 'Later',
  'app_update_prompt.generic_error': 'Something went wrong while updating.',
  'app_update_prompt.try_again': 'Try again',
  'app_update_prompt.dismiss': 'Dismiss',
  'app_update_prompt.header_ready_to_install': 'Update ready to install',
  'app_update_prompt.header_installing': 'Installing update',
  'app_update_prompt.header_restarting': 'Restarting…',
  'app_update_prompt.header_error': 'Update failed',
  'app_update_prompt.header_default': 'Update',
  'app_update_prompt.installing_new_version': 'Installing the new version…',
  'app_update_prompt.relaunching_app': 'Relaunching the app…',
  'app_update_prompt.downloaded': '{size} downloaded',
  'app_update_prompt.working': 'Working…',
  'app_update_prompt.show_less': 'Show less',
  'app_update_prompt.show_more': 'Show more',

  // settings.subscription_plans
  'settings.subscription_plans.choose_a_plan': 'Choose a Subscription Plan',
  'settings.subscription_plans.compare_plans_subtitle':
    'Compare plans, switch billing cadence, and choose a payment method.',
  'settings.subscription_plans.pay_using_crypto': 'Pay using crypto?',
  'settings.subscription_plans.pay_annually_crypto_hint':
    'You can optionally choose to pay annually using BTC/ETH/USDC.',
  'settings.subscription_plans.monthly': 'Monthly',
  'settings.subscription_plans.annual': 'Annual',
  'settings.subscription_plans.payment_confirmed': 'Payment confirmed! Your plan has been updated.',
  'settings.subscription_plans.waiting_for_payment_confirmation':
    'Waiting for payment confirmation... Complete checkout in the browser window that opened.',
  'settings.subscription_plans.popular_badge': 'Popular',
  'settings.subscription_plans.current_badge': 'Current',
  'settings.subscription_plans.per_month_suffix': '/mo',
  'settings.subscription_plans.billed_annual_price': 'Billed ${price}/yr',
  'settings.subscription_plans.save_percent': 'Save {percent}%',
  'settings.subscription_plans.current_plan': 'Current Plan',
  'settings.subscription_plans.waiting': 'Waiting…',
  'settings.subscription_plans.upgrade': 'Upgrade',

  // common.rotating_tetrahedron
  'common.rotating_tetrahedron.aria_label': 'Rotating inverted tetrahedron spacecraft',

  // intelligence.tasks_tab
  'intelligence.tasks_tab.thread_fallback_title': 'Thread {shortId}',
  'intelligence.tasks_tab.loading_task_boards': 'Loading task boards…',
  'intelligence.tasks_tab.failed_to_load': 'Failed to load task boards: {error}',
  'intelligence.tasks_tab.no_boards_yet': 'No agent task boards yet',
  'intelligence.tasks_tab.no_boards_hint':
    'Start a conversation and ask the agent to manage tasks — boards will appear here as cards are created.',
  'intelligence.tasks_tab.active_boards_count':
    '{count, plural, one{{count} active board across conversations} other{{count} active boards across conversations}}',
  'intelligence.tasks_tab.live_badge': 'live',

  // notifications.card
  'notifications.card.seconds_ago': '{count}s ago',
  'notifications.card.minutes_ago': '{count}m ago',
  'notifications.card.hours_ago': '{count}h ago',
  'notifications.card.days_ago': '{count}d ago',
  'notifications.card.importance_title': 'Importance: {percent}%',
  'notifications.card.dismiss_aria_label': 'Dismiss notification',

  // settings.screen_intelligence_permissions
  'settings.screen_intelligence_permissions.permissions_heading': 'Permissions',
  'settings.screen_intelligence_permissions.screen_recording_label': 'Screen Recording',
  'settings.screen_intelligence_permissions.accessibility_label': 'Accessibility',
  'settings.screen_intelligence_permissions.input_monitoring_label': 'Input Monitoring',
  'settings.screen_intelligence_permissions.grant_restart_hint':
    'After granting in System Settings, click “Restart & Refresh Permissions” so a new core process picks up the grants.',
  'settings.screen_intelligence_permissions.macos_privacy_prefix': 'macOS applies privacy to this executable:',
  'settings.screen_intelligence_permissions.requesting': 'Requesting…',
  'settings.screen_intelligence_permissions.request_screen_recording': 'Request Screen Recording',
  'settings.screen_intelligence_permissions.request_accessibility': 'Request Accessibility',
  'settings.screen_intelligence_permissions.open_input_monitoring': 'Open Input Monitoring',
  'settings.screen_intelligence_permissions.restarting_core': 'Restarting core…',
  'settings.screen_intelligence_permissions.restart_and_refresh_permissions': 'Restart & Refresh Permissions',
  'settings.screen_intelligence_permissions.refreshing': 'Refreshing…',
  'settings.screen_intelligence_permissions.refresh_status': 'Refresh Status',

  // upsell.usage_limit_modal
  'upsell.usage_limit_modal.reset_now': 'now',
  'upsell.usage_limit_modal.reset_in_minutes': 'in {mins}m',
  'upsell.usage_limit_modal.reset_in_hours_minutes': 'in {hours}h {mins}m',
  'upsell.usage_limit_modal.reset_in_hours': 'in {hours}h',
  'upsell.usage_limit_modal.reset_in_days_hours': 'in {days}d {hours}h',
  'upsell.usage_limit_modal.reset_in_days': 'in {days}d',
  'upsell.usage_limit_modal.resets_suffix': ' It resets {resetTime}.',
  'upsell.usage_limit_modal.body_weekly_limit':
    "You've hit your weekly limit.{resetSuffix} Upgrade your plan or top up credits to avoid limits.",
  'upsell.usage_limit_modal.body_rate_limit':
    "You've hit your 10-hour inference rate limit.{resetSuffix} Upgrade for higher limits.",
  'upsell.usage_limit_modal.title': 'Usage Limit Reached',
  'upsell.usage_limit_modal.plan_includes': '{planName} plan includes:',
  'upsell.usage_limit_modal.per_10h_window': '${amount} per 10-hour window',
  'upsell.usage_limit_modal.weekly_included_inference': '${amount}/week included inference',
  'upsell.usage_limit_modal.upgrade_plan': 'Upgrade Plan',
  'upsell.usage_limit_modal.not_now': 'Not Now',

  // channels.telegram_config
  'channels.telegram_config.managed_dm_connecting': 'Open Telegram and message the bot to complete setup.',
  'channels.telegram_config.managed_dm_timeout':
    'Managed DM verification timed out. Try connecting again.',
  'channels.telegram_config.field_required': '{field} is required',
  'channels.telegram_config.restart_required_error': 'Channel saved. Restart the app to activate it.',
  'channels.telegram_config.reconnect_button': 'Reconnect',
  'channels.telegram_config.connect_button': 'Connect',
  'channels.telegram_config.disconnect_button': 'Disconnect',

  // chat.unsubscribe_approval_card
  'chat.unsubscribe_approval_card.title': 'Unsubscribe Request',
  'chat.unsubscribe_approval_card.error_fallback': 'Missing permissions or network error',
  'chat.unsubscribe_approval_card.processing': 'Processing...',
  'chat.unsubscribe_approval_card.approve_button': 'Approve & Unsubscribe',
  'chat.unsubscribe_approval_card.deny_button': 'Deny',
  'chat.unsubscribe_approval_card.success_message': '✓ Successfully unsubscribed.',
  'chat.unsubscribe_approval_card.denied_message': '✕ Request denied.',

  // oauth.provider_button
  'oauth.provider_button.twitter_startup_failure':
    'Twitter/X sign-in could not start. Check that the Twitter OAuth app callback URL, client ID/secret, and requested scopes match the Aura backend, then try again.',
  'oauth.provider_button.generic_startup_failure': '{providerName} sign-in could not start. Please try again.',
  'oauth.provider_button.connecting': 'Connecting...',

  // skills.search_bar
  'skills.search_bar.placeholder': 'Search skills...',

  // common.persist_rehydration_screen
  'common.persist_rehydration_screen.title': 'Aura is taking longer than usual to load.',
  'common.persist_rehydration_screen.description':
    "Local app state may be corrupted. Resetting will sign you out and clear cached data on this device; you'll reconnect your accounts on next launch. Your data on the server is not affected.",
  'common.persist_rehydration_screen.resetting': 'Resetting…',
  'common.persist_rehydration_screen.reset_button': 'Reset local state',

  // common.route_loading_screen
  'common.route_loading_screen.initializing': 'Initializing Aura...',

  // privacy.what_leaves_link
  'privacy.what_leaves_link.label': 'What leaves my computer?',

  // settings.provider_key_dialog
  'settings.provider_key_dialog.api_key_required': 'Please paste your API key to continue.',
  'settings.provider_key_dialog.connect_title': 'Connect {label}',
  'settings.provider_key_dialog.subtitle': "Paste your API key. It's stored encrypted on this device only.",
  'settings.provider_key_dialog.api_key_label': 'API key',
  'settings.provider_key_dialog.cancel_button': 'Cancel',
  'settings.provider_key_dialog.saving': 'Saving…',
  'settings.provider_key_dialog.save_button': 'Save',

  // settings.workload_row
  'settings.workload_row.aura_default': 'Aura (default)',
  'settings.workload_row.default_button': 'Default',
  'settings.workload_row.custom_button': 'Custom',
};

export default en;
