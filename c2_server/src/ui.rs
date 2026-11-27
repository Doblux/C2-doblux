use ratatui::{prelude::*, widgets::*};
use crate::app::App;
use crate::markdown::render_markdown;
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
pub(crate) fn compute_max_scroll(text: &str, rect: Rect) -> u16 {
    let inner_width = rect.width.saturating_sub(2) as usize;
    let inner_height = rect.height.saturating_sub(2);
    if inner_width == 0 {
        return 0;
    }
    let total_lines: u16 = text
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                1u16
            } else {
                let n = line.chars().count();
                u16::try_from((n + inner_width - 1) / inner_width)
                    .unwrap_or(u16::MAX / 2)
            }
        })
    .fold(0u16, |acc, l| acc.saturating_add(l));
    total_lines.saturating_sub(inner_height)
}

// ─────────────────────────────────────────────────────────────────────────────
// SPINNER (animación de carga de la IA)
// ─────────────────────────────────────────────────────────────────────────────

/// Devuelve el frame de spinner correspondiente al instante actual.
/// Deriva el frame del tiempo de pared: rota ~8 fps sin estado extra en App.
fn spinner_char() -> &'static str {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as usize;
    FRAMES[(ms / 120) % FRAMES.len()]
}

// ─────────────────────────────────────────────────────────────────────────────
// DRAW
// ─────────────────────────────────────────────────────────────────────────────

