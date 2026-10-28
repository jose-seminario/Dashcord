import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import {
  Guild,
  Message,
  User,
  ConnectionStatus,
  VoiceStateUser,
  PresenceUpdate,
  VoiceConnectionStatus,
} from '../types/discord';

let connectionTimeoutId: ReturnType<typeof setTimeout> | null = null;

function clearGatewayTimeout() {
  if (connectionTimeoutId) {
    clearTimeout(connectionTimeoutId);
    connectionTimeoutId = null;
  }
}

export function resolveAvatarUrl(userId?: string, avatarHash?: string | null): string {
  if (avatarHash) {
    if (avatarHash.startsWith('http')) return avatarHash;
    return `https://cdn.discordapp.com/avatars/${userId}/${avatarHash}.png?size=128`;
  }
  try {
    if (userId && /^\d+$/.test(userId)) {
      const idx = Number((BigInt(userId) >> 22n) % 6n);
      return `https://cdn.discordapp.com/embed/avatars/${idx}.png`;
    }
  } catch {
    // ignore BigInt fallback error
  }
  return 'https://cdn.discordapp.com/embed/avatars/0.png';
}

export function formatDiscordTimestamp(isoOrText?: string): string {
  if (!isoOrText) {
    const now = new Date();
    return `Hoy a las ${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}`;
  }
  if (isoOrText.startsWith('Hoy') || isoOrText.startsWith('Ayer')) {
    return isoOrText;
  }
  const date = new Date(isoOrText);
  if (isNaN(date.getTime())) {
    return isoOrText;
  }

  const now = new Date();
  const isToday =
    date.getDate() === now.getDate() &&
    date.getMonth() === now.getMonth() &&
    date.getFullYear() === now.getFullYear();

  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  const isYesterday =
    date.getDate() === yesterday.getDate() &&
    date.getMonth() === yesterday.getMonth() &&
    date.getFullYear() === yesterday.getFullYear();

  const timeStr = `${date.getHours().toString().padStart(2, '0')}:${date.getMinutes().toString().padStart(2, '0')}`;

  if (isToday) {
    return `Hoy a las ${timeStr}`;
  }
  if (isYesterday) {
    return `Ayer a las ${timeStr}`;
  }

  const day = date.getDate().toString().padStart(2, '0');
  const month = (date.getMonth() + 1).toString().padStart(2, '0');
  const year = date.getFullYear();
  return `${day}/${month}/${year} ${timeStr}`;
}

export function parseRawDiscordMessage(raw: any): Message {
  const authorId = raw.author?.id || 'unknown';
  const authorUser: User = {
    id: authorId,
    username: raw.author?.username || 'Usuario',
    discriminator: raw.author?.discriminator || '0000',
    global_name: raw.author?.global_name || raw.author?.username || 'Usuario',
    avatar: resolveAvatarUrl(authorId, raw.author?.avatar),
    status: 'online',
    is_bot: !!raw.author?.bot,
  };

  let replyTo: Message['reply_to'] = undefined;
  if (raw.referenced_message && raw.referenced_message.author) {
    replyTo = {
      id: raw.referenced_message.id || '',
      author:
        raw.referenced_message.author.global_name ||
        raw.referenced_message.author.username ||
        'Usuario',
      content: raw.referenced_message.content || 'Archivo adjunto / Embed',
    };
  }

  const reactions = Array.isArray(raw.reactions)
    ? raw.reactions.map((r: any) => ({
        emoji: r.emoji?.name || '🔥',
        count: r.count || 1,
        me: !!r.me,
      }))
    : undefined;

  const attachments = Array.isArray(raw.attachments)
    ? raw.attachments.map((a: any) => ({
        id: a.id || '',
        filename: a.filename || 'archivo',
        url: a.url || '',
        size: a.size || 0,
      }))
    : undefined;

  return {
    id: raw.id || `msg_${Date.now()}`,
    channel_id: raw.channel_id,
    author: authorUser,
    content: raw.content || '',
    timestamp: formatDiscordTimestamp(raw.timestamp),
    edited_timestamp: raw.edited_timestamp,
    attachments,
    reactions,
    reply_to: replyTo,
  };
}

