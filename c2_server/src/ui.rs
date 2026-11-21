use ratatui::{layout::HorizontalAlignment::Center, prelude::*, widgets::*};
use crate::app::App;

// ─────────────────────────────────────────────────────────────────────────────
// SCROLL HELPERS
// ─────────────────────────────────────────────────────────────────────────────

/// Calcula el offset de scroll máximo (en filas) para un Paragraph renderizado
/// con `Wrap { trim: false }`, dado el texto completo y el Rect del bloque
/// (que incluye bordes de 1 celda por lado).
///
/// La función replica la lógica interna de ratatui: divide por `\n` (no por
/// `str::lines()`, que descarta el salto final) y hace ceil-division del ancho
/// de cada línea lógica entre el ancho visible.
///
/// Esto se usa tanto en `draw()` (para calcular el offset de display cuando
/// `auto_scroll == true`) como en el event-handler de main.rs (para clampar
/// el offset manual al hacer scroll arriba/abajo).
pub(crate) fn compute_max_scroll(text: &str, rect: Rect) -> u16 {
    // Descontamos 1 celda por borde en cada eje.
    let inner_width = rect.width.saturating_sub(2) as usize;
    let inner_height = rect.height.saturating_sub(2);

    if inner_width == 0 { return 0; }

    // ratatui usa split('\n'), NO str::lines(): "abc\n".split('\n') = ["abc", ""]
    // lo que produce una línea vacía extra al final, igual que lo que se renderiza.
    let total_lines: u16 = text
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                1u16
            } else {
                // Ceil division: cuántas filas visuales necesita esta línea lógica.
                let n = line.chars().count();
                u16::try_from((n + inner_width - 1) / inner_width)
                    .unwrap_or(u16::MAX / 2) // protección contra overflow
            }
        })
    .fold(0u16, |acc, l| acc.saturating_add(l));

    total_lines.saturating_sub(inner_height)
}

// ─────────────────────────────────────────────────────────────────────────────
// DRAW
// ─────────────────────────────────────────────────────────────────────────────

