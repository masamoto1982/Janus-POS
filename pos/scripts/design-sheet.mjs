// デザイン言語の一覧表を作る：styles.css の色トークン（4色＋無彩色 × 濃度4）と文字サイズ4段を読み、
// docs/design-language.svg（見本）と docs/design-language.md（コントラスト比と使ってはいけない組み合わせ）を書き出す。
// 実行：cd pos && npm run gen:design
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const css = readFileSync(join(root, "pos/web/src/styles.css"), "utf8");

const HUES = [
  ["grey", "グレー（無彩色）"],
  ["blue", "青（操作可能）"],
  ["green", "緑（安全・肯定）"],
  ["red", "赤（危険・否定）"],
  ["orange", "橙（着目・注意喚起）"],
];
const DENSITY = ["1 淡", "2 薄", "3 中", "4 濃"];
const SIZES = [
  ["特大", "--text-xl", 96, 800],
  ["強調", "--text-num", 28, 700],
  ["大", "--text-lg", 18, 700],
  ["小", "--text-sm", 13, 400],
];

const token = (name) => {
  const m = css.match(new RegExp(`--${name}:\\s*(oklch\\([^)]*\\))`));
  if (!m) throw new Error(`styles.css に --${name} がない`);
  return m[1];
};
for (const [, varName, px] of SIZES) {
  const m = css.match(new RegExp(`${varName}:\\s*([^;]+);`));
  if (!m || !m[1].includes(`${px}px`)) throw new Error(`${varName} が ${px}px ではない（${m?.[1]}）。SIZES を直す`);
}

const toRgb = (oklch) => {
  const [L, C, H] = oklch.match(/oklch\(([^)]*)\)/)[1].trim().split(/\s+/).map(Number);
  const a = C * Math.cos((H * Math.PI) / 180);
  const b = C * Math.sin((H * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ].map((v) => Math.min(1, Math.max(0, v)));
};
const hex = (lin) =>
  "#" + lin.map((c) => Math.round(255 * (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055)).toString(16).padStart(2, "0")).join("");
const lum = (lin) => 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
const contrast = (x, y) => {
  const [a, b] = [lum(x), lum(y)].sort((p, q) => q - p);
  return (a + 0.05) / (b + 0.05);
};

const WHITE = [1, 1, 1];
const color = (h, d) => toRgb(token(`${h}-${d}`));
// レジ画面で文字が載る背景：白と濃度1（一段目・三段目）、濃度2（帯・最下段）
const BG_LIGHT = [["白", WHITE], ...["grey", "green", "red", "orange"].map((h) => [`${h}-1`, color(h, 1)])];
const BG_DEEP = ["grey", "green", "red", "orange"].map((h) => [`${h}-2`, color(h, 2)]);
const worst = (fg, bgs) => Math.min(...bgs.map(([, bg]) => contrast(fg, bg)));

// 判定：3:1 未満は大きさに関わらず使わない（✕）。3:1 以上 4.5:1 未満は特大・強調だけ（△）。4.5:1 以上はすべて可（○）
const grade = (c) => (c < 3 ? "✕" : c < 4.5 ? "△" : "○");
const allowed = (g, sizeIdx) => g === "○" || (g === "△" && sizeIdx <= 1);

const rows = [];
for (const [h, label] of HUES) {
  for (let d = 1; d <= 4; d++) {
    const fg = color(h, d);
    const light = worst(fg, BG_LIGHT);
    const deep = worst(fg, BG_DEEP);
    rows.push({ h, label, d, fg, hex: hex(fg), oklch: token(`${h}-${d}`), white: contrast(fg, WHITE), light, deep, gl: grade(light), gd: grade(deep) });
  }
}

// ---- SVG：行＝色×濃度、列＝文字サイズ。白地に見本を置き、使ってはいけないものは斜線で消す ----
const COL_X = [250, 520, 660, 800];
const COL_W = [260, 130, 130, 120];
const ROW_H = 112;
const TOP = 70;
const W = 930;
const H = TOP + rows.length * ROW_H + 20;
const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
let svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" font-family="'Hiragino Sans','Noto Sans JP',sans-serif">
<rect width="${W}" height="${H}" fill="#ffffff"/>
<text x="16" y="28" font-size="16" font-weight="700" fill="#4a4a4a">Janus-POS 文字サイズ 4 × 色 4（＋無彩色）× 濃度 4　白地の見本</text>
<text x="16" y="50" font-size="12" fill="#4a4a4a">✕＝使ってはいけない（白・濃度1の背景でコントラスト 3:1 未満）　△＝特大・強調だけ（4.5:1 未満）</text>
`;
SIZES.forEach(([name, , px], i) => {
  svg += `<text x="${COL_X[i]}" y="${TOP - 4}" font-size="12" fill="#4a4a4a">${name} ${px}px</text>\n`;
});
rows.forEach((r, i) => {
  const y = TOP + i * ROW_H;
  if (r.d === 1) svg += `<line x1="16" y1="${y}" x2="${W - 16}" y2="${y}" stroke="#d4d4d4"/>\n`;
  svg += `<rect x="16" y="${y + 14}" width="28" height="28" fill="${r.hex}" stroke="#d4d4d4"/>
