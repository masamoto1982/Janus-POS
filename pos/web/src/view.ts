// 画面の描画。どれもスナップショットから HTML 文字列を作る純粋関数で、
// ボタンの出し分けはコアが返す actions（操作可否と理由）だけで決める。
// 画面側で「押せるかどうか」を独自に判断しない。

import type {
  ActionState,
  Command,
  CustomerView,
  FfState,
  MenuScreen,
  RegisterView,
  Snapshot,
  StaffLine,
  StaffView,
} from "./types";

export function esc(s: unknown): string {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export const yen = (n: number): string => `${n.toLocaleString("ja-JP")}円`;

const secs = (now: number, at: number): string => `${Math.max(0, Math.floor((now - at) / 1000))}秒`;

function find(actions: ActionState[], name: string): ActionState {
  return actions.find((a) => a.action === name) ?? { action: name, allowed: false, reason: null };
}

/** コマンドを持つボタン。押せないときは理由を title に出す */
export function btn(label: string, cmd: Command, a?: ActionState, cls = ""): string {
  const disabled = a && !a.allowed;
  const title = disabled && a.reason ? ` title="${esc(a.reason)}"` : "";
  return `<button class="${cls}" data-cmd="${esc(JSON.stringify(cmd))}"${disabled ? " disabled" : ""}${title}>${esc(label)}</button>`;
}

const FF_LABEL: Record<FfState, string> = {
  waiting: "お渡し前",
  received: "お渡し済",
};

/**
 * FF の明細の個数の右に置く、お渡し確認のチェックボックス。
 * 全部にチェックが入るまで会計に進めない。どちらの画面から押したかは、押した画面で決まる
 */
function ffCheck(reg: number, l: { id: number; kind: string; state?: FfState }, a: ActionState): string {
  if (l.kind !== "ff") return "";
  const title = a.allowed ? "お渡ししたらチェック" : (a.reason ?? "");
  return `<label class="ffcheck" title="${esc(title)}"><input type="checkbox" data-ffcheck="${reg}:${l.id}"${l.state === "received" ? " checked" : ""}${a.allowed ? "" : " disabled"} aria-label="お渡し確認" /></label>`;
}

const METHOD_LABEL: Record<string, string> = { cash: "現金", emoney: "電子マネー", qr: "バーコード決済", credit: "クレジットカード" };
const METHODS = ["cash", "emoney", "qr", "credit"] as const;

// ---------------------------------------------------------------------------
// レジ画面。店員側・お客様側の区別はなく、二つの画面に同じものを映す。
// 中身はモードで決まる（有人：係員の登録画面／セミセルフ：お客様の登録画面）。
// ボタンが送る side は操作側の画面（押せるのは操作側だけ）
// ---------------------------------------------------------------------------

type Side = "staff" | "customer";

/** 支払いが終わった後のお礼とおつり */
function thanks(r: RegisterView): string {
  const rc = r.last_receipt!;
  const change = rc.change.map(([d, n]) => `${d.toLocaleString()}円×${n}`).join("　");
  return `<div class="thanks">
      <div class="big">ありがとうございました</div>
      <div>お支払い ${yen(rc.total)}（${esc(rc.method)}）</div>
      ${rc.change_total ? `<div class="change">おつり <b>${yen(rc.change_total)}</b><div class="muted">${change}</div><div class="muted">紙幣と硬貨はそろえてお返しします</div></div>` : ""}
      ${rc.point_card ? `<div>ポイント ${rc.points}pt を付与しました</div>` : ""}
    </div>`;
}

/** 年齢確認ボタン（店員が「しない」を選んだとき。例外：お客様側の画面からしか押せない） */
function ageCard(reg: number, age: number, st: ActionState): string {
  return `<div class="card age-card customer-only">
      <div class="big">年齢確認</div>
      <p>年齢確認が必要な商品があります（お客様側の画面で押してください）</p>
      ${btn(`${age}歳以上です`, { type: "confirm_age", register: reg }, st, "primary big-btn")}
      ${!st.allowed && st.reason ? `<p class="muted">${esc(st.reason)}</p>` : ""}
    </div>`;
}

/** 日付を YYYY-MM-DD で（レジの日付として生年月日の判定に使う） */
export function isoDate(t: Date): string {
  return `${t.getFullYear()}-${pad(t.getMonth() + 1)}-${pad(t.getDate())}`;
}

/** 和暦（その日付の元号）。元年は「元年」と書く */
export function wareki(y: number, m: number, d: number): string {
  const ymd = y * 10000 + m * 100 + d;
  const [era, start] =
    ymd >= 20190501 ? ["令和", 2019] : ymd >= 19890108 ? ["平成", 1989] : ymd >= 19261225 ? ["昭和", 1926] : ["大正", 1912];
  const n = y - start + 1;
  return `${era}${n === 1 ? "元" : n}年`;
}

/**
 * 年確早見表。今日ちょうど age 歳になる生まれ年を中心に、前後 2 年（計 5 年）を並べる。
 * その年の今日の日付以前に生まれていれば age 歳以上
 */
export function ageChart(age: number, today: Date = new Date()): string {
  const ty = today.getFullYear();
  const m = today.getMonth() + 1;
  const d = today.getDate();
  const y0 = ty - age;
  const rows = [-2, -1, 0, 1, 2]
    .map((k) => {
      const y = y0 + k;
      const verdict =
        k < 0 ? `<span class="ok">購入できます</span>` : k === 0 ? `<b>${m}月${d}日以前の生まれなら購入できます</b>` : `<span class="warn">購入できません</span>`;
      return `<tr class="${k === 0 ? "edge" : ""}"><td>${y}年（${wareki(y, m, d)}）</td><td>${ty - y - 1}〜${ty - y}歳</td><td>${verdict}</td></tr>`;
    })
    .join("");
  return `<table class="age-chart">
      <caption>年確早見表（${ty}年${m}月${d}日：${y0}年（${wareki(y0, m, d)}）${m}月${d}日以前の生まれが${age}歳以上）</caption>
      <thead><tr><th>生まれ年</th><th>今日の年齢</th><th>判定</th></tr></thead>
      <tbody>${rows}</tbody>
    </table>`;
}

/**
 * 店員側の年齢確認（例外：店員側の画面からしか押せない）。まず「する／しない」を選び、
 * 「する」なら身分証の生年月日を入力して判定する。早見表を添える
 */
function idCard(reg: number, age: number, v: StaffView, a: (n: string) => ActionState): string {
  const choose = (idCheck: boolean, label: string, cls = "") =>
    btn(label, { type: "choose_age_check", register: reg, side: "staff", id_check: idCheck }, a("choose_age_check"), cls);
  let body: string;
  if (v.age_choice_pending) {
    body = `<div class="id-head"><b>${age}歳以上の年齢確認をしますか？</b>${choose(true, "する", "primary")}${choose(false, "しない")}</div>
      <p>「する」：身分証の生年月日を入力して判定します。「しない」：お客様が「${age}歳以上です」を押します。</p>`;
  } else if (v.id_pending) {
    const st = a("check_id");
    body = `<div class="id-head"><b>身分証の生年月日（西暦8桁）</b>
        <input type="text" inputmode="numeric" maxlength="8" placeholder="例 19821002" autocomplete="off" data-key="birth-${reg}" data-birth="${reg}" aria-label="身分証の生年月日（西暦8桁）" />
        <button class="primary" data-form="birth" data-reg="${reg}" ${st.allowed ? "" : `disabled title="${esc(st.reason ?? "")}"`}>判定</button>
        ${choose(false, "しないに変更", "small ghost")}</div>
      <p>年齢に届かない年確商品は買い物から取り除き、没収する商品を表示します。</p>`;
  } else {
    body = `<div class="id-head"><b>お客様の「${age}歳以上です」を待っています</b>${choose(true, "身分証で確認する", "small")}</div>`;
  }
  return `<div class="card id-card staff-only">${body}${ageChart(age)}</div>`;
}

/** 年確で販売できず取り除いた商品。店員がお客様から引き取る（没収する） */
function confiscatedCard(reg: number, v: StaffView): string {
  if (!v.confiscated.length) return "";
  const items = v.confiscated.map((c) => `<li><b>${esc(c.name)}</b> × ${c.qty}</li>`).join("");
  return `<div class="card id-card refused staff-only">
      <div class="id-head"><b>販売できない商品を没収してください</b>${btn("没収しました", { type: "dismiss_confiscated", register: reg }, undefined, "small")}</div>
      <ul class="confiscated">${items}</ul>
    </div>`;
}

/**
 * 代行収納の支払い内容・金額の確認。年確と同じく、お客様本人がお客様側の画面で押す
 * （.customer-only：どちらが操作側でも、お客様側の画面でだけ押せる）
 */
export function billCard(reg: number, amount: number, st: ActionState): string {
  return `<div class="card bill-card customer-only">
      <div class="big">お支払い内容の確認</div>
      <p>代行収納（公共料金）　<b>${yen(amount)}</b><br>払込票の内容・金額をご確認ください（お客様側の画面で押してください）</p>
      ${btn("内容・金額を確認しました", { type: "confirm_bill_payment", register: reg }, st, "primary big-btn")}
      ${!st.allowed && st.reason ? `<p class="muted">${esc(st.reason)}</p>` : ""}
    </div>`;
}

/** 支払方法を選んだ後の案内 */
function payHint(method: string | null, blocker: string | null): string {
  if (blocker) return `<p class="muted">${esc(blocker)}</p>`;
  if (method && method !== "cash") return `<p>Wallet の${METHOD_LABEL[method]}を、お客様側の端末にかざしてください</p>`;
  return `<p>現金を投入してください（または支払方法を選んでください）</p>`;
}

function staffLine(reg: number, l: StaffLine, v: StaffView, side: Side): string {
  const tags: string[] = [];
  if (l.age) tags.push(`<span class="tag age">年確${l.age}</span>`);
  if (l.kind === "ff") tags.push(`<span class="tag ff-${l.state}">FF ${FF_LABEL[l.state]}</span>`);
  if (l.kind === "service") tags.push(`<span class="tag ${l.done ? "ok" : "warn"}">${l.done ? "受付済" : "受付待ち"}</span>`);
  const remove = find(v.actions, "remove_line");
  let svc = "";
  if (l.kind === "service" && !l.done) {
    const a = find(v.actions, "complete_service");
    svc = `<div class="svc">
      <input type="number" inputmode="numeric" min="0" placeholder="金額" data-key="svc-${reg}-${l.id}" ${a.allowed ? "" : "disabled"} />
      <button data-form="service" data-reg="${reg}" data-line="${l.id}" ${a.allowed ? "" : "disabled"}>受付</button>
    </div>`;
  }
  return `<tr>
    <td>${esc(l.name)} ${tags.join(" ")}${svc}</td>
    <td class="num">${l.qty}${ffCheck(reg, l, find(v.actions, "check_ff"))}</td>
    <td class="num">${l.kind === "service" && !l.done ? "—" : yen(l.amount)}</td>
    <td>${btn("取消", { type: "remove_line", register: reg, side, line_id: l.id }, remove, "small ghost")}</td>
  </tr>`;
}

/** 呼び出しの種類の短い表示（店員側画面） */
const CALL_SHORT: Record<string, string> = {
  fast_food: "FF",
  tobacco: "煙草",
  service: "サービス",
  customer_help: "呼び出し",
  age_check: "年確",
};

/**
 * 呼び出し中の表示。両画面同内容の原則の、表示上の例外：
 * お客様側には定型の待機メッセージ、店員側には呼ばれた内容（「FF」「FF＆煙草」のように）を出す。
 * 情報は一つ（同じ呼び出しの記録）で、側ごとに見せ方だけを変える。
 */
export function callNotice(r: RegisterView, now: number, side: Side): string {
  const v = r.staff;
  if (side === "customer") {
    return r.customer.staff_called
      ? `<div class="banner call">係員を呼び出しています。しばらくお待ちください。</div>`
      : "";
  }
  const warn = v.mat_down.length
    ? `<div class="banner danger">${esc(v.mat_down.join("・"))} 不通：呼び出しが届きません</div>`
    : "";
  if (!v.calls.length) return warn;
  const kinds = [...new Set(v.calls.map((c) => CALL_SHORT[c.kind] ?? c.label))];
  const oldest = Math.min(...v.calls.map((c) => c.created_at));
  return `${warn}<div class="banner call">🔔 ${esc(kinds.join("＆"))}<span class="muted"> ${secs(now, oldest)}経過</span></div>`;
}

/** 会計時の年確。お客様側は「X歳以上です」、店員側は身分証の確認（はい・いいえ）と早見表 */
function ageStep(reg: number, v: StaffView, side: Side, a: (n: string) => ActionState): string {
  const age = v.required_age;
  if (!age) return "";
  if (side === "customer") return v.age_pending ? ageCard(reg, age, a("confirm_age")) : "";
  return v.age_choice_pending || v.age_pending || v.id_pending ? idCard(reg, age, v, a) : "";
}

function ageStatus(v: StaffView): string {
  if (!v.required_age) return "不要";
  const age = v.required_age;
  if (v.age_choice_pending) return `<b class="warn">${age}歳以上：店員が確認方法を選んでいます</b>`;
  if (v.age_pending) return `<b class="warn">${age}歳以上：お客様の確認待ち</b>`;
  if (v.id_pending) return `<b class="warn">${age}歳以上：身分証の確認中</b>`;
  return `<span class="ok">確認済</span>`;
}

/** 初期画面のボタンで選んだ案内（レジ:画面 → 種類）。選ばなくても、どのバーコードでも読める */
export const scanGuide = new Map<string, GuideKind>();
export type GuideKind = "item" | "bag" | "point" | "pay";
const GUIDES: [GuideKind, string, string][] = [
  ["item", "商品", "商品のバーコードをスキャンしてください"],
  ["bag", "レジ袋", "レジ袋の見本のバーコードをスキャンしてください（S・L）"],
  ["point", "ポイントカード", "ポイントカードのバーコードをスキャンしてください（お会計の後でも読めます）"],
  ["pay", "お支払い", "商品を登録して「お会計」に進むと、お支払い方法を選べます"],
];

/**
 * 初期画面。慣れた人はそのままスキャンし、不慣れな人はボタンで何を読むか選んで案内を見る
 * （自由度がなさすぎても、ありすぎても人は迷う）
 */
export function welcome(reg: number, side: Side): string {
  const on = scanGuide.get(`${reg}:${side}`);
  const guide = GUIDES.find(([k]) => k === on);
  return `<div class="welcome">
      <div class="big">バーコードをスキャンしてください</div>
      <p>すべてのバーコードを受け付けます</p>
      <div class="guide-row">${GUIDES.map(
        ([k, label]) => `<button type="button" class="guide-btn ${k === on ? "on" : ""}" data-guide="${reg}:${side}:${k}">${label}</button>`,
      ).join("")}</div>
      ${guide ? `<p class="guide">${guide[2]}</p>` : ""}
    </div>`;
}

/** 有人モードの登録画面（係員がいる）。どちらの画面にも同じものを出す */
export function attendedScreen(r: RegisterView, notice = "", side: Side = r.active_side ?? "staff"): string {
  if (r.menu) return notice + storeMenu(r);
  const v = r.staff;
  const reg = r.id;
  const a = (n: string) => find(v.actions, n);
  if (r.last_receipt && !v.lines.length) return notice + thanks(r);
  // 新しく登録した商品ほど上に出す
  const rows = [...v.lines].reverse().map((l) => staffLine(reg, l, v, side)).join("");
  const paying = v.phase === "paying";

  // 会計ボタンの後は、まずポイントカードの最終確認。済むと支払方法を選べる
  const pay = paying && v.point_pending
    ? pointCheck(reg, side, a("confirm_point_card"))
    : paying
    ? `${ageStep(reg, v, side, a)}${v.bill_check !== null ? billCard(reg, v.bill_check, a("confirm_bill_payment")) : ""}<div class="paybox">
        <div>年確：${ageStatus(v)}</div>
        <div>支払方法：${v.method ? METHOD_LABEL[v.method] : "未選択"}　投入 ${yen(v.inserted)}　残り <b>${yen(v.due)}</b></div>
        <div class="row wrap">${METHODS
          .map((m) => btn(METHOD_LABEL[m], { type: "select_payment", register: reg, side, method: m }, a("select_payment"), `seg ${v.method === m ? "on" : ""}`))
          .join("")}</div>
        ${payHint(v.method, v.payment_blocker)}
      </div>`
    : "";

  // 合計と会計ボタンは登録画面の最上部に常に出す（明細が増えてもスクロールで隠れない）
  const top = regTop(
    v.total,
    itemCount(v.lines),
    paying
      ? btn("登録に戻る", { type: "back_to_registering", register: reg, side }, a("back_to_registering"))
      : btn("会計", { type: "checkout", register: reg, side }, a("checkout"), "primary"),
    r.max_items,
  );
  return `${top}${notice}
    <div class="staff-head">
      <span>店員：<b>${esc(v.name)}</b>${v.role === "owner" ? "（オーナー）" : ""}</span>
      <span class="muted">内税 8% ${yen(v.tax8)} / 10% ${yen(v.tax10)}　${v.point_card ? "ポイントカード 読み取り済" : "ポイントカードなし"}</span>
    </div>
    ${side === "staff" ? confiscatedCard(reg, v) : ""}${pay}
    <table class="lines"><thead><tr><th>品名</th><th>数</th><th>金額</th><th></th></tr></thead>
      <tbody>${rows || `<tr><td colspan="4" class="muted">明細はありません。どのバーコードからでも読めます</td></tr>`}</tbody></table>
`;
}

/** 会計ボタンの後のポイントカードの最終確認。ポイントカードを読むか「なしで進む」で支払方法へ */
function pointCheck(reg: number, side: Side, st: ActionState): string {
  return `<div class="card point-card">
      <div class="big">ポイントカードはお持ちですか？</div>
      <p>お持ちの方は、ポイントカードを読み取ってください。読み取るとお支払い方法の選択に進みます。</p>
      ${btn("ポイントカードなしで進む", { type: "confirm_point_card", register: reg, side }, st, "primary")}
    </div>`;
}

/**
 * 登録画面の最上部に固定する帯：左から買い物点数・合計金額・会計ボタン。
 * 商品登録が一つもないときは出さない（案内を広く見せる）。
 */
export function regTop(total: number, count: number, action: string, max = Infinity): string {
  if (count === 0) return "";
  const full = count >= max ? `<small class="full">上限</small>` : "";
  return `<div class="reg-top"><span class="count"><b>${count}</b> 点${full}</span><span>合計 <b class="total">${yen(total)}</b></span>${action}</div>`;
}

/** 買い物点数（数量の合計） */
export function itemCount(lines: { qty: number }[]): number {
  return lines.reduce((n, l) => n + l.qty, 0);
}

/**
 * 二つの画面は、まったく同じ内容を映す（情報空間は一つ）。中身はモードで決まり、
 * どちらの画面もカラーで、その画面から触れない操作だけをグレーにする。
 * 例外は呼び出し中の表示（`callNotice`）で、側ごとに見せ方を変える。
 */
export function sharedScreen(r: RegisterView, snap: Snapshot, side: Side): string {
  const notice = callNotice(r, snap.now, side);
  return r.mode === "attended" ? attendedScreen(r, notice, side) : semiSelfScreen(r, notice, side);
}

// ---------------------------------------------------------------------------
// レジ画面のヘッダー：左上に歯車（店員ログイン中だけ）、中央に日付と時刻、右上に呼び出しベル。
// 時刻とベルは常に表示する。どちらが店員側・お客様側かは設置で自明なので画面には出さない
// ---------------------------------------------------------------------------

const pad = (n: number) => String(n).padStart(2, "0");

export function clockText(d: Date): string {
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日 ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function screenHead(r: RegisterView, now: Date): string {
  const call = find(r.customer.actions, "call_staff");
  const bell = `<button type="button" class="bell-btn" data-hold="${esc(JSON.stringify({ type: "call_staff", register: r.id }))}" aria-label="係員を呼ぶ（押している間鳴ります）" title="${esc(call.allowed ? "係員を呼ぶ（押している間鳴ります）" : (call.reason ?? ""))}"${call.allowed ? "" : " disabled"}>🔔</button>`;
  const gear = `<button type="button" class="gear" data-gear="${r.id}" aria-label="レジ${r.id}の店舗メニュー" title="店舗メニュー"${r.staff.state === "ready" ? "" : " hidden"}>☰</button>`;
  return `<span class="head-l">${gear}</span><time class="clock">${clockText(now)}</time><span class="head-r">${bell}</span>`;
}

// ---------------------------------------------------------------------------
// バーコードなしアイテム欄（レジ画面の下 1/3）
// - セミセルフ：店員呼び出しベルのショートカット（FF・サービス・たばこ）。触れている間だけ鳴る
// - 有人・セルフ：FF をタブ（揚げ物・常温総菜・ドリンク・コールド・中華まん・おでん）で選ぶ。
//   各タブはページを持ち、売れ筋順に並ぶので先頭ページの最上部に売れ筋が来る
// ---------------------------------------------------------------------------

export const FF_TABS = [
  ["fried", "揚げ物"],
  ["deli", "常温総菜"],
  ["drink", "ドリンク"],
  ["cold", "コールド"],
  ["bun", "中華まん"],
  ["oden", "おでん"],
] as const;
export const FF_PER_PAGE = 4;
export type FfUi = { tab: string; page: number };

export function itemsPanel(r: RegisterView, snap: Snapshot, ui: FfUi, side: Side): string {
  const reg = r.id;
  if (r.menu) return "";
  if (r.mode === "semi_self") {
    const a = (n: string) => find(r.customer.actions, n);
    const callBtn = (main: string, sub: string, cmd: Command, st: ActionState, cls: string) =>
      `<button class="call-btn ${cls}" data-hold="${esc(JSON.stringify(cmd))}"${st.allowed ? "" : ` disabled title="${esc(st.reason ?? "")}"`}><b>${esc(main)}</b>${sub ? `<small>${esc(sub)}</small>` : ""}</button>`;
    return `<div class="call-row">${[
      callBtn("FF", "揚げ物・中華まん", { type: "ff_button", register: reg }, a("ff_button"), "ff"),
      callBtn("サービス", "代行収納・宅配便", { type: "call_staff", register: reg, topic: "service" }, a("call_staff"), "svc"),
      callBtn("たばこ", "", { type: "call_staff", register: reg, topic: "tobacco" }, a("call_staff"), "tob"),
    ].join("")}</div>`;
  }
  // どちらの画面からも登録できる（もう一方が触れている一瞬だけ押せない）
  const view = r.staff;
  const can = find(view.actions, "register_ff");
  const items = snap.ff_stock.filter((f) => f.tab === ui.tab);
  const pages = Math.max(1, Math.ceil(items.length / FF_PER_PAGE));
  const page = Math.min(ui.page, pages - 1);
  const tabs = FF_TABS.map(
    ([k, label]) => `<button type="button" class="ff-tab ${k === ui.tab ? "on" : ""}" data-fftab="${reg}:${k}" aria-pressed="${k === ui.tab}">${label}</button>`,
  ).join("");
  const grid = items
    .slice(page * FF_PER_PAGE, (page + 1) * FF_PER_PAGE)
    .map((f) =>
      btn(
        `${f.name}`,
        { type: "register_ff", register: reg, side, code: f.code, qty: 1 },
        f.stock > 0 ? can : { action: "register_ff", allowed: false, reason: "売り切れです" },
        `ff-item ${f.stock > 0 ? "" : "soldout"}`,
      ).replace("</button>", `<small>${f.stock > 0 ? (r.mode === "attended" ? `残${f.stock}` : yen(f.price)) : "売り切れ"}</small></button>`),
    )
    .join("");
  // ページ送りの上下矢印はページの右端。上（前）・下（次）に隠れたページがあるときだけ出す
  const arrows =
    pages > 1
      ? `<div class="ff-arrows">${
          page > 0 ? `<button type="button" class="ff-arrow" data-ffpage="${reg}:-1" aria-label="前のページ">▲</button>` : "<span></span>"
        }${page < pages - 1 ? `<button type="button" class="ff-arrow" data-ffpage="${reg}:1" aria-label="次のページ">▼</button>` : "<span></span>"}</div>`
      : "";
  return `<div class="ff-tabs" role="tablist">${tabs}</div><div class="ff-page"><div class="ff-grid">${grid}</div>${arrows}</div>`;
}

// ---------------------------------------------------------------------------
// 店舗メニュー（お客様画面の右上の歯車から。有人モードの店員だけが使う）
// 自動釣銭機の取り出し口は売り場側にあるので、入出金の操作もお客様画面側で行う
// ---------------------------------------------------------------------------

const MENU_LABEL: Record<MenuScreen, string> = {
  top: "メニュー",
  refund: "返品返金",
  emergency: "緊急出金",
  deposit: "取引外入金",
  expense: "経費支出",
  changer: "自動釣銭機（両替）",
};

/** 一度に一つの画面だけを出す：一覧 → 各項目の画面 */
export function storeMenu(r: RegisterView): string {
  const v = r.staff;
  const reg = r.id;
  const m = r.menu!;
  const go = (screen: MenuScreen | null, extra: Record<string, unknown> = {}) => ({ type: "open_menu", register: reg, screen, ...extra });
  const head = (title: string) =>
    `<div class="menu-head">${btn("‹ 戻る", go("top"), undefined, "small ghost")}<b>${esc(title)}</b></div>`;
  const busy: ActionState | undefined = v.lines.length > 0 ? { action: "open_menu", allowed: false, reason: "取引中は使えません" } : undefined;
  switch (m.screen) {
    case "top":
      return `<div class="menu-head"><b>店舗メニュー</b><span class="muted">担当：${esc(v.name ?? "")}</span>${btn("閉じる", go(null), undefined, "small ghost")}</div>
        <div class="menu-list">${(["refund", "emergency", "deposit", "expense", "changer"] as const)
          .map((k) => btn(MENU_LABEL[k], go(k), busy, "menu-item"))
          .join("")}</div>
        <div class="menu-list">
          ${btn("取引中止", { type: "void_transaction", register: reg }, find(v.actions, "void_transaction"), "menu-item")}
          ${btn("ロック", { type: "lock", register: reg }, find(v.actions, "lock"), "menu-item")}
          ${btn("ログアウト", { type: "end_staff_mode", register: reg }, find(v.actions, "end_staff_mode"), "menu-item")}
        </div>`;
    case "refund":
    case "emergency":
    case "expense": {
      const cout = find(v.actions, "cash_out");
      return `${head(MENU_LABEL[m.screen])}
        <p class="muted">お金を抜く操作です。店舗パスワードが毎回必要で、お金は下の「出金」ボックスに出ます。</p>
        <div class="menu-form">
          <input type="number" inputmode="numeric" min="1" placeholder="金額" data-key="mo-amt-${reg}" />
          <input type="password" inputmode="numeric" placeholder="店舗パスワード" data-key="mo-pw-${reg}" autocomplete="off" />
          <button class="primary" data-form="cashout" data-reg="${reg}" data-kind="${m.screen}" ${cout.allowed ? "" : `disabled title="${esc(cout.reason ?? "")}"`}>出金</button>
        </div>`;
    }
    case "deposit":
      return `${head(MENU_LABEL.deposit)}
        <p>入金する金額を下の<b>「入金」ボックス</b>に入れて Enter を押してください。</p>
        <p class="muted">お金を入れる操作なのでパスワードは要りません。記録は担当者名つきで残ります。</p>`;
    case "changer": {
      const have = (d: number) => v.drawer.find(([x]) => x === d)?.[1] ?? 0;
      const segs = [1000, 500, 100, 50, 10]
        .map((d) => btn(`${d.toLocaleString()}円`, go("changer", { exchange_to: d }), undefined, `seg small ${m.exchange_to === d ? "on" : ""}`))
        .join("");
      return `${head(MENU_LABEL.changer)}
        <div class="row wrap">受け取る金種 ${segs}</div>
        <p>両替する金額を<b>「入金」ボックス</b>に入れて Enter を押してください。同じ額を ${m.exchange_to.toLocaleString()}円 で<b>「出金」ボックス</b>に出します。</p>
        <p class="muted">釣銭機の ${m.exchange_to.toLocaleString()}円：残り ${have(m.exchange_to)} 枚　在高 ${yen(v.drawer_total)}</p>`;
    }
  }
}

// ---------------------------------------------------------------------------
// セミセルフモードの登録画面（係員なし。お客様が登録して支払う）
// ---------------------------------------------------------------------------

export function semiSelfScreen(r: RegisterView, notice = "", side: Side = r.active_side ?? "customer"): string {
  if (r.menu) return notice + storeMenu(r);
  const c: CustomerView = r.customer;
  const reg = r.id;
  const a = (n: string) => find(c.actions, n);
  if (r.last_receipt && !c.lines.length) return notice + thanks(r);


  // 新しく登録した商品ほど上に出す
  const rows = [...c.lines]
    .reverse()
    .map((l) => {
      const tag =
        l.kind === "ff"
          ? `<span class="tag ff-${l.state}">${{ waiting: "ご用意中", received: "お受け取り済" }[l.state]}</span>`
          : l.kind === "service" && !l.done
            ? `<span class="tag warn">係員が受付します</span>`
            : "";
      const rm = c.phase === "registering" ? btn("取消", { type: "remove_line", register: reg, side, line_id: l.id }, a("remove_line"), "small ghost") : "";
      return `<tr><td>${esc(l.name)} ${tag}</td><td class="num">${l.qty}${ffCheck(reg, l, a("check_ff"))}</td><td class="num">${l.kind === "service" && !l.done ? "—" : yen(l.amount)}</td><td>${rm}</td></tr>`;
    })
    .join("");

  // 合計とお会計ボタンは登録画面の最上部に常に出す
  const top = regTop(
    c.total,
    itemCount(c.lines),
    c.phase === "registering"
      ? btn("お会計", { type: "checkout", register: reg, side }, a("checkout"), "primary")
      : btn("商品登録に戻る", { type: "back_to_registering", register: reg, side }, a("back_to_registering"), "ghost"),
    r.max_items,
  );

  if (!c.lines.length) return `${top}${notice}${welcome(reg, side)}`;
  scanGuide.delete(`${reg}:${side}`);

  let pay = "";
  if (c.phase === "paying") {
    const age =
      (c.age_check !== null ? ageCard(reg, c.age_check, a("confirm_age")) : "") +
      (c.bill_check !== null ? billCard(reg, c.bill_check, a("confirm_bill_payment")) : "");
    const methods = `<div class="row center wrap">${METHODS
      .map((m) => btn(METHOD_LABEL[m], { type: "select_payment", register: reg, side, method: m }, a("select_payment"), `seg ${c.method === m ? "on" : ""}`))
      .join("")}</div>`;
    pay = c.point_pending
      ? `${pointCheck(reg, side, a("confirm_point_card"))}${age}`
      : `${age}<div class="paybox center">
        <div>お支払い <b class="total">${yen(c.total)}</b></div>
        ${c.inserted ? `<div>お預かり ${yen(c.inserted)}　残り <b>${yen(c.due)}</b></div>` : ""}
        ${methods}${payHint(c.method, c.payment_blocker)}
      </div>`;
  }

  return `${top}${notice}
    ${pay}
    <table class="lines"><tbody>${rows}</tbody></table>
    ${c.point_card ? `<div class="totals"><span class="muted">ポイントカード 読み取り済</span></div>` : ""}`;
}

// ---------------------------------------------------------------------------
// MAT・店舗の設定
// ---------------------------------------------------------------------------

export function matPanel(s: Snapshot): string {
  return s.mats
    .map(
      (m) => `<div class="mat ${m.buzzing ? "buzz" : ""}">
        <span class="bell">🔔</span><span class="mat-id">${esc(m.id)}</span><span class="mat-state">${m.buzzing ? "鳴動中" : "待機"}</span>
      </div>`,
    )
    .join("");
}

const AGE_CHOICES = [15, 18, 20];

/** 年確ルール：売り場のすべての商品に、商品ごとの店舗ルールを付けられる */
export function ownerPanel(s: Snapshot): string {
  const rows = SHELVES.map((g) => {
    const items = s.catalog.filter(g.match);
    if (!items.length) return "";
    const body = items
      .map((p) => {
        // 酒・たばこは法令どおりで固定。店舗ルールは付けられない
        if (p.legal_age)
          return `<tr><td>${esc(p.name)}</td><td><span class="tag">法令 ${p.legal_age}歳</span></td>
          <td><span class="muted">変更できません</span></td>
          <td class="num"><b>${p.legal_age}歳以上</b></td></tr>`;
        const ages = AGE_CHOICES.slice();
        if (p.store_age && !ages.includes(p.store_age)) ages.push(p.store_age);
        ages.sort((x, y) => x - y);
        const opts = [`<option value="" ${p.store_age ? "" : "selected"}>なし</option>`]
          .concat(ages.map((a) => `<option value="${a}" ${p.store_age === a ? "selected" : ""}>${a}歳以上</option>`))
          .join("");
        return `<tr><td>${esc(p.name)}</td>
          <td><span class="muted">—</span></td>
          <td><select data-age="${esc(p.code)}" aria-label="${esc(p.name)}の店舗ルール">${opts}</select></td>
          <td class="num">${p.age ? `<b>${p.age}歳以上</b>` : `<span class="muted">年確なし</span>`}</td></tr>`;
      })
      .join("");
    return `<tr class="grp"><th colspan="4">${esc(g.label)}</th></tr>${body}`;
  }).join("");
  const hist = s.age_rules.history.length
    ? s.age_rules.history.map((h) => `<li>${esc(h.by)}：${esc(h.change)}</li>`).join("")
    : `<li class="muted">変更はまだありません</li>`;
  return `<table class="lines rules"><thead><tr><th>商品</th><th>法令</th><th>店舗ルール</th><th class="num">会計時の年確</th></tr></thead>
    <tbody>${rows}</tbody></table>
    <h4>変更履歴</h4><ul>${hist}</ul>`;
}

// ---------------------------------------------------------------------------
// ハードウェア（スキャナー・現金・電子マネー端末）
// ---------------------------------------------------------------------------

type Shelf = { label: string; match: (p: Snapshot["catalog"][number]) => boolean };

/** 売り場の棚。並び順はご指示の順（食べ物・飲み物・新聞・雑誌・代行収納・宅急便・酒・ライター）に、そのほかを足したもの */
const SHELVES: Shelf[] = [
  { label: "食べ物", match: (p) => p.category === "food" },
  { label: "飲み物", match: (p) => p.category === "drink" },
  { label: "新聞", match: (p) => p.category === "newspaper" },
  { label: "雑誌", match: (p) => p.category === "magazine" || p.category === "adult_book" },
  { label: "代行収納", match: (p) => p.category === "service" && p.name.includes("代行収納") },
  { label: "宅急便", match: (p) => p.category === "service" && p.name.includes("宅急便") },
  { label: "酒", match: (p) => p.category === "alcohol" },
  { label: "ライター・たばこ", match: (p) => p.category === "smoking_goods" || p.category === "tobacco" },
  { label: "花火", match: (p) => p.category === "fireworks" },
  { label: "そのほか", match: (p) => ["daily", "lottery", "adult_goods", "bag"].includes(p.category) },
];

function item(code: string, name: string, sub: string, cls = ""): string {
  return `<button type="button" class="item ${cls}" data-code="${esc(code)}" aria-label="${esc(name)}をドラッグ">
    <span class="item-name">${esc(name)}</span><span class="item-sub">${sub}</span></button>`;
}

/** Item：カテゴリの見出しは付けず、棚の並び順のまま一つにまとめる（表示スペースの節約） */
export function shelfPanel(s: Snapshot): string {
  const items = [...new Set(SHELVES.flatMap((g) => s.catalog.filter(g.match)))];
  return `<div class="shelf"><div class="items">${items
    .map((p) => item(p.code, p.name, `${p.price ? yen(p.price) : "金額は受付時"}${p.age ? ` ・<b>年確${p.age}</b>` : ""}`))
    .join("")}</div></div>`;
}

// ---------------------------------------------------------------------------
// 財布（お客様の持ち物）。レジに落とすと、お金は投入口へ、カードは読み取り・端末へ
// 実在のサービス名は使わない
// ---------------------------------------------------------------------------

function walletItem(key: string, name: string, sub: string, cls: string): string {
  return `<button type="button" class="item ${cls}" data-wallet="${esc(key)}" aria-label="${esc(name)}をドラッグ">
    <span class="item-name">${esc(name)}</span><span class="item-sub">${sub}</span></button>`;
}

export function walletPanel(): string {
  const bills = [10000, 5000, 2000, 1000].map((d) => walletItem(`cash:${d}`, `${d.toLocaleString()}円`, "紙幣", "bill"));
  const coins = [500, 100, 50, 10, 5, 1].map((d) => walletItem(`cash:${d}`, `${d}円`, "硬貨", "coin"));
  const cards = [
    item("PC12345678", "ポイントカード", "レジで読む", "card"),
    walletItem("pay:qr", "バーコード決済", "スマホ", "pay"),
    walletItem("pay:credit", "クレジットカード", "端末にかざす", "pay"),
    walletItem("pay:emoney", "電子マネー", "端末にかざす", "pay"),
  ];
  // 見出しなしの一つの並び（表示スペースの節約）
  return `<div class="shelf"><div class="items">${[...bills, ...coins, ...cards].join("")}</div></div>`;
}

/** コマンドがどちらの画面からの操作か（お知らせを出す場所を決める） */
export function sideOf(cmd: Command): "staff" | "customer" | "store" {
  if (cmd.side === "staff" || cmd.side === "customer") return cmd.side;
  if (["ff_button", "confirm_age", "confirm_bill_payment", "insert_cash", "tap_emoney", "call_staff"].includes(cmd.type)) return "customer";
  if (typeof cmd.register === "number") return "staff";
  return "store";
}
