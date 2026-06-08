import wasmInit, {generate_led_image_wasm, type InitInput, type LedConfig, type LedShape} from "../../src/wasm/pkg";
import { z } from 'zod';

export type { LedConfig, InitInput, LedShape };

const LedConfigSchema = z.object({
  border: z.number().int("Border must be an integer").min(0,"Border must be a non-negative integer").optional(),
  ledSize: z.number().int("LED size must be an integer").min(0,"LED size must be a non-negative integer").optional(),
  ledGap: z.number().int("LED gap must be an integer").min(0,"LED gap must be a non-negative integer").optional(),
  ledShape: z.enum(["Circle", "Square"]).optional(),
  ledExposure: z.number().min(0,"LED exposure must be a non-negative number").optional(),
  enableGlow: z.boolean().optional(),
  glowRange: z.number().min(0,"Glow range must be a non-negative number").optional(),
  glowStrength: z.number().min(0,"Glow strength must be a non-negative number").optional(),
  glowExposure: z.number().min(0,"Glow exposure must be a non-negative number").optional(),
  offLightColor: z.tuple([
    z.number()
      .int("Red must be an integer")
      .min(0,"Red must be a number between 0 and 255")
      .max(255,"Red must be a number between 0 and 255"),
    z.number()
      .int("Green must be an integer")
      .min(0,"Green must be a number between 0 and 255")
      .max(255,"Green must be a number between 0 and 255"),
    z.number()
      .int("Blue must be an integer")
      .min(0,"Blue must be a number between 0 and 255")
      .max(255,"Blue must be a number between 0 and 255")
  ]).optional(),
})

export class LedGenerator {
  private static _isInitialized = false;

  static get isInitialized() {
    return this._isInitialized;
  }

  static async initialize(module_or_path?: InitInput) {
    if (!this._isInitialized) {
      await wasmInit(module_or_path);
      this._isInitialized = true;
    }
  }

  static async processImageData(
    imageData: ImageData,
    config: LedConfig = {}
  ): Promise<ImageData> {
    // 初期化されていなければ自動で初期化
    await this.initialize();

    const ledConfigValidation = LedConfigSchema.safeParse(config);
    if (!ledConfigValidation.success) {
      const issue = ledConfigValidation.error.issues[0];
      throw new TypeError(`Invalid LedConfig at '${issue.path.join(".")}': ${issue.message}`);
    }

    const { width, height, data } = imageData;

    const rawPixels = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);

    const result = generate_led_image_wasm(rawPixels, width, height, ledConfigValidation.data);

    try {
      const output = new Uint8ClampedArray(result.data);
      return new ImageData(
        output,
        result.width,
        result.height
      );
    } finally {
      result.free();
    }
  }
}