import { create } from 'zustand';
import { Guild, Message, User, ConnectionStatus } from '../types/discord';

let connectionTimeoutId: ReturnType<typeof setTimeout> | null = null;

function clearGatewayTimeout() {
  if (connectionTimeoutId) {
    clearTimeout(connectionTimeoutId);
    connectionTimeoutId = null;
  }
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

  // Mock / Real Data
  currentUser: User;
  guilds: Guild[];
  messages: Record<string, Message[]>; // channel_id -> messages
  members: Record<string, User[]>;     // guild_id -> members
  connectionStatus: ConnectionStatus;

  // Settings & Theme
  accentColor: string;
  uiDensity: 'compact' | 'comfortable';
  reducedMotion: boolean;

  // Actions
  setSelectedGuild: (id: string) => void;
  setSelectedChannel: (id: string) => void;
  toggleMembersSidebar: () => void;
  setSettingsOpen: (open: boolean) => void;
  setCommandPaletteOpen: (open: boolean) => void;
  sendMessage: (channelId: string, content: string) => void;
  setAccentColor: (color: string) => void;
  setUiDensity: (density: 'compact' | 'comfortable') => void;
  setReducedMotion: (reduced: boolean) => void;

  setConnecting: (connecting: boolean) => void;
  setAuthError: (error: string | null) => void;
  handleGatewayReady: (payload: { user: User; guilds: Guild[] }) => void;
  handleGatewayMessage: (rawMessage: any) => void;
  handleGatewayError: (error: string) => void;
}

const MOCK_CURRENT_USER: User = {
  id: 'usr_me',
  username: 'DashOperator',
  discriminator: '0001',
  global_name: 'GhostProtocol',
  avatar: 'https://images.unsplash.com/photo-1566492031773-4f4e44671857?w=128&auto=format&fit=crop&q=80',
  status: 'online',
  custom_status: '⚡ Running DashCord via BoringSSL TLS',
};

const MOCK_GUILDS: Guild[] = [
  {
    id: 'dm',
    name: 'Mensajes Directos',
    icon: null,
    unread_count: 2,
    channels: [
      { id: 'dm_1', name: 'Alex Rivera', type: 'text', unread: true },
      { id: 'dm_2', name: 'Elena Rostova', type: 'text' },
    ],
  },
  {
    id: 'guild_1',
    name: 'Cyberpunk Syndicate',
    icon: '⚡',
    unread_count: 5,
    channels: [
      { id: 'c_general', guild_id: 'guild_1', name: 'general-chat', type: 'text' },
      { id: 'c_announcements', guild_id: 'guild_1', name: 'announcements', type: 'announcement', unread: true },
      { id: 'c_dev', guild_id: 'guild_1', name: 'rust-dev', type: 'text' },
      { id: 'c_gaming', guild_id: 'guild_1', name: 'night-city-lobby', type: 'voice' },
    ],
  },
  {
    id: 'guild_2',
    name: 'Rustaceans HQ',
    icon: '🦀',
    unread_count: 0,
    channels: [
      { id: 'c_tokio', guild_id: 'guild_2', name: 'async-tokio', type: 'text' },
      { id: 'c_tauri', guild_id: 'guild_2', name: 'tauri-apps', type: 'text' },
    ],
  },
  {
    id: 'guild_3',
    name: 'Zero-Day Research',
    icon: '🛡️',
    unread_count: 1,
    channels: [
      { id: 'c_threats', guild_id: 'guild_3', name: 'tls-fingerprinting', type: 'text' },
      { id: 'c_evasion', guild_id: 'guild_3', name: 'anti-bot-evasion', type: 'text' },
    ],
  },
];

const MOCK_MESSAGES: Record<string, Message[]> = {
  'c_general': [
    {
      id: 'm1',
      channel_id: 'c_general',
      author: {
        id: 'u1',
        username: 'Vektor',
        discriminator: '1337',
        global_name: 'Vektor_Zero',
        avatar: 'https://images.unsplash.com/photo-1535713875002-d1d0cf377fde?w=128&auto=format&fit=crop&q=80',
        status: 'online',
      },
      content: '¡Bienvenidos al servidor principal! Estamos probando la nueva compilación con BoringSSL.',
      timestamp: 'Hoy a las 10:14',
      reactions: [{ emoji: '🔥', count: 4, me: true }, { emoji: '🚀', count: 2, me: false }],
    },
    {
      id: 'm2',
      channel_id: 'c_general',
      author: {
        id: 'u2',
        username: 'Kira',
        discriminator: '4040',
        global_name: 'Kira Netrunner',
        avatar: 'https://images.unsplash.com/photo-1494790108377-be9c29b29330?w=128&auto=format&fit=crop&q=80',
        status: 'idle',
      },
      content: 'El consumo de RAM se mantiene en 118 MB estables con 4 servidores activos. Es una mejora brutal comparado con los 750 MB de Discord oficial.',
      timestamp: 'Hoy a las 10:16',
      reply_to: {
        id: 'm1',
        author: 'Vektor_Zero',
        content: '¡Bienvenidos al servidor principal! Estamos probando la nueva compilación...',
      },
    },
    {
      id: 'm3',
      channel_id: 'c_general',
      author: MOCK_CURRENT_USER,
      content: 'Verifiqué las firmas de cabecera: X-Super-Properties está enviando el build 382000 simulando Windows 10 x64. El handshake TLS enmascara perfectamente JA3/JA4.',
      timestamp: 'Hoy a las 10:20',
      reactions: [{ emoji: '🛡️', count: 3, me: true }],
    },
  ],
  'c_dev': [
    {
      id: 'md1',
      channel_id: 'c_dev',
      author: {
        id: 'u3',
        username: 'Rustacean',
        discriminator: '9999',
        global_name: 'Ferris',
        status: 'dnd',
      },
      content: 'La arquitectura modular con Tauri 2.0 y el crate wreq permite controlar cada byte del handshake TLS.',
      timestamp: 'Ayer a las 22:45',
    },
  ],
};

