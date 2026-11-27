mod app;
mod ui;
mod conn;
mod agente;
mod ai_agent;
mod theme;
mod markdown;
use ratatui::{prelude::*, crossterm::event};
use app::App;
use tokio::sync::mpsc;
use std::env;
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};
use ai_agent::{AiAgent, AiEvent};

/// Evento de entrada unificado: teclado o ratón.
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
        event::EnableMouseCapture,
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let puerto: u16 = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(4444);

    let mut app = App::new(puerto);

    let ai = match AiAgent::from_env() {
        Ok(agent) => Some(agent),
        Err(e) => {
            eprintln!("[AI] No se pudo inicializar: {e}");
            eprintln!("     Definí AI_API_KEY (y opcionalmente AI_BASE_URL / AI_MODEL)");
            None
        }
    };

    let (tx_ai, mut rx_ai) = mpsc::channel::<AiEvent>(8);


    let (tx_red, mut rx_red) = mpsc::channel(32);
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
                Ok(event::Event::Mouse(mouse)) => {
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
            // ── EVENTOS DE RED ────────────────────────────────────────────────
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
                    conn::ConnEvent::Output(addr, data) => {
                        let addr_str = addr.to_string();
                        if let Some(agente) = app.agentes.iter_mut().find(|a| a.ip == addr_str) {
                            // Una sola llamada: maneja texto, binario y transiciones internamente.
                            agente.append_terminal_output(&data);
                        }
                    }
                }
            }

            // Eventos de Inteligencia Artificial ------------------------------------------------------------
            Some(ai_event) = rx_ai.recv() => {
                match ai_event {
                    AiEvent::Response { addr, text } => {
                        if let Some(agente) = app.agentes.iter_mut().find(|a| a.ip == addr) {
                            agente.ai_response = text;
                            agente.ai_loading = false;
                        }
                    }
                    AiEvent::Error { addr, err } => {
                        if let Some(agente) = app.agentes.iter_mut().find(|a| a.ip == addr) {
                            agente.ai_response = format!("[Error IA] {err}");
                            agente.ai_loading = false;
                        }
                    }
                }
            }

            // ── EVENTOS DE ENTRADA ────────────────────────────────────────────
            Some(evento_input) = rx_input.recv() => {
                match evento_input {

                    // ── RATÓN ─────────────────────────────────────────────────
                    AppEvent::Mouse(mouse) => {
                        match mouse.kind {

                            // ── Rueda arriba ──────────────────────────────────
                            event::MouseEventKind::ScrollUp => {
                                // Leemos el rect ANTES del borrow mutable del agente.
                                // Cell::get() solo requiere &self, no hay conflicto.
                                let rect = app.output_rect.get();
                                if let Some(agente) = app.current_agent_mut() {
                                    if agente.auto_scroll {
                                        // si veníamos de auto_scroll, primero
                                        // materializamos el offset real (máximo) antes
                                        // de restar, para no partir de u16::MAX.
                                        agente.scroll_offset = rect
                                            .map(|r| ui::compute_max_scroll(&agente.terminal_output, r))
                                            .unwrap_or(0);
                                        agente.auto_scroll = false;
                                    }
                                    agente.scroll_offset = agente.scroll_offset.saturating_sub(3);
                                }
                            }

                            // ── Rueda abajo ───────────────────────────────────
                            event::MouseEventKind::ScrollDown => {
                                let rect = app.output_rect.get();
                                if let Some(agente) = app.current_agent_mut() {
                                    // clampear al máximo real y reactivar auto_scroll
                                    // si el usuario llega al fondo.
                                    let max = rect
                                        .map(|r| ui::compute_max_scroll(&agente.terminal_output, r))
                                        .unwrap_or(0);
                                    agente.scroll_offset =
                                        agente.scroll_offset.saturating_add(3).min(max);
                                    if agente.scroll_offset >= max {
                                        agente.auto_scroll = true;
                                    }
                                }
                            }

                            // ── Click izquierdo (botón Enviar) ────────────────
                            event::MouseEventKind::Down(event::MouseButton::Left) => {
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
                                                        &format!("[!] Error de envío: {e}\n"),
                                                    );
                                                }
                                            }
                                            app.force_cursor_visible();
                                        }
                                    }
                                }
                            }

                            _ => {}
                        }
                    }

                    // ── TECLADO ───────────────────────────────────────────────
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
                                    app.show_ai_panel = !app.show_ai_panel;
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
                                event::KeyCode::Char('i') | event::KeyCode::Char('I') => {
                                    if let (Some(ai_agent), Some(agente)) = (&ai, app.current_agent()) {
                                        // 1. Clonamos todo lo que necesitamos del agente
                                        let ip = agente.ip.clone();
                                        let nombre = agente.nombre.clone();
                                        let output = agente.terminal_output.clone();

                                        // 2. Ahora sí podemos pedir el borrow mutable (el inmutable ya no vive)
                                        if let Some(ag) = app.current_agent_mut() {
                                            ag.ai_loading = true;
                                            ag.ai_response = String::from("Analizando...");
                                        }

                                        // 3. Llamamos a Gemini con los clones
                                        ai_agent.request_analysis(
                                            tx_ai.clone(),
                                            ip,
                                            nombre,
                                            &output,
                                        );
                                    }
                                }
                                event::KeyCode::Char('t') | event::KeyCode::Char('T') => {
                                    app.next_theme();
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

                        // --- Alt+Enter: enviar comando ───────────────────────
                        let es_alt_enter = key.modifiers.contains(event::KeyModifiers::ALT)
                            && key.code == event::KeyCode::Enter;

                        if es_alt_enter {
                            if let Some(agente) = app.current_agent_mut() {
                                if let Err(e) = agente.send_input().await {
                                    agente.terminal_output.push_str(
                                        &format!("[!] Error de envío: {e}\n"),
                                    );
                                }
                            }
                            app.force_cursor_visible();
                            continue;
                        }

                        let es_alt_up = key.modifiers.contains(event::KeyModifiers::ALT)
                            && key.code == event::KeyCode::Up;

                        if es_alt_up {
                            if let Some(agente) = app.current_agent_mut() {
                                if let Some(cmd) = agente.history_up() {
                                    agente.terminal_input = cmd;
                                    // Mover el cursor al final del comando recuperado
                                    agente.cursor_pos = agente.terminal_input.len();
                                }
                            }
                            app.force_cursor_visible();
                            continue;
                        }

                        let es_alt_down = key.modifiers.contains(event::KeyModifiers::ALT)
                            && key.code == event::KeyCode::Down;

                        if es_alt_down {
                            if let Some(agente) = app.current_agent_mut() {
                                if let Some(cmd) = agente.history_down() {
                                    agente.terminal_input = cmd;
                                    agente.cursor_pos = agente.terminal_input.len();
                                }
                            }
                            app.force_cursor_visible();
                            continue;
                        }
                        // --- Input del agente seleccionado ──────────────────
                        // Leemos output_rect ANTES del borrow mutable para usarlo
                        // dentro de los arms de PageUp / PageDown.
                        let output_rect_snap = app.output_rect.get();

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
                                    if key.modifiers.contains(event::KeyModifiers::ALT) {
                                        // Alt+Home → ir al principio del output
                                        actual_agente.scroll_offset = 0;
                                        actual_agente.auto_scroll = false;
                                    } else {
                                        actual_agente.move_home();
                                    }
                                    app.force_cursor_visible();
                                }
                                event::KeyCode::End => {
                                    if key.modifiers.contains(event::KeyModifiers::ALT) {
                                        // Alt+End → reactivar auto_scroll.
                                        // ui::draw() calculará el offset real en el
                                        // siguiente frame; no necesitamos u16::MAX.
                                        actual_agente.auto_scroll = true;
                                    } else {
                                        actual_agente.move_end();
                                    }
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

                                // ── PageUp ────────────────────────────────────
                                event::KeyCode::PageUp => {
                                    // si veníamos de auto_scroll, materializamos
                                    // el offset máximo real antes de restar, para no
                                    // partir de u16::MAX y quedar en un valor enorme.
                                    if actual_agente.auto_scroll {
                                        actual_agente.scroll_offset = output_rect_snap
                                            .map(|r| ui::compute_max_scroll(&actual_agente.terminal_output, r))
                                            .unwrap_or(0);
                                        actual_agente.auto_scroll = false;
                                    }
                                    // Desplazamiento = altura interna del panel (página completa),
                                    // igual que en un terminal real (tmux, screen, etc.).
                                    let page = output_rect_snap
                                        .map(|r| r.height.saturating_sub(2).max(1))
                                        .unwrap_or(10);
                                    actual_agente.scroll_offset =
                                        actual_agente.scroll_offset.saturating_sub(page);
                                    app.force_cursor_visible();
                                }

                                // ── PageDown ──────────────────────────────────
                                event::KeyCode::PageDown => {
                                    // clampear al máximo y reactivar auto_scroll
                                    // si el usuario llega al fondo.
                                    let max = output_rect_snap
                                        .map(|r| ui::compute_max_scroll(&actual_agente.terminal_output, r))
                                        .unwrap_or(0);
                                    let page = output_rect_snap
                                        .map(|r| r.height.saturating_sub(2).max(1))
                                        .unwrap_or(10);
                                    actual_agente.scroll_offset =
                                        actual_agente.scroll_offset.saturating_add(page).min(max);
                                    if actual_agente.scroll_offset >= max {
                                        actual_agente.auto_scroll = true;
                                    }
                                    app.force_cursor_visible();
                                }

                                _ => {}
                            }
                        }
                    } // fin AppEvent::Key
                } // fin match evento_input
            }

            // ── Tick del cursor (parpadeo) ─────────────────────────────────
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

    // Restaurar terminal
    ratatui::crossterm::terminal::disable_raw_mode()?;
    ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::LeaveAlternateScreen,
        event::DisableMouseCapture,
    )?;
    Ok(())
}
