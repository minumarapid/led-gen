import wasmInit, {generate_led_image_wasm, type InitInput, type LedConfig,} from "../../src/wasm/pkg";

export type { LedConfig, InitInput };
export type { LedShape } from "../../src/wasm/pkg/";

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

    const { width, height, data } = imageData;

    const rawPixels = new Uint8Array(data.buffer);

    const result = generate_led_image_wasm(rawPixels, width, height, config);

    try {
      return new ImageData(
        new Uint8ClampedArray(result.data.buffer as ArrayBuffer),
        result.width,
        result.height
      );
    } finally {
      result.free();
    }
  }
}