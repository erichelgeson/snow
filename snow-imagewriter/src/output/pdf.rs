//! PDF output for ImageWriter pages

use crate::renderer::{OUTPUT_DPI, PageBuffer};
use anyhow::Result;
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// Save a page buffer as PDF
pub fn save_pdf(page: &PageBuffer, path: &Path) -> Result<()> {
    // Create PDF document
    // printpdf uses millimeters, convert from inches
    let width_mm = page.width * 25.4;
    let height_mm = page.height * 25.4;

    let (doc, page1, layer1) =
        PdfDocument::new("ImageWriter Output", Mm(width_mm), Mm(height_mm), "Layer 1");

    let current_layer = doc.get_page(page1).get_layer(layer1);

    // Convert RGBA to RGB for PDF
    let rgb_pixels: Vec<u8> = page
        .pixels
        .chunks(4)
        .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
        .collect();

    // Create image from raw RGB data
    let image_data = ImageXObject {
        width: Px(page.pixel_width as usize),
        height: Px(page.pixel_height as usize),
        color_space: ColorSpace::Rgb,
        bits_per_component: ColorBits::Bit8,
        interpolate: true,
        image_data: rgb_pixels,
        image_filter: None,
        clipping_bbox: None,
        smask: None,
    };

    let image = Image::from(image_data);

    // Simple scale: 1.0 means image pixels map to PDF points
    // This worked for the Mac graphics output
    let dpi = OUTPUT_DPI as f32;
    let scale = page.width / (page.pixel_width as f32 / dpi);

    image.add_to_layer(
        current_layer,
        ImageTransform {
            translate_x: Some(Mm(0.0)),
            translate_y: Some(Mm(0.0)),
            scale_x: Some(scale),
            scale_y: Some(scale),
            ..Default::default()
        },
    );

    // Save the PDF
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    doc.save(&mut writer)?;

    log::info!("Saved PDF: {}", path.display());
    Ok(())
}
