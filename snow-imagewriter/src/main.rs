mod output;
mod protocol;
mod renderer;

use anyhow::{Context, Result};
use clap::Parser;
use output::OutputFormat;
use protocol::ImageWriterParser;
use std::io::Read;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Parser, Debug)]
#[command(name = "snow-imagewriter")]
#[command(about = "Apple ImageWriter II printer emulation for Snow")]
#[command(version)]
struct Args {
    /// PTY device to read from (e.g., /dev/pts/3)
    #[arg(long, conflicts_with = "tcp")]
    pty: Option<PathBuf>,

    /// TCP address to connect to (e.g., localhost:2000)
    #[arg(long, conflicts_with = "pty")]
    tcp: Option<String>,

    /// Output directory for print files
    #[arg(short, long, default_value = ".")]
    output: PathBuf,

    /// Output format: pdf, png, or both
    #[arg(short, long, default_value = "pdf")]
    format: OutputFormat,

    /// Verbose output (shows ESC sequences)
    #[arg(short, long)]
    verbose: bool,

    /// Trace output (shows every byte)
    #[arg(short, long)]
    trace: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.trace {
        "trace"
    } else if args.verbose {
        "debug"
    } else {
        "info"
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    // Create output directory if needed
    std::fs::create_dir_all(&args.output)
        .with_context(|| format!("Failed to create output directory: {:?}", args.output))?;

    // Set up signal handling for graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc_handler(r);

    // Create parser
    let mut parser = ImageWriterParser::new();
    let mut page_counter = 1usize;

    // Connect to input source and process
    if let Some(pty_path) = &args.pty {
        log::info!("Connecting to PTY: {}", pty_path.display());
        process_pty(
            pty_path,
            &mut parser,
            &mut page_counter,
            &args.output,
            args.format,
            &running,
        )?;
    } else if let Some(tcp_addr) = &args.tcp {
        log::info!("Connecting to TCP: {}", tcp_addr);
        process_tcp(
            tcp_addr,
            &mut parser,
            &mut page_counter,
            &args.output,
            args.format,
            &running,
        )?;
    } else {
        log::info!("Reading from stdin...");
        process_stdin(
            &mut parser,
            &mut page_counter,
            &args.output,
            args.format,
            &running,
        )?;
    }

    // Output any remaining completed pages (skip incomplete current page on Ctrl-C)
    for page in parser.take_completed_pages() {
        if page.is_empty() {
            log::debug!("Skipping empty page");
            continue;
        }
        let saved = output::save_page(&page, &args.output, page_counter, args.format)?;
        for path in saved {
            log::info!("Saved: {}", path.display());
        }
        page_counter += 1;
    }

    log::info!(
        "ImageWriter emulation finished. {} page(s) output.",
        page_counter - 1
    );
    Ok(())
}

fn ctrlc_handler(running: Arc<AtomicBool>) {
    let _ = ctrlc::set_handler(move || {
        log::info!("Received interrupt, shutting down...");
        running.store(false, Ordering::SeqCst);
    });
}

#[cfg(unix)]
fn process_pty(
    path: &Path,
    parser: &mut ImageWriterParser,
    page_counter: &mut usize,
    output_dir: &Path,
    format: OutputFormat,
    running: &Arc<AtomicBool>,
) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("Failed to open PTY: {}", path.display()))?;

    let mut reader = std::io::BufReader::new(file);
    let mut buf = [0u8; 1024];

    while running.load(Ordering::SeqCst) {
        match reader.read(&mut buf) {
            Ok(0) => {
                // EOF
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Ok(n) => {
                process_bytes(&buf[..n], parser, page_counter, output_dir, format)?;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                return Err(e).context("Error reading from PTY");
            }
        }
    }

    Ok(())
}

#[cfg(not(unix))]
fn process_pty(
    _path: &Path,
    _parser: &mut ImageWriterParser,
    _page_counter: &mut usize,
    _output_dir: &Path,
    _format: OutputFormat,
    _running: &Arc<AtomicBool>,
) -> Result<()> {
    anyhow::bail!("PTY support is only available on Unix systems")
}

fn process_tcp(
    addr: &str,
    parser: &mut ImageWriterParser,
    page_counter: &mut usize,
    output_dir: &Path,
    format: OutputFormat,
    running: &Arc<AtomicBool>,
) -> Result<()> {
    let stream =
        TcpStream::connect(addr).with_context(|| format!("Failed to connect to {addr}"))?;

    stream.set_nonblocking(true)?;

    let mut reader = std::io::BufReader::new(stream);
    let mut buf = [0u8; 1024];

    while running.load(Ordering::SeqCst) {
        match reader.read(&mut buf) {
            Ok(0) => {
                log::info!("Connection closed");
                break;
            }
            Ok(n) => {
                process_bytes(&buf[..n], parser, page_counter, output_dir, format)?;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                return Err(e).context("Error reading from TCP");
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
fn process_stdin(
    parser: &mut ImageWriterParser,
    page_counter: &mut usize,
    output_dir: &Path,
    format: OutputFormat,
    running: &Arc<AtomicBool>,
) -> Result<()> {
    use std::os::unix::io::AsRawFd;

    let stdin = std::io::stdin();
    let fd = stdin.as_raw_fd();

    // Set stdin to non-blocking
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    let mut reader = stdin.lock();
    let mut buf = [0u8; 1024];

    while running.load(Ordering::SeqCst) {
        match reader.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                process_bytes(&buf[..n], parser, page_counter, output_dir, format)?;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                return Err(e).context("Error reading from stdin");
            }
        }
    }

    Ok(())
}

#[cfg(not(unix))]
fn process_stdin(
    parser: &mut ImageWriterParser,
    page_counter: &mut usize,
    output_dir: &Path,
    format: OutputFormat,
    _running: &Arc<AtomicBool>,
) -> Result<()> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut buf = [0u8; 1024];

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                process_bytes(&buf[..n], parser, page_counter, output_dir, format)?;
            }
            Err(e) => {
                return Err(e).context("Error reading from stdin");
            }
        }
    }

    Ok(())
}

fn process_bytes(
    bytes: &[u8],
    parser: &mut ImageWriterParser,
    page_counter: &mut usize,
    output_dir: &Path,
    format: OutputFormat,
) -> Result<()> {
    for &byte in bytes {
        parser.process_byte(byte);
    }

    // Check for completed pages (skip empty ones)
    for page in parser.take_completed_pages() {
        if page.is_empty() {
            log::debug!("Skipping empty page");
            continue;
        }
        let saved = output::save_page(&page, output_dir, *page_counter, format)?;
        for path in saved {
            log::info!("Saved: {}", path.display());
        }
        *page_counter += 1;
    }

    Ok(())
}