interface AppState {
  // Navigation & Selection
  selectedGuildId: string;
  selectedChannelId: string;
  isMembersSidebarOpen: boolean;
  isSettingsOpen: boolean;
  isCommandPaletteOpen: boolean;

  // Connection & Auth
  isConnecting: boolean;
  isAuthenticated: boolean;
  authError: string | null;

  // Real Data
  currentUser: User;
  guilds: Guild[];
  messages: Record<string, Message[]>;               // channel_id -> messages
  loadingChannels: Record<string, boolean>;          // channel_id -> loading state
  members: Record<string, User[]>;                   // guild_id -> members
  voiceStates: Record<string, VoiceStateUser[]>;     // channel_id -> users in voice
  voiceConnection: VoiceConnectionStatus;
  connectionStatus: ConnectionStatus;

  // Settings & Theme
  accentColor: string;
  uiDensity: 'compact' | 'comfortable';
  reducedMotion: boolean;

  // Actions
  setSelectedGuild: (id: string) => void;
  setSelectedChannel: (id: string) => void;
  joinVoiceChannel: (guildId: string, channelId: string) => Promise<void>;
  leaveVoiceChannel: (guildId: string) => Promise<void>;
  toggleVoiceMute: () => Promise<void>;
  toggleVoiceDeafen: () => Promise<void>;
  toggleMembersSidebar: () => void;
  setSettingsOpen: (open: boolean) => void;
  setCommandPaletteOpen: (open: boolean) => void;
  fetchMessagesForChannel: (channelId: string) => Promise<void>;
  sendMessage: (channelId: string, content: string) => Promise<void>;
  setAccentColor: (color: string) => void;
  setUiDensity: (density: 'compact' | 'comfortable') => void;
  setReducedMotion: (reduced: boolean) => void;

  setConnecting: (connecting: boolean) => void;
  setAuthError: (error: string | null) => void;
  handleGatewayReady: (payload: { user: User; guilds: Guild[] }) => void;
  handleGatewayMessage: (rawMessage: any) => void;
  handlePresenceUpdate: (update: PresenceUpdate) => void;
  handleVoiceStateUpdate: (vs: VoiceStateUser) => void;
  handleVoiceConnectionStatus: (status: VoiceConnectionStatus) => void;
  handleGatewayError: (error: string) => void;
}

const MOCK_CURRENT_USER: User = {
  id: 'usr_me',
  username: 'DashOperator',
  discriminator: '0001',
  global_name: 'GhostProtocol',
  avatar: 'https://cdn.discordapp.com/embed/avatars/0.png',
  status: 'online',
  custom_status: '⚡ Running DashCord via BoringSSL TLS',
};

const MOCK_GUILDS: Guild[] = [
  {
    id: 'guild_1',
    name: 'Cyberpunk Syndicate',
    icon: '⚡',
    unread_count: 0,
    channels: [
      { id: 'c_general', guild_id: 'guild_1', name: 'general-chat', type: 'text' },
    ],
  },
];

