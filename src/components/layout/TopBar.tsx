import React, { useState } from 'react';
import { ShieldCheck, Wifi, Command, Settings, Users, Bell, AlertOctagon } from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';

export const TopBar: React.FC = () => {
  const {
    connectionStatus,
    setSettingsOpen,
    setCommandPaletteOpen,
    toggleMembersSidebar,
    isMembersSidebarOpen,
    selectedGuildId,
    selectedChannelId,
    guilds,
  } = useAppStore();

  // Estado para simulación de fail-closed (>30 días)
  const [isFailClosedBlocked] = useState(false);

  const currentGuild = guilds.find((g) => g.id === selectedGuildId);
  const currentChannel = currentGuild?.channels.find((c) => c.id === selectedChannelId);

  return (
    <>
      {/* Banner Fail-Closed si la versión supera 30 días sin actualización */}
      {isFailClosedBlocked && (
        <div className="bg-rose-600 text-white px-4 py-1 text-xs font-semibold flex items-center justify-between border-b border-rose-700 animate-pulse select-none z-50">
          <div className="flex items-center gap-2">
            <AlertOctagon className="w-4 h-4" />
            <span>
              BLOQUEO FAIL-CLOSED: Build number de Discord con más de 30 días de antigüedad sin actualización. Conexión al Gateway deshabilitada por protección de cuenta.
            </span>
          </div>
          <button
            onClick={() => setSettingsOpen(true)}
            className="underline hover:text-rose-100 text-[11px]"
          >
            Ver Detalles
          </button>
        </div>
      )}

      <header className="h-11 gradient-gaming-bar border-b border-border-subtle flex items-center justify-between px-3 z-30 select-none">
        {/* Left: Current Location */}
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-muted text-xs font-semibold tracking-wider uppercase">
            {currentGuild?.name || 'DashCord'}
          </span>
          <span className="text-muted">/</span>
          <span className="text-primary font-medium text-sm truncate flex items-center gap-1.5">
            <span className="text-accent-primary">#</span>
            {currentChannel?.name || 'general'}
          </span>
        </div>

        {/* Center: Command Palette Trigger */}
        <button
          onClick={() => setCommandPaletteOpen(true)}
          className="flex items-center gap-2 bg-secondary/80 hover:bg-tertiary text-secondary_text hover:text-primary px-3 py-1 rounded border border-border-subtle text-xs transition-all duration-150 shadow-inner group max-w-sm w-64 justify-between"
        >
          <span className="flex items-center gap-1.5">
            <Command className="w-3.5 h-3.5 text-accent-cyan" />
            <span>Buscar o saltar a...</span>
          </span>
          <kbd className="bg-base px-1.5 py-0.5 rounded text-[10px] text-muted border border-border-subtle group-hover:border-accent-primary/40">
            Ctrl + K
          </kbd>
        </button>

        {/* Right: Security Status, Latency & Controls */}
        <div className="flex items-center gap-3">
          {/* Evasion & Latency Indicator */}
          <div className="flex items-center gap-2 bg-base/60 px-2.5 py-1 rounded border border-border-subtle">
            <div className="flex items-center gap-1 text-[11px] text-emerald-400 font-medium">
              <ShieldCheck className="w-3.5 h-3.5 text-accent-cyan" />
              <span className="hidden sm:inline">TLS Spoof (rquest)</span>
            </div>
            <div className="w-[1px] h-3 bg-border-subtle" />
            <div className="flex items-center gap-1 text-[11px] text-accent-ice font-mono">
              <Wifi className="w-3 h-3 text-accent-primary" />
              <span>{connectionStatus.ping}ms</span>
            </div>
          </div>

          {/* Action Buttons */}
          <div className="flex items-center gap-1">
            <button
              title="Notificaciones"
              className="p-1.5 text-secondary_text hover:text-primary hover:bg-hover rounded transition-colors"
            >
              <Bell className="w-4 h-4" />
            </button>
            <button
              onClick={toggleMembersSidebar}
              title={isMembersSidebarOpen ? "Ocultar miembros" : "Mostrar miembros"}
              className={`p-1.5 rounded transition-colors ${
                isMembersSidebarOpen
                  ? 'text-accent-cyan bg-active'
                  : 'text-secondary_text hover:text-primary hover:bg-hover'
              }`}
            >
              <Users className="w-4 h-4" />
            </button>
            <button
              onClick={() => setSettingsOpen(true)}
              title="Ajustes de DashCord"
              className="p-1.5 text-secondary_text hover:text-primary hover:bg-hover rounded transition-colors"
            >
              <Settings className="w-4 h-4" />
            </button>
          </div>
        </div>
      </header>
    </>
  );
};
