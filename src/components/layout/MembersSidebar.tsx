import React from 'react';
import { useAppStore } from '../../stores/useAppStore';
import { Shield } from 'lucide-react';

export const MembersSidebar: React.FC = () => {
  const {
    isMembersSidebarOpen,
    selectedGuildId,
    members,
  } = useAppStore();

  if (!isMembersSidebarOpen) return null;

  const currentMembers = members[selectedGuildId] || [];

  const onlineMembers = currentMembers.filter((m) => m.status === 'online');
  const idleMembers = currentMembers.filter((m) => m.status === 'idle');
  const dndMembers = currentMembers.filter((m) => m.status === 'dnd');
  const offlineMembers = currentMembers.filter((m) => m.status === 'offline');

  const renderMemberGroup = (title: string, group: typeof currentMembers, statusColor: string) => {
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
                src={member.avatar || 'https://images.unsplash.com/photo-1535713875002-d1d0cf377fde?w=128'}
                alt={member.username}
                className="w-8 h-8 rounded-full object-cover border border-border-subtle"
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
                {member.username === 'DashOperator' && (
                  <Shield className="w-3 h-3 text-accent-cyan flex-shrink-0" />
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
