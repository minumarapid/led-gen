use std::io::{BufRead, Write};

use image::codecs::pnm::{PnmDecoder, PnmEncoder, PnmSubtype, SampleEncoding};
use image::{ExtendedColorType, ImageDecoder, RgbImage};
use led_gen_core::{LedConfig, LedError, LedPipeline};

/// Peek at the buffered input and report whether it looks like a binary PPM
/// (`image2pipe -vcodec ppm`) stream. The check is non-consuming: the peeked
/// bytes stay in the buffer for the actual decoder.
pub fn looks_like_ppm_stream<R: BufRead + ?Sized>(reader: &mut R) -> Result<bool, LedError> {
    let peek = reader
        .fill_buf()
        .map_err(|e| LedError::FailedDecode(format!("Unable to read from the input ({e})")))?;
    Ok(peek.len() >= 2 && peek[0] == b'P' && peek[1] == b'6')
}

fn decode_one_frame<R: BufRead + ?Sized>(
    reader: &mut R,
    frame_no: usize,
) -> Result<Option<RgbImage>, LedError> {
    if reader
        .fill_buf()
        .map_err(|e| LedError::FailedDecode(format!("Failed to read ppm frame {frame_no} ({e})")))?
        .is_empty()
    {
        return Ok(None);
    }

    // Borrow the reader instead of taking ownership so the same buffered
    // stream can be reused for the next concatenated frame. `PnmDecoder`
    // only requires `Read` (no `Seek`), which is what makes stdin piping
    // possible at all.
    let decoder = PnmDecoder::new(&mut *reader).map_err(|e| {
        LedError::FailedDecode(format!("Failed to decode ppm frame {frame_no} ({e})"))
    })?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 {
        return Err(LedError::FailedDecode(format!(
            "Invalid size in ppm frame {frame_no} ({width}x{height})"
        )));
    }

    match decoder.color_type() {
        image::ColorType::Rgb8 => {
            let mut buf = vec![0u8; decoder.total_bytes() as usize];
            decoder
                .read_image(&mut buf)
                .map_err(|e| {
                    LedError::FailedDecode(format!("Failed to decode ppm frame {frame_no} ({e})"))
                })?;
            RgbImage::from_raw(width, height, buf).ok_or_else(|| {
                LedError::FailedDecode(format!("Invalid pixel data in ppm frame {frame_no}"))
            }).map(Some)
        }
        image::ColorType::L8 => {
            let mut gray = vec![0u8; decoder.total_bytes() as usize];
            decoder
                .read_image(&mut gray)
                .map_err(|e| {
                    LedError::FailedDecode(format!("Failed to decode ppm frame {frame_no} ({e})"))
                })?;
            let mut rgb = Vec::with_capacity(gray.len() * 3);
            for g in gray {
                rgb.extend_from_slice(&[g, g, g]);
            }
            RgbImage::from_raw(width, height, rgb).ok_or_else(|| {
                LedError::FailedDecode(format!("Invalid pixel data in ppm frame {frame_no}"))
            }).map(Some)
        }
        other => Err(LedError::FailedDecode(format!(
            "Unsupported pixel format in ppm frame {frame_no} ({other:?}); only P6 RGB and P5 grayscale are supported"
        ))),
    }
}

fn encode_one_frame<W: Write>(
    writer: &mut W,
    image: &RgbImage,
    frame_no: usize,
) -> Result<(), LedError> {
    let (width, height) = image.dimensions();
    PnmEncoder::new(&mut *writer)
        .with_subtype(PnmSubtype::Pixmap(SampleEncoding::Binary))
        .encode(
            image.as_raw().as_slice(),
            width,
            height,
            ExtendedColorType::Rgb8,
        )
        .map_err(|e| {
            LedError::FailedEncode(format!("Failed to encode ppm frame {frame_no} ({e})"))
        })?;
    // NOTE: no per-frame flush here; the caller flushes once per batch so a
    // piped ffmpeg still gets data promptly without a syscall per frame.
    Ok(())
}

