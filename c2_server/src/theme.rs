use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub border: Style,
    pub title: Style,
    pub highlight: Style,
    pub highlight_symbol: Style,
    pub cursor: Style,
    pub input_text: Style,
    pub output_text: Style,
    pub ai_text: Style,
    pub ai_loading: Style,
    pub help: Style,
    pub help_key: Style,
    pub error: Style,
    pub success: Style,
    pub button: Style,
    pub banner: Style,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            border: Style::default().fg(Color::Rgb(80, 80, 100)),
            title: Style::default()
                .fg(Color::Rgb(140, 180, 255))
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .bg(Color::Rgb(40, 50, 80))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            highlight_symbol: Style::default().fg(Color::Rgb(100, 200, 255)),
            cursor: Style::default()
                .bg(Color::Rgb(180, 220, 255))
                .fg(Color::Black),
            input_text: Style::default().fg(Color::Rgb(220, 220, 230)),
            output_text: Style::default().fg(Color::Rgb(200, 210, 200)),
            ai_text: Style::default().fg(Color::Rgb(200, 190, 255)),
            ai_loading: Style::default()
                .fg(Color::Rgb(255, 200, 100))
                .add_modifier(Modifier::ITALIC),
            help: Style::default().fg(Color::Rgb(150, 150, 170)),
            help_key: Style::default()
                .fg(Color::Rgb(100, 220, 160))
                .add_modifier(Modifier::BOLD),
            error: Style::default().fg(Color::Rgb(255, 120, 120)),
            success: Style::default().fg(Color::Rgb(120, 220, 150)),
            button: Style::default()
                .fg(Color::Rgb(100, 200, 255))
                .add_modifier(Modifier::BOLD),
            banner: Style::default().fg(Color::Rgb(100, 180, 255)),
        }
    }
}