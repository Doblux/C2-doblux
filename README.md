# C2 DOBLUX (`v0.1`)

Servidor **Command & Control (C2)** con interfaz de terminal (**TUI**) escrito en **Rust**, construido con [`ratatui`](https://ratatui.rs/), [`crossterm`](https://github.com/crossterm-rs/crossterm) y [`tokio`](https://tokio.rs/).

Permite aceptar múltiples conexiones TCP concurrentes, administrar agentes desde un panel lateral, enviar comandos multilínea desde un buffer `String` seguro en UTF-8 y visualizar la salida en tiempo real dentro de una interfaz de tres columnas.

---

## Características actuales

- **Servidor TCP asíncrono:** Escucha conexiones entrantes en `0.0.0.0:<puerto>` (por defecto `4444`) mediante `tokio::net::TcpListener` y separa cada conexión en mitades independientes de lectura y escritura (`into_split`).
- **Paso de mensajes por canales (`mpsc`):** Coordina tareas de red, captura de teclado/ratón y el bucle principal (`tokio::select!`) usando `tokio::sync::mpsc`.
- **Interfaz TUI de tres paneles:**
  - **Agentes (izquierda):** Lista de clientes conectados con ajuste de línea y resaltado del agente activo.
  - **Terminal (centro):** Historial de sesión del agente seleccionado (o banner de escucha cuando no hay agentes), campo de entrada multilínea de altura dinámica (`3` a `10` líneas) y botón **Enviar**.
  - **IA (derecha):** Panel lateral reservado para futuras funciones de asistencia/análisis.
- **Buffer de entrada UTF-8 seguro:** `terminal_input` basado en `String` con `cursor_pos` calculado en límites de bytes (`len_utf8`, `char_indices`) y parpadeo de cursor configurable.
- **Soporte de teclado y ratón:** Captura clics izquierdos sobre el botón **Enviar** y atajos de teclado para edición y control de agentes.
- **Gestión básica de agentes:** Cambio de agente activo, renombrado modal con respaldo/restauración (`backup_nombre`) y limpieza automática al desconectarse.

---

## Estructura del proyecto

```text
.
├── Cargo.toml
└── src/
    ├── main.rs     # Setup/restauración de terminal, loop principal y manejo de eventos
    ├── conn.rs     # Listener TCP, lectura asíncrona por agente y emisión de ConnEvent
    ├── app.rs      # Estado global de la aplicación (App), agentes, parpadeo y geometría UI
    ├── agente.rs   # Modelo Agente, edición de texto/cursor, renombrado y envío TCP
    └── ui.rs       # Renderizado de paneles, cursor y popup modal de renombrado
```

---

## Requisitos y compilación

- **Rust** (`rustc` y `cargo` actualizados).
- Dependencias principales en `Cargo.toml`: `ratatui`, `crossterm`, `tokio` (con features `rt-multi-thread`, `macros`, `net`, `io-util`, `sync`, `time`), y `color-eyre`.

Clonar y compilar:

```bash
git clone <url-del-repositorio>
cd <directorio-del-proyecto>
cargo build --release
```

---

## Ejecución y pruebas

### 1. Iniciar el servidor TUI

Podés pasar el puerto como primer argumento o dejar el valor por defecto (`4444`):

```bash
cargo run -- 4444
```

### 2. Conectar un cliente de prueba con `netcat`

En otra terminal, conectate al puerto donde escucha el servidor:

```bash
nc 127.0.0.1 4444
```

Una vez que aparezca el agente en el panel **Agentes**:
1. Escribí el texto o comando en el panel **Input**.
2. Enviálo con `Alt + Enter` o haciendo clic izquierdo en **Enviar**.
3. El texto saldrá por el socket TCP (con `\n` final si no terminaba en salto de línea), se registrará en **Terminal** como `[>] ...` y cualquier dato que escribas o devuelva el cliente conectado aparecerá en **Terminal**.

---

## Atajos y controles

| Atajo / Acción | Comportamiento |
| :--- | :--- |
| `Alt + Enter` o clic en **Enviar** | Envía el contenido de `Input` al agente seleccionado |
| `Enter` | Inserta un salto de línea (`\n`) en `Input` |
| `←` / `→` / `↑` / `↓` | Desplaza el cursor dentro del buffer multilínea |
| `Ctrl + ←` / `Ctrl + →` | Salta por palabras hacia la izquierda o derecha |
| `Home` / `End` | Va al inicio o al final de la línea actual |
| `Backspace` / `Supr` | Borra el carácter anterior o posterior al cursor |
| `Ctrl + Supr` | Borra la palabra siguiente |
| `Ctrl + B`, luego `↑` / `↓` | Cambia el agente seleccionado |
| `Ctrl + B`, luego `R` | Abre el popup para renombrar el agente (`Enter` guarda, `Esc` cancela) |
| `Ctrl + B`, luego `A` | Atajo reservado para el módulo de IA |
| `Esc` o `Ctrl + C` | Cierra la aplicación y restaura la terminal |

---

## Estado y próximos pasos (Roadmap)

- [ ] **Scroll en `Terminal`:** Desplazamiento vertical (rueda del ratón / teclas) y seguimiento automático al recibir salida nueva.
- [ ] **Selección por clic:** Elegir agentes directamente haciendo clic sobre el panel **Agentes**.
- [ ] **Módulo de IA:** Integración del panel lateral y/o popup interactivo.
- [ ] **Agente y protocolo propio:** Cliente dedicado (en C o Rust) y framing de mensajes más robusto sobre TCP.

---

## Aviso legal

Este proyecto se desarrolla con fines educativos, de aprendizaje de sistemas/redes y experimentación en entornos controlados. Utilizalo únicamente en infraestructuras y sistemas propios o donde cuentes con autorización explícita.