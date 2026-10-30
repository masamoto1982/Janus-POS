/**
 * 3-screens.ts のデータから、読むための表 3-screens.md をつくる。
 * 書き直すのは pos の `npx vitest run -u`（spec.test.ts が呼ぶ）
 */
import { ISSUES, SCENES, type Button, type Face, type Scene } from "./3-screens";

const cell = (xs: string[] | undefined): string => (xs?.length ? xs.map((x) => x.replace(/\|/g, "｜")).join("<br>") : "—");

const button = (b: Button): string => `［${b.tone}］${b.label}${b.note ? `（${b.note}）` : ""}`;

/** 一つの画面の一列。mirror / register は中身の代わりに一言で書く */
function column(f: Face): Record<string, string> {
  if (f.mirror) {
    const note = "反対側と同じものを押せない形で映す";
    return { strong: note, weak: "", buttons: cell(f.buttons?.map(button)), parts: cell(f.parts) };
  }
  if (f.register) return { strong: "登録画面のまま（本体を置き換えない）", weak: "", buttons: "", parts: "" };
  return { strong: cell(f.strong), weak: cell(f.weak), buttons: cell(f.buttons?.map(button)), parts: cell(f.parts) };
}

function scene(s: Scene): string {
  const st = column(s.staff);
  const cu = column(s.customer);
  const rows: [string, string][] = [
    ["積極テキスト", "strong"],
    ["消極テキスト", "weak"],
    ["ボタン（左 → 右）", "buttons"],
    ["そのほかの部品", "parts"],
  ];
  const body = rows
    .filter(([, k]) => st[k] || cu[k])
    .filter(([, k]) => !(st[k] === "—" && cu[k] === "—"))
    .map(([label, k]) => `| ${label} | ${st[k] || "—"} | ${cu[k] || "—"} |`)
    .join("\n");
  const meta = [
    `- 出る条件：${s.when}`,
    `- 根拠：${s.rules.length ? s.rules.join("・") : "—"}　描画：\`${s.source}\``,
    ...(s.issues?.length ? [`- 判断が要る点：${s.issues.join("・")}`] : []),
  ].join("\n");
  return `### ${s.id}　${s.title}（${s.mode}）\n\n${meta}\n\n| | 店員側 | 顧客側 |\n| --- | --- | --- |\n${body}\n`;
}

export function renderScreens(): string {
  const groups = [...new Set(SCENES.map((s) => s.group))];
  const index = SCENES.map((s) => `| ${s.id} | ${s.mode} | ${s.title} | ${s.issues?.join("・") ?? ""} |`).join("\n");
  const sections = groups.map((g) => `## ${g}\n\n${SCENES.filter((s) => s.group === g).map(scene).join("\n")}`).join("\n");
  const issues = ISSUES.map((q) => {
    const where = SCENES.filter((s) => s.issues?.includes(q.id)).map((s) => s.id);
    return `| ${q.id} | ${q.text} | ${q.rules.join("・") || "—"} | ${where.join("・")} |`;
  }).join("\n");
  return `<!-- このファイルは 3-screens.ts から生成する。直接編集しない（pos で npx vitest run -u） -->

# 第三階層　店員側と顧客側の対表

場面ごとに、店員側と顧客側の積極テキスト・消極テキスト・ボタンを並べる。〈 〉は値が入るところ。根拠の番号は [2-principles.md](2-principles.md) のルール。

## 場面の一覧

| 場面 | モード | 名前 | 判断が要る点 |
| --- | --- | --- | --- |
${index}

${sections}
## 判断が要る点

現状のコードを棚卸しして見つかった点。決まったら、データとコードを直してこの一覧から消す。

| 番号 | 内容 | ルール | 場面 |
| --- | --- | --- | --- |
${issues}
`;
}
