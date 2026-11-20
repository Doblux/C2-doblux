mod app;
mod ui;
mod conn;
mod agente;
use ratatui::{prelude::*, crossterm::event};
use app::App;
use tokio::sync::mpsc;
use std::env;
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};

/// Evento de entrada unificado: teclado o ratón.
/// Reemplaza el canal anterior que solo llevaba KeyEvent.
enum AppEvent {
    Key(event::KeyEvent),
    Mouse(event::MouseEvent),
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    ratatui::crossterm::terminal::enable_raw_mode()?;
    ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::EnterAlternateScreen,
        event::EnableMouseCapture, // ← NUEVO: habilitamos captura de mouse junto con la terminal
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let puerto: u16 = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(4444);

    let mut app = App::new(puerto);

    let (tx_red, mut rx_red) = mpsc::channel(32);

    // Canal unificado de eventos de entrada (antes era solo teclas)
    let (tx_input, mut rx_input) = mpsc::channel::<AppEvent>(32);

    // Servidor TCP en segundo plano
    tokio::spawn(async move {
        let addr = format!("0.0.0.0:{}", puerto);
        if let Err(e) = conn::listen(&addr, tx_red).await {
            eprintln!("Error en el servidor de red: {}", e);
        }
    });

    // Hilo de entrada: captura teclas Y eventos de ratón
    std::thread::spawn(move || {
        loop {
            match event::read() {
                Ok(event::Event::Key(key)) => {
                    if key.kind == event::KeyEventKind::Press {
                        if tx_input.blocking_send(AppEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                }
                Ok(event::Event::Mouse(mouse)) => { // ← NUEVO
                    if tx_input.blocking_send(AppEvent::Mouse(mouse)).is_err() {
                        break;
                    }
                }
                _ => {}
            }
        }
    });

    let mut tick = interval(Duration::from_millis(60));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            // Eventos de red
            Some(evento_red) = rx_red.recv() => {
                match evento_red {
                    conn::ConnEvent::Connected(addr, stream) => {
                        app.add_agent(addr, stream);
                    }
                    conn::ConnEvent::Disconnected(addr) => {
                        if let Some(index) = app.agentes.iter().position(|a| a.ip == addr.to_string()) {
                            app.remove_agent(index);
                        }
                    }
                    // salida del shell del agente
                    conn::ConnEvent::Output(addr, data) => {
                        let addr_str = addr.to_string();
                        if let Some(agente) = app.agentes.iter_mut().find(|a| a.ip == addr_str) {
                            // from_utf8_lossy nunca paniquea: bytes inválidos → '?'
                            // replace '\r': evita que carriage returns sobreescriban la última línea visualmente
                            let texto = String::from_utf8_lossy(&data).replace('\r', "");
                            agente.terminal_output.push_str(&texto);
                        }
                    }
                }
            }

            // Eventos de entrada (teclas + ratón)
            Some(evento_input) = rx_input.recv() => {
                match evento_input {

                    // ── RATÓN ────────────────────────────────────────────────────────────
                    AppEvent::Mouse(mouse) => {
                        // Solo nos interesa el botón izquierdo presionado
                        if let event::MouseEventKind::Down(event::MouseButton::Left) = mouse.kind {
                            // Solo actuamos si hay agentes y no estamos en un modo especial
                            if !app.agentes.is_empty() && !app.renaming && !app.ctrl_b_prefix {
                                if let Some(rect) = app.send_button_rect.get() {
                                    let dentro_del_boton = mouse.column >= rect.x
                                        && mouse.column < rect.x + rect.width
                                        && mouse.row >= rect.y
                                        && mouse.row < rect.y + rect.height;

                                    if dentro_del_boton {
                                        if let Some(agente) = app.current_agent_mut() {
                                            if let Err(e) = agente.send_input().await {
                                                agente.terminal_output.push_str(
                                                    &format!("[!] Error de envío: {e}\n")
                                                );
                                            }
                                        }
                                        app.force_cursor_visible();
                                    }
                                }
                            }
                        }
                    }

                    // ── TECLADO ──────────────────────────────────────────────────────────
                    AppEvent::Key(key) => {
                        // --- Cancelar renaming con Esc ---
                        if app.renaming && key.code == event::KeyCode::Esc {
                            if let Some(agente) = app.current_agent_mut() {
                                if let Some(respaldo) = &agente.backup_nombre {
                                    agente.nombre = respaldo.clone();
                                }
                            }
                            app.renaming = false;
                            continue;
                        }

                        // --- Salir con Esc o Ctrl+C ---
                        if key.code == event::KeyCode::Esc
                            || (key.modifiers.contains(event::KeyModifiers::CONTROL)
                                && key.code == event::KeyCode::Char('c'))
                        {
                            app.quit();
                            continue;
                        }

                        // --- Modo renaming ---
                        if app.renaming {
                            if let Some(agente) = app.current_agent_mut() {
                                match key.code {
                                    event::KeyCode::Enter => {
                                        agente.validate_or_restore_name();
                                        app.renaming = false;
                                    }
                                    event::KeyCode::Char(c) => {
                                        agente.add_char_to_name(c);
                                    }
                                    event::KeyCode::Backspace => {
                                        agente.del_char_to_name();
                                    }
                                    _ => {}
                                }
                            }
                            continue;
                        }

                        // --- Prefijo Ctrl+B ---
                        if app.ctrl_b_prefix {
                            app.ctrl_b_prefix = false;

                            match key.code {
                                event::KeyCode::Char('r') | event::KeyCode::Char('R') => {
                                    if let Some(agente) = app.current_agent_mut() {
                                        agente.backupear_nombre();
                                        app.renaming = true;
                                    }
                                }
                                event::KeyCode::Char('a') | event::KeyCode::Char('A') => {
                                    // Futuro popup de IA
                                }
                                event::KeyCode::Up => {
                                    if app.agente_seleccionado > 0 {
                                        app.agente_seleccionado -= 1;
                                    }
                                }
                                event::KeyCode::Down => {
                                    if !app.agentes.is_empty()
                                        && app.agente_seleccionado < app.agentes.len() - 1
                                    {
                                        app.agente_seleccionado += 1;
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // --- Detectar Ctrl+B ---
                        if key.modifiers.contains(event::KeyModifiers::CONTROL)
                            && key.code == event::KeyCode::Char('b')
                        {
                            app.ctrl_b_prefix = true;
                            continue;
                        }

                        // --- Enviar con alt+Enter ──────────────────────────────────────────────────────────
                        let es_alt_enter = key.modifiers.contains(event::KeyModifiers::ALT)
                            && key.code == event::KeyCode::Enter;

                        if es_alt_enter {
                            if let Some(agente) = app.current_agent_mut() {
                                if let Err(e) = agente.send_input().await {
                                    agente.terminal_output.push_str(
                                        &format!("[!] Error de envío: {e}\n")
                                    );
                                }
                            }
                            app.force_cursor_visible();
                            continue;
                        }
                        // ──────────────────────────────────────────────────────────────────

                        // --- Input del agente seleccionado ---
                        if let Some(actual_agente) = app.current_agent_mut() {
                            match key.code {
                                event::KeyCode::Char(c) => {
                                    actual_agente.insert_char(c);
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::Backspace => {
                                    actual_agente.backspace();
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::Enter => {
                                    actual_agente.insert_char('\n');
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::Left => {
                                    if key.modifiers.contains(event::KeyModifiers::CONTROL) {
                                        actual_agente.move_word_left();
                                    } else {
                                        actual_agente.move_left();
                                    }
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::Right => {
                                    if key.modifiers.contains(event::KeyModifiers::CONTROL) {
                                        actual_agente.move_word_right();
                                    } else {
                                        actual_agente.move_right();
                                    }
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::Up => {
                                    actual_agente.move_up();
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::Down => {
                                    actual_agente.move_down();
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::Home => {
                                    actual_agente.move_home();
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::End => {
                                    actual_agente.move_end();
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::Delete => {
                                    if key.modifiers.contains(event::KeyModifiers::CONTROL) {
                                        actual_agente.delete_word_forward();
                                    } else {
                                        actual_agente.supr_char();
                                    }
                                    app.force_cursor_visible();
                                }
                                _ => {}
                            }
                        }
                    } // fin AppEvent::Key
                } // fin match evento_input
            }

            // 3. Tick del cursor (parpadeo)
            _ = tick.tick() => {
                app.update_cursor_blink();
            }
        }

        // Dibujar
        terminal.draw(|f| ui::draw(f, &app))?;

        if app.should_quit {
            break;
        }
    }

    // Restaurar terminal — siempre en el mismo orden inverso al setup
    ratatui::crossterm::terminal::disable_raw_mode()?;
    ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::LeaveAlternateScreen,
        event::DisableMouseCapture, // par de EnableMouseCapture
    )?;
    Ok(())
}
