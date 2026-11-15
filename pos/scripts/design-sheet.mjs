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

// 濃度の役割（user 2026-11-15）：4 濃＝アクセント（枠線・文字だけ、広い面は塗らない）、3＝添え字だけ、1・2＝塗りつぶしだけ（文字に使わない）
const ROLE = {
  1: "塗りつぶし（文字に使わない）",
  2: "塗りつぶし（文字に使わない）",
  3: "添え字（小テキスト）だけ",
  4: "アクセント（文字・枠線）",
};

const rows = [];
for (const [h, label] of HUES) {
  for (let d = 1; d <= 4; d++) {
    const fg = color(h, d);
    const light = worst(fg, BG_LIGHT);
    const deep = worst(fg, BG_DEEP);
    rows.push({ h, label, d, fg, hex: hex(fg), oklch: token(`${h}-${d}`), white: contrast(fg, WHITE), light, deep, gl: grade(light), gd: grade(deep) });
  }
}
const ink4 = (h) => rows.find((r) => r.h === h && r.d === 4);

// ---- SVG：行＝色×濃度。各行には、その濃度の役割どおりの見本だけを置く ----
const W = 930;
const ROW_H = { 1: 64, 2: 64, 3: 64, 4: 120 };
const TOP = 78;
const H = TOP + HUES.length * (ROW_H[1] + ROW_H[2] + ROW_H[3] + ROW_H[4]) + 20;
const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
const X = 250;
let svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" font-family="'Hiragino Sans','Noto Sans JP',sans-serif">
<rect width="${W}" height="${H}" fill="#ffffff"/>
<text x="16" y="28" font-size="16" font-weight="700" fill="#4a4a4a">Janus-POS 色 4（＋無彩色）× 濃度 4 の役割と、文字サイズ 4 の見本（白地）</text>
<text x="16" y="50" font-size="12" fill="#4a4a4a">濃度1・2＝塗りつぶし　濃度3＝添え字　濃度4＝アクセント（文字・枠線）。✕＝使ってはいけない（白・濃度1の背景で 4.5:1 未満の添え字、3:1 未満の文字）</text>
<text x="16" y="68" font-size="12" fill="#4a4a4a">見本の文字サイズ：特大 96px・強調 28px・大 18px・小 13px</text>
`;
let y = TOP;
for (const r of rows) {
  const rh = ROW_H[r.d];
  if (r.d === 1) svg += `<line x1="16" y1="${y}" x2="${W - 16}" y2="${y}" stroke="#d4d4d4"/>\n`;
  svg += `<rect x="16" y="${y + 12}" width="28" height="28" fill="${r.hex}" stroke="#d4d4d4"/>
<text x="54" y="${y + 24}" font-size="12" font-weight="700" fill="#333">${esc(r.label)}　濃度${DENSITY[r.d - 1]}</text>
<text x="54" y="${y + 40}" font-size="11" fill="#555">--${r.h}-${r.d}　白に ${r.white.toFixed(2)}:1</text>
<text x="54" y="${y + 56}" font-size="11" fill="#555">${esc(ROLE[r.d])}</text>
`;
  if (r.d <= 2) {
    // 塗りつぶしの見本：濃度1・2の面に、同じ色の濃度4の文字と枠線を載せる
    const k = ink4(r.h);
    svg += `<rect x="${X}" y="${y + 10}" width="${W - X - 16}" height="${rh - 20}" fill="${r.hex}"/>
<text x="${X + 12}" y="${y + rh / 2 + 6}" font-size="18" font-weight="700" fill="${k.hex}">塗りつぶしの上の文字は濃度4（${contrast(k.fg, r.fg).toFixed(2)}:1）</text>
<rect x="${W - 16 - 128}" y="${y + 18}" width="112" height="${rh - 36}" rx="4" fill="none" stroke="${k.hex}" stroke-width="1.5"/><text x="${W - 16 - 72}" y="${y + rh / 2 + 5}" font-size="13" text-anchor="middle" fill="${k.hex}">枠線は濃度4</text>
`;
  } else if (r.d === 3) {
    const ok = r.light >= 4.5;
    svg += `<text x="${X}" y="${y + 30}" font-size="18" font-weight="700" fill="${ink4(r.h).hex}">サンドイッチ</text>
<text x="${X}" y="${y + 50}" font-size="13" fill="${r.hex}">320円　販売 11/15 19:00</text>
`;
    if (!ok) svg += `<line x1="${X - 4}" y1="${y + 36}" x2="${X + 190}" y2="${y + 56}" stroke="#d00" stroke-width="2"/><text x="${X + 200}" y="${y + 52}" font-size="13" font-weight="700" fill="#d00">✕ 添え字に足りない（${r.light.toFixed(2)}:1）→ グレー3を使う</text>\n`;
    else svg += `<text x="${X + 200}" y="${y + 52}" font-size="13" fill="#555">○ 白・濃度1の上で ${r.light.toFixed(2)}:1${r.deep < 4.5 ? `（濃度2の上は ${r.deep.toFixed(2)}:1 で ✕）` : ""}</text>\n`;
  } else {
    svg += `<text x="${X}" y="${y + 96}" font-size="96" font-weight="800" fill="${r.hex}">安全</text>