pub fn draw(f: &mut Frame, app: &App) {
    app.send_button_rect.set(None);
    // output_rect se sobreescribirá más abajo si hay agentes; si no hay agentes
    // lo dejamos como estaba (el handler lo ignorará porque current_agent es None).

    let chunks = if app.show_ai_panel {
        // Agentes 18% | Terminal 52% | IA 30%
        Layout::horizontal([
            Constraint::Percentage(18),
            Constraint::Percentage(52),
            Constraint::Percentage(30),
        ]).split(f.area())
    } else {
        // Agentes 20% | Terminal 80%  (sin panel de IA)
        Layout::horizontal([
            Constraint::Percentage(20),
            Constraint::Percentage(80),
        ]).split(f.area())
    };

    // --- PANEL IZQUIERDO: lista de agentes ---
    let max_width = chunks[0].width.saturating_sub(4) as usize;

    let items: Vec<ListItem> = app.agentes
        .iter()
        .map(|agente| {
            let wrapped_name = wrap_text(&agente.nombre, max_width);
            ListItem::new(wrapped_name)
        })
    .collect();

    let agentes_list = List::new(items)
        .block(Block::bordered().title(" Agentes "))
        .highlight_style(
            Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    if !app.agentes.is_empty() {
        state.select(Some(app.agente_seleccionado));
    }
    f.render_stateful_widget(agentes_list, chunks[0], &mut state);

    // ─────────────────────────────────────────────────────────────────────────
    // --- PANEL CENTRAL ---
    // ─────────────────────────────────────────────────────────────────────────
    if app.agentes.is_empty() {
        let welcome_text = Paragraph::new(app.current_terminal_output())
            .block(Block::bordered().title(" Terminal "))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });
        f.render_widget(welcome_text, chunks[1]);
    } else {
        // Altura dinámica del input según líneas con \n
        let input_lines = app.current_agent()
            .map(|a| a.terminal_input.chars().filter(|&c| c == '\n').count() + 1)
            .unwrap_or(1);
        let input_height = (input_lines as u16 + 2).clamp(3, 10);

        let center_chunks = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(input_height),
        ]).split(chunks[1]);

        // ← Guardamos el rect del panel de output para que el event-handler
        //   pueda calcular compute_max_scroll sin tener acceso al Frame.
        app.output_rect.set(Some(center_chunks[0]));

        // ─── Título de ayuda Ctrl+B ───────────────────────────────────────
        let bottom_title = if app.ctrl_b_prefix {
            Line::from(vec![
                Span::styled(" Ctrl+B: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled("↑/↓", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" Agente | "),
                Span::styled("R", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" Renombrar | "),
                Span::styled("A", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" IA "),
            ])
        } else {
            Line::from("")
        };

        // ─── Terminal output ──────────────────────────────────────────────
        let terminal_text = if let Some(agente) = app.current_agent() {
            // FIX: cuando auto_scroll está activo calculamos el offset REAL
            // en cada render en lugar de guardar u16::MAX en el agente.
            // Esto garantiza que el Paragraph siempre muestre la última línea,
            // incluso si el panel cambia de tamaño o llega nuevo output.
            let display_scroll = if agente.auto_scroll {
                compute_max_scroll(&agente.terminal_output, center_chunks[0])
            } else {
                agente.scroll_offset
            };

            Paragraph::new(agente.terminal_output.as_str())
                .block(
                    Block::bordered()
                    .title(" Terminal ")
                    .title_bottom(bottom_title),
                )
                .wrap(Wrap { trim: false })
                .scroll((display_scroll, 0))
        } else {
            Paragraph::new(app.current_terminal_output())
                .block(Block::bordered().title(" Terminal "))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false })
        };

        f.render_widget(terminal_text, center_chunks[0]);

        // ─── Sub-layout inferior: input + botón ───────────────────────────
        let input_and_button = Layout::horizontal([
            Constraint::Min(10),
            Constraint::Length(12),
        ]).split(center_chunks[1]);

        app.send_button_rect.set(Some(input_and_button[1]));

        // ─── Widget de input con cursor ───────────────────────────────────
        let input_text = if let Some(agente) = app.current_agent() {
            let text = &agente.terminal_input;
            let cursor = agente.cursor_pos;
            let mut lines: Vec<Line> = Vec::new();
            let mut current_spans: Vec<Span> = Vec::new();
            let show_cursor = app.cursor_visible;

            for (byte_i, c) in text.char_indices() {
                if c == '\n' {
                    if byte_i == cursor && show_cursor {
                        current_spans.push(Span::styled(
                                " ",
                                Style::default().bg(Color::White).fg(Color::Black),
                        ));
                    }
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                } else if byte_i == cursor && show_cursor {
                    current_spans.push(Span::styled(
                            c.to_string(),
                            Style::default().bg(Color::White).fg(Color::Black),
                    ));
                } else {
                    current_spans.push(Span::raw(c.to_string()));
                }
            }

            if cursor >= text.len() && show_cursor {
                current_spans.push(Span::styled("█", Style::default().fg(Color::White)));
            }
            lines.push(Line::from(current_spans));

            Paragraph::new(Text::from(lines))
        } else {
            Paragraph::new("")
        };

        f.render_widget(
            input_text
            .block(Block::bordered().title(" Input "))
            .wrap(Wrap { trim: false }),
            input_and_button[0],
        );

        let send_button = Paragraph::new(" Enviar ")
            .block(Block::bordered().title(" Acción "))
            .alignment(Alignment::Center);
        f.render_widget(send_button, input_and_button[1]);
    }

    // ── PANEL DERECHO: IA (solo si está visible) ──────────────────────
    if app.show_ai_panel {
        let ai_content = if let Some(agente) = app.current_agent() {
            if agente.ai_loading {
                "Analizando con Gemini...".to_string()
            } else if agente.ai_response.is_empty() {
                "Presioná Ctrl+B → I para analizar la salida del agente.".to_string()
            } else {
                agente.ai_response.clone()
            }
        } else {
            "No hay agente seleccionado.".to_string()
        };

        let ai_paragraph = Paragraph::new(ai_content)
            .block(Block::bordered().title(" IA (Gemini) "))
            .wrap(Wrap { trim: false });

        f.render_widget(ai_paragraph, chunks[2]);
    }
    // ─────────────────────────────────────────────────────────────────────────
    // POPUP: renombrar agente
    // ─────────────────────────────────────────────────────────────────────────
    if app.renaming {
        let overlay = Block::default().style(Style::default().bg(Color::DarkGray));
        f.render_widget(overlay, f.area());

        let nombre_actual = app.current_agent()
            .map(|a| format!("{}_", a.nombre))
            .unwrap_or_default();

        let titulo = " Renombrar Agente (Enter: guardar, Esc: cancelar) ";

        let ancho_maximo = (f.area().width as f32 * 0.8) as u16;
        let ancho_titulo = titulo.len() as u16 + 4;
        let ancho_texto = nombre_actual.len() as u16 + 4;
        let ancho_popup = ancho_texto.max(ancho_titulo).min(ancho_maximo);

        let ancho_interno = ancho_popup.saturating_sub(2);
        let lineas = if ancho_interno > 0 {
            (nombre_actual.len() as u16 + ancho_interno - 1) / ancho_interno
        } else {
            1
        };
        let alto_popup = lineas + 2;

        let area = centered_rect_abs(ancho_popup, alto_popup, f.area());
        f.render_widget(Clear, area);

        let popup_message = Paragraph::new(nombre_actual)
            .block(
                Block::bordered()
                .title(titulo)
                .style(Style::default().bg(Color::Black).fg(Color::White)),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        f.render_widget(popup_message, area);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FUNCIONES AUXILIARES
// ─────────────────────────────────────────────────────────────────────────────

fn centered_rect_abs(width: u16, height: u16, r: Rect) -> Rect {
    let safe_w = width.min(r.width.saturating_sub(2)).max(12);
    let safe_h = height.min(r.height.saturating_sub(2)).max(3);

    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(safe_h),
        Constraint::Fill(1),
    ])
        .split(r);

    Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(safe_w),
        Constraint::Fill(1),
    ])
        .split(vertical[1])[1]
}

fn wrap_text(text: &str, width: usize) -> String {
    if width == 0 {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len() + text.len() / width);
    for (i, c) in text.chars().enumerate() {
        if i > 0 && i % width == 0 {
            result.push('\n');
        }
        result.push(c);
    }
    result
}
