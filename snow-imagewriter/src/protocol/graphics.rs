//! Graphics mode handling for ImageWriter

#![allow(dead_code)]

use super::{GraphicsDensity, ImageWriterParser};

impl ImageWriterParser {
    /// Process a graphics data byte
    ///
    /// Uses start_x and column index to calculate position precisely,
    /// avoiding floating-point accumulation errors.
    pub fn process_graphics_byte(
        &mut self,
        byte: u8,
        density: GraphicsDensity,
        start_x: f32,
        column: usize,
    ) {
        let dpi = density.horizontal_dpi();
        let dot_width = 1.0 / dpi;

        // Calculate x position from start_x and column to avoid accumulation error
        let x = start_x + (column as f32 * dot_width);

        // Each byte represents 8 vertical dots (8-pin print head)
        let mut dots_drawn = 0;
        for bit in 0..8 {
            if byte & (1 << bit) != 0 {
                // Calculate dot position
                let y = (bit as f32).mul_add(1.0 / 72.0, self.cursor.y); // Vertical is always 72 DPI base

                self.page.set_pixel_at_inches(x, y, self.color);
                dots_drawn += 1;
            }
        }

        if dots_drawn > 0 {
            log::trace!(
                "Graphics byte 0x{:02X}: {} dots at ({:.3}\", {:.3}\") col={}",
                byte,
                dots_drawn,
                x,
                self.cursor.y,
                column
            );
        }
    }

    /// Get the graphics line height in inches (for advancing after graphics row)
    pub fn graphics_line_height(&self) -> f32 {
        8.0 / 72.0 // 8 pins at 72 DPI vertical = 8/72 inch
    }
}

/// Graphics row data for rendering
#[derive(Debug, Clone)]
pub struct GraphicsRow {
    /// Raw byte data for the row
    pub data: Vec<u8>,
    /// Horizontal density
    pub density: GraphicsDensity,
    /// X position where row starts (inches)
    pub x_start: f32,
    /// Y position of row (inches)
    pub y_pos: f32,
}
