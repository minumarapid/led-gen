# led-gen

[![npm version](https://img.shields.io/npm/v/led-gen.svg?style=flat-square)](https://www.npmjs.com/package/led-gen)
![npm package minimized gzipped size](https://img.shields.io/bundlejs/size/led-gen?style=flat-square)
[![License: BSD-2-Clause](https://img.shields.io/badge/License-BSD%202--Clause-orange.svg?style=flat-square)](https://opensource.org/licenses/BSD-2-Clause)

`led-gen` is a TypeScript wrapper around a Rust/WebAssembly image processor for
converting browser `ImageData` into an LED-style image.

## Before / After

### Original image

![Original image](https://github.com/minumarapid/led-gen/blob/master/test/input-0001.png?raw=true)

### LED-style image

![LED-style image](https://github.com/minumarapid/led-gen/blob/master/test/output-0001.png?raw=true)

## Installation

```bash
# npm
npm install led-gen

# pnpm
pnpm add led-gen
```

## Quick Start

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

`LedGenerator.processImageData` initializes the WASM module automatically. If
your bundler needs an explicit WASM URL or module input, initialize it first:

```ts
await LedGenerator.initialize(wasmUrl);
```

## API

### `LedGenerator.initialize(moduleOrPath?)`

Initializes the WASM module. Calling this more than once is safe.

### `LedGenerator.processImageData(imageData, config?)`

Converts an `ImageData` object and returns a new `ImageData` object. The input is
treated as RGBA data; the generated output is also RGBA.

## Configuration Options

| Name | Type                       | Default        | Description |
|------|----------------------------|----------------|-------------|
| `border` | `number`                   | `10`           | Margin around the generated LED grid in pixels. |
| `ledSize` | `number`                   | `4`            | Diameter or side length of each LED in pixels. |
| `ledGap` | `number`                   | `2`            | Spacing between LEDs in pixels. |
| `ledShape` | `"circle" \| "square"`     | `"circle"`     | Shape of each LED. |
| `ledExposure` | `number`                   | `1.0`          | Exposure multiplier applied to the LED color layer. |
| `enableGlow` | `boolean`                  | `true`         | Enables the glow layer. |
| `glowRange` | `number`                   | `3.0`          | Blur radius for the glow layer. |
| `glowStrength` | `number`                   | `1.75`         | Strength multiplier for the glow layer. |
| `glowExposure` | `number`                   | `1.0`          | Exposure multiplier applied to the glow layer. |
| `offLightColor` | `[number, number, number]` | `[64, 64, 64]` | Minimum RGB color used for each LED. |
| `canvasBackground` | `[number, number, number]` | `[16, 16, 16]` | RGB background color for the generated canvas. |

## Development

Build the WASM package first, then build the TypeScript wrapper:

```bash
wasm-pack build ../../crates/led-gen-wasm --target web
pnpm install
pnpm run build
```

## License

BSD 2-Clause License. See [LICENSE](../LICENSE) for details.
