import React from 'react';
import { MessageSquare, Plus, Compass } from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';

export const ServerSidebar: React.FC = () => {
  const { guilds, selectedGuildId, setSelectedGuild } = useAppStore();

  return (
    <aside className="w-[72px] bg-base flex flex-col items-center py-3 gap-2 border-r border-border-subtle z-20 select-none">
      {/* Direct Messages Icon */}
      <div className="relative group">
        <div
          className={`absolute left-0 top-1/2 -translate-y-1/2 w-1 bg-accent-primary rounded-r transition-all duration-200 ${
            selectedGuildId === 'dm'
              ? 'h-9 opacity-100'
              : 'h-2 opacity-0 group-hover:opacity-100 group-hover:h-5'
          }`}
        />
        <button
          onClick={() => setSelectedGuild('dm')}
          title="Mensajes Directos"
          className={`w-12 h-12 flex items-center justify-center transition-all duration-200 ${
            selectedGuildId === 'dm'
              ? 'bg-accent-primary text-white rounded-2xl shadow-glow-sm'
              : 'bg-surface hover:bg-tertiary text-secondary_text hover:text-primary rounded-3xl hover:rounded-2xl border border-border-subtle'
          }`}
        >
          <MessageSquare className="w-5 h-5" />
        </button>
      </div>

      {/* Separator */}
      <div className="w-8 h-[2px] bg-border-subtle rounded my-1" />

      {/* Guilds List */}
      <div className="flex-1 w-full flex flex-col items-center gap-2 overflow-y-auto no-scrollbar">
        {guilds
          .filter((g) => g.id !== 'dm')
          .map((guild) => {
            const isSelected = selectedGuildId === guild.id;
            return (
              <div key={guild.id} className="relative group w-full flex justify-center">
                {/* Active/Hover Pill */}
                <div
                  className={`absolute left-0 top-1/2 -translate-y-1/2 w-1 bg-accent-cyan rounded-r transition-all duration-200 ${
                    isSelected
                      ? 'h-9 opacity-100'
                      : 'h-2 opacity-0 group-hover:opacity-100 group-hover:h-5'
                  }`}
                />

                <button
                  onClick={() => setSelectedGuild(guild.id)}
                  title={guild.name}
                  className={`w-12 h-12 flex items-center justify-center font-bold text-base transition-all duration-200 relative ${
                    isSelected
                      ? 'bg-surface border-2 border-accent-cyan text-primary rounded-2xl shadow-glow-sm'
                      : 'bg-surface/80 hover:bg-tertiary text-secondary_text hover:text-primary rounded-3xl hover:rounded-2xl border border-border-subtle'
                  }`}
                >
                  {guild.icon ? (
                    <span className="text-xl">{guild.icon}</span>
                  ) : (
                    guild.name.substring(0, 2).toUpperCase()
                  )}

                  {/* Unread Badge */}
                  {guild.unread_count && guild.unread_count > 0 ? (
                    <span className="absolute -bottom-1 -right-1 bg-accent-primary text-white text-[10px] font-bold px-1.5 py-0.5 rounded-full border-2 border-base">
                      {guild.unread_count}
                    </span>
                  ) : null}
                </button>
              </div>
            );
          })}

        {/* Add Server Placeholder */}
        <button
          title="Añadir Servidor"
          className="w-12 h-12 flex items-center justify-center bg-surface/50 hover:bg-emerald-600/20 text-emerald-400 rounded-3xl hover:rounded-2xl border border-border-subtle hover:border-emerald-500/40 transition-all duration-200"
        >
          <Plus className="w-5 h-5" />
        </button>

        {/* Explore Placeholder */}
        <button
          title="Explorar Comunidades"
          className="w-12 h-12 flex items-center justify-center bg-surface/50 hover:bg-accent-primary/20 text-accent-ice rounded-3xl hover:rounded-2xl border border-border-subtle hover:border-accent-primary/40 transition-all duration-200"
        >
          <Compass className="w-5 h-5" />
        </button>
      </div>
    </aside>
  );
};
