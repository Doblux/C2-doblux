use std::cell::Cell;
use ratatui::layout::Rect;

use tokio::net::TcpStream;
use std::net::SocketAddr;
use crate::agente::Agente;
use std::time::{Duration, Instant};

pub struct App {
    pub agentes: Vec<Agente>,
    pub agente_seleccionado: usize,
    pub should_quit: bool,
    pub puerto: u16,
    pub banner: String,
    pub ctrl_b_prefix: bool,
    pub renaming: bool,
    pub cursor_visible: bool,
    pub last_blink: Instant,
    pub blink_interval: Duration,
    /// Rect del botón "Enviar", actualizado en cada frame por ui::draw.
    pub send_button_rect: Cell<Option<Rect>>,
    /// Rect del panel de output de la terminal, actualizado en cada frame por ui::draw.
    /// Permite que el event handler calcule compute_max_scroll sin necesitar acceso al Frame.
    pub output_rect: Cell<Option<Rect>>,
    pub show_ai_panel: bool,
}

impl App {
    pub fn new(puerto: u16) -> Self {
        let banner = format!(
            r#"
██████╗ ██████╗     ██████╗  ██████╗ ██████╗ ██╗     ██╗   ██╗██╗  ██╗
██╔════╝╚════██╗    ██╔══██╗██╔═══██╗██╔══██╗██║     ██║   ██║╚██╗██╔╝
██║      █████╔╝    ██║  ██║██║   ██║██████╔╝██║     ██║   ██║ ╚███╔╝ 
██║     ██╔═══╝     ██║  ██║██║   ██║██╔══██╗██║     ██║   ██║ ██╔██╗ 
╚██████╗███████╗    ██████╔╝╚██████╔╝██████╔╝███████╗╚██████╔╝██╔╝ ██╗
 ╚═════╝╚══════╝    ╚═════╝  ╚═════╝ ╚═════╝ ╚══════╝ ╚═════╝ ╚═╝  ╚═╝

    [*] ESTADO: ESCUCHANDO EN EL PUERTO {}
    [*] MÓDULO: C2 DOBLUX v0.1
    [*] ESPERANDO CONEXIONES DE AGENTES...
    "#, puerto);
        Self {
            agentes: vec![],
            agente_seleccionado: 0,
            should_quit: false,
            ctrl_b_prefix: false,
            renaming: false,
            puerto,
            banner,
            cursor_visible: true,
            last_blink: Instant::now(),
            blink_interval: Duration::from_millis(530),
            send_button_rect: Cell::new(None),
            output_rect: Cell::new(None),
            show_ai_panel: false,
        }
    }

    pub fn add_agent(&mut self, addr: SocketAddr, stream: tokio::net::tcp::OwnedWriteHalf) {
        let nuevo_agente = Agente {
            ip: addr.to_string(),
            nombre: format!("Agente-{}", addr),
            backup_nombre: None,
            stream,
            terminal_input: String::new(),
            cursor_pos: 0,
            terminal_output: String::from("[*] Conexión establecida con éxito.\n"),
            scroll_offset: 0,
            auto_scroll: true,
            ai_response: String::new(),
            ai_loading: false,
        };
        self.agentes.push(nuevo_agente);
    }

    pub fn remove_agent(&mut self, index: usize) {
        if index < self.agentes.len() {
            self.agentes.remove(index);
            if !self.agentes.is_empty() && self.agente_seleccionado >= self.agentes.len() {
                self.agente_seleccionado = self.agentes.len() - 1;
            } else if self.agentes.is_empty() {
                self.agente_seleccionado = 0;
            }
        }
    }

    pub fn current_agent_mut(&mut self) -> Option<&mut Agente> {
        self.agentes.get_mut(self.agente_seleccionado)
    }

    pub fn current_agent(&self) -> Option<&Agente> {
        self.agentes.get(self.agente_seleccionado)
    }

    pub fn current_terminal_output(&self) -> &str {
        if let Some(agente) = self.current_agent() {
            &agente.terminal_output
        } else {
            &self.banner
        }
    }

    pub fn current_terminal_input(&self) -> String {
        if let Some(agente) = self.current_agent() {
            agente.terminal_input.clone()
        } else {
            String::new()
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn update_cursor_blink(&mut self) {
        if self.last_blink.elapsed() >= self.blink_interval {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = Instant::now();
        }
    }

    pub fn force_cursor_visible(&mut self) {
        self.cursor_visible = true;
        self.last_blink = Instant::now();
    }
}
