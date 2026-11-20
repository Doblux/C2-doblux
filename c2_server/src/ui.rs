use ratatui::{layout::HorizontalAlignment::Center, prelude::*, widgets::*};
use crate::app::App; // Importamos el App

// La función recibe el Frame y el App
pub fn draw(f: &mut Frame, app: &App) {
    app.send_button_rect.set(None);

    let chunks = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(50),
        Constraint::Percentage(25)
    ]).split(f.area());

    // --- LÓGICA DEL PANEL DE AGENTES (Izquierda) ---
    // Calculamos el ancho útil del panel restando los bordes (2) y el símbolo "> " (2)
    let max_width = chunks[0].width.saturating_sub(4) as usize;

    // Mapeamos la lista de agentes
    let items: Vec<ListItem> = app.agentes
        .iter()
        .map(|agente| {
            // Envolvemos el nombre si es muy largo
            let wrapped_name = wrap_text(&agente.nombre, max_width);
            ListItem::new(wrapped_name)
        })
    .collect();

    let agentes_list = List::new(items)
        .block(Block::bordered().title(" Agentes "))
        .highlight_style(
            Style::default()
            .bg(Color::DarkGray) // Color de fondo cuando está seleccionado
            .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> "); // Símbolo indicador a la izquierda del elemento activo

    // Creamos el estado y le pasamos el índice actual de tu App envuelto en Some(_)
    let mut state = ListState::default();
    if !app.agentes.is_empty() {
        state.select(Some(app.agente_seleccionado));
    }

    // 3. Renderizamos el widget con estado en chunks[0]
    f.render_stateful_widget(agentes_list, chunks[0], &mut state);
    // ======================================================================================================================
    // ======================================================================================================================
    // ======================================================================================================================
    // --- LÓGICA DEL PANEL CENTRAL ---
    if app.agentes.is_empty() {
        // No hay agentes. Mostramos el ASCII art en todo el panel.
        let welcome_text = Paragraph::new(app.current_terminal_output())
            .block(Block::bordered().title(" Terminal "))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });
        f.render_widget(welcome_text, chunks[1]);
    } else {
        // --- CAMBIO 1: Calcular la altura dinámica del input según los '\n' ---
        let input_lines = app.current_agent()
            .map(|a| a.terminal_input.chars().filter(|&c| c == '\n').count() + 1)
            .unwrap_or(1);

        // El alto será el número de líneas + 2 (por los bordes superior e inferior). 
        // Usamos .clamp(3, 10) para que mida al menos 3 y como máximo 10 líneas para no tapar la terminal.
        let input_height = (input_lines as u16 + 2).clamp(3, 10);

        let center_chunks = Layout::vertical([
            Constraint::Min(0),             // Output (arriba - se achica si el input crece)
            Constraint::Length(input_height), // Input dinámico (abajo)
        ]).split(chunks[1]);

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

        let terminal_text = Paragraph::new(app.current_terminal_output())
            .block(
                Block::bordered()
                .title(" Terminal ")
                .title_bottom(bottom_title) // <--- ACÁ AGREGAMOS EL TEXTO
            )
            .wrap(Wrap { trim: false });
        f.render_widget(terminal_text, center_chunks[0]);

        // --- SUB-LAYOUT INFERIOR: Input (izq) y Botón Enviar (der) ---
        let input_and_button = Layout::horizontal([
            Constraint::Min(10),     
            Constraint::Length(12),  
        ]).split(center_chunks[1]);

        // guardamos el rect del botón para detectar clics en main.rs
        app.send_button_rect.set(Some(input_and_button[1]));

        let input_text = if let Some(agente) = app.current_agent() {
            let text = &agente.terminal_input;
            let cursor = agente.cursor_pos; // byte index
            let mut lines: Vec<Line> = Vec::new();
            let mut current_spans: Vec<Span> = Vec::new();
            let show_cursor = app.cursor_visible;

            // char_indices() devuelve (byte_offset, char).
            // La comparación byte_i == cursor es segura: cursor siempre cae en una frontera,
            // y byte_i también lo hace por definición de char_indices().
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

            // Cursor más allá del último byte → bloque al final
            if cursor >= text.len() && show_cursor {
                current_spans.push(Span::styled("█", Style::default().fg(Color::White)));
            }
            lines.push(Line::from(current_spans));

            Paragraph::new(Text::from(lines))
        } else {
            Paragraph::new("")
        };

        // Dibujamos el input
        f.render_widget(
            input_text.block(Block::bordered().title(" Input ")).wrap(Wrap { trim: false }),
            input_and_button[0],
        );

        let send_button = Paragraph::new(" Enviar ")
            .block(Block::bordered().title(" Acción "))
            .alignment(Alignment::Center);
        f.render_widget(send_button, input_and_button[1]);
    }
    // ======================================================================================================================
    // ======================================================================================================================
    // ======================================================================================================================
    f.render_widget(Block::bordered().title(" IA "), chunks[2]);

    // ======================================================================================================================
    // ======================================================================================================================
    // ======================================================================================================================
    // POPUP RENOMBRADO
    if app.renaming {
        // Capa de oscurecimiento
        let overlay = Block::default().style(Style::default().bg(Color::DarkGray));
        f.render_widget(overlay, f.area());

        // Obtenemos el nombre actual
        let nombre_actual = app.current_agent()
            .map(|a| format!("{}_", a.nombre))
            .unwrap_or_default();

        let titulo = " Renombrar Agente (Enter: guardar, Esc: cancelar) ";

        // Cálculo dinámico del ancho
        // El ancho mínimo es el largo del título + 4 (bordes), el máximo es el 80% de la pantalla
        let ancho_maximo = (f.area().width as f32 * 0.8) as u16;
        let ancho_titulo = titulo.len() as u16 + 4;
        let ancho_texto = nombre_actual.len() as u16 + 4;

        let ancho_popup = ancho_texto.max(ancho_titulo).min(ancho_maximo);

        // Cálculo dinámico de la altura
        // Calculamos cuántas líneas ocupará el texto según el ancho disponible
        let ancho_interno = ancho_popup.saturating_sub(2); // Le restamos los bordes
        let lineas = if ancho_interno > 0 {
            (nombre_actual.len() as u16 + ancho_interno - 1) / ancho_interno
        } else {
            1
        };
        let alto_popup = lineas + 2; // Líneas de texto + 2 bordes

        // 5. Calculamos el área y dibujamos
        let area = centered_rect_abs(ancho_popup, alto_popup, f.area());
        f.render_widget(Clear, area);

        let popup_message = Paragraph::new(nombre_actual)
            .block(
                Block::bordered()
                .title(titulo)
                .style(Style::default().bg(Color::Black).fg(Color::White))
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }); // <-- CLAVE: Permite que el texto baje de línea

        f.render_widget(popup_message, area);
    }
}


// Función para calcular un rectángulo centrado con medidas exactas (ancho y alto)
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

// Función auxiliar para cortar textos largos (incluso sin espacios) en varias líneas
fn wrap_text(text: &str, width: usize) -> String {
    if width == 0 {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len() + text.len() / width);
    for (i, c) in text.chars().enumerate() {
        if i > 0 && i % width == 0 {
            result.push('\n'); // Insertamos el salto de línea
        }
        result.push(c);
    }
    result
}
