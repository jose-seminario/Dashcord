# DashCord ⚡

**DashCord** es un cliente de escritorio no oficial para Discord desarrollado en **Rust + Tauri v2** y una interfaz moderna con tecnología web ligera (Vite + React + Tailwind CSS).

Diseñado con un objetivo principal: ofrecer una experiencia de usuario ultra rápida, con una estética gaming azulada (*Midnight Blue*), un consumo de recursos muy inferior al cliente oficial basado en Electron (< 150 MB de RAM en reposo), y una capa técnica de evasión de detección que emula rigurosamente los fingerprints TLS/HTTP2 y las `super_properties` del cliente oficial.

---

## ⚠️ Advertencia de Seguridad y ToS

> [!WARNING]
> El uso de clientes no oficiales de Discord y la conexión con tokens de usuario viola los **Términos de Servicio de Discord** (apartado de API no autorizada y self-botting).
>
> **DashCord** implementa las mejores técnicas conocidas de evasión (suplantación de firmas TLS JA3/HTTP2 vía `wreq` con BoringSSL, generación exacta de `X-Super-Properties`, jitter en heartbeats de Gateway y bloqueo de endpoints de telemetría). Sin embargo, **no existe la indetectabilidad absoluta**. Úsalo bajo tu propia responsabilidad.

---

## 📌 Declaración Oficial de Estado: Fase 2.5

Fase 2.5 queda en estado OPEN/BLOCKED en este entorno universitario. La compilación y los tests unitarios se ejecutan EXCLUSIVAMENTE en GitHub Actions sobre un runner windows-latest. Los criterios 1, 2, 3, 4, 5 y 8 de la tabla de cierre se validan compilando y ejecutando los tests en el runner de CI, y se verifican descargando el artifact y ejecutando el .exe en la PC del usuario (solo ejecución, sin compilación). Los criterios 6 y 7 se validan vía tests unitarios ejecutados en CI. Ningún criterio se marcará como PASS hasta que se adjunte evidencia del run de GitHub Actions y del artifact descargado.

---

## 🛡️ Dependencia TLS

DashCord usa wreq 6.0.0-rc.31 con BoringSSL para suplantación de fingerprint TLS. La dependencia anterior, rquest, fue retirada de crates.io por su autor y no se usa. Se verifica en CI que openssl-sys no esté presente en el grafo para evitar conflictos de enlazado.

---

## 🎯 Alcance del Proyecto

### En Alcance (Versión 1.0)
1. **Inicio de sesión y autenticación segura**: Almacenamiento seguro del token de usuario exclusivamente a través del Keychain del sistema operativo (`keyring-rs`). No se almacena texto plano ni se expone en logs.
2. **Navegación de servidores, canales y DMs**: Árbol completo de canales de texto, categorías y servidores con selector interactivo y badges de notificaciones.
3. **Mensajería en tiempo real (Gateway v10)**: Envío y recepción bidireccional de mensajes con gestión de secuencias, replies, edición y borrado de mensajes propios, y reacciones básicas.
4. **Capa de Evasión Integrada**: Suplantación TLS/JA3 basada en BoringSSL (`wreq`), simulación de entorno Windows en todas las plataformas, generación de cabeceras `X-Super-Properties`, resolución dinámica de build number con regla fail-closed y bloqueo estricto de telemetría (`/api/science`, Datadog, Sentry).
5. **UI Gaming 'Midnight Blue' y Rendimiento**: Interfaz fluida, personalización básica de densidades y fuentes, métricas de conexión (latencia, ping en vivo) y Command Palette (`Ctrl + K`).

### Fuera de Alcance v1 (Planificado para v2 o Descartado)
- **Llamadas de Voz y Video (WebRTC)**: Pospuesto para versiones posteriores para no comprometer la estabilidad del runtime ni la evasión de transporte en el lanzamiento inicial.
- **Plugins de terceros sin sandbox**: Rechazado por seguridad para evitar robo de credenciales y desincronizaciones de fingerprinting.
- **Telemetría y Crash Reporting Externo**: Descartado por diseño; DashCord nunca recolecta ni envía información del usuario.
- **Tienda o Marketplace de Temas**: v1 incluye temas locales basados en variables CSS (`:root`).
- **Soporte de Cuentas Múltiples Simultáneas**: Pospuesto para v1.2.

---

## 🛠️ Tecnologías y Arquitectura de Evasión

### Estado de `discord-client-rs`
- **Declaración formal**: `discord-client-rs` se mantiene únicamente como **referencia documental** y conceptual en la arquitectura de DashCord. Dado que el repositorio original no recibe commits activos desde hace más de 6 meses, **no forma parte del árbol de dependencias (`Cargo.toml`)**.
- **Reemplazo auditado**:
  - **REST / TLS**: `wreq` (BoringSSL nativo vía `btls`).
  - **WebSocket Gateway**: `tokio-tungstenite` con inyección manual de cabeceras de navegador (`Origin`, `User-Agent`, `X-Super-Properties`).
  - **Auditoría de grafo**: Confirmado 0% presencia de `reqwest` y 0% presencia de `openssl-sys` en cualquier nivel del proyecto.

### Política de Logs y Privacidad
- Subsistema implementado con `tracing` y formateador de seguridad.
- Redacción estricta en caliente: `Authorization`, `Cookie`, `X-Super-Properties`, contraseñas y tokens son reemplazados por `[REDACTED]`.
- Contenido de mensajes: Suprimido de logs.
- Retención: Rotación automática diaria con retención estricta de 7 días.

---

## 🚀 Integración Continua (GitHub Actions)

La compilación y el empaquetado del binario `DashCord.exe` se realizan exclusivamente de forma automatizada en el entorno seguro de CI mediante GitHub Actions:

- **Workflow**: `.github/workflows/dashcord.yml`
- **Runner**: `windows-latest`
- **Pasos**:
  1. Verificación de compilación del frontend (`npm run build`).
  2. Auditoría estricta de dependencias: Guard - No `openssl-sys`.
  3. Ejecución de la suite de pruebas unitarias (`cargo test --workspace --all-features`).
  4. Generación del bundle con `tauri-action`.
  5. Publicación del artefacto `windows-dashcord`.

---

## 📥 Cómo descargar el ejecutable (.exe)

Los binarios de DashCord no se compilan localmente en este entorno universitario por restricciones de privilegios y herramientas, sino que se generan de forma reproducible y segura en GitHub Actions.

Para probar la aplicación:
1. Dirígete a la pestaña **Actions** en el repositorio de GitHub: `https://github.com/jose-seminario/audio-dash/actions`.
2. Haz clic en la última ejecución exitosa (en **verde**) del workflow **Build DashCord (Windows x64)**.
3. Al final de la página, en la sección **Artifacts**, descarga el archivo comprimido **`windows-dashcord`**.
4. Descomprime el archivo ZIP en una carpeta de tu equipo.
5. Abre PowerShell y calcula el hash criptográfico para verificar la integridad del binario:
   ```powershell
   Get-FileHash .\dashcord.exe -Algorithm SHA256
   ```
6. Para ejecutar el protocolo de reconocimiento pasivo (**Sub-fase A**):
   ```powershell
   .\dashcord.exe --dry-run
   ```
