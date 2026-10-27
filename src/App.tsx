import React, { useEffect } from 'react';
import { Shield, Loader2, Sparkles, AlertCircle } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { TopBar } from './components/layout/TopBar';
import { ServerSidebar } from './components/layout/ServerSidebar';
import { ChannelSidebar } from './components/layout/ChannelSidebar';
import { ChatArea } from './components/layout/ChatArea';
import { MembersSidebar } from './components/layout/MembersSidebar';
import { CommandPalette } from './components/modals/CommandPalette';
import { SettingsModal } from './components/modals/SettingsModal';
import { useAppStore } from './stores/useAppStore';

// Evita doble autoconexión en modo React.StrictMode
let initialAutoConnectTriggered = false;

export const App: React.FC = () => {
  const {
    uiDensity,
    reducedMotion,
    isConnecting,
    setConnecting,
    isAuthenticated,
    authError,
    setSettingsOpen,
  } = useAppStore();

  useEffect(() => {
    if (reducedMotion) {
      document.documentElement.classList.add('reduce-motion');
    } else {
      document.documentElement.classList.remove('reduce-motion');
    }
  }, [reducedMotion]);

  // 1. Listeners globales (DOM CustomEvents inyectados por Rust + Tauri v2 Event API)
  useEffect(() => {
    let unlistenReady: UnlistenFn | undefined;
    let unlistenMsg: UnlistenFn | undefined;
    let unlistenErr: UnlistenFn | undefined;
    let isMounted = true;

    // A) Listeners DOM nativos síncronos (canal directo desde window.eval en Rust)
    const onDomReady = (e: Event) => {
      const customEvt = e as CustomEvent;
      if (customEvt.detail) {
        console.log('[IPC DOM] dashcord:gateway-ready recibido:', customEvt.detail);
        useAppStore.getState().handleGatewayReady(customEvt.detail);
      }
    };

    const onDomMessage = (e: Event) => {
      const customEvt = e as CustomEvent;
      if (customEvt.detail) {
        useAppStore.getState().handleGatewayMessage(customEvt.detail);
      }
    };

    const onDomError = (e: Event) => {
      const customEvt = e as CustomEvent;
      const detail = customEvt.detail;
      const msg =
        typeof detail === 'string'
          ? detail
          : detail?.message || 'Error en la conexión con el Gateway';
      console.error('[IPC DOM] dashcord:gateway-error recibido:', msg);
      useAppStore.getState().handleGatewayError(msg);
    };

    window.addEventListener('dashcord:gateway-ready', onDomReady);
    window.addEventListener('dashcord:gateway-message-create', onDomMessage);
    window.addEventListener('dashcord:gateway-error', onDomError);

    // B) Listeners de la API de eventos de Tauri v2 (@tauri-apps/api/event)
    const setupTauriListeners = async () => {
      try {
        const uReady = await listen<any>('gateway-ready', (event) => {
          if (!isMounted) return;
          console.log('[IPC TAURI] gateway-ready recibido:', event.payload);
          useAppStore.getState().handleGatewayReady(event.payload);
        });
        if (isMounted) unlistenReady = uReady;
        else uReady();
      } catch (err) {
        console.warn('[IPC TAURI] Aviso al registrar listener gateway-ready:', err);
      }

      try {
        const uMsg = await listen<any>('gateway-message-create', (event) => {
          if (!isMounted) return;
          useAppStore.getState().handleGatewayMessage(event.payload);
        });
        if (isMounted) unlistenMsg = uMsg;
        else uMsg();
      } catch (err) {
        console.warn('[IPC TAURI] Aviso al registrar listener gateway-message-create:', err);
      }

      try {
        const uErr = await listen<any>('gateway-error', (event) => {
          if (!isMounted) return;
          const msg =
            typeof event.payload === 'string'
              ? event.payload
              : event.payload?.message || 'Error en la conexión con el Gateway';
          console.error('[IPC TAURI] gateway-error recibido:', msg);
          useAppStore.getState().handleGatewayError(msg);
        });
        if (isMounted) unlistenErr = uErr;
        else uErr();
      } catch (err) {
        console.warn('[IPC TAURI] Aviso al registrar listener gateway-error:', err);
      }

      // C) Autoconexión única al iniciar si existe un token en el Keychain
      if (!initialAutoConnectTriggered) {
        initialAutoConnectTriggered = true;
        try {
          const savedToken = await invoke<string>('get_token').catch(() => '');
          if (savedToken && savedToken.trim().length > 0) {
            useAppStore.getState().setConnecting(true);
            await invoke<boolean>('connect_saved_token');
          } else if (!useAppStore.getState().isAuthenticated) {
            useAppStore.getState().setSettingsOpen(true);
          }
        } catch (err) {
          console.warn('No se pudo autoconectar con token previo:', err);
          if (isMounted && !useAppStore.getState().isAuthenticated) {
            useAppStore.getState().setConnecting(false);
            useAppStore.getState().setSettingsOpen(true);
          }
        }
      }
    };

    setupTauriListeners();

    return () => {
      isMounted = false;
      window.removeEventListener('dashcord:gateway-ready', onDomReady);
      window.removeEventListener('dashcord:gateway-message-create', onDomMessage);
      window.removeEventListener('dashcord:gateway-error', onDomError);
      if (unlistenReady) unlistenReady();
      if (unlistenMsg) unlistenMsg();
      if (unlistenErr) unlistenErr();
    };
  }, []);

  // 2. Respaldo de sincronización por comando IPC (`get_gateway_snapshot`)
  // Garantiza que si el evento READY ocurrió en milisegundos, React lo reciba inmediatamente
  useEffect(() => {
    const intervalMs = isConnecting ? 300 : 1200;
    const timer = setInterval(async () => {
      try {
        const snap = await invoke<{
          status: string;
          ready_payload?: any;
          error_message?: string;
          recent_messages?: any[];
        }>('get_gateway_snapshot');

        const state = useAppStore.getState();

        if (snap.status === 'ready' && snap.ready_payload && !state.isAuthenticated) {
          console.log('[IPC SNAPSHOT] READY sincronizado desde backend:', snap.ready_payload);
          state.handleGatewayReady(snap.ready_payload);
        } else if (snap.status === 'error' && snap.error_message && state.isConnecting) {
          console.error('[IPC SNAPSHOT] Error sincronizado desde backend:', snap.error_message);
          state.handleGatewayError(snap.error_message);
        }

        if (snap.recent_messages && snap.recent_messages.length > 0) {
          for (const msg of snap.recent_messages) {
            state.handleGatewayMessage(msg);
          }
        }
      } catch {
        // Ignorar errores silenciosos de polling
      }
    }, intervalMs);

    return () => clearInterval(timer);
  }, [isConnecting, isAuthenticated]);

  return (
    <div
      className={`w-screen h-screen flex flex-col bg-base text-primary overflow-hidden font-sans select-none ${
        uiDensity === 'compact' ? 'text-xs' : 'text-sm'
      }`}
    >
      {/* Top Application Header */}
      <TopBar />

      {/* Main Multi-Column Split Workspace */}
      <div className="flex-1 flex min-h-0 overflow-hidden relative">
        {/* Leftmost Server List */}
        <ServerSidebar />

        {/* Channels & User Card Bar */}
        <ChannelSidebar />

        {/* Center Chat View */}
        <ChatArea />

        {/* Right Members Drawer */}
        <MembersSidebar />

        {/* Pantalla de Carga y Conexión en progreso */}
        {isConnecting && !isAuthenticated && (
          <div className="absolute inset-0 bg-base/90 backdrop-blur-md z-40 flex flex-col items-center justify-center gap-4 animate-in fade-in duration-200">
            <div className="relative">
              <div className="w-16 h-16 rounded-2xl bg-surface border border-accent-cyan/40 shadow-glow-md flex items-center justify-center text-accent-cyan">
                <Loader2 className="w-8 h-8 animate-spin text-accent-cyan" />
              </div>
              <Sparkles className="w-5 h-5 text-accent-ice absolute -top-1 -right-1 animate-pulse" />
            </div>

            <div className="text-center space-y-1">
              <h2 className="text-base font-bold text-primary flex items-center justify-center gap-2">
                <span>Conectando al Gateway v10...</span>
              </h2>
              <p className="text-xs text-muted">
                Estableciendo WebSocket con Jitter y huella TLS suplantada (Build 382000)
              </p>
            </div>

            <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-surface/80 border border-border-subtle text-[11px] text-emerald-400 font-mono mt-2">
              <Shield className="w-3.5 h-3.5 text-accent-cyan" />
              <span>TLS JA3/JA4 Spoof Activo • Chrome 128 x64</span>
            </div>

            <button
              onClick={() => {
                setConnecting(false);
                setSettingsOpen(true);
              }}
              className="mt-4 px-4 py-1.5 rounded-lg text-xs text-secondary_text hover:text-primary bg-surface hover:bg-hover border border-border-subtle transition-colors"
            >
              Cancelar / Cambiar Token
            </button>
          </div>
        )}

        {/* Banner si no hay sesión iniciada y no está conectando */}
        {!isAuthenticated && !isConnecting && (
          <div className="absolute inset-0 bg-base/95 backdrop-blur-md z-30 flex flex-col items-center justify-center gap-4 p-6">
            <div className="w-16 h-16 rounded-2xl bg-surface border border-border-subtle flex items-center justify-center text-accent-primary shadow-glow-sm">
              <Shield className="w-8 h-8 text-accent-cyan" />
            </div>

            <div className="text-center max-w-sm space-y-2">
              <h2 className="text-lg font-bold text-primary">Bienvenido a DashCord</h2>
              <p className="text-xs text-muted leading-relaxed">
                Para comenzar a recibir tus servidores, canales y mensajes en tiempo real, ingresa tu Discord User Token en los ajustes.
              </p>
            </div>

            {authError && (
              <div className="p-3 max-w-md rounded-lg bg-rose-950/50 border border-rose-500/60 flex items-center gap-2 text-xs text-rose-200 shadow-lg">
                <AlertCircle className="w-4 h-4 text-rose-400 flex-shrink-0" />
                <span>{authError}</span>
              </div>
            )}

            <button
              onClick={() => setSettingsOpen(true)}
              className="px-5 py-2.5 bg-accent-primary hover:bg-accent-cyan text-white text-xs font-semibold rounded-xl shadow-glow-md transition-all duration-150 cursor-pointer"
            >
              Configurar Token de Usuario
            </button>
          </div>
        )}
      </div>

      {/* Global Command Center (Ctrl+K) */}
      <CommandPalette />

      {/* Global Settings Window */}
      <SettingsModal />
    </div>
  );
};

export default App;
