# led-gen

[![npm version](https://img.shields.io/npm/v/led-gen.svg?style=flat-square)](https://www.npmjs.com/package/led-gen)
![npm package minimized gzipped size](https://img.shields.io/bundlejs/size/led-gen?style=flat-square)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)

A high-performance library that includes WASM code to convert images into an LED-style appearance.

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

## Features
- Fast image processing using WebAssembly (WASM) for performance-critical code.
- Configurable LED grid generation with options for LED size, gap, shape, and glow effects.
- Simple syntax through encapsulation

## Quick Start
```typescript
import { LedGenerator } from 'led-gen';

// Example: convert an HTMLCanvas ImageData to an LED-style image
const imageData = srcCtx.getImageData(0, 0, img.width, img.height);
const result = await LedGenerator.processImageData(imageData, {
  ledShape: "Square",
  enableGlow: true,
});
```

## Configuration options

| Name            | Type                       | Default        | Description |
|-----------------|----------------------------|----------------|-------------|
| `border`        | `number`                   | `10`           | Margin around the generated LED grid (pixels). |
| `ledSize`       | `number`                   | `4`            | Diameter/side length of each LED (pixels). |
| `ledGap`        | `number`                   | `2`            | Spacing between LEDs (pixels). |
| `ledShape`      | `"Square" \| "Circle"`     | `"Circle"`     | Shape of individual LEDs. |
| `ledExposure`   | `number`                   | `1.0`          | Exposure (gamma) applied to LED brightness. |
| `enableGlow`    | `boolean`                  | `true`         | Enable glow/halo effect around LEDs. |
| `glowRange`     | `number`                   | `3.0`          | Radius (in LED units) of the glow effect. |
| `glowStrength`  | `number`                   | `1.75`         | Intensity of the glow effect. |
| `glowExposure`  | `number`                   | `1.0`          | Exposure (gamma) for the glow layer. |
| `offLightColor` | `[number, number, number]` | `[50, 50, 50]` | Minimum RGB color used when an LED is off (dark baseline). |

## License
MIT
