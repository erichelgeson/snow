mod commands;
mod graphics;

use crate::renderer::PageBuffer;

/// Graphics density modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsDensity {
    /// 72 DPI single density
    Single72,
    /// 80 DPI
    Dpi80,
    /// 96 DPI
    Dpi96,
    /// 107 DPI
    Dpi107,
    /// 120 DPI
    Dpi120,
    /// 136 DPI
    Dpi136,
    /// 144 DPI double density
    Double144,
    /// 160 DPI
    Dpi160,
}

impl GraphicsDensity {
    /// Get the horizontal DPI for this density mode
    pub fn horizontal_dpi(self) -> f32 {
        match self {
            Self::Single72 => 72.0,
            Self::Dpi80 => 80.0,
            Self::Dpi96 => 96.0,
            Self::Dpi107 => 107.0,
            Self::Dpi120 => 120.0,
            Self::Dpi136 => 136.0,
            Self::Double144 => 144.0,
            Self::Dpi160 => 160.0,
        }
    }
}

/// Parser state machine states
#[derive(Debug, Clone, Default)]
pub enum ParserState {
    /// Normal text processing
    #[default]
    Normal,
    /// ESC character seen, waiting for command byte
    EscapeSeen,
    /// Collecting parameters for a command
    CollectingParams {
        cmd: u8,
        params: Vec<u8>,
        needed: usize,
    },
    /// In graphics mode, collecting bitmap data
    GraphicsMode {
        density: GraphicsDensity,
        bytes_remaining: usize,
        /// Starting X position in inches (to avoid accumulation error)
        start_x: f32,
        /// Current column index (0-based)
        column: usize,
    },
}

/// Cursor position on the page
#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    /// X position in inches from left edge
    pub x: f32,
    /// Y position in inches from top edge
    pub y: f32,
}

/// ImageWriter printer parser and state
#[allow(dead_code)]
pub struct ImageWriterParser {
    state: ParserState,
    page: PageBuffer,
    cursor: Position,

    /// Characters per inch (10, 12, 15, 17)
    cpi: u8,
    /// Line spacing in inches
    line_spacing: f32,
    /// Bold mode
    bold: bool,
    /// Underline mode
    underline: bool,
    /// Italic mode (ImageWriter LQ)
    italic: bool,
    /// Superscript mode
    superscript: bool,
    /// Subscript mode
    subscript: bool,
    /// Bidirectional printing
    bidirectional: bool,
    /// Half-height mode (condensed)
    half_height: bool,
    /// Double-strike mode
    double_strike: bool,
    /// Proportional spacing
    proportional: bool,

    /// Left margin in inches
    left_margin: f32,
    /// Right margin in inches
    right_margin: f32,
    /// Page width in inches
    page_width: f32,
    /// Page height in inches
    page_height: f32,
    /// Top of form position
    top_of_form: f32,
    /// Skip-over perforation lines
    skip_perf: u8,

    /// Current color (0 = black, 1-3 = colors for color ribbon)
    color: u8,
    /// Graphics density for graphics mode
    graphics_density: GraphicsDensity,

    /// Tab stops
    tabs: Vec<f32>,

    /// Completed pages waiting to be output
    completed_pages: Vec<PageBuffer>,
}

impl Default for ImageWriterParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageWriterParser {
    /// Create a new parser with default settings
    pub fn new() -> Self {
        let page_width = 8.5;
        let page_height = 11.0;

        Self {
            state: ParserState::Normal,
            page: PageBuffer::new(page_width, page_height),
            cursor: Position { x: 0.0, y: 0.0 },
            cpi: 10,
            line_spacing: 1.0 / 6.0, // 1/6 inch default (6 LPI)
            bold: false,
            underline: false,
            italic: false,
            superscript: false,
            subscript: false,
            bidirectional: true,
            half_height: false,
            double_strike: false,
            proportional: false,
            left_margin: 0.0,
            right_margin: page_width,
            page_width,
            page_height,
            top_of_form: 0.0,
            skip_perf: 0,
            color: 0,
            graphics_density: GraphicsDensity::Single72,
            tabs: Self::default_tabs(),
            completed_pages: Vec::new(),
        }
    }

    /// Default tab stops every 0.5 inches
    fn default_tabs() -> Vec<f32> {
        (1..=16).map(|i| i as f32 * 0.5).collect()
    }

