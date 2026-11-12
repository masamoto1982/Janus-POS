# Janus POS

コンビニエンスストアにおけるセルフレジ運用の理想形を追求するPOSソリューションシミュレータ。

- `pos/core` … Rust で書いた決定的なコア（wasm-bindgen で Web UI に公開）
- `pos/web` … TypeScript + Vite の Web UI（店員画面・客側画面・呼び鈴）

仕様は [docs/spec.md](docs/spec.md)。

## 開発

```sh
bash scripts/install-wasm-pack.sh   # 初回のみ
cd pos
npm install
npm run build:wasm   # Rust コアを wasm にビルド
npm run dev          # 開発サーバー
npm test             # Vitest
(cd core && cargo test)
npm run gen:types    # Rust の型を変えたら web/src/types.ts を作り直す
```

## ライセンス

MIT（[LICENSE](LICENSE)）
