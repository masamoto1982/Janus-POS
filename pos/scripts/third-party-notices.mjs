#!/usr/bin/env node
// 公開するビルド（dist）に入る第三者のコードのライセンス表記を web/public/THIRD-PARTY-NOTICES.txt に書き出す。
//   node scripts/third-party-notices.mjs          … 書き出す
//   node scripts/third-party-notices.mjs --check  … 書き出し済みのファイルと食い違えば失敗（CI 用）
//
// 対象は dist に実際に入るものだけ：
//   - wasm に組み込まれる Rust クレート（wasm32 向けの通常依存、proc-macro とビルド時依存は除く）
//   - Rust 標準ライブラリ（core / alloc / std）
//   - Vite がバンドルに差し込むコード（modulepreload のポリフィル）
// devDependencies（TypeScript・Vitest など）は配布物に入らないので載せない。
//
// 二者択一のライセンス（MIT OR Apache-2.0 など）は MIT を選ぶ。MIT を選べないクレートが入ってきたら
// 失敗させ、人が確かめるまで先へ進めない（コピーレフトの混入をここで止める）。

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const posRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const coreDir = join(posRoot, "core");
const outFile = join(posRoot, "web", "public", "THIRD-PARTY-NOTICES.txt");

const MIT_TEXT = `Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;

function cargo(args) {
  return execFileSync("cargo", args, { cwd: coreDir, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
}

// wasm に入るクレートの名前
const tree = cargo([
  "tree", "-q", "-e", "normal,no-proc-macro", "--target", "wasm32-unknown-unknown",
  "--features", "wasm", "--prefix", "none", "-f", "{p}",
]);
const names = new Set(
  tree.split("\n").map((l) => l.trim().split(" ")[0]).filter((n) => n && n !== "janus-pos-core"),
);

const meta = JSON.parse(cargo(["metadata", "-q", "--format-version", "1", "--features", "wasm"]));
const crates = meta.packages
  .filter((p) => names.has(p.name) && p.source)
  .sort((a, b) => a.name.localeCompare(b.name));

function licenseFiles(pkg) {
  const dir = dirname(pkg.manifest_path);
  const spdx = (pkg.license ?? "").replace("/", " OR ");
  if (!/\bMIT\b/.test(spdx)) {
    throw new Error(`${pkg.name}: MIT を選べないライセンス「${pkg.license}」。配布してよいか確かめてから、このスクリプトに扱いを足してください`);
  }
  const files = [];
  const mit = ["LICENSE-MIT", "LICENSE-MIT.md", "LICENSE", "LICENSE.md"].find((f) => existsSync(join(dir, f)));
  if (!mit) throw new Error(`${pkg.name}: MIT のライセンス文が見つかりません（${dir}）`);
  files.push(mit);
  // MIT と組み合わせて必ず従う追加のライセンス（例：unicode-ident の Unicode-3.0）
  if (/\bAND\b/.test(spdx)) {
    for (const f of ["LICENSE-UNICODE"]) if (existsSync(join(dir, f))) files.push(f);
  }
  return files.map((f) => readFileSync(join(dir, f), "utf8").trim());
}

const sections = [];
for (const pkg of crates) {
  const repo = pkg.repository ? ` (${pkg.repository})` : "";
  sections.push(`${pkg.name}${repo}\nLicense: ${pkg.license}${/\bOR\b|\//.test(pkg.license) ? "（MIT を選択）" : ""}\n\n${licenseFiles(pkg).join("\n\n")}`);
}

sections.push(`Rust standard library (core, alloc, std) (https://github.com/rust-lang/rust)
License: MIT OR Apache-2.0（MIT を選択）

Copyright (c) The Rust Project Contributors

${MIT_TEXT}`);

const viteLicense = readFileSync(join(posRoot, "node_modules", "vite", "LICENSE.md"), "utf8");
const viteCopyright = viteLicense.match(/^Copyright .*$/m)?.[0];
if (!viteCopyright) throw new Error("vite: LICENSE.md に Copyright 行が見つかりません");
sections.push(`Vite (https://github.com/vitejs/vite) … バンドルに差し込まれる modulepreload ポリフィル
License: MIT

${viteCopyright}

${MIT_TEXT}`);

const rule = "-".repeat(78);
const text = `Janus POS — Third-party notices

この Web アプリ（JavaScript と WebAssembly）には、次の第三者のソフトウェアが含まれています。
This web app (JavaScript and WebAssembly) includes the following third-party software.
Janus POS 自身のライセンスは MIT（https://github.com/masamoto1982/Janus-POS/blob/main/LICENSE）。

このファイルは pos/scripts/third-party-notices.mjs が生成します。手で編集しないでください。

${sections.map((s) => `${rule}\n${s}\n`).join("\n")}`;

if (process.argv.includes("--check")) {
  const current = existsSync(outFile) ? readFileSync(outFile, "utf8") : "";
  if (current !== text) {
    console.error("THIRD-PARTY-NOTICES.txt が依存関係と食い違っています。cd pos && npm run gen:notices で作り直してください");
    process.exit(1);
  }
  console.log(`third-party-notices: OK（${crates.length} crates + Rust std + Vite）`);
} else {
  writeFileSync(outFile, text);
  console.log(`third-party-notices: wrote ${outFile}（${crates.length} crates + Rust std + Vite）`);
}
