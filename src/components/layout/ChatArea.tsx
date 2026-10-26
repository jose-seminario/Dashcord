import React, { useState, useRef, useEffect } from 'react';
import { Send, Paperclip, Smile, Hash, Reply, CornerDownRight } from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';

export const ChatArea: React.FC = () => {
  const {
    selectedGuildId,
    selectedChannelId,
    guilds,
    messages,
    sendMessage,
  } = useAppStore();

  const [inputContent, setInputContent] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const currentGuild = guilds.find((g) => g.id === selectedGuildId);
  const currentChannel = currentGuild?.channels.find((c) => c.id === selectedChannelId);
  const currentMessages = messages[selectedChannelId] || [];

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  useEffect(() => {
    scrollToBottom();
  }, [currentMessages.length, selectedChannelId]);

  const handleSend = (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!inputContent.trim()) return;
    sendMessage(selectedChannelId, inputContent);
    setInputContent('');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <main className="flex-1 bg-base flex flex-col min-w-0">
      {/* Messages List Area */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Welcome Banner for empty or initial channels */}
        <div className="pt-6 pb-4 border-b border-border-subtle/50 mb-4">
          <div className="w-14 h-14 rounded-2xl bg-surface border border-border-subtle flex items-center justify-center text-accent-cyan shadow-glow-sm mb-3">
            <Hash className="w-8 h-8" />
          </div>
          <h2 className="text-xl font-bold text-primary">
            ¡Te damos la bienvenida a #{currentChannel?.name || 'chat'}!
          </h2>
          <p className="text-xs text-muted mt-1">
            Este es el comienzo del canal #{currentChannel?.name || 'general'}. Las transmisiones están enrutadas con huella TLS/HTTP2 suplantada.
          </p>
        </div>

        {/* Message Items */}
        {currentMessages.map((msg) => (
          <div
            key={msg.id}
            className="flex gap-3 group hover:bg-surface/40 -mx-4 px-4 py-1.5 rounded transition-colors relative"
          >
            {/* User Avatar */}
            <img
              src={msg.author.avatar || 'https://images.unsplash.com/photo-1535713875002-d1d0cf377fde?w=128'}
              alt={msg.author.username}
              className="w-10 h-10 rounded-full object-cover border border-border-subtle flex-shrink-0 mt-0.5"
            />

            {/* Message Body */}
            <div className="flex-1 min-w-0">
              {/* Optional Reply Banner */}
              {msg.reply_to && (
                <div className="flex items-center gap-1.5 text-xs text-muted mb-1">
                  <CornerDownRight className="w-3 h-3 text-accent-primary" />
                  <span className="font-semibold text-accent-ice">@{msg.reply_to.author}</span>
                  <span className="truncate italic max-w-md">{msg.reply_to.content}</span>
                </div>
              )}

              {/* Author & Timestamp */}
              <div className="flex items-baseline gap-2">
                <span className="font-semibold text-sm text-primary hover:underline cursor-pointer">
                  {msg.author.global_name || msg.author.username}
                </span>
                <span className="text-[11px] text-muted">{msg.timestamp}</span>
              </div>

              {/* Text Content */}
              <p className="text-sm text-secondary_text mt-0.5 whitespace-pre-wrap break-words leading-relaxed">
                {msg.content}
              </p>

              {/* Reactions */}
              {msg.reactions && msg.reactions.length > 0 && (
                <div className="flex items-center gap-1.5 mt-2">
                  {msg.reactions.map((reaction, i) => (
                    <button
                      key={i}
                      className={`flex items-center gap-1 px-2 py-0.5 rounded-full text-xs border transition-colors ${
                        reaction.me
                          ? 'bg-active border-accent-cyan/50 text-accent-cyan'
                          : 'bg-surface/80 border-border-subtle text-secondary_text hover:border-accent-primary/40'
                      }`}
                    >
                      <span>{reaction.emoji}</span>
                      <span className="font-mono text-[11px]">{reaction.count}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Hover Actions Menu */}
            <div className="absolute right-4 top-1 hidden group-hover:flex items-center gap-1 bg-surface border border-border-subtle px-1.5 py-0.5 rounded shadow-lg">
              <button
                title="Responder"
                className="p-1 text-muted hover:text-accent-cyan rounded hover:bg-hover transition-colors"
              >
                <Reply className="w-3.5 h-3.5" />
              </button>
              <button
                title="Añadir reacción"
                className="p-1 text-muted hover:text-amber-400 rounded hover:bg-hover transition-colors"
              >
                <Smile className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>

      {/* Input Box Area */}
      <div className="p-4 pt-1 bg-base">
        <form
          onSubmit={handleSend}
          className="bg-surface border border-border-subtle rounded-xl flex items-center px-3 py-2 gap-2 focus-within:border-accent-primary/60 focus-within:shadow-glow-sm transition-all duration-200"
        >
          {/* Attachment Button */}
          <button
            type="button"
            title="Adjuntar archivo"
            className="p-1.5 text-muted hover:text-accent-cyan rounded-lg hover:bg-hover transition-colors flex-shrink-0"
          >
            <Paperclip className="w-4 h-4" />
          </button>

          {/* Text Input */}
          <input
            type="text"
            value={inputContent}
            onChange={(e) => setInputContent(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={`Enviar mensaje a #${currentChannel?.name || 'canal'}...`}
            className="flex-1 bg-transparent border-none outline-none text-sm text-primary placeholder-muted"
          />

          {/* Emoji Button */}
          <button
            type="button"
            title="Emojis"
            className="p-1.5 text-muted hover:text-amber-400 rounded-lg hover:bg-hover transition-colors flex-shrink-0"
          >
            <Smile className="w-4 h-4" />
          </button>

          {/* Send Button */}
          <button
            type="submit"
            disabled={!inputContent.trim()}
            title="Enviar mensaje"
            className={`p-1.5 rounded-lg transition-all flex-shrink-0 ${
              inputContent.trim()
                ? 'bg-accent-primary text-white hover:bg-accent-cyan shadow-glow-sm cursor-pointer'
                : 'text-muted/40 cursor-not-allowed'
            }`}
          >
            <Send className="w-4 h-4" />
          </button>
        </form>
      </div>
    </main>
  );
};
