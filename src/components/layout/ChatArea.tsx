import React, { useState, useRef, useEffect } from 'react';
import {
  Send,
  Paperclip,
  Smile,
  Hash,
  Reply,
  CornerDownRight,
  Loader2,
  MessageSquarePlus,
  ExternalLink,
} from 'lucide-react';
import { useAppStore, resolveAvatarUrl } from '../../stores/useAppStore';

export const ChatArea: React.FC = () => {
  const {
    selectedGuildId,
    selectedChannelId,
    guilds,
    messages,
    loadingChannels,
    sendMessage,
    fetchMessagesForChannel,
    isAuthenticated,
  } = useAppStore();

  const [inputContent, setInputContent] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const currentGuild = guilds.find((g) => g.id === selectedGuildId);
  const currentChannel = currentGuild?.channels.find((c) => c.id === selectedChannelId);
  const currentMessages = messages[selectedChannelId] || [];
  const isLoadingHistory = !!loadingChannels[selectedChannelId];

  // Cargar historial si cambia el canal seleccionado y aún no se ha cargado
  useEffect(() => {
    if (
      isAuthenticated &&
      selectedChannelId &&
      !messages[selectedChannelId] &&
      !loadingChannels[selectedChannelId]
    ) {
      fetchMessagesForChannel(selectedChannelId);
    }
  }, [selectedChannelId, isAuthenticated]);

  // Auto-scroll hacia el final al cambiar de canal, terminar de cargar historial o recibir un nuevo mensaje
  useEffect(() => {
    const timer = setTimeout(() => {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, 40);
    return () => clearTimeout(timer);
  }, [currentMessages.length, selectedChannelId, isLoadingHistory]);

  const handleSend = (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!inputContent.trim() || !selectedChannelId) return;
    const textToSend = inputContent;
    setInputContent('');
    sendMessage(selectedChannelId, textToSend);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const isImageAttachment = (filename: string) =>
    /\.(png|jpe?g|gif|webp|bmp)$/i.test(filename);

  return (
    <main className="flex-1 bg-base flex flex-col min-w-0">
      {/* Messages List Area */}
      <div
        ref={scrollContainerRef}
        className="flex-1 overflow-y-auto p-4 space-y-4"
      >
        {/* Welcome Banner for channels */}
        <div className="pt-6 pb-4 border-b border-border-subtle/50 mb-4">
          <div className="w-14 h-14 rounded-2xl bg-surface border border-border-subtle flex items-center justify-center text-accent-cyan shadow-glow-sm mb-3">
            <Hash className="w-8 h-8" />
          </div>
          <h2 className="text-xl font-bold text-primary">
            ¡Te damos la bienvenida a #{currentChannel?.name || 'chat'}!
          </h2>
          <p className="text-xs text-muted mt-1">
            {currentChannel?.topic
              ? currentChannel.topic
              : `Este es el comienzo del canal #${currentChannel?.name || 'general'}. Las transmisiones están enrutadas con huella TLS/HTTP2 suplantada (BoringSSL).`}
          </p>
        </div>

        {/* Estado de carga de historial */}
        {isLoadingHistory && currentMessages.length === 0 && (
          <div className="py-12 flex flex-col items-center justify-center gap-3 text-muted animate-in fade-in">
            <Loader2 className="w-6 h-6 animate-spin text-accent-cyan" />
            <span className="text-xs">
              Sincronizando historial de #{currentChannel?.name || 'canal'}...
            </span>
          </div>
        )}

        {/* Estado vacío si el canal no tiene mensajes */}
        {!isLoadingHistory && currentMessages.length === 0 && (
          <div className="py-14 flex flex-col items-center justify-center text-center gap-3">
            <div className="w-12 h-12 rounded-2xl bg-surface/70 border border-border-subtle flex items-center justify-center text-accent-ice">
              <MessageSquarePlus className="w-6 h-6 text-accent-cyan" />
            </div>
            <div className="space-y-1">
              <p className="text-sm font-semibold text-primary">
                Sé el primero en hablar
              </p>
              <p className="text-xs text-muted max-w-xs">
                Aún no hay mensajes en #{currentChannel?.name || 'este canal'}. ¡Envía un mensaje abajo para romper el hielo!
              </p>
            </div>
          </div>
        )}

        {/* Message Items */}
        {currentMessages.map((msg) => (
          <div
            key={msg.id}
            className={`flex gap-3 group hover:bg-surface/40 -mx-4 px-4 py-1.5 rounded transition-colors relative ${
              msg.pending ? 'opacity-65' : ''
            }`}
          >
            {/* User Avatar */}
            <img
              src={resolveAvatarUrl(msg.author.id, msg.author.avatar)}
              alt={msg.author.username}
              className="w-10 h-10 rounded-full object-cover border border-border-subtle flex-shrink-0 mt-0.5 bg-surface"
              onError={(e) => {
                e.currentTarget.src = 'https://cdn.discordapp.com/embed/avatars/0.png';
              }}
            />

            {/* Message Body */}
            <div className="flex-1 min-w-0">
              {/* Optional Reply Banner */}
              {msg.reply_to && (
                <div className="flex items-center gap-1.5 text-xs text-muted mb-1">
                  <CornerDownRight className="w-3 h-3 text-accent-primary" />
                  <span className="font-semibold text-accent-ice">
                    @{msg.reply_to.author}
                  </span>
                  <span className="truncate italic max-w-md">
                    {msg.reply_to.content}
                  </span>
                </div>
              )}

              {/* Author & Timestamp */}
              <div className="flex items-baseline gap-2">
                <span className="font-semibold text-sm text-primary hover:underline cursor-pointer">
                  {msg.author.global_name || msg.author.username}
                </span>
                {msg.author.is_bot && (
                  <span className="px-1.5 py-0.2 bg-accent-primary/20 border border-accent-primary/40 text-accent-cyan text-[9px] font-bold uppercase rounded">
                    BOT
                  </span>
                )}
                <span className="text-[11px] text-muted">{msg.timestamp}</span>
                {msg.pending && (
                  <span className="text-[10px] text-accent-cyan italic">
                    (Enviando...)
                  </span>
                )}
                {msg.failed && (
                  <span className="text-[10px] text-rose-400 font-semibold">
                    (Error al enviar)
                  </span>
                )}
              </div>

              {/* Text Content */}
              {msg.content && (
                <p className="text-sm text-secondary_text mt-0.5 whitespace-pre-wrap break-words leading-relaxed select-text">
                  {msg.content}
                </p>
              )}

              {/* Attachments */}
              {msg.attachments && msg.attachments.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-2">
                  {msg.attachments.map((att) =>
                    isImageAttachment(att.filename) ? (
                      <a
                        key={att.id}
                        href={att.url}
                        target="_blank"
                        rel="noreferrer"
                        className="block max-w-sm rounded-lg overflow-hidden border border-border-subtle hover:border-accent-cyan/50 transition-colors"
                      >
                        <img
                          src={att.url}
                          alt={att.filename}
                          className="max-h-64 object-contain bg-surface"
                        />
                      </a>
                    ) : (
                      <a
                        key={att.id}
                        href={att.url}
                        target="_blank"
                        rel="noreferrer"
                        className="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface border border-border-subtle hover:border-accent-cyan/50 text-xs text-accent-ice transition-colors"
                      >
                        <Paperclip className="w-3.5 h-3.5 text-accent-cyan" />
                        <span className="truncate max-w-xs">{att.filename}</span>
                        <ExternalLink className="w-3 h-3 text-muted" />
                      </a>
                    )
                  )}
                </div>
              )}

              {/* Reactions */}
              {msg.reactions && msg.reactions.length > 0 && (
                <div className="flex flex-wrap items-center gap-1.5 mt-2">
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
