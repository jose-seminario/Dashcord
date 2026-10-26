import React, { useEffect } from 'react';
import { TopBar } from './components/layout/TopBar';
import { ServerSidebar } from './components/layout/ServerSidebar';
import { ChannelSidebar } from './components/layout/ChannelSidebar';
import { ChatArea } from './components/layout/ChatArea';
import { MembersSidebar } from './components/layout/MembersSidebar';
import { CommandPalette } from './components/modals/CommandPalette';
import { SettingsModal } from './components/modals/SettingsModal';
import { useAppStore } from './stores/useAppStore';

export const App: React.FC = () => {
  const { uiDensity, reducedMotion } = useAppStore();

  useEffect(() => {
    if (reducedMotion) {
      document.documentElement.classList.add('reduce-motion');
    } else {
      document.documentElement.classList.remove('reduce-motion');
    }
  }, [reducedMotion]);

  return (
    <div
      className={`w-screen h-screen flex flex-col bg-base text-primary overflow-hidden font-sans select-none ${
        uiDensity === 'compact' ? 'text-xs' : 'text-sm'
      }`}
    >
      {/* Top Application Header */}
      <TopBar />

      {/* Main Multi-Column Split Workspace */}
      <div className="flex-1 flex min-h-0 overflow-hidden">
        {/* Leftmost Server List */}
        <ServerSidebar />

        {/* Channels & User Card Bar */}
        <ChannelSidebar />

        {/* Center Chat View */}
        <ChatArea />

        {/* Right Members Drawer */}
        <MembersSidebar />
      </div>

      {/* Global Command Center (Ctrl+K) */}
      <CommandPalette />

      {/* Global Settings Window */}
      <SettingsModal />
    </div>
  );
};

export default App;
