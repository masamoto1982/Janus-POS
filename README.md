# Janus POS

コンビニエンスストア向けセミセルフ式 POS レジのシミュレーター。

- `pos/core` … Rust で書いた決定的なコア（wasm-bindgen で Web UI に公開）
- `pos/web` … TypeScript + Vite の Web UI（店員画面・客側画面・呼び鈴）

## 開発

```sh
bash scripts/install-wasm-pack.sh   # 初回のみ
cd pos
npm install
npm run build:wasm   # Rust コアを wasm にビルド
npm run dev          # 開発サーバー
npm test             # Vitest
(cd core && cargo test)
```

## ライセンス

MIT（[LICENSE](LICENSE)）
