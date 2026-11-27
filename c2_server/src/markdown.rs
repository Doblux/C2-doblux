use crate::theme::Theme;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

/// Convierte markdown a `Text` usando los colores de `Theme`.
/// El `Text` devuelto ya trae un `Span` por tramo: no hay que pisarlo
/// con `.style()` en el `Paragraph`.
pub fn render_markdown(input: &str, theme: &Theme) -> Text<'static> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(input, options);
    let mut renderer = Renderer::new(theme);
    for event in parser {
        renderer.handle(event);
    }
    renderer.finish()
}

struct ListFrame {
    ordered: bool,
    next: u64,
}

struct TableBuf {
    rows: Vec<Vec<String>>,
    header_rows: usize,
}

struct Renderer<'a> {
    theme: &'a Theme,
    lines: Vec<Line<'static>>,
    spans: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    heading_style: Option<Style>,
    lists: Vec<ListFrame>,
    quote_depth: usize,
    pending_marker: bool,
    in_code: bool,
    in_cell: bool,
    cell_buf: String,
    row: Vec<String>,
    table: Option<TableBuf>,
    skip_depth: u32,
    link_url: Vec<String>,
}

impl<'a> Renderer<'a> {
    fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            lines: Vec::new(),
            spans: Vec::new(),
            style_stack: Vec::new(),
            heading_style: None,
            lists: Vec::new(),
            quote_depth: 0,
            pending_marker: false,
            in_code: false,
            in_cell: false,
            cell_buf: String::new(),
            row: Vec::new(),
            table: None,
            skip_depth: 0,
            link_url: Vec::new(),
        }
    }

    fn handle(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.write_text(&text),
            Event::Code(code) => self.write_inline_code(&code),
            Event::InlineMath(text) | Event::DisplayMath(text) => self.write_inline_code(&text),
            Event::Html(html) | Event::InlineHtml(html) => self.write_html(&html),
            Event::FootnoteReference(name) => {
                let label = format!("[^{name}]");
                self.write_plain(&label, self.theme.help);
            }
            Event::SoftBreak => self.soft_break(),
            Event::HardBreak => self.hard_break(),
            Event::Rule => self.horizontal_rule(),
            Event::TaskListMarker(checked) => self.task_marker(checked),
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match &tag {
            Tag::FootnoteDefinition(_) | Tag::MetadataBlock(_) => {
                self.skip_depth += 1;
                return;
            }
            _ => {}
        }
        if self.skipping() {
            return;
        }

        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => self.start_heading(level),
            Tag::BlockQuote(_) => {
                self.flush_line();
                self.quote_depth += 1;
            }
            Tag::CodeBlock(kind) => self.start_code(kind),
            Tag::HtmlBlock => {}
            Tag::List(start) => {
                self.lists.push(ListFrame {
                    ordered: start.is_some(),
                    next: start.unwrap_or(1),
                });
            }
            Tag::Item => {
                self.flush_line();
                self.pending_marker = true;
            }
            Tag::FootnoteDefinition(_) | Tag::MetadataBlock(_) => {}
            Tag::DefinitionList | Tag::DefinitionListDefinition => {}
            Tag::DefinitionListTitle => {
                self.flush_line();
                self.style_stack.push(self.theme.title);
            }
            Tag::Table(_) => {
                self.flush_line();
                self.table = Some(TableBuf {
                    rows: Vec::new(),
                    header_rows: 0,
                });
            }
            Tag::TableHead => {}
            Tag::TableRow => {
                self.row.clear();
            }
            Tag::TableCell => {
                self.in_cell = true;
                self.cell_buf.clear();
            }
            Tag::Emphasis => self.push_modifier(Modifier::ITALIC),
            Tag::Strong => self.push_modifier(Modifier::BOLD),
            Tag::Strikethrough => self.push_modifier(Modifier::CROSSED_OUT),
            Tag::Superscript | Tag::Subscript => {}
            Tag::Link { dest_url, .. } => self.start_link(&dest_url),
            Tag::Image { dest_url, .. } => {
                self.write_plain("[img] ", self.theme.help);
                self.start_link(&dest_url);
            }
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match &tag {
            TagEnd::FootnoteDefinition | TagEnd::MetadataBlock(_) => {
                if self.skip_depth > 0 {
                    self.skip_depth -= 1;
                }
                return;
            }
            _ => {}
        }
        if self.skipping() {
            return;
        }

        match tag {
            TagEnd::Paragraph => self.end_paragraph(),
            TagEnd::Heading(_) => self.end_heading(),
            TagEnd::BlockQuote(_) => {
                self.flush_line();
                if self.quote_depth > 0 {
                    self.quote_depth -= 1;
                }
                if self.quote_depth == 0 && self.lists.is_empty() {
                    self.push_blank();
                }
            }
            TagEnd::CodeBlock => self.end_code(),
            TagEnd::HtmlBlock => {}
            TagEnd::List(_) => {
                self.flush_line();
                self.lists.pop();
                if self.lists.is_empty() && self.quote_depth == 0 {
                    self.push_blank();
                }
            }
            TagEnd::Item => self.close_item(),
            TagEnd::FootnoteDefinition | TagEnd::MetadataBlock(_) => {}
            TagEnd::DefinitionList | TagEnd::DefinitionListDefinition => {}
            TagEnd::DefinitionListTitle => {
                self.flush_line();
                self.style_stack.pop();
            }
            TagEnd::Table => self.end_table(),
            TagEnd::TableHead => {
                self.commit_row();
                if let Some(table) = self.table.as_mut() {
                    table.header_rows = table.rows.len();
                }
            }
            TagEnd::TableRow => self.commit_row(),
            TagEnd::TableCell => self.commit_cell(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.style_stack.pop();
            }
            TagEnd::Superscript | TagEnd::Subscript => {}
            TagEnd::Link | TagEnd::Image => self.end_link(),
        }
    }

    fn start_heading(&mut self, level: HeadingLevel) {
        self.flush_line();
        self.heading_style = Some(self.theme.title);
        let spaces = match level {
            HeadingLevel::H1 => 0,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 4,
            HeadingLevel::H4 => 6,
            HeadingLevel::H5 => 8,
            HeadingLevel::H6 => 10,
        };
        if spaces > 0 {
            let mut indent = String::new();
            let mut i = 0;
            while i < spaces {
                indent.push(' ');
                i += 1;
            }
            self.spans.push(Span::styled(indent, self.theme.title));
        }
    }

    fn end_heading(&mut self) {
        self.flush_line();
        self.heading_style = None;
        self.push_blank();
    }

    fn end_paragraph(&mut self) {
        if self.skipping() {
            self.spans.clear();
            return;
        }
        self.flush_line();
        if self.lists.is_empty() {
            self.push_blank();
        }
    }

    fn close_item(&mut self) {
        if self.pending_marker {
            self.emit_marker(None);
        }
        self.flush_line();
    }

    fn start_code(&mut self, kind: CodeBlockKind<'_>) {
        let mut lang = String::new();
        if let CodeBlockKind::Fenced(raw) = kind {
            for ch in raw.chars() {
                if ch.is_whitespace() {
                    break;
                }
                lang.push(ch);
            }
        }
        if self.in_cell {
            self.in_code = true;
            if !lang.is_empty() {
                self.cell_buf.push_str(&lang);
                self.cell_buf.push(' ');
            }
            return;
        }
        self.emit_marker(None);
        self.flush_line();
        self.in_code = true;
        let mut open = String::from("  ┌");
        if !lang.is_empty() {
            open.push(' ');
            open.push_str(&lang);
        }
        self.lines.push(Line::from(Span::styled(open, self.theme.help)));
    }

    fn end_code(&mut self) {
        if self.in_cell {
            self.in_code = false;
            return;
        }
        if !self.spans.is_empty() {
            self.flush_line();
        }
        self.lines.push(Line::from(Span::styled("  └", self.theme.help)));
        self.in_code = false;
        if self.lists.is_empty() {
            self.push_blank();
        }
    }

    fn start_link(&mut self, url: &str) {
        self.link_url.push(url.to_string());
        self.style_stack.push(self.theme.help_key);
    }

    fn end_link(&mut self) {
        self.style_stack.pop();
        let url = match self.link_url.pop() {
            Some(url) => url,
            None => return,
        };
        if self.skipping() {
            return;
        }
        if self.in_cell {
            self.cell_buf.push_str(" (");
            self.cell_buf.push_str(&url);
            self.cell_buf.push(')');
            return;
        }
        let same = match self.spans.last() {
            Some(span) => span.content.as_ref() == url,
            None => false,
        };
        if !same {
            self.spans.push(Span::styled(format!(" ({url})"), self.theme.help));
        }
    }

    fn task_marker(&mut self, checked: bool) {
        if self.skipping() {
            return;
        }
        if self.pending_marker {
            self.emit_marker(Some(checked));
            return;
        }
        let mark = if checked { "[x] " } else { "[ ] " };
        let style = if checked { self.theme.success } else { self.style() };
        self.write_plain(mark, style);
    }

    fn soft_break(&mut self) {
        if self.skipping() {
            return;
        }
        if self.in_code {
            self.write_code_chunk("\n");
            return;
        }
        if self.in_cell {
            self.cell_buf.push(' ');
            return;
        }
        self.ensure_prefix();
        self.spans.push(Span::styled(" ", self.style()));
    }

    fn hard_break(&mut self) {
        if self.skipping() {
            return;
        }
        if self.in_code {
            self.write_code_chunk("\n");
            return;
        }
        if self.in_cell {
            self.cell_buf.push(' ');
            return;
        }
        self.flush_line();
    }

    fn horizontal_rule(&mut self) {
        if self.skipping() {
            return;
        }
        self.flush_line();
        let mut line = self.continuation_prefix();
        line.push_str("────────");
        self.lines.push(Line::from(Span::styled(line, self.theme.help)));
    }

    fn write_text(&mut self, text: &str) {
        if self.skipping() || text.is_empty() {
            return;
        }
        if self.in_code {
            if self.in_cell {
                self.cell_buf.push_str(text);
            } else {
                self.write_code_chunk(text);
            }
            return;
        }
        if self.in_cell {
            self.cell_buf.push_str(text);
            return;
        }
        self.ensure_prefix();
        self.spans.push(Span::styled(text.to_string(), self.style()));
    }

    fn write_inline_code(&mut self, code: &str) {
        if self.skipping() {
            return;
        }
        let mut flat = String::new();
        for ch in code.chars() {
            if ch == '\n' || ch == '\r' {
                flat.push(' ');
            } else {
                flat.push(ch);
            }
        }
        if self.in_cell {
            self.cell_buf.push('`');
            self.cell_buf.push_str(&flat);
            self.cell_buf.push('`');
            return;
        }
        if self.in_code {
            self.write_code_chunk(&flat);
            return;
        }
        self.ensure_prefix();
        self.spans.push(Span::styled(format!("`{flat}`"), self.theme.help_key));
    }

    fn write_html(&mut self, html: &str) {
        if self.skipping() || html.is_empty() {
            return;
        }
        self.write_plain(html, self.theme.help);
    }

    fn write_plain(&mut self, text: &str, style: Style) {
        if self.skipping() || text.is_empty() {
            return;
        }
        if self.in_cell {
            self.cell_buf.push_str(text);
            return;
        }
        self.ensure_prefix();
        self.spans.push(Span::styled(text.to_string(), style));
    }

    fn write_code_chunk(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let mut start = 0;
        let mut i = 0;
        while i < bytes.len() {
            let is_cr = bytes[i] == b'\r';
            let is_lf = bytes[i] == b'\n';
            if is_cr || is_lf {
                self.write_code_piece(&text[start..i]);
                if self.spans.is_empty() {
                    self.spans.push(Span::styled("  │ ", self.theme.help));
                }
                self.flush_line();
                if is_cr && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    i += 1;
                }
                start = i + 1;
            }
            i += 1;
        }
        if start < text.len() {
            self.write_code_piece(&text[start..]);
        }
    }

    fn write_code_piece(&mut self, piece: &str) {
        if piece.is_empty() {
            return;
        }
        if self.spans.is_empty() {
            self.spans.push(Span::styled("  │ ", self.theme.help));
        }
        self.spans.push(Span::styled(piece.to_string(), self.theme.help_key));
    }

    fn commit_cell(&mut self) {
        self.in_cell = false;
        let text = self.cell_buf.trim().to_string();
        self.cell_buf.clear();
        self.row.push(text);
    }

    fn commit_row(&mut self) {
        if self.in_cell {
            self.commit_cell();
        }
        if self.row.is_empty() {
            return;
        }
        if let Some(table) = self.table.as_mut() {
            table.rows.push(std::mem::take(&mut self.row));
        } else {
            self.row.clear();
        }
    }

    fn end_table(&mut self) {
        self.commit_row();
        let table = match self.table.take() {
            Some(table) => table,
            None => return,
        };

        let mut widths: Vec<usize> = Vec::new();
        let mut r = 0;
        while r < table.rows.len() {
            let mut c = 0;
            while c < table.rows[r].len() {
                let width = table.rows[r][c].chars().count();
                if c >= widths.len() {
                    widths.push(width);
                } else if width > widths[c] {
                    widths[c] = width;
                }
                c += 1;
            }
            r += 1;
        }

        r = 0;
        while r < table.rows.len() {
            let header = r < table.header_rows;
            let style = if header { self.theme.title } else { self.theme.ai_text };
            self.lines.push(Line::from(Span::styled(
                join_cells(&table.rows[r], &widths, " │ "),
                style,
            )));
            if header {
                self.lines.push(Line::from(Span::styled(
                    table_separator(&widths),
                    self.theme.help,
                )));
            }
            r += 1;
        }
        self.push_blank();
    }

    fn emit_marker(&mut self, task: Option<bool>) {
        if !self.pending_marker {
            return;
        }
        self.pending_marker = false;

        let quote = self.quote_prefix();
        if !quote.is_empty() {
            self.spans.push(Span::styled(quote, self.theme.help));
        }

        let mut indent = String::new();
        let depth = self.lists.len();
        let mut i = 1;
        while i < depth {
            indent.push_str("  ");
            i += 1;
        }

        let ordered_n = match self.lists.last_mut() {
            Some(list) if list.ordered => {
                let n = list.next;
                list.next += 1;
                Some(n)
            }
            _ => None,
        };

        let mut marker = indent;
        if let Some(n) = ordered_n {
            marker.push_str(&format!("{n}. "));
        }
        if let Some(checked) = task {
            if checked {
                marker.push_str("[x] ");
            } else {
                marker.push_str("[ ] ");
            }
        } else if ordered_n.is_none() {
            marker.push_str("• ");
        }

        let style = if task == Some(true) {
            self.theme.success
        } else {
            self.theme.ai_text
        };
        self.spans.push(Span::styled(marker, style));
    }

    fn ensure_prefix(&mut self) {
        if self.pending_marker {
            self.emit_marker(None);
        }
        if !self.spans.is_empty() {
            return;
        }
        let prefix = self.continuation_prefix();
        if prefix.is_empty() {
            return;
        }
        let style = if self.quote_depth > 0 {
            self.theme.help
        } else {
            self.theme.ai_text
        };
        self.spans.push(Span::styled(prefix, style));
    }

    fn quote_prefix(&self) -> String {
        let mut prefix = String::new();
        let mut i = 0;
        while i < self.quote_depth {
            prefix.push_str("│ ");
            i += 1;
        }
        prefix
    }

    fn continuation_prefix(&self) -> String {
        let mut prefix = self.quote_prefix();
        let mut i = 0;
        while i < self.lists.len() {
            prefix.push_str("  ");
            i += 1;
        }
        prefix
    }

    fn push_modifier(&mut self, modifier: Modifier) {
        let style = self.style().add_modifier(modifier);
        self.style_stack.push(style);
    }

    fn style(&self) -> Style {
        if let Some(style) = self.style_stack.last() {
            return *style;
        }
        if let Some(style) = self.heading_style {
            return style;
        }
        self.theme.ai_text
    }

    fn skipping(&self) -> bool {
        self.skip_depth > 0
    }

    fn flush_line(&mut self) {
        if self.spans.is_empty() {
            return;
        }
        let line = Line::from(std::mem::take(&mut self.spans));
        self.lines.push(line);
    }

    fn push_blank(&mut self) {
        self.flush_line();
        if self.lines.is_empty() {
            return;
        }
        if let Some(last) = self.lines.last() {
            if last.spans.is_empty() {
                return;
            }
        }
        self.lines.push(Line::default());
    }

    fn finish(mut self) -> Text<'static> {
        if self.pending_marker {
            self.emit_marker(None);
        }
        self.flush_line();

        while self.lines.first().is_some_and(|line| line.spans.is_empty()) {
            self.lines.remove(0);
        }
        while self.lines.last().is_some_and(|line| line.spans.is_empty()) {
            self.lines.pop();
        }
        if self.lines.is_empty() {
            return Text::default();
        }
        Text::from(self.lines)
    }
}

fn join_cells(cells: &[String], widths: &[usize], sep: &str) -> String {
    let mut line = String::from(" ");
    let mut c = 0;
    while c < cells.len() {
        if c > 0 {
            line.push_str(sep);
        }
        line.push_str(&cells[c]);
        let width = if c < widths.len() { widths[c] } else { 0 };
        let len = cells[c].chars().count();
        let mut pad = len;
        while pad < width {
            line.push(' ');
            pad += 1;
        }
        c += 1;
    }
    line
}

fn table_separator(widths: &[usize]) -> String {
    let mut line = String::from(" ");
    let mut c = 0;
    while c < widths.len() {
        if c > 0 {
            line.push_str("─┼─");
        }
        let mut n = 0;
        let width = widths[c].max(1);
        while n < width {
            line.push('─');
            n += 1;
        }
        c += 1;
    }
    line
}