//! `aview`: explicit multimodal attachment tool over `$AGENT_CTL` (Section 7.2).
//!
//! `aview [--max-dim <px, default 1568>] [--pages <1-3,5>] [FILE | -]`

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode as StdExitCode};

use agent_tools::{create_secure_tmp_file, send_ctl_frame};
use clap::Parser;
use image::ImageFormat;
use image::imageops::FilterType;
use os_agent_proto::{ctl::CtlFrame, exit_code::ExitCode};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(
    name = "aview",
    version,
    about = "Attach an image or rendered PDF pages to the current agent turn via $AGENT_CTL"
)]
struct Cli {
    /// Maximum width/height in pixels for proportional resizing (default: 1568).
    #[arg(long = "max-dim", default_value_t = 1568)]
    max_dim: u32,

    /// PDF page selection (e.g. `1-3,5`). Defaults to page 1 for PDFs.
    #[arg(long = "pages")]
    pages: Option<String>,

    /// Input file path or `-` for stdin.
    input: Option<PathBuf>,
}

fn main() -> StdExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let _ = err.print();
            return if err.exit_code() == 0 {
                ExitCode::Success.into()
            } else {
                ExitCode::UsageError.into()
            };
        }
    };

    if cli.max_dim == 0 {
        eprintln!("aview: --max-dim must be >= 1");
        return ExitCode::UsageError.into();
    }

    let (source_name, raw_bytes, disk_path) = match load_input(cli.input.as_deref()) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("aview: {err}");
            return ExitCode::RuntimeError.into();
        }
    };

    if raw_bytes.starts_with(b"%PDF-") {
        let pages = match parse_pages(cli.pages.as_deref().unwrap_or("1")) {
            Ok(p) => p,
            Err(msg) => {
                eprintln!("aview: {msg}");
                return ExitCode::UsageError.into();
            }
        };
        match process_pdf(
            &raw_bytes,
            disk_path.as_deref(),
            &source_name,
            &pages,
            cli.max_dim,
        ) {
            Ok(()) => ExitCode::Success.into(),
            Err(err) => {
                eprintln!("aview: {err}");
                ExitCode::RuntimeError.into()
            }
        }
    } else {
        if cli.pages.is_some() {
            eprintln!("aview: warning: --pages ignored for non-PDF image input");
        }
        match process_image(&raw_bytes, &source_name, cli.max_dim) {
            Ok(()) => ExitCode::Success.into(),
            Err(err) => {
                eprintln!("aview: {err}");
                ExitCode::RuntimeError.into()
            }
        }
    }
}

fn load_input(input: Option<&Path>) -> Result<(String, Vec<u8>, Option<PathBuf>), String> {
    match input {
        None => read_stdin().map(|b| ("stdin".into(), b, None)),
        Some(p) if p == Path::new("-") => read_stdin().map(|b| ("stdin".into(), b, None)),
        Some(p) => {
            let bytes =
                fs::read(p).map_err(|e| format!("failed to read `{}`: {e}", p.display()))?;
            let label = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string());
            Ok((label, bytes, Some(p.to_path_buf())))
        }
    }
}

fn read_stdin() -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    io::stdin()
        .read_to_end(&mut buf)
        .map_err(|e| format!("failed to read stdin: {e}"))?;
    if buf.is_empty() {
        return Err("stdin is empty".into());
    }
    Ok(buf)
}

