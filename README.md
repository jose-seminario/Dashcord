# DashCord ⚡

DashCord es un cliente de escritorio no oficial para Discord, desarrollado en **Rust + Tauri v2** con una interfaz moderna (Vite + React + Tailwind CSS).

Diseñado con un objetivo claro: ofrecer una experiencia de usuario ultrarrápida, con una estética **Gamer Neón (Azul/Morado)**, un consumo de recursos mínimo (superando los ~28 MB de RAM en reposo frente a los 600-900 MB del cliente oficial basado en Electron) y una capa técnica de evasión de detección.

---

### ⚠️ Advertencia de Seguridad y ToS

> **El uso de clientes no oficiales de Discord y la conexión con tokens de usuario viola los Términos de Servicio de Discord** (apartado de API no autorizada y self-botting). 
> 
> DashCord implementa técnicas de evasión (suplantación de firmas TLS/HTTP2 vía `wreq` con BoringSSL, generación exacta de `X-Super-Properties`, jitter en heartbeats y bloqueo de endpoints de telemetría). Sin embargo, **no existe la indetectabilidad absoluta**. Úsalo bajo tu propia responsabilidad.

---

### ✨ Características Principales

*   **Rendimiento Extremo:** Consumo de RAM y CPU mínimo comparado con Electron.
*   **UI Gamer Neón:** Tema oscuro con acentos en violeta y cian, transiciones suaves y diseño minimalista.
*   **Autenticación Segura:** El token de usuario se almacena exclusivamente en el Keychain del sistema operativo. No se guarda en texto plano ni se expone en logs.
*   **Gateway v10 en Tiempo Real:** Mensajería fluida con soporte para replies, reacciones y edición.
*   **Privacidad por Diseño:** Cero telemetría, cero crash reporting externo. Logs rotativos con redacción estricta de tokens y cookies (`[REDACTED]`).

---

### 🛠️ Stack Tecnológico

*   **Core & Backend:** Rust + Tauri v2.
*   **Frontend:** React, Vite, Tailwind CSS.
*   **Red y Evasión:** 
    *   REST/TLS: `wreq` (BoringSSL nativo).
    *   WebSocket: `tokio-tungstenite` con inyección manual de cabeceras.
    *   *Nota:* El proyecto `discord-client-rs` se mantiene solo como referencia conceptual, ya que el original está sin mantenimiento.

---

### 🚀 Integración Continua (GitHub Actions)

La compilación y el empaquetado del binario se realizan de forma automatizada en GitHub Actions mediante el workflow `build.yml`:
1. Verificación de compilación del frontend (`npm run build`).
2. Auditoría estricta de dependencias (bloqueo de `openssl-sys` y `reqwest`).
3. Ejecución de la suite de pruebas unitarias (`cargo test`).
4. Generación del bundle y publicación del artefacto.

---

### 📥 Cómo descargar y usar el ejecutable (.exe)

Dado que el binario no se compila localmente, se genera de forma reproducible y segura en GitHub Actions.

1. Ve a la pestaña **Actions** en el repositorio: [https://github.com/jose-seminario/Dashcord/actions](https://github.com/jose-seminario/Dashcord/actions).
2. Haz clic en el workflow **Build DashCord (Windows x64)** en la barra lateral izquierda.
3. Haz clic en el botón **Run workflow** (Ejecutar workflow) para compilar la versión más reciente.
4. Espera a que termine (se pondrá en verde).
5. Al final de la página del run, en la sección **Artifacts**, descarga el archivo `windows-dashcord`.
6. Descomprime el archivo ZIP y ejecuta `DashCord.exe`.

*(Opcional)* Para verificar la integridad del binario descargado, abre PowerShell y calcula el hash:
```powershell
Get-FileHash .\DashCord.exe -Algorithm SHA256
   ```
6. Para ejecutar el protocolo de reconocimiento pasivo (**Sub-fase A**):
   ```powershell
   .\dashcord.exe --dry-run
   ```
