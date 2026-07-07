# led-gen WASM

This crate exposes the shared Rust image-processing core through
`wasm-bindgen`. It is intended to be consumed by the TypeScript wrapper in
`../../ts`.

## Build

```bash
wasm-pack build --target web
```

When building from the repository root:

```bash
wasm-pack build src/wasm --target web
```

The generated package is written to `src/wasm/pkg`. The TypeScript wrapper
imports from that generated package during its build.

## Exported API

### `generate_led_image_wasm(imageData, width, height, config)`

Converts RGBA image data into an LED-style image.

Parameters:

| Name | Type | Description |
|------|------|-------------|
| `imageData` | `Uint8Array` or byte slice | RGBA pixel data. |
| `width` | `number` | Source image width. |
| `height` | `number` | Source image height. |
| `config` | `LedConfig` | LED rendering options. |

Returns a `ProcessedImageResult` with `width`, `height`, and RGBA `data`.

## Configuration

`LedConfig` is defined in the shared Rust core and exported to TypeScript via
`tsify`.

| Name | Default        |
|------|----------------|
| `border` | `10`           |
| `ledSize` | `4`            |
| `ledGap` | `2`            |
| `ledShape` | `"circle"`     |
| `ledExposure` | `1.0`          |
| `enableGlow` | `true`         |
| `glowRange` | `3.0`          |
| `glowStrength` | `1.75`         |
| `glowExposure` | `1.0`          |
| `offLightColor` | `[64, 64, 64]` |
| `canvasBackground` | `[16, 16, 16]` |

## Development Notes

Run the workspace check from the repository root:

```bash
cargo check
```

Browser tests can be run with `wasm-pack` when the required browser driver is
available:

```bash
wasm-pack test --headless --firefox
```
