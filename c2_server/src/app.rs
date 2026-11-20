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
    /// Cell permite escritura a través de &App sin romper la firma de draw.  // ← NUEVO
    pub send_button_rect: Cell<Option<Rect>>,
}

impl App {
    pub fn new(puerto: u16) -> Self {
        // Inicializamos la App (arranca con 0 agentes para probar el estado de escucha)
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
            send_button_rect: Cell::new(None), // ← NUEVO
        }
    }

    pub fn add_agent(&mut self, addr: SocketAddr, stream: tokio::net::tcp::OwnedWriteHalf) {
        let nuevo_agente = Agente {
            ip: addr.to_string(),
            nombre: format!("Agente-{}", addr),
            backup_nombre: None,
            stream,
            terminal_input: String::new(), // ← antes: Vec::new()
            cursor_pos: 0,
            terminal_output: String::from("[*] Conexión establecida con éxito.\n"),
        };
        self.agentes.push(nuevo_agente);
    }

    pub fn remove_agent(&mut self, index: usize) {
        if index < self.agentes.len() {
            self.agentes.remove(index);

            // Si borramos el agente que teníamos seleccionado y la lista queda vacía o nos pasamos de largo,
            // ajustamos el índice de selección para que no dé un error de "Index Out of Bounds".
            if !self.agentes.is_empty() && self.agente_seleccionado >= self.agentes.len() {
                self.agente_seleccionado = self.agentes.len() - 1;
            } else if self.agentes.is_empty() {
                self.agente_seleccionado = 0;
            }
        }
    }


    // Devuelve una referencia al agente seleccionado actualmente de forma segura
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
            agente.terminal_input.clone() // ← antes: agente.terminal_input.iter().collect()
        } else {
            String::new()
        }
    }

    pub fn quit(&mut self){
        self.should_quit = true;
    }

    /// Llamar en cada frame / tick
    pub fn update_cursor_blink(&mut self) {
        if self.last_blink.elapsed() >= self.blink_interval {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = Instant::now();
        }
    }

    /// Llamar siempre que el usuario escriba o mueva el cursor
    pub fn force_cursor_visible(&mut self) {
        self.cursor_visible = true;
        self.last_blink = Instant::now();
    }
}
