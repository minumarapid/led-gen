# led-gen

`led-gen` converts regular images into an LED-style rendering. The image
processing core is written in Rust and is shared by the command-line tool and
the WebAssembly/TypeScript package.

## Packages

| Path | Package | Purpose |
|------|---------|---------|
| `src/core` | `core` | Shared Rust image-processing implementation. |
| `src/cli` | `led-gen` | Command-line image converter. |
| `src/wasm` | `wasm` | wasm-bindgen bindings for the Rust core. |
| `ts` | `led-gen` | TypeScript wrapper around the generated WASM package. |

## Before / After

### Original image

![Original image](https://github.com/minumarapid/led-gen/blob/master/test/input-0001.png?raw=true)

### LED-style image

![LED-style image](https://github.com/minumarapid/led-gen/blob/master/test/output-0001.png?raw=true)

## CLI Usage

Run the CLI from the workspace:

```bash
cargo run -p led-gen -- test/input-0001.png --output test/output.png
```

Useful options:

```bash
cargo run -p led-gen -- input.png \
  --output output.png \
  --format png \
  --led-size 4 \
  --led-gap 2 \
  --led-shape circle \
  --no-glow
```

The CLI reads `led-gen.config.toml` from the current directory when it exists.
Use `--config <path>` to load a specific config file. CLI options override
values from the config file.

See [src/cli/readme.md](src/cli/readme.md) for the full CLI documentation.

## TypeScript Usage

Install the npm package:

```bash
npm install led-gen
```

Use it with browser `ImageData`:

```ts
import { LedGenerator } from "led-gen";

const source = ctx.getImageData(0, 0, image.width, image.height);
const result = await LedGenerator.processImageData(source, {
  ledShape: "Circle",
  ledSize: 4,
  enableGlow: true,
});

ctx.putImageData(result, 0, 0);
```

See [ts/readme.md](ts/readme.md) for TypeScript API details.

## Development

Check the Rust workspace:

```bash
cargo check
```

Build the WebAssembly package before building the TypeScript wrapper:

```bash
wasm-pack build src/wasm --target web
cd ts
pnpm install
pnpm run build
```

## License

BSD 2-Clause License. See [LICENSE](LICENSE) for details.
