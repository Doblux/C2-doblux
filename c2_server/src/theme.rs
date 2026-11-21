use ratatui::style::{Color, Modifier, Style};

#[derive(Clone)] // <-- Necesario para poder ciclar temas
pub struct Theme {
    pub name: &'static str,
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
            name: "Dark",
            border: Style::default().fg(Color::Rgb(80, 80, 100)),
            title: Style::default().fg(Color::Rgb(140, 180, 255)).add_modifier(Modifier::BOLD),
            highlight: Style::default().bg(Color::Rgb(40, 50, 80)).fg(Color::White).add_modifier(Modifier::BOLD),
            highlight_symbol: Style::default().fg(Color::Rgb(100, 200, 255)),
            cursor: Style::default().bg(Color::Rgb(180, 220, 255)).fg(Color::Black),
            input_text: Style::default().fg(Color::Rgb(220, 220, 230)),
            output_text: Style::default().fg(Color::Rgb(200, 210, 200)),
            ai_text: Style::default().fg(Color::Rgb(200, 190, 255)),
            ai_loading: Style::default().fg(Color::Rgb(255, 200, 100)).add_modifier(Modifier::ITALIC),
            help: Style::default().fg(Color::Rgb(150, 150, 170)),
            help_key: Style::default().fg(Color::Rgb(100, 220, 160)).add_modifier(Modifier::BOLD),
            error: Style::default().fg(Color::Rgb(255, 120, 120)),
            success: Style::default().fg(Color::Rgb(120, 220, 150)),
            button: Style::default().fg(Color::Rgb(100, 200, 255)).add_modifier(Modifier::BOLD),
            banner: Style::default().fg(Color::Rgb(100, 180, 255)),
        }
    }

    pub fn dracula() -> Self {
        Self {
            name: "Dracula",
            border: Style::default().fg(Color::Rgb(98, 114, 164)),
            title: Style::default().fg(Color::Rgb(189, 147, 249)).add_modifier(Modifier::BOLD), // purple
            highlight: Style::default().bg(Color::Rgb(68, 71, 90)).fg(Color::Rgb(248, 248, 242)),
            highlight_symbol: Style::default().fg(Color::Rgb(80, 250, 123)), // green
            cursor: Style::default().bg(Color::Rgb(255, 121, 198)).fg(Color::Black), // pink
            input_text: Style::default().fg(Color::Rgb(248, 248, 242)),
            output_text: Style::default().fg(Color::Rgb(139, 233, 253)), // cyan
            ai_text: Style::default().fg(Color::Rgb(241, 250, 140)), // yellow
            ai_loading: Style::default().fg(Color::Rgb(255, 184, 108)).add_modifier(Modifier::ITALIC), // orange
            help: Style::default().fg(Color::Rgb(98, 114, 164)),
            help_key: Style::default().fg(Color::Rgb(189, 147, 249)).add_modifier(Modifier::BOLD),
            error: Style::default().fg(Color::Rgb(255, 85, 85)),
            success: Style::default().fg(Color::Rgb(80, 250, 123)),
            button: Style::default().fg(Color::Rgb(255, 121, 198)).add_modifier(Modifier::BOLD),
            banner: Style::default().fg(Color::Rgb(189, 147, 249)),
        }
    }

    pub fn nord() -> Self {
        Self {
            name: "Nord",
            border: Style::default().fg(Color::Rgb(76, 86, 106)),
            title: Style::default().fg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD), // frost blue
            highlight: Style::default().bg(Color::Rgb(67, 76, 94)).fg(Color::Rgb(236, 239, 244)),
            highlight_symbol: Style::default().fg(Color::Rgb(163, 190, 140)), // green
            cursor: Style::default().bg(Color::Rgb(235, 203, 139)).fg(Color::Black), // yellow
            input_text: Style::default().fg(Color::Rgb(236, 239, 244)),
            output_text: Style::default().fg(Color::Rgb(208, 135, 112)), // orange
            ai_text: Style::default().fg(Color::Rgb(180, 142, 173)), // purple
            ai_loading: Style::default().fg(Color::Rgb(235, 203, 139)).add_modifier(Modifier::ITALIC),
            help: Style::default().fg(Color::Rgb(76, 86, 106)),
            help_key: Style::default().fg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD),
            error: Style::default().fg(Color::Rgb(191, 97, 106)), // red
            success: Style::default().fg(Color::Rgb(163, 190, 140)),
            button: Style::default().fg(Color::Rgb(136, 192, 208)).add_modifier(Modifier::BOLD),
            banner: Style::default().fg(Color::Rgb(136, 192, 208)),
        }
    }

    pub fn gruvbox() -> Self {
        Self {
            name: "Gruvbox",
            border: Style::default().fg(Color::Rgb(80, 73, 69)), // bg3
            title: Style::default().fg(Color::Rgb(250, 189, 47)).add_modifier(Modifier::BOLD), // yellow
            highlight: Style::default().bg(Color::Rgb(60, 56, 54)).fg(Color::Rgb(235, 219, 178)),
            highlight_symbol: Style::default().fg(Color::Rgb(184, 187, 38)), // green
            cursor: Style::default().bg(Color::Rgb(251, 73, 52)).fg(Color::Black), // red
            input_text: Style::default().fg(Color::Rgb(235, 219, 178)),
            output_text: Style::default().fg(Color::Rgb(131, 165, 152)), // aqua
            ai_text: Style::default().fg(Color::Rgb(211, 134, 155)), // purple
            ai_loading: Style::default().fg(Color::Rgb(254, 128, 25)).add_modifier(Modifier::ITALIC), // orange
            help: Style::default().fg(Color::Rgb(146, 131, 116)),
            help_key: Style::default().fg(Color::Rgb(250, 189, 47)).add_modifier(Modifier::BOLD),
            error: Style::default().fg(Color::Rgb(251, 73, 52)),
            success: Style::default().fg(Color::Rgb(184, 187, 38)),
            button: Style::default().fg(Color::Rgb(131, 165, 152)).add_modifier(Modifier::BOLD),
            banner: Style::default().fg(Color::Rgb(250, 189, 47)),
        }
    }

    pub fn light() -> Self {
        Self {
            name: "Light",
            border: Style::default().fg(Color::Rgb(180, 180, 180)),
            title: Style::default().fg(Color::Rgb(40, 40, 40)).add_modifier(Modifier::BOLD),
            highlight: Style::default().bg(Color::Rgb(220, 220, 220)).fg(Color::Rgb(0, 0, 0)),
            highlight_symbol: Style::default().fg(Color::Rgb(30, 100, 200)),
            cursor: Style::default().bg(Color::Rgb(30, 100, 200)).fg(Color::White),
            input_text: Style::default().fg(Color::Rgb(30, 30, 30)),
            output_text: Style::default().fg(Color::Rgb(40, 60, 40)),
            ai_text: Style::default().fg(Color::Rgb(80, 40, 120)),
            ai_loading: Style::default().fg(Color::Rgb(200, 120, 0)).add_modifier(Modifier::ITALIC),
            help: Style::default().fg(Color::Rgb(100, 100, 100)),
            help_key: Style::default().fg(Color::Rgb(30, 100, 200)).add_modifier(Modifier::BOLD),
            error: Style::default().fg(Color::Rgb(200, 30, 30)),
            success: Style::default().fg(Color::Rgb(30, 120, 30)),
            button: Style::default().fg(Color::Rgb(30, 100, 200)).add_modifier(Modifier::BOLD),
            banner: Style::default().fg(Color::Rgb(30, 100, 200)),
        }
    }

    /// Devuelve una lista con todos los temas disponibles.
    pub fn all() -> Vec<Theme> {
        vec![
            Self::dark(),
            Self::dracula(),
            Self::nord(),
            Self::gruvbox(),
            Self::light(),
        ]
    }
}