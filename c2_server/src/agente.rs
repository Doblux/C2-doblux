use tokio::io::AsyncWriteExt;

pub struct Agente {
    pub ip: String,
    pub stream: tokio::net::tcp::OwnedWriteHalf,
    pub nombre: String,
    pub backup_nombre: Option<String>,
    pub terminal_output: String,
    pub terminal_input: String,
    pub cursor_pos: usize,
}

impl Agente {
    /// Inserta `c` en la posición del cursor y avanza cursor_pos por los bytes que ocupa el char.
    pub fn insert_char(&mut self, c: char) {
        self.terminal_input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Elimina el carácter anterior al cursor (Backspace).
    /// Retrocedemos primero la cantidad de bytes que ocupa ese char y luego removemos.
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

        // Ya estamos en la primera línea → nada que hacer
        if current_start == 0 {
            return;
        }

        // Columna visual = número de chars desde el inicio de la línea actual hasta el cursor
        let column = self.terminal_input[current_start..self.cursor_pos]
            .chars()
            .count();

        // El byte del '\n' que separa ambas líneas
        let prev_newline_byte = current_start - 1; // seguro: current_start > 0
        let prev_start = self.line_start_at(prev_newline_byte);

        // Longitud en chars de la línea anterior (sin contar el '\n')
        let prev_len_chars = self.terminal_input[prev_start..prev_newline_byte]
            .chars()
            .count();

        // Si la columna actual excede el largo de la línea destino, nos quedamos al final de ella
        let target_col = column.min(prev_len_chars);

        // Convertimos la columna char a byte offset dentro de la línea anterior.
        // char_indices sobre [prev_start..] devuelve offsets relativos a prev_start.
        // Si target_col == prev_len_chars aterrizamos sobre el '\n', que es el
        // "fin visual" de esa línea (igual que en la implementación original con Vec<char>).
        let byte_offset = self.terminal_input[prev_start..]
            .char_indices()
            .nth(target_col)
            .map(|(i, _)| i)
            .unwrap_or(prev_newline_byte - prev_start); // fallback defensivo

        self.cursor_pos = prev_start + byte_offset;
    }

    pub fn move_down(&mut self) {
        let current_start = self.line_start_at(self.cursor_pos);
        let current_end = self.line_end_at(current_start);

        // Ya estamos en la última línea → nada que hacer
        if current_end >= self.terminal_input.len() {
            return;
        }

        let column = self.terminal_input[current_start..self.cursor_pos]
            .chars()
            .count();

        let next_start = current_end + 1; // byte después del '\n'; seguro: '\n' es 1 byte
        let next_end = self.line_end_at(next_start);

        let next_len_chars = self.terminal_input[next_start..next_end]
            .chars()
            .count();

        let target_col = column.min(next_len_chars);

        let byte_offset = self.terminal_input[next_start..]
            .char_indices()
            .nth(target_col)
            .map(|(i, _)| i)
            .unwrap_or(next_end - next_start); // fallback defensivo

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

        // Fase 1: retroceder sobre blancos (gap antes de la palabra anterior)
        while pos > 0 {
            // Seguro: pos > 0 garantiza Some; pos es frontera de char por invariante del campo
            let ch = self.terminal_input[..pos].chars().next_back().unwrap();
            if !ch.is_whitespace() { break; }
            pos -= ch.len_utf8();
        }

        // Fase 2: retroceder sobre la palabra hasta su inicio
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

        // Fase 1: saltar la palabra actual (no-blancos)
        while pos < len {
            // Seguro: pos < len garantiza Some; pos es frontera de char por invariante del campo
            let ch = self.terminal_input[pos..].chars().next().unwrap();
            if ch.is_whitespace() { break; }
            pos += ch.len_utf8();
        }

        // Fase 2: saltar el blanco hasta el inicio de la siguiente palabra
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

        // Si estamos sobre espacios, primero avanzamos hasta el inicio de la palabra más cercana
        while end_pos < len {
            // Seguro: end_pos < len garantiza Some; end_pos es frontera de char por invariante del campo
            let ch = self.terminal_input[end_pos..].chars().next().unwrap();
            if !ch.is_whitespace() {
                break;
            }
            end_pos += ch.len_utf8();
        }

        // Avanzamos hasta el final de la palabra
        while end_pos < len {
            let ch = self.terminal_input[end_pos..].chars().next().unwrap();
            if ch.is_whitespace() {
                break;
            }
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

        // Garantizamos '\n' final para compatibilidad con netcat
        let mut payload = self.terminal_input.clone();
        if !payload.ends_with('\n') {
            payload.push('\n');
        }

        // Envío real por el socket
        self.stream.write_all(payload.as_bytes()).await?;

        // Log en la salida: mostramos lo enviado (sin el '\n' extra que agregamos nosotros)
        let texto_enviado = self.terminal_input.trim_end_matches('\n');
        self.terminal_output.push_str(&format!("[>] {texto_enviado}\n"));

        // Reset del input
        self.terminal_input.clear();
        self.cursor_pos = 0;

        Ok(())
    }

    // PRIVADO =====================================================================

    /// Retorna el byte-index del primer char de la línea que contiene `index`.
    /// Resultado: 0  si es la primera línea,  o  pos_del_newline + 1  si no.
    /// Siempre es una frontera de char válida: '\n' es ASCII (1 byte) → pos+1 es seguro.
    fn line_start_at(&self, index: usize) -> usize {
        let clamped = index.min(self.terminal_input.len());
        match self.terminal_input[..clamped].rfind('\n') {
            Some(pos) => pos + 1,
            None => 0,
        }
    }

    /// Retorna el byte-index del '\n' que termina la línea, o len() si es la última.
    /// Siempre es una frontera de char válida por las mismas razones.
    fn line_end_at(&self, line_start: usize) -> usize {
        match self.terminal_input[line_start..].find('\n') {
            Some(pos) => line_start + pos,
            None => self.terminal_input.len(),
        }
    }
}
