mod utils;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use image::{ImageReader, RgbImage};
use led_gen_core::{generate_led_image, LedConfig, LedError};
use utils::{load_config, CliLedConfig};

#[derive(Parser)]
#[command(author, version, about = "A tool for converting images into an LED-style look")]
struct Cli {
    /// input image path
    input: Option<PathBuf>,

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
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn process_image(config: LedConfig, binary: Vec<u8>) -> Result<RgbImage, LedError> {

    let original_img = ImageReader::new(Cursor::new(binary))
        .with_guessed_format().map_err(|e| { LedError::FailedDecode(e.to_string()) })?
        .decode().map_err(|e| { LedError::FailedDecode(e.to_string()) })?
        .into_rgb8();

    generate_led_image(original_img, &config)
}

fn read_input(input: Option<&Path>) -> Result<Vec<u8>, LedError> {
    match input {
        Some(path) => fs::read(path)
            .map_err(|e| LedError::FailedDecode(
                format!("Unable to read the input file ({e})")
            )),
        None => {
            let mut buffer = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buffer)
                .map_err(|e| LedError::FailedDecode(
                    format!("Unable to read from stdin ({e})")
                ))?;
            Ok(buffer)
        }
    }
}

fn write_to_stdout(image: RgbImage, format: Option<OutputFormat>) -> Result<(), LedError> {
    let mut buffer = Vec::new();
    if let Some(format) = format {
        let dynamic = image::DynamicImage::ImageRgb8(image);
        dynamic.write_to(&mut Cursor::new(&mut buffer), format.to_image_format())
            .map_err(|e| LedError::FailedEncode(format!("Failed to encode the output image ({})", e)))?;
    } else {
        image.write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Png)
            .map_err(|e| LedError::FailedEncode(format!("Failed to encode the output image ({})", e)))?;
    }
    std::io::stdout().write_all(&buffer)
        .map_err(|e| LedError::FailedEncode(format!("Failed to write to stdout ({})", e)))?;
    Ok(())
}

fn write_to_file(output_path: &Path, image: RgbImage, format: Option<OutputFormat>) -> Result<(), LedError> {
    let write_result = if let Some(format) = format {
        let dynamic = image::DynamicImage::ImageRgb8(image);
        dynamic.save_with_format(&output_path, format.to_image_format())
    } else {
        image.save(&output_path)
    };
    write_result.map_err(|e| LedError::FailedEncode(format!("Failed to write the output file ({})", e)))?;
    println!("Success: Output to {}", output_path.display());
    Ok(())
}

fn write_output(output_path: Option<&Path>, image: RgbImage, format: Option<OutputFormat>) -> Result<(), LedError> {
    match output_path {
        Some(path) => write_to_file(path, image, format),
        None => write_to_stdout(image, format),
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

    let binary = read_input(cli.input.as_deref())?;
    let output_path = cli.output
        .or_else(|| cli.input.as_deref().map(|path| resolve_output_path(
            path,
            None,
            cli.format,
        )));

    write_output(
        output_path.as_deref(),
        process_image(config, binary)?,
        cli.format)?;

    Ok(())
}