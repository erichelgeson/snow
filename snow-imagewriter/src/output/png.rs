//! PNG output for ImageWriter pages

use crate::renderer::PageBuffer;
use anyhow::Result;
use image::{ImageBuffer, Rgba};
use std::path::Path;

/// Save a page buffer as PNG
pub fn save_png(page: &PageBuffer, path: &Path) -> Result<()> {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(page.pixel_width, page.pixel_height, page.pixels.clone())
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

    img.save(path)?;
    log::info!("Saved PNG: {}", path.display());
    Ok(())
}
