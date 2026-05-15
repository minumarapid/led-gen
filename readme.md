# led-image-gen-rs
このライブラリは現在実験目的でのみ使用されます。

通常使用では[led-image-gen](https://github.com/minumarapid/led-image-gen)の使用を検討してください。

## CLI

### 使い方

```bash
cargo run -p cli -- ./test/input-0001.png
cargo run -p cli -- ./test/input-0001.png -o ./test/output.png
cargo run -p cli -- ./test/input-0001.png --led-size 6 --led-gap 3 --format png
```

### 主なオプション

- `-o, --output <PATH>`: 出力パス（未指定なら `<入力名>_led.<拡張子>`）
- `-f, --format <png|jpeg|bmp|gif|tiff|webp>`: 出力フォーマット
- `--border <PX>`: 余白
- `--led-size <PX>` / `--led-gap <PX>` / `--led-shape <square|circle>`
- `--led-exposure <FLOAT>` / `--glow-range <FLOAT>` / `--glow-strength <FLOAT>`
- `--no-glow`: グロー効果無効
- `--off-light-color <R,G,B>`: 消灯時の最低輝度