/// Punto de entrada del render.
/// El `Theme` se recibe por referencia para aplicar estilos coherentes
/// en cada widget sin hardcodear colores en este módulo.
pub fn draw(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    app.send_button_rect.set(None);

    // ── Layout principal ──────────────────────────────────────────────────────
    let chunks = if app.show_ai_panel {
        Layout::horizontal([
            Constraint::Percentage(18),
            Constraint::Percentage(52),
            Constraint::Percentage(30),
        ])
            .split(f.area())
    } else {
        Layout::horizontal([
            Constraint::Percentage(20),
            Constraint::Percentage(80),
        ])
            .split(f.area())
    };

    // ── PANEL IZQUIERDO: lista de agentes ─────────────────────────────────────
    let max_width = chunks[0].width.saturating_sub(4) as usize;

    let items: Vec<ListItem> = app
        .agentes
        .iter()
        .map(|agente| ListItem::new(wrap_text(&agente.nombre, max_width)))
        .collect();

    let agentes_list = List::new(items)
        .block(
            Block::bordered()
            .border_style(theme.border)
            .title(Span::styled(" Agentes ", theme.title)),
        )
        .highlight_style(theme.highlight)
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    if !app.agentes.is_empty() {
        list_state.select(Some(app.agente_seleccionado));
    }
    f.render_stateful_widget(agentes_list, chunks[0], &mut list_state);

    // ── PANEL CENTRAL ─────────────────────────────────────────────────────────
    if app.agentes.is_empty() {
        // Sin agentes: banner de bienvenida
        f.render_widget(
            Paragraph::new(app.current_terminal_output())
            .block(
                Block::bordered()
                .border_style(theme.border)
                .title(Span::styled(" Terminal ", theme.title)),
            )
            .style(theme.banner)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
            chunks[1],
        );
    } else {
        // Altura dinámica del input (mínimo 3, máximo 10 filas)
        let input_lines = app
            .current_agent()
            .map(|a| a.terminal_input.chars().filter(|&c| c == '\n').count() + 1)
            .unwrap_or(1);
        let input_height = (input_lines as u16 + 2).clamp(3, 10);

        let center_chunks = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(input_height),
        ])
            .split(chunks[1]);

        // Guardamos el rect del panel de output para que el event-handler
        // pueda calcular compute_max_scroll sin acceso al Frame.
        app.output_rect.set(Some(center_chunks[0]));

        // ── Hint Ctrl+B en la barra inferior del bloque terminal ──────────────
        let bottom_title = if app.ctrl_b_prefix {
            Line::from(vec![
                Span::styled(" Ctrl+B: ",     theme.help_key),
                Span::styled("↑/↓",           theme.help_key),
                Span::styled(" Agente | ",    theme.help),
                Span::styled("R",             theme.help_key),
                Span::styled(" Renombrar | ", theme.help),
                Span::styled("I",             theme.help_key),
                Span::styled(" Analizar | ",  theme.help),
                Span::styled("A",             theme.help_key),
                Span::styled(" IA | ",        theme.help),
                Span::styled("T",             theme.help_key),
                Span::styled(format!(" Tema ({}) ", theme.name), theme.help),
            ])
        } else {
            Line::from("")
        };

        // ── Terminal output ───────────────────────────────────────────────────
        let terminal_widget = if let Some(agente) = app.current_agent() {
            let display_scroll = if agente.auto_scroll {
                compute_max_scroll(&agente.terminal_output, center_chunks[0])
            } else {
                agente.scroll_offset
            };
            let lines: Vec<Line> = agente
                .terminal_output
                .split('\n')
                .map(|line| {
                    if line.contains("Conexión establecida") || line.contains("[FIN BINARIO]") {
                        Line::from(Span::styled(line, theme.success))
                    } else {
                        Line::from(Span::styled(line, theme.output_text))
                    }
                })
            .collect();

            Paragraph::new(Text::from(lines))
                .block(
                    Block::bordered()
                    .border_style(theme.border)
                    .title(Span::styled(" Terminal ", theme.title))
                    .title_bottom(bottom_title),
                )
                .wrap(Wrap { trim: false })
                .scroll((display_scroll, 0))
        } else {
            Paragraph::new(app.current_terminal_output())
                .block(
                    Block::bordered()
                    .border_style(theme.border)
                    .title(Span::styled(" Terminal ", theme.title)),
                )
                .style(theme.banner)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false })
        };
        f.render_widget(terminal_widget, center_chunks[0]);

        // ── Sub-layout inferior: input + botón ────────────────────────────────
        let input_and_button = Layout::horizontal([
            Constraint::Min(10),
            Constraint::Length(12),
        ])
            .split(center_chunks[1]);

        app.send_button_rect.set(Some(input_and_button[1]));

        // ── Widget de input con cursor inline ─────────────────────────────────
        let input_widget = if let Some(agente) = app.current_agent() {
            let text   = &agente.terminal_input;
            let cursor = agente.cursor_pos;
            let show   = app.cursor_visible;

            let mut lines: Vec<Line> = Vec::new();
            let mut spans: Vec<Span> = Vec::new();

            for (byte_i, c) in text.char_indices() {
                if c == '\n' {
                    // Cursor sobre el salto de línea: bloque de un espacio
                    if byte_i == cursor && show {
                        spans.push(Span::styled(" ", theme.cursor));
                    }
                    lines.push(Line::from(std::mem::take(&mut spans)));
                } else if byte_i == cursor && show {
                    // Carácter bajo el cursor: fondo del tema de cursor
                    spans.push(Span::styled(c.to_string(), theme.cursor));
                } else {
                    // Carácter normal de input
                    spans.push(Span::styled(c.to_string(), theme.input_text));
                }
            }

            // Cursor más allá del último byte → bloque lleno al final.
            // Usamos highlight_symbol (azul claro) para que sea visible
            // sobre el fondo oscuro sin necesitar un campo extra en Theme.
            if cursor >= text.len() && show {
                spans.push(Span::styled("█", theme.highlight_symbol));
            }
            lines.push(Line::from(spans));

            Paragraph::new(Text::from(lines))
        } else {
            Paragraph::new("")
        };

        f.render_widget(
            input_widget
            .block(
                Block::bordered()
                .border_style(theme.border)
                .title(Span::styled(" Input ", theme.title)),
            )
            .wrap(Wrap { trim: false }),
            input_and_button[0],
        );

        // ── Botón Enviar ──────────────────────────────────────────────────────
        f.render_widget(
            Paragraph::new(Span::styled(" Enviar ", theme.button))
            .block(
                Block::bordered()
                .border_style(theme.border)
                .title(Span::styled(" Acción ", theme.title)),
            )
            .alignment(Alignment::Center),
            input_and_button[1],
        );
    }

    // ── PANEL DERECHO: IA ─────────────────────────────────────────────────────
    if app.show_ai_panel {
        let ai_loading = app.current_agent().map(|a| a.ai_loading).unwrap_or(false);

        let (title_style, border_style) = if ai_loading {
            (theme.ai_loading, theme.ai_loading)
        } else {
            (theme.title, theme.border)
        };

        let ai_title = if ai_loading {
            format!(" IA {} ", spinner_char())
        } else {
            " IA ".to_string()
        };

        let ai_block = Block::bordered()
            .border_style(border_style)
            .title(Span::styled(ai_title, title_style));

        let ai_widget = if let Some(agente) = app.current_agent() {
            if agente.ai_loading {
                let preview = if agente.ai_response.is_empty() {
                    String::new()
                } else {
                    format!("\n\n── Análisis anterior ──\n{}", agente.ai_response)
                };
                Paragraph::new(format!(
                        "{} Analizando salida del agente...{}",
                        spinner_char(),
                        preview
                ))
                    .block(ai_block)
                    .style(theme.ai_loading)
                    .wrap(Wrap { trim: false })
            } else if agente.ai_response.is_empty() {
                Paragraph::new(
                    " Sin análisis.\n\n Presioná Ctrl+B → I para analizar la salida del agente seleccionado.",
                )
                    .block(ai_block)
                    .style(theme.help)
                    .wrap(Wrap { trim: false })
            } else if agente.ai_response.starts_with("[Error IA]") {
                Paragraph::new(agente.ai_response.clone())
                    .block(ai_block)
                    .style(theme.error)
                    .wrap(Wrap { trim: false })
            } else {
                // Sin .style() aquí: render_markdown ya estiliza cada Span.
                Paragraph::new(render_markdown(&agente.ai_response, theme))
                    .block(ai_block)
                    .wrap(Wrap { trim: false })
            }
        } else {
            Paragraph::new(" Sin agente seleccionado.")
                .block(ai_block)
                .style(theme.help)
                .wrap(Wrap { trim: false })
        };

        f.render_widget(ai_widget, chunks[2]);
    }

    // ── POPUP: renombrar agente ───────────────────────────────────────────────
    if app.renaming {
        // Overlay que oscurece el fondo (efecto de modal estándar en TUI)
        f.render_widget(
            Block::default().style(Style::default().bg(Color::DarkGray)),
            f.area(),
        );

        let nombre_actual = app
            .current_agent()
            .map(|a| format!("{}_", a.nombre))
            .unwrap_or_default();

        let titulo      = " Renombrar Agente (Enter: guardar, Esc: cancelar) ";
        let ancho_max   = (f.area().width as f32 * 0.8) as u16;
        let ancho_titulo = titulo.len() as u16 + 4;
        let ancho_texto  = nombre_actual.len() as u16 + 4;
        let ancho_popup  = ancho_texto.max(ancho_titulo).min(ancho_max);

        let ancho_interno = ancho_popup.saturating_sub(2);
        let lineas = if ancho_interno > 0 {
            (nombre_actual.len() as u16 + ancho_interno - 1) / ancho_interno
        } else {
            1
        };
        let alto_popup = lineas + 2;

        let area = centered_rect_abs(ancho_popup, alto_popup, f.area());
        f.render_widget(Clear, area); // limpia el overlay antes del popup

        f.render_widget(
            Paragraph::new(nombre_actual)
            .block(
                Block::bordered()
                // Borde y título del tema; fondo oscuro propio del popup
                .border_style(theme.border)
                .title(Span::styled(titulo, theme.title))
                .style(Style::default().bg(Color::Rgb(10, 12, 20))),
            )
            // Texto del nombre en color input_text del tema
            .style(theme.input_text)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
            area,
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FUNCIONES AUXILIARES
// ─────────────────────────────────────────────────────────────────────────────

/// Rectángulo centrado con dimensiones exactas (en caracteres).
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

/// Inserta `\n` cada `width` chars para forzar wrap en la lista de agentes.
/// Necesario porque `List` de ratatui no hace wrap automático de items largos.
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
