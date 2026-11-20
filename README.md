# Janus POS — 実行できる要件定義

コンビニエンスストアのセミセルフ式 POS レジの要件を、動く画面・業務ルール・それを固定するシナリオテスト・テストと一致する仕様書の四つで示す。POS ベンダーは次の順に読む。

1. 仕様書 [docs/spec.md](docs/spec.md) … 何をするか（事実の表はコードから生成）
2. シナリオテスト [pos/core/tests/scenarios.rs](pos/core/tests/scenarios.rs) … 受け入れ試験（一覧は仕様書の付録 A）
3. シミュレーター（`npm run dev`、または GitHub Pages） … 実際の画面と操作

- `pos/core` … Rust で書いた決定的なコア（wasm-bindgen で Web UI に公開）
- `pos/web` … TypeScript + Vite の Web UI（レジカウンター側・売り場側の画面、呼び鈴）

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

## 謝辞

Janus-POS は、先人の研究と多くの人の仕事の上に成り立っています。

- **色**：四つの色（赤 #FF4B00・青 #005AFF・緑 #03AF7A・橙 #F6AA00）は、NPO法人カラーユニバーサルデザイン機構（CUDO）の「カラーユニバーサルデザイン推奨配色セット」をもとに、OKLCH で定義し直したものです。色の見え方の多様性に向き合い、誰にとっても見分けやすい配色を研究して公開してくださった皆さまに、深く感謝します。この配色セットに出会うきっかけをくださった記事「[見やすいグラフの色分けをまとめました（カラーコード）](https://contents-open.hatenablog.com/entry/2021/08/19/231157)」の筆者にも感謝します。
- **オープンソース**：Rust、Serde、wasm-bindgen、TypeScript、Vite、Vitest をはじめ、Janus-POS を支えるオープンソースソフトウェアの作者と貢献者の皆さまに感謝します（ライセンス表記は下記）。
- **生成 AI**：Janus-POS の設計と実装は Anthropic の Claude とともに進め、仮のロゴは Google の Gemini で作りました。Anthropic、Google をはじめ、生成 AI をつくり育ててきたすべての皆さんへ。

ありがとう。

## ライセンス

MIT（[LICENSE](LICENSE)）

公開するビルドに入る第三者のソフトウェア（wasm に組み込まれる Rust クレート、Rust 標準ライブラリ、Vite が差し込むコード）のライセンス表記は [pos/web/public/THIRD-PARTY-NOTICES.txt](pos/web/public/THIRD-PARTY-NOTICES.txt)。ビルドすると `dist/THIRD-PARTY-NOTICES.txt` として一緒に配布される。依存関係を変えたら `cd pos && npm run gen:notices` で作り直す（CI が食い違いを止める）。
