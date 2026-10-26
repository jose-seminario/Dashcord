import React, { useState, useEffect } from 'react';
import { Search, Hash, Volume2, Settings, Shield, X } from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';

export const CommandPalette: React.FC = () => {
  const {
    isCommandPaletteOpen,
    setCommandPaletteOpen,
    guilds,
    setSelectedGuild,
    setSelectedChannel,
    setSettingsOpen,
  } = useAppStore();

  const [query, setQuery] = useState('');

  // Keyboard shortcut listener (Ctrl+K or Cmd+K)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setCommandPaletteOpen(!isCommandPaletteOpen);
      } else if (e.key === 'Escape' && isCommandPaletteOpen) {
        setCommandPaletteOpen(false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isCommandPaletteOpen, setCommandPaletteOpen]);

  if (!isCommandPaletteOpen) return null;

  // Flatten searchable channels
  const allChannels = guilds.flatMap((g) =>
    g.channels.map((c) => ({
      guildName: g.name,
      guildId: g.id,
      ...c,
    }))
  );

  const filteredChannels = allChannels.filter(
    (c) =>
      c.name.toLowerCase().includes(query.toLowerCase()) ||
      c.guildName.toLowerCase().includes(query.toLowerCase())
  );

  const handleSelectChannel = (guildId: string, channelId: string) => {
    setSelectedGuild(guildId);
    setSelectedChannel(channelId);
    setCommandPaletteOpen(false);
  };

  return (
    <div className="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-start justify-center pt-24 animate-in fade-in duration-150">
      <div className="w-full max-w-xl bg-surface border border-border-subtle rounded-xl shadow-2xl overflow-hidden glass-dropdown">
        {/* Search Input Bar */}
        <div className="flex items-center px-4 py-3 border-b border-border-subtle gap-3">
          <Search className="w-5 h-5 text-accent-cyan" />
          <input
            autoFocus
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="¿A dónde quieres ir? (Escribe un canal o comando)..."
            className="flex-1 bg-transparent border-none outline-none text-primary placeholder-muted text-sm"
          />
          <button
            onClick={() => setCommandPaletteOpen(false)}
            className="p-1 text-muted hover:text-primary rounded"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Results List */}
        <div className="max-h-80 overflow-y-auto p-2 space-y-1">
          {/* Quick Actions */}
          <div className="px-2 py-1 text-[11px] font-bold text-muted uppercase tracking-wider">
            Acciones Rápidas
          </div>
          <button
            onClick={() => {
              setCommandPaletteOpen(false);
              setSettingsOpen(true);
            }}
            className="w-full flex items-center justify-between px-3 py-2 rounded-lg text-sm text-secondary_text hover:text-primary hover:bg-hover transition-colors"
          >
            <div className="flex items-center gap-2.5">
              <Settings className="w-4 h-4 text-accent-ice" />
              <span>Abrir Ajustes de DashCord</span>
            </div>
            <kbd className="text-[10px] bg-base px-2 py-0.5 rounded text-muted border border-border-subtle">
              Preferencias
            </kbd>
          </button>

          <button
            onClick={() => {
              setCommandPaletteOpen(false);
              setSettingsOpen(true);
            }}
            className="w-full flex items-center justify-between px-3 py-2 rounded-lg text-sm text-secondary_text hover:text-primary hover:bg-hover transition-colors"
          >
            <div className="flex items-center gap-2.5">
              <Shield className="w-4 h-4 text-emerald-400" />
              <span>Estado de Evasión TLS / Fingerprint</span>
            </div>
            <span className="text-xs text-emerald-400 font-mono">Activo</span>
          </button>

          {/* Channels Section */}
          <div className="px-2 pt-3 pb-1 text-[11px] font-bold text-muted uppercase tracking-wider">
            Canales
          </div>
          {filteredChannels.length === 0 ? (
            <div className="text-center py-6 text-sm text-muted">
              No se encontraron resultados para "{query}"
            </div>
          ) : (
            filteredChannels.map((c) => (
              <button
                key={`${c.guildId}-${c.id}`}
                onClick={() => handleSelectChannel(c.guildId, c.id)}
                className="w-full flex items-center justify-between px-3 py-2 rounded-lg text-sm text-secondary_text hover:text-primary hover:bg-hover transition-colors group"
              >
                <div className="flex items-center gap-2.5 min-w-0">
                  {c.type === 'voice' ? (
                    <Volume2 className="w-4 h-4 text-accent-cyan flex-shrink-0" />
                  ) : (
                    <Hash className="w-4 h-4 text-muted group-hover:text-accent-primary flex-shrink-0" />
                  )}
                  <span className="truncate">{c.name}</span>
                </div>
                <span className="text-xs text-muted font-normal truncate max-w-[150px]">
                  {c.guildName}
                </span>
              </button>
            ))
          )}
        </div>

        {/* Footer Navigation Hints */}
        <div className="bg-base/90 px-4 py-2 border-t border-border-subtle flex items-center justify-between text-[11px] text-muted">
          <span>Navega con flechas y presiona Enter</span>
          <span>ESC para cerrar</span>
        </div>
      </div>
    </div>
  );
};