    /// Process a byte of input
    pub fn process_byte(&mut self, byte: u8) {
        log::trace!(
            "Byte: 0x{:02X} ({}) state={:?}",
            byte,
            if byte.is_ascii_graphic() || byte == b' ' {
                byte as char
            } else {
                '.'
            },
            std::mem::discriminant(&self.state)
        );
        match &self.state {
            ParserState::Normal => self.process_normal(byte),
            ParserState::EscapeSeen => self.process_escape_cmd(byte),
            ParserState::CollectingParams {
                cmd,
                params,
                needed,
            } => {
                let cmd = *cmd;
                let needed = *needed;
                let mut params = params.clone();
                params.push(byte);
                if params.len() >= needed {
                    self.execute_command(cmd, &params);
                    // Don't override GraphicsMode if the command set it
                    if !matches!(self.state, ParserState::GraphicsMode { .. }) {
                        self.state = ParserState::Normal;
                    }
                } else {
                    self.state = ParserState::CollectingParams {
                        cmd,
                        params,
                        needed,
                    };
                }
            }
            ParserState::GraphicsMode {
                density,
                bytes_remaining,
                start_x,
                column,
            } => {
                let density = *density;
                let remaining = bytes_remaining.saturating_sub(1);
                let start_x = *start_x;
                let column = *column;
                self.process_graphics_byte(byte, density, start_x, column);
                if remaining == 0 {
                    // Update cursor.x to final position after graphics row
                    let dot_width = 1.0 / density.horizontal_dpi();
                    let final_x = start_x + ((column + 1) as f32 * dot_width);

                    // Debug: show final column position
                    use crate::renderer::OUTPUT_DPI;
                    let last_px = (start_x + (column as f32 * dot_width)) * OUTPUT_DPI as f32;
                    log::debug!(
                        "Graphics row done: {} cols, start_x={:.6}, last_col_x={:.6}, last_px={:.2}",
                        column + 1,
                        start_x,
                        start_x + (column as f32 * dot_width),
                        last_px
                    );

                    self.cursor.x = final_x;
                    self.state = ParserState::Normal;
                } else {
                    self.state = ParserState::GraphicsMode {
                        density,
                        bytes_remaining: remaining,
                        start_x,
                        column: column + 1,
                    };
                }
            }
        }
    }

    /// Process a byte in normal mode
    fn process_normal(&mut self, byte: u8) {
        match byte {
            0x00 => {} // NUL - ignore
            0x07 => {} // BEL - ignore (would beep)
            0x08 => self.backspace(),
            0x09 => self.horizontal_tab(),
            0x0A => self.line_feed(),
            0x0B => self.vertical_tab(),
            0x0C => self.form_feed(),
            0x0D => self.carriage_return(),
            0x0E => self.half_height = false, // Cancel half-height
            0x0F => self.half_height = true,  // Half-height mode
            0x11 => {}                        // DC1 - Select printer
            0x12 => {}                        // DC2 - 10 CPI (Pica)
            0x13 => {}                        // DC3 - Deselect printer
            0x14 => self.cpi = 17,            // DC4 - 17 CPI (Condensed)
            0x18 => self.cancel_line(),       // CAN - Cancel line
            0x1B => self.state = ParserState::EscapeSeen,
            0x20..=0x7E => self.print_char(byte),
            0x80..=0xFF => self.print_char(byte), // High ASCII/extended chars
            _ => {}                               // Ignore other control codes
        }
    }

    /// Process escape sequence command byte
    fn process_escape_cmd(&mut self, cmd: u8) {
        self.state = ParserState::Normal;
        log::debug!(
            "ESC sequence: 0x{:02X} ('{}')",
            cmd,
            if cmd.is_ascii_graphic() {
                cmd as char
            } else {
                '.'
            }
        );

        match cmd {
            // Commands with no parameters
            b'<' => self.bidirectional = false, // Unidirectional
            b'>' => self.bidirectional = true,  // Bidirectional
            b'!' => self.bold = true,           // Bold on
            b'"' => self.bold = false,          // Bold off
            b'X' => self.underline = true,      // Underline on
            b'Y' => self.underline = false,     // Underline off
            b'x' => self.superscript = true,    // Superscript on
            b'y' => self.superscript = false,   // Superscript off
            b'w' => self.subscript = true,      // Subscript on
            b'v' => self.subscript = false,     // Subscript off
            b'A' => self.line_spacing = 1.0 / 6.0, // 1/6" line spacing
            b'B' => self.line_spacing = 1.0 / 8.0, // 1/8" line spacing
            b'N' => self.cpi = 10,              // Pica (10 CPI)
            b'E' => self.cpi = 12,              // Elite (12 CPI)
            b'q' => self.cpi = 15,              // Semi-condensed (15 CPI)
            b'Q' => self.cpi = 17,              // Condensed (17 CPI)
            b'p' => self.proportional = true,   // Proportional on
            b'P' => self.proportional = false,  // Proportional off
            b'0' => self.tabs = Vec::new(),     // Clear tabs
            b'1' => {}                          // Set tab at current position
            b'(' => {}                          // 8-bit mode
            b')' => {}                          // 7-bit mode
            b'9' => self.double_strike = true,  // Double-strike on
            // Note: ESC H with params = page length, handled below
            b'a' => self.italics_on(),  // Italics on (LQ)
            b'b' => self.italics_off(), // Italics off (LQ)
            b'c' => self.reset(),       // Reset printer
            b'?' => {}                  // Printer status request (ignore)
            b'r' => {}                  // Reverse line feed mode (no params, ignore for now)

            // ESC o - One-line proportional (4 ASCII digit parameter)
            b'o' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 4,
                }
            }

