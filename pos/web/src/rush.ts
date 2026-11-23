// 混雑シミュレーション（仕様 15.10）：お客様とスタッフを自動で動かし、レジのハングアップ（立ち往生）が起きないかを確かめる
import type { RushIncident, RushParams, RushView } from "./types";
import { esc } from "./view";

/** 「画面で再生」は店を作り直して読み直すので、条件をここに預けて起動時に始める */
export const RUSH_PENDING_KEY = "janus-pos-rush";
export const RUSH_SPEEDS = [1, 10, 30, 60] as const;
/** 「まとめて試す」の回数（乱数の種を 1 つずつ変える） */
export const RUSH_BATCH = 20;
/** 立ち往生として数える長さ。これより短いものは、スタッフが離れた直後の行き違いとして数えない */
export const STRANDED_MIN_MS = 10_000;

export const DEFAULT_RUSH: RushParams = {
  registers: 3,
  staff: 2,
  customers: 60,
  minutes: 30,
  spend: 600,
  drink_only_pct: 30,
  cash_pct: 20,
  trouble_pct: 10,
  trouble_secs: 180,
  open_at_queue: 2,
  pre_login: true,
  touch_secs: 20,
  patience_secs: 600,
  seed: 1,
};

type NumKey = Exclude<keyof RushParams, "pre_login">;
const FIELDS: [NumKey, string, number, number][] = [
  ["registers", "レジ台数（台）", 2, 5],
  ["staff", "スタッフ（人）", 0, 3],
  ["customers", "客数（人）", 1, 1000],
  ["minutes", "来店の時間帯（分）", 1, 240],
  ["spend", "客単価（円）", 100, 10000],
  ["drink_only_pct", "飲み物ひとつの客（%）", 0, 100],
  ["cash_pct", "現金で払う客（%）", 0, 100],
  ["trouble_pct", "手のかかる客（%）", 0, 100],
  ["trouble_secs", "手のかかる客の対応（秒）", 0, 3600],
  ["open_at_queue", "有人レジを開ける列（人、0＝開けない）", 0, 50],
  ["touch_secs", "困った客が画面に触れる間隔（秒、0＝触れない）", 0, 600],
  ["patience_secs", "客が待てる時間（秒）", 30, 7200],
  ["seed", "乱数の種", 1, 99999],
];

export function rushForm(p: RushParams): string {
  const fields = FIELDS.map(
    ([k, label, min, max]) =>
      `<label class="rush-field"><span>${label}</span><input type="number" data-rush-key="${k}" min="${min}" max="${max}" value="${p[k]}" /></label>`,
  ).join("");
  return `<div class="rush-form">${fields}
    <label class="rush-field rush-check"><input type="checkbox" data-rush-key="pre_login"${p.pre_login ? " checked" : ""} /><span>有人レジはお客様が来る前にログインして待つ</span></label>
  </div>`;
}

export function readRushForm(root: ParentNode): RushParams {
  const p: RushParams = { ...DEFAULT_RUSH };
  root.querySelectorAll<HTMLInputElement>("[data-rush-key]").forEach((i) => {
    const k = i.dataset.rushKey as keyof RushParams;
    if (k === "pre_login") p.pre_login = i.checked;
    else {
      const f = FIELDS.find((x) => x[0] === k)!;
      const n = Math.round(Number(i.value));
      p[k] = Number.isFinite(n) ? Math.min(f[3], Math.max(f[2], n)) : DEFAULT_RUSH[k];
    }
  });
  return p;
}

