import { useEffect, useState } from 'react';
import { HashRouter as Router, Navigate, Route, Routes } from 'react-router-dom';
import ChatPage from './pages/Chat';
import HomePage from './pages/Home';
import HumanPage from './pages/Human';
import InboxPage from './pages/Notifications';
import IntelligencePage from './pages/Intelligence';
import InvitesPage from './pages/Invites';
import MeetingsPage from './pages/Meetings';
import OnboardingPage from './pages/Onboarding';
import RewardsPage from './pages/Rewards';
import SettingsPage from './pages/Settings';
import SkillsPage from './pages/Skills';
import { settingsGet } from './api';
import FloatingLauncher from './components/FloatingLauncher';
import LicenseGate from './components/LicenseGate';

// ---------------------------------------------------------------------------
// App shell — renders inside the floating panel
// ---------------------------------------------------------------------------

export function AppShell() {
  return (
    <div className="relative h-full flex flex-col overflow-hidden">
      <div className="absolute inset-0 pointer-events-none app-bg" />
      <div className="relative z-10 flex-1 flex flex-col overflow-hidden">
        <div className="h-full overflow-y-auto">
          <Routes>
            <Route path="/" element={<Navigate to="/home" replace />} />
            <Route path="/home" element={<HomePage />} />
            <Route path="/human" element={<HumanPage />} />
            <Route path="/chat" element={<ChatPage />} />
            <Route path="/skills" element={<SkillsPage />} />
            <Route path="/meetings" element={<MeetingsPage />} />
            <Route path="/intelligence" element={<IntelligencePage />} />
            <Route path="/rewards" element={<RewardsPage />} />
            <Route path="/inbox" element={<InboxPage />} />
            <Route path="/invites" element={<InvitesPage />} />
            <Route path="/settings/*" element={<SettingsPage />} />
            <Route path="*" element={<Navigate to="/home" replace />} />
          </Routes>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Onboarding gate — checks DB on first launch
// ---------------------------------------------------------------------------

export function AppGate() {
  const [ready, setReady] = useState(false);
  const [needsOnboarding, setNeedsOnboarding] = useState(false);

  useEffect(() => {
    settingsGet('onboarding_complete')
      .then(val => {
        setNeedsOnboarding(val !== '1');
        setReady(true);
      })
      .catch(() => {
        setNeedsOnboarding(false);
        setReady(true);
      });
  }, []);

  if (!ready) {
    return (
      <div className="fixed inset-0 bg-stone-50 flex items-center justify-center">
        <div className="w-8 h-8 border-2 border-primary-500 border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (needsOnboarding) {
    return <OnboardingPage onComplete={() => setNeedsOnboarding(false)} />;
  }

  return <AppShell />;
}

// ---------------------------------------------------------------------------
// Root
// ---------------------------------------------------------------------------

function App() {
  return (
    <Router>
      <FloatingLauncher>
        <LicenseGate>
          <AppGate />
        </LicenseGate>
      </FloatingLauncher>
    </Router>
  );
}

export default App;