<text x="${X + 220}" y="${y + 70}" font-size="28" font-weight="700" fill="${r.hex}">1,280</text>
<text x="${X + 340}" y="${y + 68}" font-size="18" font-weight="700" fill="${r.hex}">お会計</text>
<text x="${X + 420}" y="${y + 66}" font-size="13" fill="${r.hex}">お会計</text>
<rect x="${X + 490}" y="${y + 40}" width="116" height="40" rx="4" fill="none" stroke="${r.hex}" stroke-width="1.5"/><text x="${X + 548}" y="${y + 65}" font-size="13" font-weight="700" text-anchor="middle" fill="${r.hex}">枠線</text>
`;
  }
  y += rh;
}
svg += "</svg>\n";
writeFileSync(join(root, "docs/design-language.svg"), svg);

// ---- Markdown ----
const f = (c) => `${c.toFixed(2)}`;
const name = (r) => `${r.label.replace(/（.*）/, "")}${r.d}（\`--${r.h}-${r.d}\`）`;
const sub = rows.filter((r) => r.d === 3);
const subOk = sub.filter((r) => r.light >= 4.5);
const subNg = sub.filter((r) => r.light < 4.5);
const accDeep = rows.filter((r) => r.d === 4 && r.deep < 4.5);
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

## 濃度の役割

| 濃度 | 役割 | 使ってはいけないこと |
| --- | --- | --- |
| 4 濃 | アクセントカラー。文字の色と枠線の色 | 広い面の塗りつぶし |
| 3 中 | 添え字（小テキストの補足：値段・期限・案内の二行目など） | 本文の文字、塗りつぶし |
| 2 薄 | 塗りつぶし（帯・最下段など濃いめの段、行の色） | 文字、枠線 |
| 1 淡 | 塗りつぶし（一段目・三段目など明るい段、確認画面） | 文字、枠線 |

塗りつぶしの上に載せる文字と枠線は濃度4。濃度4の文字は、どの大きさでも、白・濃度1・濃度2のどの背景でも読める。

## 見本

![色・濃度の役割と文字サイズの見本](design-language.svg)

## 使ってはいけない組み合わせ

文字の色と、その文字が載る背景とのコントラスト比で決める。小テキスト（添え字を含む）は 4.5:1 以上、特大・強調は 3:1 以上がいる。

- 濃度1・2は文字に使わない（背景に対して ${f(Math.min(...rows.filter((r) => r.d <= 2).map((r) => r.light)))}〜${f(Math.max(...rows.filter((r) => r.d <= 2).map((r) => r.white)))}:1）。
- 添え字に使えるのは ${subOk.map(name).join("、")}（白・濃度1の上）。${subNg.map(name).join("、")} は添え字に足りないので、文字には使わない。色の意味は濃度4の文字・枠線で示し、添え字はグレー3にする。
${subOk.filter((r) => r.deep < 4.5).length ? `- 濃度2の背景（帯・最下段）の上では、${subOk.filter((r) => r.deep < 4.5).map((r) => `${name(r)}も ${f(r.deep)}:1`).join("、")} で添え字に足りない。帯と最下段に添え字を置くときの扱いは未決。\n` : ""}${accDeep.length ? `- 濃度4でも濃度2の背景で 4.5:1 に届かないもの：${accDeep.map(name).join("、")}。\n` : ""}
## 面の塗り分け（設計メモ）

隣り合う面は違う色で塗る。いまのレジ画面は段が縦に積み重なっているだけなので、隣り合うのは上下の段だけで、濃度1と2を交互に塗れば足りる（一段目・三段目＝淡、帯・最下段＝薄）。画面どうしは、操作できる側（緑）とできない側（赤）の色相で分ける。

ある領域を奇数個の領域が囲む配置ができたら、2〜3色では塗り分けられない。そのときは四色定理（平面の地図は 4 色あれば隣り合う領域を必ず塗り分けられる）を応用する。色相には意味があるので、塗り分けのためだけに色相を使わず、明るさの段で分けることを先に考える。

## コントラスト比の表

背景は、白と濃度1（グレー・緑・赤・橙の 1）のうち一番低いもの、濃度2（同じく 2）のうち一番低いもの。○ 4.5:1 以上、△ 3:1 以上（特大・強調だけ）、✕ 3:1 未満。

| 色 | 役割 | OKLCH | 白 | 白・濃度1 | 濃度2 |
| --- | --- | --- | --- | --- | --- |
${rows.map((r) => `| ${name(r)} | ${ROLE[r.d]} | \`${r.oklch}\` | ${f(r.white)} | ${f(r.light)} ${r.gl} | ${f(r.deep)} ${r.gd} |`).join("\n")}
`;
writeFileSync(join(root, "docs/design-language.md"), md);
console.log(`design-sheet: 添え字に使える ${subOk.map((r) => r.h).join(",")}、使えない ${subNg.map((r) => r.h).join(",")}`);