export const mmss = (ms: number): string => {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

const counted = (v: RushView): RushIncident[] => v.incidents.filter((i) => i.duration_ms >= STRANDED_MIN_MS || i.ended_by === null);

function incidentRows(list: RushIncident[]): string {
  if (!list.length) return `<p class="rush-ok">立ち往生はありませんでした</p>`;
  return `<table class="rush-table"><thead><tr><th>時刻</th><th>レジ</th><th>お客様</th><th>長さ</th><th>時計が戻った回数</th><th>原因</th><th>終わり方</th></tr></thead><tbody>${list
    .map(
      (i) =>
        `<tr><td>${mmss(i.start_ms)}</td><td>${i.register}</td><td>客#${i.customer}（${esc(i.kind)}）</td><td class="num">${mmss(i.duration_ms)}</td><td class="num">${i.resets}</td><td>${esc(i.cause)}</td><td>${esc(i.ended_by ?? "続いている")}</td></tr>`,
    )
    .join("")}</tbody></table>`;
}

function summary(v: RushView): string {
  const list = counted(v);
  const max = Math.max(0, ...list.map((i) => i.duration_ms));
  return `<dl class="rush-sum">
    <div><dt>経過</dt><dd>${mmss(v.elapsed_ms)}</dd></div>
    <div><dt>来店</dt><dd>${v.arrived}／${v.params.customers}人</dd></div>
    <div><dt>お買い物を終えた</dt><dd>${v.served}人</dd></div>
    <div><dt>諦めて帰った</dt><dd class="${v.gave_up ? "bad" : ""}">${v.gave_up}人</dd></div>
    <div><dt>列</dt><dd>${v.queue}人（最大 ${v.max_queue}）</dd></div>
    <div><dt>来店から帰るまで</dt><dd>平均 ${mmss(v.avg_wait_ms)}・最長 ${mmss(v.max_wait_ms)}</dd></div>
    <div><dt>客単価</dt><dd>${v.avg_spend.toLocaleString()}円</dd></div>
    <div><dt>立ち往生</dt><dd class="${list.length ? "bad" : ""}">${list.length}件・最長 ${mmss(max)}</dd></div>
  </dl>`;
}

/** 結果（一気に流したとき・再生が終わったとき） */
export function rushResult(v: RushView): string {
  return `<h3>結果</h3>${summary(v)}<h3>立ち往生（スタッフ操作モードのレジで、スタッフも呼び出しもないまま待たされた）</h3>${incidentRows(counted(v))}`;
}

/** まとめて試した結果 */
export function rushBatch(views: RushView[]): string {
  const runs = views.length;
  const hit = views.filter((v) => counted(v).length > 0).length;
  const all = views.flatMap(counted);
  const max = Math.max(0, ...all.map((i) => i.duration_ms));
  const avg = all.length ? all.reduce((s, i) => s + i.duration_ms, 0) / all.length : 0;
  const gave = views.reduce((s, v) => s + v.gave_up, 0);
  const worst = all.slice().sort((a, b) => b.duration_ms - a.duration_ms).slice(0, 5);
  return `<h3>まとめて試した結果（乱数の種 ${views[0].params.seed}〜${views[0].params.seed + runs - 1}）</h3>
    <dl class="rush-sum">
      <div><dt>立ち往生が起きた回</dt><dd class="${hit ? "bad" : ""}">${hit}／${runs}回</dd></div>
      <div><dt>立ち往生の件数</dt><dd>${all.length}件（平均 ${mmss(avg)}・最長 ${mmss(max)}）</dd></div>
      <div><dt>諦めて帰った客</dt><dd>${gave}人（${runs}回の合計）</dd></div>
    </dl>
    <h4>長かった立ち往生</h4>${incidentRows(worst)}`;
}

/** 再生中の帯（ヘッダーの下） */
export function rushPanel(v: RushView | null, speed: number, paused: boolean): string {
  if (!v) return "";
  const speeds = RUSH_SPEEDS.map(
    (s) => `<button type="button" class="neg small rush-btn${s === speed ? " on" : ""}" data-rush-speed="${s}">${s}×</button>`,
  ).join("");
  const ctl = v.finished
    ? `<button type="button" class="neg small rush-btn" data-rush-close>閉じる</button>`
    : `${speeds}<button type="button" class="neg small rush-btn" data-rush-pause>${paused ? "再開" : "一時停止"}</button><button type="button" class="neg small rush-btn" data-rush-close>終了</button>`;
  const staff = v.staff.length
    ? v.staff.map((s) => `<li><b>${esc(s.name)}</b>：${esc(s.doing)}${s.post ? `（持ち場 レジ${s.post}）` : ""}</li>`).join("")
    : "<li>スタッフなし</li>";
  const log = v.log
    .slice(-8)
    .reverse()
    .map((l) => `<li class="${l.text.startsWith("⚠") ? "bad" : ""}"><span class="t">${mmss(l.at_ms)}</span>${esc(l.text)}</li>`)
    .join("");
  return `<section class="rush-panel" aria-label="混雑シミュレーション">
    <div class="rush-head"><h2>混雑シミュレーション${v.finished ? "（終了）" : paused ? "（一時停止中）" : ""}</h2><div class="rush-ctl">${ctl}</div></div>
    ${summary(v)}
    <div class="rush-cols"><ul class="rush-staff">${staff}</ul><ol class="rush-log">${log}</ol></div>
    ${v.finished ? `<h3>立ち往生</h3>${incidentRows(counted(v))}` : ""}
  </section>`;
}

/** レジの見出しに出す、いまそのレジにいるお客様 */
export function rushBadge(v: RushView | null, id: number): string {
  const r = v?.registers.find((x) => x.register === id);
  if (!r || r.customer === null) return "";
  // 見出しの下の「売り場側」の右に収まるよう、お客様の様子（点数・現金など）はカーソルを重ねたときだけ出す
  const who = `客#${r.customer} ${mmss(r.waited_ms)}`;
  const tip = `title="${esc(r.kind)}"`;
  return r.stranded ? `<span class="rush-badge bad" ${tip}>⚠ 立ち往生 ${who}</span>` : `<span class="rush-badge" ${tip}>${who}</span>`;
}