export const useAppStore = create<AppState>((set, get) => ({
  selectedGuildId: 'guild_1',
  selectedChannelId: 'c_general',
  isMembersSidebarOpen: true,
  isSettingsOpen: false,
  isCommandPaletteOpen: false,

  isConnecting: false,
  isAuthenticated: false,
  authError: null,

  currentUser: MOCK_CURRENT_USER,
  guilds: MOCK_GUILDS,
  messages: {},
  loadingChannels: {},
  members: {},
  voiceStates: {},
  voiceConnection: {
    state: 'disconnected',
  },

  connectionStatus: {
    gateway: 'disconnected',
    ping: 28,
    tls_spoofed: true,
    build_number: 382000,
  },

  accentColor: '#3B82F6',
  uiDensity: 'comfortable',
  reducedMotion: false,

  setSelectedGuild: (id) => {
    const state = get();
    const targetGuild = state.guilds.find((g) => g.id === id);
    // Seleccionar preferentemente el primer canal de texto
    const firstTextChannel =
      targetGuild?.channels.find((c) => c.type === 'text' || c.type === 'announcement') ||
      targetGuild?.channels[0];
    const firstChannelId = firstTextChannel?.id || '';

    set({
      selectedGuildId: id,
      selectedChannelId: firstChannelId,
    });

    if (firstChannelId && state.isAuthenticated) {
      get().fetchMessagesForChannel(firstChannelId);
    }
  },

  setSelectedChannel: (id) => {
    set({ selectedChannelId: id });
    if (id && get().isAuthenticated) {
      get().fetchMessagesForChannel(id);
    }
  },

  joinVoiceChannel: async (guildId, channelId) => {
    if (!guildId || !channelId) return;
    set({
      voiceConnection: {
        state: 'connecting',
        guild_id: guildId,
        channel_id: channelId,
      },
    });
    try {
      await invoke('join_voice_channel', { guildId, channelId });
    } catch (err) {
      console.error('[VOICE] Error solicitando unión a canal de voz:', err);
      set({
        voiceConnection: {
          state: 'error',
          guild_id: guildId,
          channel_id: channelId,
          error: String(err),
        },
      });
    }
  },

  leaveVoiceChannel: async (guildId) => {
    set((s) => ({
      voiceConnection: {
        state: 'disconnected',
        self_mute: s.voiceConnection.self_mute,
        self_deaf: s.voiceConnection.self_deaf,
        speaking: false,
      },
    }));
    if (!guildId) return;
    try {
      await invoke('leave_voice_channel', { guildId });
    } catch (err) {
      console.error('[VOICE] Error desconectando del canal de voz:', err);
    }
  },

  toggleVoiceMute: async () => {
    const state = get();
    const nextMuted = !state.voiceConnection.self_mute;
    const nextDeafened = nextMuted ? !!state.voiceConnection.self_deaf : false;

    set((s) => ({
      voiceConnection: {
        ...s.voiceConnection,
        self_mute: nextMuted,
        self_deaf: nextDeafened,
        speaking: nextMuted ? false : s.voiceConnection.speaking,
      },
    }));

    try {
      const updated = await invoke<VoiceConnectionStatus>('set_voice_mute_deaf', {
        guildId: state.voiceConnection.guild_id || state.selectedGuildId,
        muted: nextMuted,
        deafened: nextDeafened,
      });
      if (updated) {
        set({ voiceConnection: updated });
      }
    } catch (err) {
      console.error('[VOICE] Error alternando Mute:', err);
    }
  },

  toggleVoiceDeafen: async () => {
    const state = get();
    const nextDeafened = !state.voiceConnection.self_deaf;
    const nextMuted = nextDeafened ? true : false;

    set((s) => ({
      voiceConnection: {
        ...s.voiceConnection,
        self_mute: nextMuted,
        self_deaf: nextDeafened,
        speaking: nextDeafened ? false : s.voiceConnection.speaking,
      },
    }));

    try {
      const updated = await invoke<VoiceConnectionStatus>('set_voice_mute_deaf', {
        guildId: state.voiceConnection.guild_id || state.selectedGuildId,
        muted: nextMuted,
        deafened: nextDeafened,
      });
      if (updated) {
        set({ voiceConnection: updated });
      }
    } catch (err) {
      console.error('[VOICE] Error alternando Deafen:', err);
    }
  },

  toggleMembersSidebar: () => set((state) => ({ isMembersSidebarOpen: !state.isMembersSidebarOpen })),
  setSettingsOpen: (open) => set({ isSettingsOpen: open }),
  setCommandPaletteOpen: (open) => set({ isCommandPaletteOpen: open }),

  fetchMessagesForChannel: async (channelId: string) => {
    if (!channelId || channelId.startsWith('c_') || channelId.startsWith('dm_')) return;

    set((state) => ({
      loadingChannels: { ...state.loadingChannels, [channelId]: true },
    }));

    try {
      const rawMessages = await invoke<any[]>('fetch_channel_messages', {
        channelId,
        limit: 50,
      });

      if (Array.isArray(rawMessages)) {
        // Discord devuelve los mensajes del más reciente al más antiguo; los invertimos para orden cronológico
        const chronological = [...rawMessages].reverse().map(parseRawDiscordMessage);

        set((state) => {
          // También enriquecemos la lista de miembros del servidor con los autores de los mensajes
          const guildId = state.selectedGuildId;
          const existingMembers = [...(state.members[guildId] || [])];
          const existingIds = new Set(existingMembers.map((m) => m.id));

          for (const msg of chronological) {
            if (msg.author.id && !existingIds.has(msg.author.id)) {
              existingIds.add(msg.author.id);
              existingMembers.push({
                ...msg.author,
                status: 'offline',
              });
            }
          }

          return {
            messages: {
              ...state.messages,
              [channelId]: chronological,
            },
            members: {
              ...state.members,
              [guildId]: existingMembers,
            },
            loadingChannels: {
              ...state.loadingChannels,
              [channelId]: false,
            },
          };
        });
      } else {
        set((state) => ({
          loadingChannels: { ...state.loadingChannels, [channelId]: false },
        }));
      }
    } catch (err) {
      console.error(`[REST] Error cargando mensajes del canal ${channelId}:`, err);
      set((state) => ({
        loadingChannels: { ...state.loadingChannels, [channelId]: false },
      }));
    }
  },

  sendMessage: async (channelId, content) => {
    const trimmed = content.trim();
    if (!trimmed || !channelId) return;

    const state = get();
    const tempId = `temp_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;

    const optimisticMsg: Message = {
      id: tempId,
      channel_id: channelId,
      author: state.currentUser,
      content: trimmed,
      timestamp: formatDiscordTimestamp(new Date().toISOString()),
      pending: true,
    };

    // 1. Actualización optimista inmediata en la UI
    set((s) => {
      const currentList = s.messages[channelId] || [];
      return {
        messages: {
          ...s.messages,
          [channelId]: [...currentList, optimisticMsg],
        },
      };
    });

    // Si estamos en canal mock sin autenticar, terminar aquí
    if (!state.isAuthenticated || channelId.startsWith('c_')) {
      set((s) => ({
        messages: {
          ...s.messages,
          [channelId]: (s.messages[channelId] || []).map((m) =>
            m.id === tempId ? { ...m, pending: false } : m
          ),
        },
      }));
      return;
    }

    // 2. Enviar POST real a Discord API v10 vía wreq en Rust
    try {
      const realRaw = await invoke<any>('send_discord_message', {
        channelId,
        content: trimmed,
      });

      const realMsg = parseRawDiscordMessage(realRaw);

      set((s) => {
        const list = s.messages[channelId] || [];
        // Si MESSAGE_CREATE ya insertó el mensaje real por WebSocket, eliminamos el temporal
        const alreadyFromGateway = list.some((m) => m.id === realMsg.id);
        const updatedList = alreadyFromGateway
          ? list.filter((m) => m.id !== tempId)
          : list.map((m) => (m.id === tempId ? realMsg : m));

        return {
          messages: {
            ...s.messages,
            [channelId]: updatedList,
          },
        };
      });
    } catch (err) {
      console.error('[REST] Error enviando mensaje:', err);
      set((s) => ({
        messages: {
          ...s.messages,
          [channelId]: (s.messages[channelId] || []).map((m) =>
            m.id === tempId ? { ...m, pending: false, failed: true } : m
          ),
        },
      }));
    }
  },

  setConnecting: (connecting) => {
    clearGatewayTimeout();
    set({ isConnecting: connecting });

    if (connecting) {
      connectionTimeoutId = setTimeout(() => {
        const current = get();
        if (current.isConnecting && !current.isAuthenticated) {
          set((state) => ({
            isConnecting: false,
            authError: 'Tiempo de espera agotado. Verifica tu conexión o tu token',
            isSettingsOpen: true,
            connectionStatus: {
              ...state.connectionStatus,
              gateway: 'disconnected',
            },
          }));
        }
      }, 10000);
    }
  },

  setAuthError: (error) => set({ authError: error }),

  handleGatewayReady: (payload) => {
    clearGatewayTimeout();
    if (!payload || !payload.user) return;
    const { user, guilds } = payload;

    const currentState = get();
    if (
      currentState.isAuthenticated &&
      currentState.currentUser.id === user.id &&
      !currentState.isConnecting
    ) {
      return;
    }

    const updatedUser: User = {
      ...currentState.currentUser,
      ...user,
      avatar: resolveAvatarUrl(user.id, user.avatar),
      status: 'online',
    };

    const mergedGuilds = guilds && guilds.length > 0 ? guilds : currentState.guilds;
    const firstGuild = mergedGuilds[0];
    const initialGuildId = firstGuild?.id || '';
    const firstTextChannel =
      firstGuild?.channels.find((c) => c.type === 'text' || c.type === 'announcement') ||
      firstGuild?.channels[0];
    const initialChannelId = firstTextChannel?.id || '';

    // Construir mapa inicial de miembros y estados de voz por servidor
    const newMembersMap: Record<string, User[]> = {};
    const newVoiceMap: Record<string, VoiceStateUser[]> = {};

    for (const g of mergedGuilds) {
      const gMembers: User[] = (g.members || []).map((m) => ({
        ...m,
        avatar: resolveAvatarUrl(m.id, m.avatar),
        status: m.status || 'offline',
      }));

      if (!gMembers.some((m) => m.id === updatedUser.id)) {
        gMembers.unshift(updatedUser);
      }
      newMembersMap[g.id] = gMembers;

      if (Array.isArray(g.voice_states)) {
        for (const vs of g.voice_states) {
          if (vs.channel_id) {
            const chList = newVoiceMap[vs.channel_id] || [];
            chList.push({
              ...vs,
              avatar: resolveAvatarUrl(vs.user_id, vs.avatar),
            });
            newVoiceMap[vs.channel_id] = chList;
          }
        }
      }
    }

    set((state) => ({
      currentUser: updatedUser,
      guilds: mergedGuilds,
      members: newMembersMap,
      voiceStates: newVoiceMap,
      selectedGuildId: initialGuildId,
      selectedChannelId: initialChannelId,
      isAuthenticated: true,
      isConnecting: false,
      authError: null,
      isSettingsOpen: false,
      connectionStatus: {
        ...state.connectionStatus,
        gateway: 'connected',
      },
    }));

    // Cargar historial del canal inicial automáticamente
    if (initialChannelId) {
      get().fetchMessagesForChannel(initialChannelId);
    }
  },

  handleGatewayMessage: (raw) => {
    if (!raw || !raw.channel_id) return;
    const newMsg = parseRawDiscordMessage(raw);

    set((state) => {
      const currentList = state.messages[raw.channel_id] || [];

      // Si ya existe con el mismo ID exacto, ignorar
      if (currentList.some((m) => m.id === newMsg.id)) {
        return state;
      }

      // Si hay un mensaje optimista pendiente del mismo autor y con el mismo contenido, reemplazarlo
      const pendingIdx = currentList.findIndex(
        (m) =>
          m.pending &&
          (m.author.id === newMsg.author.id ||
            m.author.id === state.currentUser.id ||
            m.author.username === newMsg.author.username) &&
          m.content.trim() === newMsg.content.trim()
      );

      let updatedList: Message[];
      if (pendingIdx !== -1) {
        updatedList = [...currentList];
        updatedList[pendingIdx] = newMsg;
      } else {
        updatedList = [...currentList, newMsg];
      }

      // Asegurar que el autor aparezca en la lista de miembros como conectado
      const guildId = raw.guild_id || state.selectedGuildId;
      const guildMembers = [...(state.members[guildId] || [])];
      const memberIdx = guildMembers.findIndex((m) => m.id === newMsg.author.id);
      if (memberIdx === -1) {
        guildMembers.push({ ...newMsg.author, status: 'online' });
      } else if (guildMembers[memberIdx].status === 'offline') {
        guildMembers[memberIdx] = { ...guildMembers[memberIdx], status: 'online' };
      }

      return {
        messages: {
          ...state.messages,
          [raw.channel_id]: updatedList,
        },
        members: {
          ...state.members,
          [guildId]: guildMembers,
        },
      };
    });
  },

  handlePresenceUpdate: (update) => {
    if (!update || !update.user_id) return;

    set((state) => {
      const targetGuildIds = update.guild_id
        ? [update.guild_id]
        : Object.keys(state.members);

      const updatedMembers = { ...state.members };

      for (const gid of targetGuildIds) {
        const list = [...(updatedMembers[gid] || [])];
        const idx = list.findIndex((m) => m.id === update.user_id);

        if (idx !== -1) {
          list[idx] = {
            ...list[idx],
            status: update.status || list[idx].status,
            custom_status:
              update.custom_status !== undefined
                ? update.custom_status
                : list[idx].custom_status,
            global_name: update.global_name || list[idx].global_name,
            avatar: update.avatar
              ? resolveAvatarUrl(update.user_id, update.avatar)
              : list[idx].avatar,
          };
        } else if (update.username) {
          list.push({
            id: update.user_id,
            username: update.username,
            discriminator: '0000',
            global_name: update.global_name || update.username,
            avatar: resolveAvatarUrl(update.user_id, update.avatar),
            status: update.status || 'online',
            custom_status: update.custom_status,
          });
        }

        updatedMembers[gid] = list;
      }

      return { members: updatedMembers };
    });
  },

  handleVoiceStateUpdate: (vs) => {
    if (!vs || !vs.user_id) return;

    set((state) => {
      const nextVoiceStates: Record<string, VoiceStateUser[]> = {};

      // 1. Remover al usuario de cualquier canal de voz donde estuviera antes
      for (const [chId, users] of Object.entries(state.voiceStates)) {
        const filtered = users.filter((u) => u.user_id !== vs.user_id);
        if (filtered.length > 0) {
          nextVoiceStates[chId] = filtered;
        }
      }

      // 2. Si tiene channel_id, añadirlo al nuevo canal de voz
      if (vs.channel_id) {
        const currentInChannel = nextVoiceStates[vs.channel_id] || [];
        currentInChannel.push({
          ...vs,
          avatar: resolveAvatarUrl(vs.user_id, vs.avatar),
        });
        nextVoiceStates[vs.channel_id] = currentInChannel;
      }

      return { voiceStates: nextVoiceStates };
    });
  },

  handleVoiceConnectionStatus: (status) => {
    if (!status) return;
    set({ voiceConnection: status });
  },

  handleGatewayError: (error) => {
    clearGatewayTimeout();
    set((state) => ({
      authError: error,
      isConnecting: false,
      isAuthenticated: false,
      isSettingsOpen: true,
      connectionStatus: {
        ...state.connectionStatus,
        gateway: 'disconnected',
      },
    }));
  },

  setAccentColor: (color) => {
    document.documentElement.style.setProperty('--accent-primary', color);
    set({ accentColor: color });
  },

  setUiDensity: (density) => set({ uiDensity: density }),
  setReducedMotion: (reduced) => set({ reducedMotion: reduced }),
}));
