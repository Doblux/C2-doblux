use tokio::io::AsyncWriteExt;
use std::collections::VecDeque;

pub struct Agente {
    pub ip: String,
    pub stream: tokio::net::tcp::OwnedWriteHalf,
    pub nombre: String,
    pub backup_nombre: Option<String>,
    pub terminal_output: String,
    pub terminal_input: String,
    pub cursor_pos: usize,
    pub scroll_offset: u16,
    pub auto_scroll: bool,
    pub ai_response: String,
    pub ai_loading: bool,
    pub binary_mode: bool,
    pub history: VecDeque<String>,
    pub history_index: usize,
}

impl Agente {
    /// Inserta `c` en la posición del cursor y avanza cursor_pos por los bytes que ocupa el char.
    pub fn insert_char(&mut self, c: char) {
        self.terminal_input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn history_up(&mut self) -> Option<String> {
        if self.history.is_empty() {
            return None;
        }
        // Nos movemos hacia atrás (hacia comandos más viejos)
        // Solo restamos si no estamos ya en el primer comando (índice 0)
        if self.history_index > 0 {
            self.history_index -= 1;
        }
        self.history.get(self.history_index).cloned()
    }

    pub fn history_down(&mut self) -> Option<String> {
        if self.history.is_empty() {
            return None;
        }
        // Nos movemos hacia adelante (hacia comandos más nuevos o el prompt vacío)
        // Solo sumamos si no estamos ya en el prompt vacío (índice == len)
        if self.history_index < self.history.len() {
            self.history_index += 1;
        }
        
        // Si llegamos al final, significa que estamos en la "línea en blanco" actual
        if self.history_index == self.history.len() {
            Some(String::new())
        } else {
            self.history.get(self.history_index).cloned()
        }
    }

    /// Elimina el carácter anterior al cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev_len = self.terminal_input[..self.cursor_pos]
                .chars()
                .next_back()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev_len;
            self.terminal_input.remove(self.cursor_pos);
        }
    }

    pub fn supr_char(&mut self) {
        if self.cursor_pos < self.terminal_input.len() {
            self.terminal_input.remove(self.cursor_pos);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            let prev_len = self.terminal_input[..self.cursor_pos]
                .chars()
                .next_back()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev_len;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_pos < self.terminal_input.len() {
            let next_len = self.terminal_input[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos += next_len;
        }
    }

    pub fn move_up(&mut self) {
        let current_start = self.line_start_at(self.cursor_pos);
        if current_start == 0 {
            return;
        }
        let column = self.terminal_input[current_start..self.cursor_pos]
            .chars()
            .count();
        let prev_newline_byte = current_start - 1;
        let prev_start = self.line_start_at(prev_newline_byte);
        let prev_len_chars = self.terminal_input[prev_start..prev_newline_byte]
            .chars()
            .count();
        let target_col = column.min(prev_len_chars);
        let byte_offset = self.terminal_input[prev_start..]
            .char_indices()
            .nth(target_col)
            .map(|(i, _)| i)
            .unwrap_or(prev_newline_byte - prev_start);
        self.cursor_pos = prev_start + byte_offset;
    }

    pub fn move_down(&mut self) {
        let current_start = self.line_start_at(self.cursor_pos);
        let current_end = self.line_end_at(current_start);
        if current_end >= self.terminal_input.len() {
            return;
        }
        let column = self.terminal_input[current_start..self.cursor_pos]
            .chars()
            .count();
        let next_start = current_end + 1;
        let next_end = self.line_end_at(next_start);
        let next_len_chars = self.terminal_input[next_start..next_end]
            .chars()
            .count();
        let target_col = column.min(next_len_chars);
        let byte_offset = self.terminal_input[next_start..]
            .char_indices()
            .nth(target_col)
            .map(|(i, _)| i)
            .unwrap_or(next_end - next_start);
        self.cursor_pos = next_start + byte_offset;
    }

    pub fn move_home(&mut self) {
        self.cursor_pos = self.line_start_at(self.cursor_pos);
    }

    pub fn move_end(&mut self) {
        let current_start = self.line_start_at(self.cursor_pos);
        self.cursor_pos = self.line_end_at(current_start);
    }

    pub fn add_char_to_name(&mut self, c: char) {
        self.nombre.push(c);
    }

    pub fn del_char_to_name(&mut self) {
        self.nombre.pop();
    }

    pub fn validate_or_restore_name(&mut self) {
        if self.nombre.trim().is_empty() {
            if let Some(respaldo) = &self.backup_nombre {
                self.nombre = respaldo.clone();
            }
        }
        self.backup_nombre = None;
    }

    pub fn backupear_nombre(&mut self) {
        self.backup_nombre = Some(self.nombre.clone());
    }

    pub fn move_word_left(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let mut pos = self.cursor_pos;
        while pos > 0 {
            let ch = self.terminal_input[..pos].chars().next_back().unwrap();
            if !ch.is_whitespace() { break; }
            pos -= ch.len_utf8();
        }
        while pos > 0 {
            let ch = self.terminal_input[..pos].chars().next_back().unwrap();
            if ch.is_whitespace() { break; }
            pos -= ch.len_utf8();
        }
        self.cursor_pos = pos;
    }

    pub fn move_word_right(&mut self) {
        let len = self.terminal_input.len();
        if self.cursor_pos >= len {
            return;
        }
        let mut pos = self.cursor_pos;
        while pos < len {
            let ch = self.terminal_input[pos..].chars().next().unwrap();
            if ch.is_whitespace() { break; }
            pos += ch.len_utf8();
        }
        while pos < len {
            let ch = self.terminal_input[pos..].chars().next().unwrap();
            if !ch.is_whitespace() { break; }
            pos += ch.len_utf8();
        }
        self.cursor_pos = pos;
    }

    pub fn delete_word_forward(&mut self) {
        let len = self.terminal_input.len();
        if self.cursor_pos >= len {
            return;
        }
        let mut end_pos = self.cursor_pos;
        while end_pos < len {
            let ch = self.terminal_input[end_pos..].chars().next().unwrap();
            if !ch.is_whitespace() { break; }
            end_pos += ch.len_utf8();
        }
        while end_pos < len {
            let ch = self.terminal_input[end_pos..].chars().next().unwrap();
            if ch.is_whitespace() { break; }
            end_pos += ch.len_utf8();
        }
        if end_pos > self.cursor_pos {
            self.terminal_input.drain(self.cursor_pos..end_pos);
        }
    }

    pub async fn send_input(&mut self) -> tokio::io::Result<()> {
        if self.terminal_input.is_empty() {
            return Ok(());
        }

        let cmd = self.terminal_input.trim().to_string();
        if matches!(cmd.as_str(), "clear" | "cls") {
            self.terminal_output.clear();
            self.scroll_offset = 0;
            self.auto_scroll   = true;
            self.binary_mode   = false;
            self.terminal_input.clear();
            self.cursor_pos = 0;
            self.guardar_en_historial(cmd);
            return Ok(());
        }

        let mut payload = self.terminal_input.clone();
        if !payload.ends_with('\n') {
            payload.push('\n');
        }

        self.stream.write_all(payload.as_bytes()).await?;

        let texto_enviado = self.terminal_input.trim_end_matches('\n').to_string();
        self.terminal_output.push_str(&format!("[>] {}\n", texto_enviado));

        self.guardar_en_historial(texto_enviado);

        self.terminal_input.clear();
        self.cursor_pos = 0;
        self.auto_scroll = true;
        self.binary_mode = false;
        Ok(())
    }

    pub fn guardar_en_historial(&mut self, texto_enviado: String){
        const MAX_HISTORY: usize = 100;

        if !texto_enviado.is_empty() {
            self.history.push_back(texto_enviado); // O(1)

            // Si nos pasamos del límite, sacamos el más viejo del frente en O(1)
            // arreglo circular
            if self.history.len() > MAX_HISTORY {
                self.history.pop_front(); // O(1) (magia del VecDeque)
            }
            // Reseteamos el índice al final para el próximo comando
            self.history_index = self.history.len();
        }
    }

    pub fn append_terminal_output(&mut self, data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }

        if Self::looks_binary(data) {
            if !self.binary_mode {
                self.binary_mode = true;
                self.terminal_output.push_str(
                    "\n[!] <BINARY> Stream binario detectado. Contenido omitido.\n\
                     [!] <BINARY> Si esperabas texto, revisá tu comando (ej: no hagas `cat` de un binario).\n",
                );
            }
            // Mientras dure el stream binario, no agregamos nada más.
            return true;
        }

        // Volvimos a texto plano: si estábamos en binario, avisamos y salimos.
        if self.binary_mode {
            self.binary_mode = false;
            self.terminal_output.push_str("\n[!] <FIN BINARIO> Volviendo a salida de texto.\n");
        }

        let texto = Self::bytes_to_terminal_text(data);
        if texto.is_empty() {
            return false;
        }
        self.terminal_output.push_str(&texto);
        true
    }


    // PRIVADO =====================================================================

    fn looks_binary(data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }
        if data.contains(&0u8) {
            return true;
        }
        let non_printable = data
            .iter()
            .filter(|&&b| b < 8 || (b > 13 && b < 32) || b == 127)
            .count();
        let ratio = non_printable as f32 / data.len() as f32;
        data.len() >= 16 && ratio > 0.25
    }

    /// Procesa bytes de texto: descarta ANSI/OSC, procesa `\b` y filtra `\r`.
    fn bytes_to_terminal_text(data: &[u8]) -> String {
        let lossy = String::from_utf8_lossy(data);
        let mut out = String::with_capacity(lossy.len());
        let mut chars = lossy.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '\n' | '\t' => out.push(c),
                // NUEVO: descartamos '\r'. Un Paragraph de ratatui no puede
                // sobreescribir una línea, así que dejarlo ensuciaría la salida
                // con basura de las barras de progreso (curl, wget, etc.).
                '\r' => { /* descartado */ }
                '\u{0008}' => {
                    if let Some(last) = out.chars().next_back() {
                        if last != '\n' && last != '\r' {
                            out.pop();
                        }
                    }
                }
                '\u{1b}' => {
                    match chars.peek().copied() {
                        Some('[') => {
                            chars.next();
                            while let Some(&n) = chars.peek() {
                                chars.next();
                                if n.is_ascii_alphabetic() || n == '~' {
                                    break;
                                }
                            }
                        }
                        Some(']') => {
                            chars.next();
                            while let Some(n) = chars.next() {
                                if n == '\u{0007}' {
                                    break;
                                }
                                if n == '\u{1b}' && chars.peek() == Some(&'\\') {
                                    chars.next();
                                    break;
                                }
                            }
                        }
                        Some(_) => { chars.next(); }
                        None => {}
                    }
                }
                c if c.is_control() => out.push(' '),
                c => out.push(c),
            }
        }
        out
    }


    fn line_start_at(&self, index: usize) -> usize {
        let clamped = index.min(self.terminal_input.len());
        match self.terminal_input[..clamped].rfind('\n') {
            Some(pos) => pos + 1,
            None => 0,
        }
    }

    fn line_end_at(&self, line_start: usize) -> usize {
        match self.terminal_input[line_start..].find('\n') {
            Some(pos) => line_start + pos,
            None => self.terminal_input.len(),
        }
    }
}
