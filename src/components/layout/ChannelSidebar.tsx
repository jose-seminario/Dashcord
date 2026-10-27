import React from 'react';
import {
  Hash,
  Volume2,
  Megaphone,
  Mic,
  MicOff,
  Headphones,
  Settings,
  ChevronDown,
  PhoneOff,
  Radio,
} from 'lucide-react';
import { useAppStore, resolveAvatarUrl } from '../../stores/useAppStore';

export const ChannelSidebar: React.FC = () => {
  const {
    guilds,
    selectedGuildId,
    selectedChannelId,
    setSelectedChannel,
    joinVoiceChannel,
    leaveVoiceChannel,
    currentUser,
    voiceStates,
    voiceConnection,
    setSettingsOpen,
  } = useAppStore();

  const currentGuild = guilds.find((g) => g.id === selectedGuildId);
  const channels = currentGuild?.channels || [];
  const activeVoiceChannel = channels.find((c) => c.id === voiceConnection.channel_id);

  const getChannelIcon = (type: string) => {
    switch (type) {
      case 'voice':
        return <Volume2 className="w-4 h-4 text-accent-cyan flex-shrink-0" />;
      case 'announcement':
        return <Megaphone className="w-4 h-4 text-amber-400 flex-shrink-0" />;
      default:
        return <Hash className="w-4 h-4 text-muted flex-shrink-0" />;
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

  const userAvatarSrc = resolveAvatarUrl(currentUser.id, currentUser.avatar);

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
          <span>Canales de Texto y Voz</span>
          <span className="text-[10px] text-muted/60">{channels.length}</span>
        </div>

        {channels.length === 0 ? (
          <div className="p-3 text-xs text-muted text-center italic">
            No hay canales disponibles
          </div>
        ) : (
          channels.map((channel) => {
            const isSelected = selectedChannelId === channel.id;
            const isVoiceConnectedHere =
              channel.type === 'voice' &&
              voiceConnection.state !== 'disconnected' &&
              voiceConnection.channel_id === channel.id;
            const voiceUsers = voiceStates[channel.id] || [];

            return (
              <div key={channel.id} className="space-y-0.5">
                <button
                  onClick={() => {
                    if (channel.type === 'voice' && currentGuild) {
                      joinVoiceChannel(currentGuild.id, channel.id);
                    } else {
                      setSelectedChannel(channel.id);
                    }
                  }}
                  className={`w-full flex items-center gap-2 px-2.5 py-1.5 rounded text-sm transition-all duration-150 ${
                    isSelected || isVoiceConnectedHere
                      ? 'bg-active text-primary font-medium border-l-2 border-accent-cyan'
                      : 'text-secondary_text hover:text-primary hover:bg-hover'
                  }`}
                >
                  {getChannelIcon(channel.type)}
                  <span className="truncate">{channel.name}</span>
                  {voiceUsers.length > 0 && (
                    <span className="ml-auto text-[10px] font-mono px-1.5 py-0.2 rounded-full bg-accent-cyan/15 text-accent-cyan border border-accent-cyan/30">
                      {voiceUsers.length}
                    </span>
                  )}
                  {channel.unread && !isSelected && voiceUsers.length === 0 && (
                    <div className="w-1.5 h-1.5 rounded-full bg-accent-cyan ml-auto" />
                  )}
                </button>

                {/* Usuarios conectados al canal de voz */}
                {voiceUsers.length > 0 && (
                  <div className="pl-6 pr-2 py-1 space-y-1">
                    {voiceUsers.map((vu) => (
                      <div
                        key={vu.user_id}
                        className="flex items-center gap-2 px-2 py-1 rounded-md bg-base/40 border border-border-subtle/50 text-xs text-secondary_text"
                      >
                        <div className="relative flex-shrink-0">
                          <img
                            src={resolveAvatarUrl(vu.user_id, vu.avatar)}
                            alt={vu.username}
                            className="w-5 h-5 rounded-full object-cover ring-1 ring-emerald-400/70"
                            onError={(e) => {
                              e.currentTarget.src =
                                'https://cdn.discordapp.com/embed/avatars/0.png';
                            }}
                          />
                        </div>
                        <span className="truncate flex-1 text-[11px] font-medium text-primary">
                          {vu.username}
                        </span>
                        {vu.self_mute ? (
                          <MicOff className="w-3 h-3 text-rose-400 flex-shrink-0" />
                        ) : (
                          <Volume2 className="w-3 h-3 text-emerald-400 flex-shrink-0 animate-pulse" />
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>

      {/* Active Voice Gateway Connection Status Bar */}
      {voiceConnection.state !== 'disconnected' && (
        <div className="bg-base/95 border-t border-accent-cyan/30 px-3 py-2 flex items-center justify-between gap-2">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <Radio
                className={`w-3.5 h-3.5 flex-shrink-0 ${
                  voiceConnection.state === 'connected'
                    ? 'text-emerald-400 animate-pulse'
                    : voiceConnection.state === 'error'
                    ? 'text-rose-400'
                    : 'text-amber-400 animate-spin'
                }`}
              />
              <span
                className={`text-xs font-semibold truncate ${
                  voiceConnection.state === 'connected'
                    ? 'text-emerald-400'
                    : voiceConnection.state === 'error'
                    ? 'text-rose-400'
                    : 'text-amber-400'
                }`}
              >
                {voiceConnection.state === 'connected'
                  ? 'Voz Conectada (UDP)'
                  : voiceConnection.state === 'error'
                  ? 'Error de Voz'
                  : 'Handshake de Voz...'}
              </span>
            </div>
            <div className="text-[10px] text-muted truncate font-mono mt-0.5">
              {voiceConnection.ip && voiceConnection.port
                ? `${voiceConnection.ip}:${voiceConnection.port} • ${voiceConnection.encryption_mode || 'xsalsa20'}`
                : activeVoiceChannel?.name || 'Estableciendo túnel...'}
            </div>
          </div>

          <button
            onClick={() =>
              leaveVoiceChannel(voiceConnection.guild_id || selectedGuildId)
            }
            title="Desconectar del canal de voz"
            className="p-1.5 rounded-md bg-rose-500/15 hover:bg-rose-500/30 text-rose-400 border border-rose-500/30 transition-colors flex-shrink-0"
          >
            <PhoneOff className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {/* User Bottom Status Card */}
      <div className="h-14 bg-base/80 border-t border-border-subtle px-2 flex items-center justify-between">
        <div className="flex items-center gap-2 min-w-0">
          <div className="relative">
            <img
              src={userAvatarSrc}
              alt={currentUser.username}
              className="w-8 h-8 rounded-full object-cover border border-border-subtle bg-surface"
              onError={(e) => {
                e.currentTarget.src = 'https://cdn.discordapp.com/embed/avatars/0.png';
              }}
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
              {currentUser.custom_status || `@${currentUser.username}`}
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
