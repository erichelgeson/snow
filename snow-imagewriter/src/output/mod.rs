//! Output format handlers for ImageWriter

mod pdf;
mod png;

pub use pdf::save_pdf;
pub use png::save_png;

use crate::renderer::PageBuffer;
use std::path::Path;

/// Output format selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Pdf,
    Png,
    Both,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pdf" => Ok(Self::Pdf),
            "png" => Ok(Self::Png),
            "both" => Ok(Self::Both),
            _ => Err(format!("Unknown format: {s}")),
        }
    }
}

/// Save a page to file(s) based on format
pub fn save_page(
    page: &PageBuffer,
    base_path: &Path,
    page_num: usize,
    format: OutputFormat,
) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut saved = Vec::new();

    match format {
        OutputFormat::Pdf => {
            let path = base_path.join(format!("print_{page_num:03}.pdf"));
            save_pdf(page, &path)?;
            saved.push(path);
        }
        OutputFormat::Png => {
            let path = base_path.join(format!("print_{page_num:03}.png"));
            save_png(page, &path)?;
            saved.push(path);
        }
        OutputFormat::Both => {
            let pdf_path = base_path.join(format!("print_{page_num:03}.pdf"));
            save_pdf(page, &pdf_path)?;
            saved.push(pdf_path);

            let png_path = base_path.join(format!("print_{page_num:03}.png"));
            save_png(page, &png_path)?;
            saved.push(png_path);
        }
    }

    Ok(saved)
}