const MOCK_MEMBERS: Record<string, User[]> = {
  'guild_1': [
    MOCK_CURRENT_USER,
    {
      id: 'u1',
      username: 'Vektor',
      discriminator: '1337',
      global_name: 'Vektor_Zero',
      avatar: 'https://images.unsplash.com/photo-1535713875002-d1d0cf377fde?w=128&auto=format&fit=crop&q=80',
      status: 'online',
      custom_status: 'Compiling kernel...',
    },
    {
      id: 'u2',
      username: 'Kira',
      discriminator: '4040',
      global_name: 'Kira Netrunner',
      avatar: 'https://images.unsplash.com/photo-1494790108377-be9c29b29330?w=128&auto=format&fit=crop&q=80',
      status: 'idle',
      custom_status: 'AFK en Neon District',
    },
    {
      id: 'u3',
      username: 'Rustacean',
      discriminator: '9999',
      global_name: 'Ferris',
      status: 'dnd',
      custom_status: 'No molestar (Testing Gateway v10)',
    },
    {
      id: 'u4',
      username: 'Cipher',
      discriminator: '0101',
      global_name: 'Cipher_Ghost',
      status: 'offline',
    },
  ],
};

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
  messages: MOCK_MESSAGES,
  members: MOCK_MEMBERS,

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
    set((state) => {
      const targetGuild = state.guilds.find((g) => g.id === id);
      const firstChannelId = targetGuild?.channels[0]?.id || '';
      return {
        selectedGuildId: id,
        selectedChannelId: firstChannelId,
      };
    });
  },

  setSelectedChannel: (id) => set({ selectedChannelId: id }),
  toggleMembersSidebar: () => set((state) => ({ isMembersSidebarOpen: !state.isMembersSidebarOpen })),
  setSettingsOpen: (open) => set({ isSettingsOpen: open }),
  setCommandPaletteOpen: (open) => set({ isCommandPaletteOpen: open }),

  setConnecting: (connecting) => {
    clearGatewayTimeout();
    set({ isConnecting: connecting });

    if (connecting) {
      // Timeout de seguridad de 10 segundos si el backend no responde con READY ni ERROR
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
    const { user, guilds } = payload;
    set((state) => {
      const mergedGuilds = guilds && guilds.length > 0 ? guilds : state.guilds;
      const initialGuildId = mergedGuilds[0]?.id || '';
      const initialChannelId = mergedGuilds[0]?.channels[0]?.id || '';

      const updatedUser: User = {
        ...state.currentUser,
        ...user,
        avatar: user.avatar
          ? (user.avatar.startsWith('http')
              ? user.avatar
              : `https://cdn.discordapp.com/avatars/${user.id}/${user.avatar}.png?size=128`)
          : state.currentUser.avatar,
      };

      return {
        currentUser: updatedUser,
        guilds: mergedGuilds,
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
      };
    });
  },

  handleGatewayMessage: (raw) => {
    if (!raw || !raw.channel_id) return;
    const authorUser: User = {
      id: raw.author?.id || 'unknown',
      username: raw.author?.username || 'Usuario',
      discriminator: raw.author?.discriminator || '0000',
      global_name: raw.author?.global_name || raw.author?.username,
      avatar: raw.author?.avatar
        ? `https://cdn.discordapp.com/avatars/${raw.author.id}/${raw.author.avatar}.png?size=128`
        : 'https://images.unsplash.com/photo-1535713875002-d1d0cf377fde?w=128',
      status: 'online',
    };

    const newMsg: Message = {
      id: raw.id || `msg_${Date.now()}`,
      channel_id: raw.channel_id,
      author: authorUser,
      content: raw.content || '',
      timestamp: raw.timestamp
        ? new Date(raw.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
        : 'Ahora',
      attachments: raw.attachments,
      reactions: raw.reactions,
    };

    set((state) => {
      const currentList = state.messages[raw.channel_id] || [];
      return {
        messages: {
          ...state.messages,
          [raw.channel_id]: [...currentList, newMsg],
        },
      };
    });
  },

  handleGatewayError: (error) => {
    clearGatewayTimeout();
    set((state) => ({
      authError: error,
      isConnecting: false,
      isSettingsOpen: true,
      connectionStatus: {
        ...state.connectionStatus,
        gateway: 'disconnected',
      },
    }));
  },

  sendMessage: (channelId, content) => {
    if (!content.trim()) return;
    const now = new Date();
    const timeString = `Hoy a las ${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}`;
    
    set((state) => {
      const currentList = state.messages[channelId] || [];
      const newMsg: Message = {
        id: `msg_${Date.now()}`,
        channel_id: channelId,
        author: state.currentUser,
        content,
        timestamp: timeString,
      };
      return {
        messages: {
          ...state.messages,
          [channelId]: [...currentList, newMsg],
        },
      };
    });
  },

  setAccentColor: (color) => {
    document.documentElement.style.setProperty('--accent-primary', color);
    set({ accentColor: color });
  },

  setUiDensity: (density) => set({ uiDensity: density }),
  setReducedMotion: (reduced) => set({ reducedMotion: reduced }),
}));
