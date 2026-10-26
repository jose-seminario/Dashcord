export interface User {
  id: string;
  username: string;
  discriminator: string;
  global_name?: string | null;
  avatar?: string | null;
  status: 'online' | 'idle' | 'dnd' | 'offline';
  custom_status?: string;
  is_bot?: boolean;
}

export interface Guild {
  id: string;
  name: string;
  icon?: string | null;
  unread_count?: number;
  channels: Channel[];
}

export interface Channel {
  id: string;
  guild_id?: string;
  name: string;
  type: 'text' | 'voice' | 'announcement';
  topic?: string;
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
