mod ppm;
mod utils;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use image::{ImageReader, RgbImage};
use led_gen_core::{generate_led_image, LedConfig, LedError};
use utils::{load_config, CliLedConfig};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "A tool for converting images into an LED-style look"
)]
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

fn resolve_output_path(
    input: &Path,
    output: Option<PathBuf>,
    format: Option<OutputFormat>,
) -> PathBuf {
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
        .with_guessed_format()
        .map_err(|e| LedError::FailedDecode(e.to_string()))?
        .decode()
        .map_err(|e| LedError::FailedDecode(e.to_string()))?
        .into_rgb8();

    generate_led_image(original_img, &config)
}

fn read_single_image(reader: &mut dyn BufRead, from_file: bool) -> Result<Vec<u8>, LedError> {
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).map_err(|e| {
        LedError::FailedDecode(if from_file {
            format!("Unable to read the input file ({e})")
        } else {
            format!("Unable to read from stdin ({e})")
        })
    })?;
    if buffer.is_empty() {
        return Err(LedError::FailedDecode("Empty input".to_string()));
    }
    Ok(buffer)
}

fn write_to_stdout(image: RgbImage, format: Option<OutputFormat>) -> Result<(), LedError> {
    let mut buffer = Vec::new();
    if let Some(format) = format {
        let dynamic = image::DynamicImage::ImageRgb8(image);
        dynamic
            .write_to(&mut Cursor::new(&mut buffer), format.to_image_format())
            .map_err(|e| {
                LedError::FailedEncode(format!("Failed to encode the output image ({})", e))
            })?;
    } else {
        image
            .write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Png)
            .map_err(|e| {
                LedError::FailedEncode(format!("Failed to encode the output image ({})", e))
            })?;
    }
    std::io::stdout()
        .write_all(&buffer)
        .map_err(|e| LedError::FailedEncode(format!("Failed to write to stdout ({})", e)))?;
    Ok(())
}

fn write_to_file(
    output_path: &Path,
    image: RgbImage,
    format: Option<OutputFormat>,
) -> Result<(), LedError> {
    let write_result = if let Some(format) = format {
        let dynamic = image::DynamicImage::ImageRgb8(image);
        dynamic.save_with_format(output_path, format.to_image_format())
    } else {
        image.save(&output_path)
    };
    write_result
        .map_err(|e| LedError::FailedEncode(format!("Failed to write the output file ({})", e)))?;
    println!("Success: Output to {}", output_path.display());
    Ok(())
}

fn write_output(
    output_path: Option<&Path>,
    image: RgbImage,
    format: Option<OutputFormat>,
) -> Result<(), LedError> {
    match output_path {
        Some(path) => write_to_file(path, image, format),
        None => write_to_stdout(image, format),
    }
}

/// Resolve where a concatenated ppm stream goes. Single-image extension
/// inference is reused with `format = None` so `video.ppm` becomes
/// `video_led.ppm` next to the input.
fn resolve_stream_output(input_path: Option<&Path>, output: Option<&Path>) -> Option<PathBuf> {
    match (input_path, output) {
        (Some(input), Some(out)) => Some(resolve_output_path(input, Some(out.to_path_buf()), None)),
        (None, Some(out)) => Some(out.to_path_buf()),
        (Some(input), None) => Some(resolve_output_path(input, None, None)),
        (None, None) => None,
    }
}

fn run_ppm_stream_mode(
    output: Option<&Path>,
    input_path: Option<&Path>,
    reader: &mut dyn BufRead,
    config: &LedConfig,
) -> Result<(), LedError> {
    match resolve_stream_output(input_path, output) {
        Some(resolved) => {
            let file = fs::File::create(&resolved).map_err(|e| {
                LedError::FailedEncode(format!("Failed to create the output file ({e})"))
            })?;
            let mut writer = BufWriter::new(file);
            let frames = ppm::run_ppm_stream(reader, &mut writer, config)?;
            eprintln!("Success: {frames} frame(s) -> {}", resolved.display());
            Ok(())
        }
        None => {
            let stdout = std::io::stdout();
            let mut writer = BufWriter::new(stdout.lock());
            let frames = ppm::run_ppm_stream(reader, &mut writer, config)?;
            eprintln!("Success: {frames} frame(s) written to stdout");
            Ok(())
        }
    }
}

fn run_single_image_mode(
    input: Option<&Path>,
    output: Option<PathBuf>,
    format: Option<OutputFormat>,
    config: LedConfig,
    reader: &mut dyn BufRead,
) -> Result<(), LedError> {
    let binary = read_single_image(reader, input.is_some())?;
    let output_path = output.or_else(|| input.map(|path| resolve_output_path(path, None, format)));

    write_output(
        output_path.as_deref(),
        process_image(config, binary)?,
        format,
    )?;

    Ok(())
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

    // Buffer the input up front so the ppm/pgm magic (`P6`/`P5`) can be
    // peeked at without consuming anything. Files and stdin share this
    // path, which keeps single-image behavior identical while enabling
    // streaming.
    let mut input: Box<dyn BufRead> = match cli.input.as_deref() {
        Some(path) => Box::new(BufReader::new(fs::File::open(path).map_err(|e| {
            LedError::FailedDecode(format!("Unable to read the input file ({e})"))
        })?)),
        None => Box::new(BufReader::new(std::io::stdin())),
    };

    // A leading `P6`/`P5` on *stdin* means a concatenated ppm/pgm stream
    // (`ffmpeg -f image2pipe -vcodec ppm -`); anything else is one still
    // image. Detection is stdin-only so that `.ppm`/`.pgm` file arguments
    // keep working as stills (with `--format` honored). To stream a
    // concatenated file, pipe it through stdin instead.
    let is_stream = cli.input.is_none() && ppm::looks_like_ppm_stream(&mut input)?;
    if is_stream {
        if cli.format.is_some() {
            eprintln!("Warning: --format is ignored for ppm streams (output is always ppm)");
        }
        return run_ppm_stream_mode(
            cli.output.as_deref(),
            cli.input.as_deref(),
            &mut input,
            &config,
        );
    }

    run_single_image_mode(
        cli.input.as_deref(),
        cli.output,
        cli.format,
        config,
        &mut input,
    )
}
