# Janus POS — 実行できる要件定義

**これは店舗で動かす POS 製品ではなく、要件を固定するためのシミュレーターです。** 実機の周辺機器・決済ネットワークにはつながず、店舗の状態はブラウザの中だけにある。

コンビニエンスストアのセミセルフ式 POS レジの要件を、動く画面・業務ルール・それを固定するシナリオテスト・テストと一致する仕様書の四つで示す。POS ベンダーは次の順に読む。

1. 仕様書 [docs/spec.md](docs/spec.md) … 何をするか（事実の表はコードから生成）
2. シナリオテスト [pos/core/tests/scenarios.rs](pos/core/tests/scenarios.rs) … 受け入れ試験。一覧は仕様書の [付録 A](docs/spec.md#付録-a-受け入れシナリオ一覧) で、節ごとに並び、シナリオのない節も「シナリオなし」と出るので穴が見える。ベンダーの実装は、この一覧のシナリオを満たすことを受け入れの基準にできる（`cd pos/core && cargo test --test scenarios` で受け入れ試験だけを流せる）
3. シミュレーター（[GitHub Pages](https://masamoto1982.github.io/Janus-POS/)、または `npm run dev`） … 実際の画面と操作

- `pos/core` … Rust で書いた決定的なコア（wasm-bindgen で Web UI に公開）
- `pos/web` … TypeScript + Vite の Web UI（レジカウンター側・売り場側の画面、呼び鈴）

変更の経緯は [CHANGELOG.md](CHANGELOG.md)。

## シミュレーターを触る

一画面に、左にレジ 2 台と呼び鈴（Bell）、右に売り場（Item）とお客様の財布（Wallet）が並ぶ（仕様書 15.1）。最初の一回は次の順に触ると全体がつかめる。

1. Item の商品をレジの売り場側の画面へドラッグする（売り場側のスキャナーで読む）。セルフ操作モードの買い物になる
2. 売り場側で「お会計」を押し、画面の案内に沿って Wallet の電子マネーを決済端末へドラッグする
3. レジの見出しのセレクタでスタッフを選ぶ（スタッフバーコードでログインする代わり）と、スタッフ操作モードになる。商品をレジカウンター側へドラッグし、Wallet のお金を入金ボックスへドラッグすると現金で払える

Item・Wallet のボタンにカーソルを重ねると使い方が出る。上部の「リセット」で最初の状態に戻る。

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
