# led-gen

`led-gen` converts regular images into an LED-style rendering. The image
processing core is written in Rust and is shared by the command-line tool and
the WebAssembly/TypeScript package.

## Packages

| Path | Package | Purpose |
|------|---------|---------|
| `crates/led-gen-core` | `led-gen-core` | Shared Rust image-processing implementation. |
| `crates/led-gen-cli` | `led-gen-cli` | Command-line image converter. |
| `crates/led-gen-wasm` | `led-gen-wasm` | wasm-bindgen bindings for the Rust core. |
| `packages/led-gen` | `led-gen` | TypeScript wrapper around the generated WASM package. |

## Before / After

### Original image

![Original image](https://github.com/minumarapid/led-gen/blob/master/test/input-0001.png?raw=true)

### LED-style image

![LED-style image](https://github.com/minumarapid/led-gen/blob/master/test/output-0001.png?raw=true)

## CLI Usage

Run the CLI from the workspace:

```bash
cargo run -p led-gen-cli -- test/input-0001.png --output test/output.png
```

Useful options:

```bash
cargo run -p led-gen-cli -- input.png \
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

See [crates/led-gen-cli/readme.md](crates/led-gen-cli/readme.md) for the full CLI documentation.

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
  ledShape: "circle",
  ledSize: 4,
  ledGap: 2,
  enableGlow: true,
});

ctx.putImageData(result, 0, 0);
```

See [packages/led-gen/readme.md](packages/led-gen/readme.md) for TypeScript API details.

## Development

Check the Rust workspace:

```bash
cargo check
```

Build the WebAssembly package before building the TypeScript wrapper:

```bash
wasm-pack build crates/led-gen-wasm --target web
cd packages/led-gen
pnpm install
pnpm run build
```

## License

BSD 2-Clause License. See [LICENSE](LICENSE) for details.
