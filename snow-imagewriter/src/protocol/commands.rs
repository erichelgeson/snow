//! ESC sequence command handling utilities
//!
//! These constants are provided for reference and future use.

#![allow(dead_code)]

/// ImageWriter control codes
pub mod control_codes {
    pub const NUL: u8 = 0x00;
    pub const BEL: u8 = 0x07;
    pub const BS: u8 = 0x08;
    pub const HT: u8 = 0x09;
    pub const LF: u8 = 0x0A;
    pub const VT: u8 = 0x0B;
    pub const FF: u8 = 0x0C;
    pub const CR: u8 = 0x0D;
    pub const SO: u8 = 0x0E;
    pub const SI: u8 = 0x0F;
    pub const DC1: u8 = 0x11;
    pub const DC2: u8 = 0x12;
    pub const DC3: u8 = 0x13;
    pub const DC4: u8 = 0x14;
    pub const CAN: u8 = 0x18;
    pub const ESC: u8 = 0x1B;
}

/// ESC sequence command bytes
pub mod esc_commands {
    // Direction control
    pub const UNIDIRECTIONAL: u8 = b'<';
    pub const BIDIRECTIONAL: u8 = b'>';

    // Text style
    pub const BOLD_ON: u8 = b'!';
    pub const BOLD_OFF: u8 = b'"';
    pub const UNDERLINE_ON: u8 = b'X';
    pub const UNDERLINE_OFF: u8 = b'Y';
    pub const SUPERSCRIPT_ON: u8 = b'x';
    pub const SUPERSCRIPT_OFF: u8 = b'y';
    pub const SUBSCRIPT_ON: u8 = b'w';
    pub const SUBSCRIPT_OFF: u8 = b'v';
    pub const ITALIC_ON: u8 = b'a';
    pub const ITALIC_OFF: u8 = b'b';

    // Line spacing
    pub const LINE_SPACING_6LPI: u8 = b'A';
    pub const LINE_SPACING_8LPI: u8 = b'B';
    pub const LINE_SPACING_N144: u8 = b'T';

    // Character pitch
    pub const PICA_10CPI: u8 = b'N';
    pub const ELITE_12CPI: u8 = b'E';
    pub const SEMI_CONDENSED_15CPI: u8 = b'q';
    pub const CONDENSED_17CPI: u8 = b'Q';
    pub const PROPORTIONAL_ON: u8 = b'p';
    pub const PROPORTIONAL_OFF: u8 = b'P';

    // Other styles
    pub const DOUBLE_STRIKE_ON: u8 = b'9';
    pub const DOUBLE_STRIKE_OFF: u8 = b'H';

    // Tab control
    pub const CLEAR_TABS: u8 = b'0';
    pub const SET_TAB: u8 = b'1';

    // Margins and positioning
    pub const LEFT_MARGIN: u8 = b'L';
    pub const RIGHT_MARGIN: u8 = b'M';
    pub const HORIZONTAL_POS: u8 = b'l';

    // Color
    pub const COLOR_SELECT: u8 = b'K';

    // Graphics
    pub const GRAPHICS_72: u8 = b'G';
    pub const GRAPHICS_144: u8 = b'V';
    pub const GRAPHICS_SINGLE: u8 = b'g';
    pub const GRAPHICS_EXTENDED: u8 = b's';

    // Immediate print
    pub const PRINT_IMMEDIATE: u8 = b'n';

    // Reset
    pub const RESET: u8 = b'c';

    // Character set
    pub const EIGHT_BIT: u8 = b'(';
    pub const SEVEN_BIT: u8 = b')';
}

/// Color ribbon colors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrinterColor {
    Black = 0,
    Yellow = 1,
    Red = 2,
    Blue = 3,
}

impl From<u8> for PrinterColor {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Yellow,
            2 => Self::Red,
            3 => Self::Blue,
            _ => Self::Black,
        }
    }
}

impl PrinterColor {
    /// Get RGB color value
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Black => (0, 0, 0),
            Self::Yellow => (255, 255, 0),
            Self::Red => (255, 0, 0),
            Self::Blue => (0, 0, 255),
        }
    }
}
