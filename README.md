# Janus POS — 実行できる要件定義

コンビニエンスストアのセミセルフ式 POS レジの要件を、動く画面・業務ルール・それを固定するシナリオテスト・テストと一致する仕様書の四つで示す。POS ベンダーは次の順に読む。

1. 仕様書 [docs/spec.md](docs/spec.md) … 何をするか（事実の表はコードから生成）
2. シナリオテスト [pos/core/tests/scenarios.rs](pos/core/tests/scenarios.rs) … 受け入れ試験（一覧は仕様書の付録 A）
3. シミュレーター（`npm run dev`、または GitHub Pages） … 実際の画面と操作

- `pos/core` … Rust で書いた決定的なコア（wasm-bindgen で Web UI に公開）
- `pos/web` … TypeScript + Vite の Web UI（店員画面・客側画面・呼び鈴）

変更の経緯は [CHANGELOG.md](CHANGELOG.md)。

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
