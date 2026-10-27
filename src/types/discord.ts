export interface User {
  id: string;
  username: string;
  discriminator: string;
  global_name?: string | null;
  avatar?: string | null;
  status: 'online' | 'idle' | 'dnd' | 'offline' | string;
  custom_status?: string | null;
  is_bot?: boolean;
}

export interface VoiceStateUser {
  user_id: string;
  channel_id?: string | null;
  guild_id?: string | null;
  username: string;
  avatar?: string | null;
  self_mute?: boolean;
  self_deaf?: boolean;
}

export interface PresenceUpdate {
  user_id: string;
  guild_id?: string | null;
  status: 'online' | 'idle' | 'dnd' | 'offline' | string;
  custom_status?: string | null;
  username?: string | null;
  global_name?: string | null;
  avatar?: string | null;
}

export interface Guild {
  id: string;
  name: string;
  icon?: string | null;
  unread_count?: number;
  channels: Channel[];
  members?: User[];
  voice_states?: VoiceStateUser[];
}

export interface Channel {
  id: string;
  guild_id?: string;
  name: string;
  type: 'text' | 'voice' | 'announcement' | string;
  topic?: string | null;
  unread?: boolean;
}

export interface Message {
  id: string;
  channel_id: string;
  author: User;
  content: string;
  timestamp: string;
  edited_timestamp?: string | null;
  attachments?: Attachment[];
  reactions?: Reaction[];
  pending?: boolean;
  failed?: boolean;
  reply_to?: {
    id: string;
    author: string;
    content: string;
  };
}

export interface Attachment {
  id: string;
  filename: string;
  url: string;
  size: number;
}

export interface Reaction {
  emoji: string;
  count: number;
  me: boolean;
}

export interface ConnectionStatus {
  gateway: 'connected' | 'connecting' | 'disconnected' | 'reconnecting';
  ping: number;
  tls_spoofed: boolean;
  build_number: number;
}

export interface VoiceConnectionStatus {
  state: 'disconnected' | 'connecting' | 'connected' | 'error' | string;
  guild_id?: string | null;
  channel_id?: string | null;
  endpoint?: string | null;
  ip?: string | null;
  port?: number | null;
  ssrc?: number | null;
  encryption_mode?: string | null;
  has_secret_key?: boolean;
  error?: string | null;
}