fn process_image(raw: &[u8], source: &str, max_dim: u32) -> Result<(), String> {
    let img = image::load_from_memory(raw)
        .map_err(|e| format!("failed to decode image `{source}`: {e}"))?;
    let (w, h) = (img.width(), img.height());
    let resized = if w > max_dim || h > max_dim {
        img.resize(max_dim, max_dim, FilterType::Lanczos3)
    } else {
        img
    };
    let (out_w, out_h) = (resized.width(), resized.height());

    let (out_path, mut file) = create_secure_tmp_file("aview", "png")
        .map_err(|e| format!("failed to create /tmp output PNG: {e}"))?;
    resized
        .write_to(&mut file, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    file.flush()
        .map_err(|e| format!("failed to flush PNG: {e}"))?;

    let frame = CtlFrame::Media {
        mime: "image/png".into(),
        path: out_path.display().to_string(),
        source: Some(source.to_owned()),
    };
    send_ctl_frame(&frame).map_err(|e| e.to_string())?;

    println!(
        "[aview] attached `{source}` ({out_w}x{out_h}) -> {}",
        out_path.display()
    );
    Ok(())
}

fn process_pdf(
    raw: &[u8],
    existing_path: Option<&Path>,
    source: &str,
    pages: &[u32],
    max_dim: u32,
) -> Result<(), String> {
    let mut tmp_pdf: Option<PathBuf> = None;
    let pdf_path = match existing_path {
        Some(p) => p.to_path_buf(),
        None => {
            let (p, mut f) = create_secure_tmp_file("aview_in", "pdf")
                .map_err(|e| format!("failed to stage stdin PDF: {e}"))?;
            f.write_all(raw)
                .map_err(|e| format!("failed to write staged PDF: {e}"))?;
            f.flush()
                .map_err(|e| format!("failed to flush staged PDF: {e}"))?;
            tmp_pdf = Some(p.clone());
            p
        }
    };

    let result = (|| {
        for &page in pages {
            let rendered_bytes = render_pdf_page_png(&pdf_path, page)?;
            let page_source = format!("{source}#page={page}");
            process_image(&rendered_bytes, &page_source, max_dim)?;
        }
        Ok(())
    })();

    if let Some(p) = tmp_pdf {
        let _ = fs::remove_file(p);
    }
    result
}

fn render_pdf_page_png(pdf_path: &Path, page: u32) -> Result<Vec<u8>, String> {
    let work_dir = PathBuf::from(format!("/tmp/aview_pdf_{}", Uuid::new_v4()));
    fs::create_dir_all(&work_dir).map_err(|e| format!("failed to create PDF scratch dir: {e}"))?;

    let res = (|| -> Result<Vec<u8>, String> {
        let page_str = page.to_string();

        // 1. Prefer `pdftoppm` (poppler-utils)
        if command_exists("pdftoppm") {
            let prefix = work_dir.join("page");
            let output = Command::new("pdftoppm")
                .args([
                    "-png",
                    "-r",
                    "150",
                    "-singlefile",
                    "-f",
                    &page_str,
                    "-l",
                    &page_str,
                ])
                .arg(pdf_path)
                .arg(&prefix)
                .output()
                .map_err(|e| format!("failed to execute pdftoppm: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "pdftoppm failed on page {page}: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            let png_path = work_dir.join("page.png");
            return fs::read(&png_path)
                .map_err(|e| format!("pdftoppm produced no output for page {page}: {e}"));
        }

        // 2. Fallback to `mutool` (MuPDF)
        if command_exists("mutool") {
            let png_path = work_dir.join("page.png");
            let output = Command::new("mutool")
                .args(["draw", "-F", "png", "-r", "150", "-o"])
                .arg(&png_path)
                .arg(pdf_path)
                .arg(&page_str)
                .output()
                .map_err(|e| format!("failed to execute mutool: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "mutool failed on page {page}: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            return fs::read(&png_path)
                .map_err(|e| format!("mutool produced no output for page {page}: {e}"));
        }

        // 3. Fallback to Ghostscript (`gs`)
        if command_exists("gs") {
            let png_path = work_dir.join("page.png");
            let out_arg = format!("-sOutputFile={}", png_path.display());
            let first_arg = format!("-dFirstPage={page}");
            let last_arg = format!("-dLastPage={page}");
            let output = Command::new("gs")
                .args([
                    "-q",
                    "-dNOPAUSE",
                    "-dBATCH",
                    "-sDEVICE=png16m",
                    "-r150",
                    &first_arg,
                    &last_arg,
                    &out_arg,
                ])
                .arg(pdf_path)
                .output()
                .map_err(|e| format!("failed to execute gs: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "gs failed on page {page}: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            return fs::read(&png_path)
                .map_err(|e| format!("gs produced no output for page {page}: {e}"));
        }

        Err("PDF rendering requires `pdftoppm` (poppler-utils), `mutool`, or `gs` in PATH".into())
    })();

    let _ = fs::remove_dir_all(&work_dir);
    res
}

fn command_exists(name: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| {
        let p = dir.join(name);
        p.is_file()
    })
}

fn parse_pages(spec: &str) -> Result<Vec<u32>, String> {
    let mut set = BTreeSet::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("invalid --pages specification `{spec}`"));
        }
        if let Some((start_s, end_s)) = part.split_once('-') {
            let start: u32 = start_s
                .trim()
                .parse()
                .map_err(|_| format!("invalid page number `{start_s}`"))?;
            let end: u32 = end_s
                .trim()
                .parse()
                .map_err(|_| format!("invalid page number `{end_s}`"))?;
            if start == 0 || end < start || end - start > 100 {
                return Err(format!(
                    "invalid page range `{part}` (must be 1-based and <= 100 pages)"
                ));
            }
            for p in start..=end {
                set.insert(p);
            }
        } else {
            let p: u32 = part
                .parse()
                .map_err(|_| format!("invalid page number `{part}`"))?;
            if p == 0 {
                return Err("page numbers must be >= 1".into());
            }
            set.insert(p);
        }
    }
    if set.is_empty() {
        return Err("no pages selected".into());
    }
    Ok(set.into_iter().collect())
}
