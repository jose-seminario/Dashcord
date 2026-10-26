import React from 'react';
import { Hash, Volume2, Megaphone, Mic, Headphones, Settings, ChevronDown } from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';

export const ChannelSidebar: React.FC = () => {
  const {
    guilds,
    selectedGuildId,
    selectedChannelId,
    setSelectedChannel,
    currentUser,
    setSettingsOpen,
  } = useAppStore();

  const currentGuild = guilds.find((g) => g.id === selectedGuildId);
  const channels = currentGuild?.channels || [];

  const getChannelIcon = (type: string) => {
    switch (type) {
      case 'voice':
        return <Volume2 className="w-4 h-4 text-accent-cyan" />;
      case 'announcement':
        return <Megaphone className="w-4 h-4 text-amber-400" />;
      default:
        return <Hash className="w-4 h-4 text-muted" />;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'online':
        return 'bg-emerald-500';
      case 'idle':
        return 'bg-amber-500';
      case 'dnd':
        return 'bg-rose-500';
      default:
        return 'bg-slate-500';
    }
  };

  return (
    <nav className="w-60 bg-surface flex flex-col border-r border-border-subtle select-none">
      {/* Guild Header Dropdown */}
      <button className="h-12 px-4 flex items-center justify-between border-b border-border-subtle hover:bg-tertiary/60 transition-colors">
        <span className="font-semibold text-sm truncate text-primary">
          {currentGuild?.name || 'DashCord'}
        </span>
        <ChevronDown className="w-4 h-4 text-muted" />
      </button>

      {/* Channels List */}
      <div className="flex-1 overflow-y-auto px-2 py-3 space-y-1">
        <div className="text-[11px] font-bold text-muted uppercase tracking-wider px-2 py-1 flex items-center justify-between">
          <span>Canales de Texto</span>
        </div>

        {channels.map((channel) => {
          const isSelected = selectedChannelId === channel.id;
          return (
            <button
              key={channel.id}
              onClick={() => setSelectedChannel(channel.id)}
              className={`w-full flex items-center gap-2 px-2.5 py-1.5 rounded text-sm transition-all duration-150 ${
                isSelected
                  ? 'bg-active text-primary font-medium border-l-2 border-accent-cyan'
                  : 'text-secondary_text hover:text-primary hover:bg-hover'
              }`}
            >
              {getChannelIcon(channel.type)}
              <span className="truncate">{channel.name}</span>
              {channel.unread && !isSelected && (
                <div className="w-1.5 h-1.5 rounded-full bg-accent-cyan ml-auto" />
              )}
            </button>
          );
        })}
      </div>

      {/* User Bottom Status Card */}
      <div className="h-14 bg-base/80 border-t border-border-subtle px-2 flex items-center justify-between">
        <div className="flex items-center gap-2 min-w-0">
          <div className="relative">
            <img
              src={currentUser.avatar || ''}
              alt={currentUser.username}
              className="w-8 h-8 rounded-full object-cover border border-border-subtle"
            />
            <span
              className={`absolute bottom-0 right-0 w-2.5 h-2.5 rounded-full ring-2 ring-base ${getStatusColor(
                currentUser.status
              )}`}
            />
          </div>
          <div className="flex flex-col min-w-0">
            <span className="text-xs font-semibold text-primary truncate leading-tight">
              {currentUser.global_name || currentUser.username}
            </span>
            <span className="text-[10px] text-muted truncate">
              {currentUser.custom_status || `#${currentUser.discriminator}`}
            </span>
          </div>
        </div>

        {/* User Quick Action Toggles */}
        <div className="flex items-center gap-0.5">
          <button
            title="Silenciar micrófono"
            className="p-1.5 text-secondary_text hover:text-primary hover:bg-hover rounded transition-colors"
          >
            <Mic className="w-3.5 h-3.5" />
          </button>
          <button
            title="Ensordecer audio"
            className="p-1.5 text-secondary_text hover:text-primary hover:bg-hover rounded transition-colors"
          >
            <Headphones className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={() => setSettingsOpen(true)}
            title="Ajustes de usuario"
            className="p-1.5 text-secondary_text hover:text-primary hover:bg-hover rounded transition-colors"
          >
            <Settings className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </nav>
  );
};
