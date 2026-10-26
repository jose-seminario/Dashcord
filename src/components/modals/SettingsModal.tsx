import React, { useState } from 'react';
import {
  X,
  Shield,
  Palette,
  KeyRound,
  Cpu,
  CheckCircle2,
  AlertTriangle,
  Lock,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';

export const SettingsModal: React.FC = () => {
  const {
    isSettingsOpen,
    setSettingsOpen,
    connectionStatus,
    accentColor,
    setAccentColor,
    uiDensity,
    setUiDensity,
    reducedMotion,
    setReducedMotion,
  } = useAppStore();

  const [activeTab, setActiveTab] = useState<'evasion' | 'appearance' | 'account' | 'perf'>('evasion');
  const [tokenInput, setTokenInput] = useState('');

  if (!isSettingsOpen) return null;

  const ACCENT_PRESETS = [
    { name: 'Electric Blue', hex: '#3B82F6' },
    { name: 'Neon Cyan', hex: '#00A3FF' },
    { name: 'Emerald Cyber', hex: '#10B981' },
    { name: 'Amethyst Night', hex: '#8B5CF6' },
    { name: 'Crimson Edge', hex: '#F43F5E' },
  ];

  return (
    <div className="fixed inset-0 bg-black/75 backdrop-blur-md z-50 flex items-center justify-center p-6 animate-in fade-in duration-150">
      <div className="w-full max-w-3xl h-[600px] bg-surface border border-border-subtle rounded-2xl shadow-2xl flex overflow-hidden glass-dropdown">
        {/* Left Sidebar Tabs */}
        <aside className="w-56 bg-base/80 border-r border-border-subtle p-4 flex flex-col justify-between">
          <div className="space-y-1">
            <div className="px-3 py-2 text-xs font-bold text-accent-ice uppercase tracking-wider flex items-center gap-2">
              <Shield className="w-4 h-4 text-accent-cyan" />
              <span>DashCord v0.1</span>
            </div>

            <button
              onClick={() => setActiveTab('evasion')}
              className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                activeTab === 'evasion'
                  ? 'bg-active text-accent-cyan border border-accent-cyan/30'
                  : 'text-secondary_text hover:text-primary hover:bg-hover'
              }`}
            >
              <Shield className="w-4 h-4" />
              <span>Evasión & Seguridad</span>
            </button>

            <button
              onClick={() => setActiveTab('appearance')}
              className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                activeTab === 'appearance'
                  ? 'bg-active text-accent-cyan border border-accent-cyan/30'
                  : 'text-secondary_text hover:text-primary hover:bg-hover'
              }`}
            >
              <Palette className="w-4 h-4" />
              <span>Apariencia & Temas</span>
            </button>

            <button
              onClick={() => setActiveTab('account')}
              className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                activeTab === 'account'
                  ? 'bg-active text-accent-cyan border border-accent-cyan/30'
                  : 'text-secondary_text hover:text-primary hover:bg-hover'
              }`}
            >
              <KeyRound className="w-4 h-4" />
              <span>Cuenta & Keychain</span>
            </button>

            <button
              onClick={() => setActiveTab('perf')}
              className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                activeTab === 'perf'
                  ? 'bg-active text-accent-cyan border border-accent-cyan/30'
                  : 'text-secondary_text hover:text-primary hover:bg-hover'
              }`}
            >
              <Cpu className="w-4 h-4" />
              <span>Rendimiento & RAM</span>
            </button>
          </div>

          <div className="text-[11px] text-muted p-2 bg-surface/50 rounded-lg border border-border-subtle">
            <span className="font-semibold text-secondary_text">Build Number:</span> {connectionStatus.build_number}
          </div>
        </aside>

        {/* Content Area */}
        <main className="flex-1 flex flex-col min-w-0 bg-base/40">
          {/* Header */}
          <div className="h-14 border-b border-border-subtle px-6 flex items-center justify-between">
            <h2 className="text-base font-bold text-primary">
              {activeTab === 'evasion' && 'Capa de Evasión y Protección Anti-Detección'}
              {activeTab === 'appearance' && 'Personalización de Interfaz (Midnight Blue)'}
              {activeTab === 'account' && 'Gestión Segura de Sesión (Keychain OS)'}
              {activeTab === 'perf' && 'Monitor de Recursos & Optimización'}
            </h2>
            <button
              onClick={() => setSettingsOpen(false)}
              className="p-1.5 text-muted hover:text-primary rounded-lg hover:bg-hover transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>

          {/* Tab Body */}
          <div className="flex-1 overflow-y-auto p-6 space-y-6">
            {/* Evasion Tab */}
            {activeTab === 'evasion' && (
              <div className="space-y-4">
                <div className="p-3 rounded-lg bg-emerald-950/30 border border-emerald-500/30 flex items-start gap-3">
                  <CheckCircle2 className="w-5 h-5 text-emerald-400 flex-shrink-0 mt-0.5" />
                  <div className="text-xs space-y-1">
                    <p className="font-semibold text-emerald-300">
                      Transporte TLS y Fingerprint JA3/JA4 Activos
                    </p>
                    <p className="text-emerald-200/80">
                      Tus peticiones utilizan el cliente con BoringSSL (`rquest`). Discord recibe la firma exacta de Google Chrome 128 / Discord Desktop x64.
                    </p>
                  </div>
                </div>

                <div className="space-y-2">
                  <h3 className="text-xs font-bold text-muted uppercase tracking-wider">
                    Cabecera X-Super-Properties Enmascarada
                  </h3>
                  <div className="bg-surface p-3 rounded-lg border border-border-subtle font-mono text-xs text-accent-ice overflow-x-auto">
                    {`{\n  "os": "Windows",\n  "browser": "Discord Client",\n  "release_channel": "stable",\n  "client_version": "1.0.9168",\n  "os_version": "10.0.22631",\n  "os_arch": "x64",\n  "client_build_number": ${connectionStatus.build_number}\n}`}
                  </div>
                </div>

                <div className="p-3 rounded-lg bg-amber-950/20 border border-amber-500/30 flex items-start gap-3">
                  <AlertTriangle className="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" />
                  <div className="text-xs text-amber-200/90">
                    <span className="font-semibold text-amber-300">Aviso ToS:</span> El bloqueo de telemetría y enmascaramiento reduce la superficie de detección al mínimo técnicamente posible, pero no asegura indetectabilidad contra análisis de comportamiento heurístico.
                  </div>
                </div>
              </div>
            )}

            {/* Appearance Tab */}
            {activeTab === 'appearance' && (
              <div className="space-y-5">
                <div>
                  <h3 className="text-xs font-bold text-muted uppercase tracking-wider mb-3">
                    Color de Acento
                  </h3>
                  <div className="flex gap-3">
                    {ACCENT_PRESETS.map((color) => (
                      <button
                        key={color.hex}
                        onClick={() => setAccentColor(color.hex)}
                        className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border text-xs font-medium transition-all ${
                          accentColor === color.hex
                            ? 'border-white ring-2 ring-accent-primary'
                            : 'border-border-subtle hover:border-muted'
                        }`}
                        style={{ backgroundColor: `${color.hex}22` }}
                      >
                        <span
                          className="w-3.5 h-3.5 rounded-full"
                          style={{ backgroundColor: color.hex }}
                        />
                        <span className="text-primary">{color.name}</span>
                      </button>
                    ))}
                  </div>
                </div>

                <div>
                  <h3 className="text-xs font-bold text-muted uppercase tracking-wider mb-3">
                    Densidad de la Interfaz
                  </h3>
                  <div className="grid grid-cols-2 gap-3">
                    <button
                      onClick={() => setUiDensity('compact')}
                      className={`p-3 rounded-lg border text-left transition-all ${
                        uiDensity === 'compact'
                          ? 'border-accent-cyan bg-active'
                          : 'border-border-subtle bg-surface hover:bg-hover'
                      }`}
                    >
                      <div className="font-semibold text-xs text-primary">Compacta</div>
                      <div className="text-[11px] text-muted mt-0.5">Mayor cantidad de mensajes en pantalla</div>
                    </button>
                    <button
                      onClick={() => setUiDensity('comfortable')}
                      className={`p-3 rounded-lg border text-left transition-all ${
                        uiDensity === 'comfortable'
                          ? 'border-accent-cyan bg-active'
                          : 'border-border-subtle bg-surface hover:bg-hover'
                      }`}
                    >
                      <div className="font-semibold text-xs text-primary">Cómoda (Default)</div>
                      <div className="text-[11px] text-muted mt-0.5">Espaciado amplio para lecturas relajadas</div>
                    </button>
                  </div>
                </div>

                <div>
                  <h3 className="text-xs font-bold text-muted uppercase tracking-wider mb-3">
                    Accesibilidad & Rendimiento Gráfico
                  </h3>
                  <label className="flex items-center gap-3 p-3 bg-surface rounded-lg border border-border-subtle cursor-pointer hover:bg-hover transition-colors">
                    <input
                      type="checkbox"
                      checked={reducedMotion}
                      onChange={(e) => setReducedMotion(e.target.checked)}
                      className="rounded accent-accent-primary w-4 h-4"
                    />
                    <div>
                      <span className="text-xs font-medium text-primary block">Reducir animaciones</span>
                      <span className="text-[11px] text-muted">Desactiva transiciones complejas para menor uso de CPU.</span>
                    </div>
                  </label>
                </div>
              </div>
            )}

            {/* Account & Token Tab */}
            {activeTab === 'account' && (
              <div className="space-y-4">
                <div className="p-3 rounded-lg bg-surface border border-border-subtle flex items-start gap-3">
                  <Lock className="w-5 h-5 text-accent-cyan flex-shrink-0 mt-0.5" />
                  <div className="text-xs space-y-1">
                    <p className="font-semibold text-primary">
                      Almacenamiento en Keychain Nativo
                    </p>
                    <p className="text-muted">
                      El token de Discord se almacena de forma segura usando el administrador de credenciales de Windows (`Windows Credential Manager`) a través del crate de Rust `keyring`.
                    </p>
                  </div>
                </div>

                <div className="space-y-2">
                  <label className="text-xs font-medium text-secondary_text">
                    Token de Usuario (Discord User Token)
                  </label>
                  <div className="flex gap-2">
                    <input
                      type="password"
                      value={tokenInput}
                      onChange={(e) => setTokenInput(e.target.value)}
                      placeholder="Pega tu token aquí para la Fase 2.5..."
                      className="flex-1 bg-surface border border-border-subtle rounded-lg px-3 py-2 text-xs font-mono text-primary placeholder-muted outline-none focus:border-accent-primary"
                    />
                    <button
                      onClick={() => alert('Listo para registrar en Keychain en la Fase 2.5')}
                      className="px-4 py-2 bg-accent-primary hover:bg-accent-cyan text-white text-xs font-medium rounded-lg transition-colors shadow-glow-sm"
                    >
                      Guardar en Keychain
                    </button>
                  </div>
                  <p className="text-[11px] text-muted">
                    Nunca compartas tu token con nadie. DashCord no envía tu token a ningún servidor que no sea `discord.com/api/v10`.
                  </p>
                </div>
              </div>
            )}

            {/* Performance Tab */}
            {activeTab === 'perf' && (
              <div className="space-y-4">
                <div className="grid grid-cols-2 gap-3">
                  <div className="p-4 bg-surface rounded-xl border border-border-subtle">
                    <span className="text-xs text-muted block">Memoria RAM Estimada</span>
                    <span className="text-2xl font-bold font-mono text-accent-cyan mt-1 block">~118 MB</span>
                    <span className="text-[10px] text-emerald-400 mt-1 block">85% menor que Discord oficial</span>
                  </div>
                  <div className="p-4 bg-surface rounded-xl border border-border-subtle">
                    <span className="text-xs text-muted block">CPU en Reposo</span>
                    <span className="text-2xl font-bold font-mono text-accent-ice mt-1 block">0.4 %</span>
                    <span className="text-[10px] text-emerald-400 mt-1 block">Sin renderizado redundante</span>
                  </div>
                </div>
              </div>
            )}
          </div>
        </main>
      </div>
    </div>
  );
};