            // Commands with ASCII digit parameters (nnn format)
            b'L' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 3,
                }
            } // Left margin
            b'M' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 3,
                }
            } // Right margin
            b'l' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 3,
                }
            } // Absolute horizontal position

            // Commands with 2-digit ASCII parameters
            b'T' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 2,
                }
            } // Line spacing n/144"
            b'H' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 4,
                }
            } // Page length in 1/144"
            b'f' => {} // Forward paper feed mode (no params)

            // Commands with 1-byte binary parameter
            b'K' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 1,
                }
            } // Color select
            b'n' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 1,
                }
            } // Immediate print character

            // ESC F - Set horizontal position (4 ASCII digit parameter)
            b'F' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 4,
                }
            }

            // Graphics mode commands
            b'G' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 4,
                }
            } // Graphics mode
            b'V' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 4,
                }
            } // 144 DPI graphics
            b'g' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 4,
                }
            } // Single density graphics (LQ)
            b's' => {
                self.state = ParserState::CollectingParams {
                    cmd,
                    params: Vec::new(),
                    needed: 5,
                }
            } // Extended graphics (LQ)

            _ => {
                log::debug!("Unknown ESC command: 0x{:02X} ('{}')", cmd, cmd as char);
            }
        }
    }

    /// Execute a command with collected parameters
    fn execute_command(&mut self, cmd: u8, params: &[u8]) {
        match cmd {
            b'L' => self.set_left_margin(params),
            b'M' => self.set_right_margin(params),
            b'l' => self.set_horizontal_position(params),
            b'F' => self.set_horizontal_position_720(params),
            b'T' => self.set_line_spacing_144(params),
            b'K' => self.set_color(params),
            b'n' => self.print_immediate(params),
            b'G' => self.enter_graphics_72(params),
            b'V' => self.enter_graphics_144(params),
            b'g' => self.enter_graphics_single(params),
            b's' => self.enter_graphics_extended(params),
            b'H' => self.set_page_length(params),
            b'o' => self.set_one_line_proportional(params),
            _ => {}
        }
    }

    /// Set page length in 1/144 inch (ESC H nnnn)
    fn set_page_length(&mut self, params: &[u8]) {
        if let Some(units) = Self::parse_ascii_digits(params) {
            // Page length in 1/144 inch units
            // If 0, use continuous paper (very long page)
            if units == 0 {
                self.page_height = 100.0; // Effectively continuous
            } else {
                self.page_height = units as f32 / 144.0;
            }
            log::debug!(
                "Set page length: {} /144\" = {:.2}\"",
                units,
                self.page_height
            );
        }
    }

    /// Print a character at the current position
    fn print_char(&mut self, ch: u8) {
        let char_width = 1.0 / self.cpi as f32;

        // Check for right margin wrap
        if self.cursor.x + char_width > self.right_margin {
            self.carriage_return();
            self.line_feed();
        }

        let char_info = CharacterInfo {
            ch,
            bold: self.bold,
            underline: self.underline,
            italic: self.italic,
            superscript: self.superscript,
            subscript: self.subscript,
            half_height: self.half_height,
            double_strike: self.double_strike,
            cpi: self.cpi,
            color: self.color,
        };

        self.page.draw_character(self.cursor, &char_info);
        self.cursor.x += char_width;
    }

    /// Backspace
    fn backspace(&mut self) {
        let char_width = 1.0 / self.cpi as f32;
        self.cursor.x = (self.cursor.x - char_width).max(self.left_margin);
    }

    /// Horizontal tab
    fn horizontal_tab(&mut self) {
        for &tab in &self.tabs {
            if tab > self.cursor.x {
                self.cursor.x = tab.min(self.right_margin);
                return;
            }
        }
    }

    /// Vertical tab (same as line feed on ImageWriter)
    fn vertical_tab(&mut self) {
        self.line_feed();
    }

    /// Line feed
    fn line_feed(&mut self) {
        self.cursor.y += self.line_spacing;
        log::debug!(
            "Line feed: cursor.y={:.3}\" line_spacing={:.4}\"",
            self.cursor.y,
            self.line_spacing
        );

        // Check for page overflow
        if self.cursor.y >= self.page_height - self.top_of_form {
            log::debug!(
                "Page overflow: cursor.y={:.3}\" >= page_height={:.1}\" - ejecting",
                self.cursor.y,
                self.page_height
            );
            self.eject_page();
        }
    }

    /// Form feed (eject page)
    fn form_feed(&mut self) {
        log::debug!("Form feed (0x0C) received - ejecting page");
        self.eject_page();
    }

    /// Carriage return
    fn carriage_return(&mut self) {
        log::debug!(
            "Carriage return: x {} -> {}",
            self.cursor.x,
            self.left_margin
        );
        self.cursor.x = self.left_margin;
    }

    /// Cancel the current line (backspace to left margin)
    fn cancel_line(&mut self) {
        self.cursor.x = self.left_margin;
    }

    /// Set left margin from ASCII digits
    fn set_left_margin(&mut self, params: &[u8]) {
        if let Some(cols) = Self::parse_ascii_digits(params) {
            // Column numbers are in 10 CPI, so 10 cols = 1 inch
            self.left_margin = cols as f32 / 10.0;
            if self.cursor.x < self.left_margin {
                self.cursor.x = self.left_margin;
            }
        }
    }

    /// Set right margin from ASCII digits
    fn set_right_margin(&mut self, params: &[u8]) {
        if let Some(cols) = Self::parse_ascii_digits(params) {
            self.right_margin = cols as f32 / 10.0;
        }
    }

    /// Set absolute horizontal position (ESC l - 3 ASCII digits, columns at 10 CPI)
    fn set_horizontal_position(&mut self, params: &[u8]) {
        if let Some(cols) = Self::parse_ascii_digits(params) {
            self.cursor.x = self.left_margin + cols as f32 / 10.0;
            log::debug!("ESC l: cursor.x = {} cols = {:.4}\"", cols, self.cursor.x);
        }
    }

    /// Set absolute horizontal position (ESC F - 4 ASCII digits, 1/720 inch units)
    fn set_horizontal_position_720(&mut self, params: &[u8]) {
        if let Some(units) = Self::parse_ascii_digits(params) {
            // ESC F: Horizontal position in 1/720 inch units (fine positioning)
            self.cursor.x = units as f32 / 720.0;
            log::debug!("ESC F: cursor.x = {}/720\" = {:.4}\"", units, self.cursor.x);
        }
    }

    /// Set one-line proportional spacing (ESC o - 4 ASCII digits)
    /// This command enables proportional spacing for the current line only
    fn set_one_line_proportional(&mut self, params: &[u8]) {
        if let Some(value) = Self::parse_ascii_digits(params) {
            // Parameter meaning varies - just log and consume for now
            log::debug!("ESC o: one-line proportional = {}", value);
        }
    }

    /// Set line spacing in n/144 inch
    fn set_line_spacing_144(&mut self, params: &[u8]) {
        if let Some(n) = Self::parse_ascii_digits(params) {
            self.line_spacing = n as f32 / 144.0;
            log::debug!(
                "ESC T: line_spacing = {}/144\" = {:.4}\"",
                n,
                self.line_spacing
            );
        }
    }

    /// Set color (0=black, 1=yellow, 2=red, 3=blue)
    fn set_color(&mut self, params: &[u8]) {
        if let Some(&c) = params.first() {
            self.color = c.min(3);
        }
    }

    /// Print a character immediately without advancing
    fn print_immediate(&mut self, params: &[u8]) {
        if let Some(&ch) = params.first() {
            let saved_x = self.cursor.x;
            self.print_char(ch);
            self.cursor.x = saved_x;
        }
    }

    /// Enter 72 DPI graphics mode
    fn enter_graphics_72(&mut self, params: &[u8]) {
        if let Some(count) = Self::parse_graphics_count(params) {
            // Round start_x to nearest pixel to eliminate accumulated float error
            // from any text/positioning that happened before graphics mode
            let start_x = Self::round_to_pixel(self.cursor.x);
            log::debug!(
                "Entering graphics mode: {} bytes at cursor ({:.3}\", {:.3}\") start_x={:.6}",
                count,
                self.cursor.x,
                self.cursor.y,
                start_x
            );
            self.state = ParserState::GraphicsMode {
                density: GraphicsDensity::Single72,
                bytes_remaining: count,
                start_x,
                column: 0,
            };
        }
    }

    /// Round an inch value to the nearest output pixel boundary
    fn round_to_pixel(inches: f32) -> f32 {
        use crate::renderer::OUTPUT_DPI;
        (inches * OUTPUT_DPI as f32).round() / OUTPUT_DPI as f32
    }

    /// Enter 144 DPI graphics mode
    fn enter_graphics_144(&mut self, params: &[u8]) {
        if let Some(count) = Self::parse_graphics_count(params) {
            self.state = ParserState::GraphicsMode {
                density: GraphicsDensity::Double144,
                bytes_remaining: count,
                start_x: Self::round_to_pixel(self.cursor.x),
                column: 0,
            };
        }
    }

    /// Enter single density graphics (LQ)
    fn enter_graphics_single(&mut self, params: &[u8]) {
        if let Some(count) = Self::parse_graphics_count(params) {
            self.state = ParserState::GraphicsMode {
                density: GraphicsDensity::Single72,
                bytes_remaining: count,
                start_x: Self::round_to_pixel(self.cursor.x),
                column: 0,
            };
        }
    }

    /// Enter extended graphics mode (LQ) with density selection
    fn enter_graphics_extended(&mut self, params: &[u8]) {
        if params.len() >= 5 {
            let density = match params[0] {
                0 => GraphicsDensity::Single72,
                1 => GraphicsDensity::Dpi80,
                2 => GraphicsDensity::Dpi96,
                3 => GraphicsDensity::Dpi107,
                4 => GraphicsDensity::Dpi120,
                5 => GraphicsDensity::Dpi136,
                6 => GraphicsDensity::Double144,
                7 => GraphicsDensity::Dpi160,
                _ => GraphicsDensity::Single72,
            };
            if let Some(count) = Self::parse_graphics_count(&params[1..]) {
                self.state = ParserState::GraphicsMode {
                    density,
                    bytes_remaining: count,
                    start_x: Self::round_to_pixel(self.cursor.x),
                    column: 0,
                };
            }
        }
    }

    /// Parse ASCII digit parameters (e.g., "080" -> 80)
    fn parse_ascii_digits(params: &[u8]) -> Option<u32> {
        let s: String = params.iter().map(|&b| b as char).collect();
        s.parse().ok()
    }

    /// Parse graphics count from 4 ASCII digits (e.g., "0252" = 252 bytes)
    fn parse_graphics_count(params: &[u8]) -> Option<usize> {
        if params.len() >= 4 {
            // ImageWriter II uses 4 ASCII digits for byte count
            let s: String = params.iter().take(4).map(|&b| b as char).collect();
            let count = s.parse::<usize>().ok()?;
            log::debug!("Graphics mode: {} bytes", count);
            Some(count)
        } else {
            None
        }
    }

    /// Turn italics on (ImageWriter LQ)
    fn italics_on(&mut self) {
        self.italic = true;
    }

    /// Turn italics off (ImageWriter LQ)
    fn italics_off(&mut self) {
        self.italic = false;
    }

    /// Reset printer to defaults
    fn reset(&mut self) {
        *self = Self::new();
    }

    /// Eject the current page and start a new one
    fn eject_page(&mut self) {
        let old_page = std::mem::replace(
            &mut self.page,
            PageBuffer::new(self.page_width, self.page_height),
        );
        self.completed_pages.push(old_page);
        self.cursor = Position {
            x: self.left_margin,
            y: 0.0,
        };
        log::info!(
            "Page ejected, total completed pages: {}",
            self.completed_pages.len()
        );
    }

    /// Take completed pages
    pub fn take_completed_pages(&mut self) -> Vec<PageBuffer> {
        std::mem::take(&mut self.completed_pages)
    }

    /// Force eject current page (for final output)
    pub fn force_eject(&mut self) {
        if !self.page.is_empty() {
            self.eject_page();
        }
    }

    /// Get current page (for preview/debugging)
    #[allow(dead_code)]
    pub fn current_page(&self) -> &PageBuffer {
        &self.page
    }
}

/// Character rendering information
#[derive(Debug, Clone)]
pub struct CharacterInfo {
    pub ch: u8,
    pub bold: bool,
    pub underline: bool,
    pub italic: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub half_height: bool,
    pub double_strike: bool,
    pub cpi: u8,
    pub color: u8,
}