<text x="54" y="${y + 26}" font-size="12" font-weight="700" fill="#333">${esc(r.label)}</text>
<text x="54" y="${y + 42}" font-size="11" fill="#555">濃度${DENSITY[r.d - 1]}　--${r.h}-${r.d}</text>
<text x="54" y="${y + 58}" font-size="11" fill="#555">白に ${r.white.toFixed(2)}:1</text>
`;
  SIZES.forEach(([, , px, weight], c) => {
    const ok = allowed(r.gl, c);
    const base = y + ROW_H / 2 + px * 0.36;
    const fs = Math.min(px, 96);
    const sample = c === 0 ? "安全" : c === 1 ? "1,280" : "お会計";
    svg += `<text x="${COL_X[c]}" y="${base}" font-size="${fs}" font-weight="${weight}" fill="${r.hex}"${ok ? "" : ' opacity="0.9"'}>${sample}</text>\n`;
    if (!ok) {
      const x0 = COL_X[c] - 4, x1 = COL_X[c] + COL_W[c] - 14;
      svg += `<line x1="${x0}" y1="${y + 10}" x2="${x1}" y2="${y + ROW_H - 10}" stroke="#d00" stroke-width="2"/><text x="${x1 - 14}" y="${y + 24}" font-size="16" font-weight="700" fill="#d00">${r.gl}</text>\n`;
    }
  });
});
svg += "</svg>\n";
writeFileSync(join(root, "docs/design-language.svg"), svg);

// ---- Markdown ----
const f = (c) => `${c.toFixed(2)}`;
const forbidden = rows.filter((r) => r.gl === "✕");
const largeOnly = rows.filter((r) => r.gl === "△");
const deepWorse = rows.filter((r) => r.gl !== "✕" && r.gd !== r.gl);
const name = (r) => `${r.label.replace(/（.*）/, "")}${r.d}（\`--${r.h}-${r.d}\`）`;
let md = `# Janus-POS デザイン言語

<!-- このファイルと design-language.svg は pos/scripts/design-sheet.mjs が styles.css から作る。手で直さず \`cd pos && npm run gen:design\` -->

UI はレゴのように、少ない部品の掛け合わせで組み立てる。軸はそれぞれ独立した四つずつ。

| 軸 | 四つ |
| --- | --- |
| 文字サイズ | ${SIZES.map(([n, v, px]) => `${n} ${px === 96 ? "最大 96" : px}px（\`${v}\`）`).join("・")} |
| 色 | 青 #005AFF（操作可能）・緑 #03AF7A（安全・肯定）・赤 #FF4B00（危険・否定）・橙 #F6AA00（着目・注意喚起）。白・黒・グレーは色に数えない無彩色 |
| 濃度 | 1 淡（明るさ 0.95）・2 薄（0.87）・3 中（その色そのもの。グレーは 0.52）・4 濃（0.44） |
| ボタンの形 | 積極・消極・案内（強い丸み。円は案内の正方形）・タブ |
| ボタンの大きさ | 特大 160×64・大 116×48・中 64×34・小 56×26 |

## 文字サイズ × 色 × 濃度の見本

![文字サイズ・色・濃度の見本](design-language.svg)

## 使ってはいけない組み合わせ

文字の色と、その文字が載る背景とのコントラスト比で決める。

- ✕：コントラスト比が 3:1 未満。大きさに関わらず、文字に使ってはいけない。
- △：3:1 以上 4.5:1 未満。特大・強調（大きな文字）にだけ使える。大・小には使ってはいけない。
- ○：4.5:1 以上。すべての大きさに使える。

白と濃度1の背景（一段目・三段目、確認画面）の上で：

- ✕ 文字に使わない：${forbidden.map(name).join("、")}。淡・薄は背景の色で、文字の色ではない。
- △ 特大・強調だけ：${largeOnly.map(name).join("、") || "なし"}。
${deepWorse.length ? `
濃度2の背景（帯・最下段）の上では、さらに次の判定が下がる。帯と最下段に置く文字は濃度4（濃）を基本にする。

${deepWorse.map((r) => `- ${name(r)}：${r.gl} → ${r.gd}（濃度2の背景で ${f(r.deep)}:1）`).join("\n")}
` : ""}
## コントラスト比の表

背景は、白と濃度1（グレー・緑・赤・橙の 1）のうち一番低いもの、濃度2（同じく 2）のうち一番低いもの。

| 文字の色 | OKLCH | 白 | 白・濃度1 | 濃度2 |
| --- | --- | --- | --- | --- |
${rows.map((r) => `| ${name(r)} | \`${r.oklch}\` | ${f(r.white)} | ${f(r.light)} ${r.gl} | ${f(r.deep)} ${r.gd} |`).join("\n")}
`;
writeFileSync(join(root, "docs/design-language.md"), md);
console.log(`design-sheet: ✕ ${forbidden.length}、△ ${largeOnly.length}、濃度2で下がる ${deepWorse.length}`);