/// Number of frames converted concurrently.
///
/// Follows the CPU count (rayon pool size when initialized, otherwise
/// `available_parallelism`), clamped so one batch never holds more than
/// ~512MB of output canvas bytes. Frame order is unaffected: batches are
/// still written sequentially.
fn parallel_batch_size(canvas_bytes_per_frame: Option<u64>) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(1);
    let Some(bytes) = canvas_bytes_per_frame else {
        return cpus;
    };
    const MAX_BATCH_BYTES: u64 = 512 * 1024 * 1024;
    let mem_cap = (MAX_BATCH_BYTES / bytes.max(1)).max(1) as usize;
    cpus.min(mem_cap).max(1)
}

/// Decode a concatenated ppm (`image2pipe -vcodec ppm`) stream frame by
/// frame, convert frames in CPU-count-sized parallel batches, and write the
/// results back as a concatenated ppm stream in input order.
/// Returns the number of processed frames.
pub fn run_ppm_stream<R: BufRead + ?Sized, W: Write>(
    reader: &mut R,
    writer: &mut W,
    config: &LedConfig,
) -> Result<usize, LedError> {
    let pipeline = LedPipeline::new(config);
    let mut frames = 0usize;
    let mut batch: Vec<RgbImage> = Vec::new();
    let mut batch_size: Option<usize> = None;

    loop {
        // Fill one batch sequentially: the concatenated ppm format has no
        // frame index, so decoding itself must stay sequential.
        let limit = batch_size.unwrap_or_else(|| parallel_batch_size(None));
        while batch.len() < limit {
            let frame_no = frames + batch.len() + 1;
            match decode_one_frame(reader, frame_no)? {
                Some(frame) => {
                    if batch_size.is_none() {
                        let (w, h) = frame.dimensions();
                        let (cw, ch) = pipeline.output_dimensions(w, h);
                        // base + glow canvases per frame.
                        let bytes = cw as u64 * ch as u64 * 3 * 2;
                        batch_size = Some(parallel_batch_size(Some(bytes)));
                    }
                    batch.push(frame);
                }
                None => break,
            }
        }
        if batch.is_empty() {
            break;
        }

        let converted = pipeline.process_batch(std::mem::take(&mut batch))?;
        for image in &converted {
            frames += 1;
            encode_one_frame(writer, image, frames)?;
        }
        // One flush per batch: prompt enough for a piped ffmpeg while
        // avoiding a syscall per frame.
        writer.flush().map_err(|e| {
            LedError::FailedEncode(format!("Failed to write ppm frame {frames} ({e})"))
        })?;
    }
    if frames == 0 {
        return Err(LedError::FailedDecode("Empty input".to_string()));
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use led_gen_core::LedShape;

    fn test_config() -> LedConfig {
        LedConfig {
            border: 0,
            led_size: 1,
            led_gap: 0,
            led_shape: LedShape::Square,
            led_exposure: 1.0,
            enable_glow: false,
            glow_range: 0.0,
            glow_strength: 1.0,
            glow_exposure: 1.0,
            off_light_color: [0, 0, 0],
            canvas_background: [0, 0, 0],
        }
    }

    fn encode_ppm_frame(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        PnmEncoder::new(&mut out)
            .with_subtype(PnmSubtype::Pixmap(SampleEncoding::Binary))
            .encode(rgb, width, height, ExtendedColorType::Rgb8)
            .expect("test ppm encode must succeed");
        out
    }

    fn decode_all_frames(mut bytes: &[u8]) -> Vec<(u32, u32, Vec<u8>)> {
        let mut reader = std::io::BufReader::new(&mut bytes);
        let mut frames = Vec::new();
        loop {
            let done = reader.fill_buf().expect("fill_buf must succeed").is_empty();
            if done {
                break;
            }
            let decoder = PnmDecoder::new(&mut reader).expect("test ppm decode must succeed");
            let (w, h) = decoder.dimensions();
            let mut buf = vec![0u8; decoder.total_bytes() as usize];
            decoder
                .read_image(&mut buf)
                .expect("read_image must succeed");
            frames.push((w, h, buf));
        }
        frames
    }

    #[test]
    fn detects_ppm_magic_without_consuming() {
        let mut ppm = std::io::BufReader::new(&b"P6\n2 1\n255\nABCD"[..]);
        assert!(looks_like_ppm_stream(&mut ppm).unwrap());
        // The peeked bytes must still be there for the real decoder.
        let remaining: Vec<u8> = {
            let mut v = Vec::new();
            use std::io::Read;
            ppm.read_to_end(&mut v).unwrap();
            v
        };
        assert_eq!(&remaining[..2], b"P6");

        let mut png = std::io::BufReader::new(&b"\x89PNG\r\n\x1a\n0000"[..]);
        assert!(!looks_like_ppm_stream(&mut png).unwrap());

        let mut empty = std::io::BufReader::new(&b""[..]);
        assert!(!looks_like_ppm_stream(&mut empty).unwrap());
    }

    #[test]
    fn streams_many_frames_in_order_with_glow() {
        use led_gen_core::LedPipeline;
        // More frames than typical CPU counts so batching + remainder paths run.
        let config = LedConfig {
            border: 1,
            led_size: 2,
            led_gap: 1,
            led_shape: LedShape::Circle,
            led_exposure: 1.0,
            enable_glow: true,
            glow_range: 3.0,
            glow_strength: 1.5,
            glow_exposure: 1.0,
            off_light_color: [10, 10, 10],
            canvas_background: [16, 16, 16],
        };
        let pipeline = LedPipeline::new(&config);
        let mut input = Vec::new();
        let mut expected = Vec::new();
        for i in 0..25u8 {
            let rgb: Vec<u8> = (0..4 * 3 * 3)
                .map(|j| (j as u8).wrapping_add(i.wrapping_mul(17)))
                .collect();
            input.extend_from_slice(&encode_ppm_frame(4, 3, &rgb));
            let frame = RgbImage::from_raw(4, 3, rgb).expect("test frame must be valid");
            expected.push(pipeline.generate(frame).unwrap().into_raw());
        }

        let mut reader = std::io::BufReader::new(&input[..]);
        let mut output = Vec::new();
        let frames = run_ppm_stream(&mut reader, &mut output, &config).unwrap();
        assert_eq!(frames, 25);

        let decoded = decode_all_frames(&output);
        assert_eq!(decoded.len(), 25);
        for (i, (_, _, buf)) in decoded.iter().enumerate() {
            assert_eq!(buf, &expected[i], "frame {i} out of order or corrupt");
        }
    }

    #[test]
    fn batch_size_follows_cpus_with_memory_clamp() {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .max(1);
        assert_eq!(parallel_batch_size(None), cpus);
        // Tiny frames: no clamping.
        assert_eq!(parallel_batch_size(Some(12)), cpus);
        // Absurdly large frame: clamped to 1.
        assert_eq!(parallel_batch_size(Some(u64::MAX)), 1);
        assert!(parallel_batch_size(Some(1024 * 1024)) >= 1);
    }

    #[test]
    fn streams_two_frames_with_identity_config() {
        // led_size=1, no gap/border/glow => output pixels must equal input pixels.
        let frame_a = [255u8, 0, 0, 0, 255, 0];
        let frame_b = [0u8, 0, 255, 255, 255, 255];
        let mut input = Vec::new();
        input.extend_from_slice(&encode_ppm_frame(2, 1, &frame_a));
        input.extend_from_slice(&encode_ppm_frame(2, 1, &frame_b));

        let mut reader = std::io::BufReader::new(&input[..]);
        let mut output = Vec::new();
        let frames = run_ppm_stream(&mut reader, &mut output, &test_config()).unwrap();
        assert_eq!(frames, 2);

        let decoded = decode_all_frames(&output);
        assert_eq!(decoded.len(), 2);
        assert_eq!((decoded[0].0, decoded[0].1), (2, 1));
        assert_eq!((decoded[1].0, decoded[1].1), (2, 1));
        assert_eq!(decoded[0].2, frame_a);
        assert_eq!(decoded[1].2, frame_b);
    }

    #[test]
    fn rejects_empty_stream() {
        let mut reader = std::io::BufReader::new(&b""[..]);
        let mut output = Vec::new();
        assert!(run_ppm_stream(&mut reader, &mut output, &test_config()).is_err());
    }

    #[test]
    fn rejects_truncated_frame() {
        let mut full = encode_ppm_frame(2, 1, &[1u8, 2, 3, 4, 5, 6]);
        full.truncate(full.len() - 2);
        let mut reader = std::io::BufReader::new(&full[..]);
        let mut output = Vec::new();
        assert!(run_ppm_stream(&mut reader, &mut output, &test_config()).is_err());
    }
}
