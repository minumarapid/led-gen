# led-gen CLI

`led-gen` is a command-line tool that converts image files into an LED-style
rendering.

## Usage

```bash
cargo run -p led-gen-cli -- <input> [options]
```

Example:

```bash
cargo run -p led-gen-cli -- test/input-0001.png --output test/output.png --format png
```

## Options

| Option | Description |
|--------|-------------|
| `<input>` | Input image path. |
| `-o, --output <path>` | Output image path. If a directory is supplied, the output name is generated from the input name. |
| `-f, --format <format>` | Output format. Supported values: `png`, `jpeg`, `bmp`, `gif`, `tiff`, `webp`. |
| `-c, --config <path>` | Config file path. |
| `--border <number>` | Canvas border size in pixels. |
| `--led-size <number>` | LED size in pixels. |
| `--led-gap <number>` | Gap between LEDs in pixels. |
| `--led-shape <shape>` | LED shape. Supported values: `square`, `circle`. |
| `--led-exposure <number>` | LED color exposure multiplier. |
| `--no-glow` | Disable glow. |
| `--glow-range <number>` | Glow blur radius. |
| `--glow-strength <number>` | Glow strength multiplier. |
| `--glow-exposure <number>` | Glow color exposure multiplier. |
| `--off-light-color <R,G,B>` | Minimum LED color. Example: `64,64,64`. |
| `--canvas-background <R,G,B>` | Canvas background color. Example: `16,16,16`. |

## Config File

By default, the CLI reads `led-gen.config.toml` from the current directory if it
exists. Use `--config <path>` to load a specific file. Command-line options
override values from the config file.

```toml
border = 10
led_size = 4
led_gap = 2
led_shape = "circle"
led_exposure = 1.0
enable_glow = true
glow_range = 3.0
glow_strength = 1.75
glow_exposure = 1.0
off_light_color = [64, 64, 64]
canvas_background = [16, 16, 16]
```

You can also start from the repository sample:

```bash
cp led-gen.config.toml.example led-gen.config.toml
```

## Examples

Write next to the input with an inferred file name:

```bash
cargo run -p led-gen-cli -- input.png
```

Write to a directory:

```bash
cargo run -p led-gen-cli -- input.png --output ./dist
```

Use a config file and override one value:

```bash
cargo run -p led-gen-cli -- input.png --config led-gen.config.toml --led-size 6
```
