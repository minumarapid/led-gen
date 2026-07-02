mod utils;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use image::ImageReader;
use led_core::{generate_led_image, LedConfig, LedError};
use utils::{load_config, CliLedConfig};

#[derive(Parser)]
#[command(author, version, about = "A tool for converting images into an LED-style look")]
struct Cli {
    /// input image path
    input: PathBuf,

    /// export image path (If no file name is specified, append "_led" to the output)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format (if not specified, inferred from the file extension)
    #[arg(short, long, value_enum)]
    format: Option<OutputFormat>,

    /// Config file path
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    config: Option<PathBuf>,

    #[command(flatten)]
    led_config: CliLedConfig,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormat {
    Png,
    Jpeg,
    Bmp,
    Gif,
    Tiff,
    Webp,
}

impl OutputFormat {
    fn extension(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
            OutputFormat::Bmp => "bmp",
            OutputFormat::Gif => "gif",
            OutputFormat::Tiff => "tiff",
            OutputFormat::Webp => "webp",
        }
    }

    fn to_image_format(self) -> image::ImageFormat {
        match self {
            OutputFormat::Png => image::ImageFormat::Png,
            OutputFormat::Jpeg => image::ImageFormat::Jpeg,
            OutputFormat::Bmp => image::ImageFormat::Bmp,
            OutputFormat::Gif => image::ImageFormat::Gif,
            OutputFormat::Tiff => image::ImageFormat::Tiff,
            OutputFormat::Webp => image::ImageFormat::WebP,
        }
    }
}


fn default_file_name(input: &Path) -> OsString {
    let stem = input.file_stem().unwrap_or_else(|| OsStr::new("output"));
    let mut name = stem.to_os_string();
    name.push("_led");
    name
}

fn resolve_extension(input: &Path, format: Option<OutputFormat>) -> String {
    if let Some(format) = format {
        return format.extension().to_string();
    }
    input
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png")
        .to_string()
}

fn resolve_output_path(input: &Path, output: Option<PathBuf>, format: Option<OutputFormat>) -> PathBuf {
    let extension = resolve_extension(input, format);

    if let Some(mut out) = output {
        if out.exists() && out.is_dir() {
            let file_name = default_file_name(input);
            let mut candidate = out.clone();
            candidate.push(&file_name);
            candidate.set_extension(&extension);
            return candidate;
        }

        if format.is_some() && out.extension().is_none() {
            out.set_extension(&extension);
        }
        return out;
    }

    let mut out = input.with_file_name(default_file_name(input));
    out.set_extension(&extension);
    out
}

fn main() {
    if let Err(e) = run() {
        match e {
            LedError::FailedDecode(msg) => eprintln!("Error: Decoding failed ({})", msg),
            LedError::InvalidConfiguration(msg) => eprintln!("Error: The configuration is invalid ({})", msg),
            LedError::FailedEncode(msg) => eprintln!("Error: Encoding failed ({})", msg),
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), LedError> {
    let cli = Cli::parse();

    let mut config = if let Some(config_file_path) = cli.config.as_deref() {
        load_config(config_file_path)?
    } else if Path::new("led-gen.config.toml").exists() {
        load_config(Path::new("led-gen.config.toml"))?
    } else {
        LedConfig::default()
    };
    cli.led_config.apply_to(&mut config);

    let binary = fs::read(&cli.input)
        .map_err(|e| LedError::FailedDecode(format!("Unable to read the input file ({})", e)))?;

    let original_img = ImageReader::new(Cursor::new(binary))
        .with_guessed_format().map_err(|e| { LedError::FailedDecode(e.to_string()) })?
        .decode().map_err(|e| { LedError::FailedDecode(e.to_string()) })?
        .into_rgb8();

    let result = generate_led_image(original_img, &config)?;

    let output_path = resolve_output_path(&cli.input, cli.output, cli.format);

    let write_result = if let Some(format) = cli.format {
        let dynamic = image::DynamicImage::ImageRgb8(result);
        dynamic.save_with_format(&output_path, format.to_image_format())
    } else {
        result.save(&output_path)
    };

    write_result.map_err(|e| LedError::FailedEncode(format!("Failed to write the output file ({})", e)))?;
    println!("Success: Output to {}", output_path.display());
    Ok(())
}