import React from 'react';
import { useAppStore, resolveAvatarUrl } from '../../stores/useAppStore';
import { Shield } from 'lucide-react';

export const MembersSidebar: React.FC = () => {
  const {
    isMembersSidebarOpen,
    selectedGuildId,
    members,
    currentUser,
  } = useAppStore();

  if (!isMembersSidebarOpen) return null;

  const currentMembers = members[selectedGuildId] || [currentUser];

  const onlineMembers = currentMembers.filter((m) => m.status === 'online');
  const idleMembers = currentMembers.filter((m) => m.status === 'idle');
  const dndMembers = currentMembers.filter((m) => m.status === 'dnd');
  const offlineMembers = currentMembers.filter(
    (m) => !m.status || m.status === 'offline' || m.status === 'invisible'
  );

  const renderMemberGroup = (
    title: string,
    group: typeof currentMembers,
    statusColor: string
  ) => {
    if (group.length === 0) return null;
    return (
      <div className="space-y-1 mb-4">
        <h3 className="text-[11px] font-bold text-muted uppercase tracking-wider px-2">
          {title} — {group.length}
        </h3>
        {group.map((member) => (
          <div
            key={member.id}
            className="flex items-center gap-2.5 px-2 py-1.5 rounded-lg hover:bg-hover cursor-pointer group transition-colors"
          >
            <div className="relative flex-shrink-0">
              <img
                src={resolveAvatarUrl(member.id, member.avatar)}
                alt={member.username}
                className="w-8 h-8 rounded-full object-cover border border-border-subtle bg-base"
                onError={(e) => {
                  e.currentTarget.src = 'https://cdn.discordapp.com/embed/avatars/0.png';
                }}
              />
              <span
                className={`absolute bottom-0 right-0 w-2.5 h-2.5 rounded-full ring-2 ring-surface ${statusColor}`}
              />
            </div>
            <div className="flex flex-col min-w-0">
              <div className="flex items-center gap-1.5">
                <span className="text-xs font-medium text-secondary_text group-hover:text-primary truncate">
                  {member.global_name || member.username}
                </span>
                {member.id === currentUser.id && (
                  <Shield className="w-3 h-3 text-accent-cyan flex-shrink-0" />
                )}
                {member.is_bot && (
                  <span className="px-1 py-0.1 bg-accent-primary/20 border border-accent-primary/40 text-accent-cyan text-[8px] font-bold uppercase rounded">
                    BOT
                  </span>
                )}
              </div>
              {member.custom_status && (
                <span className="text-[10px] text-muted truncate">
                  {member.custom_status}
                </span>
              )}
            </div>
          </div>
        ))}
      </div>
    );
  };

  return (
    <aside className="w-60 bg-surface border-l border-border-subtle p-3 overflow-y-auto select-none">
      {renderMemberGroup('En línea', onlineMembers, 'bg-emerald-500')}
      {renderMemberGroup('Ausente / Idle', idleMembers, 'bg-amber-500')}
      {renderMemberGroup('No Molestar', dndMembers, 'bg-rose-500')}
      {renderMemberGroup('Desconectado', offlineMembers, 'bg-slate-500')}
    </aside>
  );
};
