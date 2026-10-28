// 画面の描画。どれもスナップショットから HTML 文字列を作る純粋関数で、
// ボタンの出し分けはコアが返す actions（操作可否と理由）だけで決める。
// 画面側で「押せるかどうか」を独自に判断しない。

import type {
  ActionState,
  BillSlips,
  Unavailable,
  Coupon,
  Returned,
  Command,
  CustomerView,
  FfState,
  MenuScreen,
  RegisterView,
  Snapshot,
  Rounding,
  StaffLine,
  StaffView,
  TaxKind,
  TaxTotal,
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
/**
 * FF の明細のお渡し状況。お渡し前は押せるアイコンで、押すとお渡し照合の画面になる。
 * 照合ではお渡しした個数を入力させる（チェックだけだと数量を意識せずに済ませられ、渡し忘れを防げない）
 */
function ffTag(reg: number, side: Side, l: { kind: string; state?: FfState }, a: ActionState, ro: boolean, labels = FF_LABEL): string {
  if (l.kind !== "ff" || !l.state) return "";
  const text = labels[l.state];
  if (l.state === "received" || ro) return `<span class="tag ff-${l.state}">${text}</span>`;
  const title = a.allowed ? "押してお渡しした個数を照合" : (a.reason ?? "");
  return `<button type="button" class="tag ff-waiting ff-verify" data-ffverify="${reg}:${side}" title="${esc(title)}"${a.allowed ? "" : " disabled"}>${text}</button>`;
}

/** お渡し照合の画面を開いている画面（レジ:画面）。「商品登録に戻る」で閉じる */
export const ffVerifying = new Set<string>();
/** 照合画面に入力したお渡し数（レジ:画面:明細番号 → 入力） */
export const ffvInput = new Map<string, string>();

/** 照合画面を閉じた（または出せなくなった）ら、その画面の入力も忘れる */
function closeFfVerify(reg: number, side: Side): void {
  ffVerifying.delete(`${reg}:${side}`);
  for (const k of [...ffvInput.keys()]) if (k.startsWith(`${reg}:${side}:`)) ffvInput.delete(k);
}

/** 照合画面に並べる FF：未照合（お渡し前）のものと、この画面で入力したもの */
function ffVerifyRows<L extends { id: number; kind: string; state?: FfState }>(reg: number, side: Side, lines: L[]): L[] {
  return lines.filter((l) => l.kind === "ff" && (l.state === "waiting" || ffvInput.has(`${reg}:${side}:${l.id}`)));
}

/**
 * お渡し照合：表で 品目｜注文数｜お渡し数｜照合。手続きを引き返す「商品登録に戻る」は左下に置く
 * （物事は右へ進む感覚に合わせ、進めるボタンは右、引き返すボタンは左）。お渡し数を入力すると、その場で注文数と比べて OK / NG を出す
 * （判定ボタンはない）。注文数を見ながら数を打ち込むことで、いくつ渡したかに注意が向く。
 * 画面が切り替わると直前に見た数は忘れやすいので、注文数はここにも出す
 */
function ffVerifyScreen(reg: number, side: Side, lines: { id: number; name: string; kind: string; state?: FfState; touched: number; qty: number }[], customer: boolean): string {
  // 並べるのは未照合の FF だけ（この画面で入力して OK になった行は、判定が見えるよう残す）。
  // 並びは登録画面と同じ（新しい動きのあった行ほど上）
  const ff = ffVerifyRows(reg, side, lines).sort((a, b) => b.touched - a.touched);
  const rows = ff
    .map((l) => {
      const key = `${reg}:${side}:${l.id}`;
      const value = ffvInput.get(key) ?? (l.state === "received" ? String(l.qty) : "");
      const judge = value === "" ? "" : l.state === "received" ? `<span class="ffv-ok">OK</span>` : `<span class="ffv-ng">NG</span>`;
      return `<tr><td>${esc(l.name)}</td><td class="num">${l.qty}</td><td class="num"><input type="text" inputmode="numeric" autocomplete="off" class="ffv-input" value="${esc(value)}" data-key="ffv-${key}" data-ffv-line="${key}" aria-label="${esc(l.name)}の${customer ? "お受け取り" : "お渡し"}数" /></td><td class="ffv-judge">${judge}</td></tr>`;
    })
    .join("");
  // 見出しは「お渡しの確認」（荷物や支払いの控えのお渡しにも同じ形の画面を使える）
  return `<div class="ffv-screen">
      <div class="big">お渡しの確認</div>
      <table class="lines ffv"><thead><tr><th>品目</th><th class="num">注文数</th><th class="num">${customer ? "お受け取り数" : "お渡し数"}</th><th class="ffv-judge">照合</th></tr></thead><tbody>${rows}</tbody></table>
      <div class="ffv-foot"><button type="button" class="small ghost" data-ffverify-close="${reg}:${side}">商品登録に戻る</button></div>
    </div>`;
}

const METHOD_LABEL: Record<string, string> = { cash: "現金", emoney: "電子マネー", qr: "バーコード決済", credit: "クレジットカード", points: "ポイント利用" };
const METHODS = ["cash", "emoney", "qr", "credit", "points"] as const;

// ---------------------------------------------------------------------------
// レジ画面。店員側・お客様側の区別はなく、二つの画面に同じものを映す。
// 中身はモードで決まる（有人：係員の登録画面／セミセルフ：お客様の登録画面）。
// ボタンが送る side は操作側の画面（押せるのは操作側だけ）
// ---------------------------------------------------------------------------

type Side = "staff" | "customer";

/** 支払いが終わった後のお礼とおつり */
/**
 * 決済後の画面。3 秒（core THANKS_MS）で新しいお客様の待ち受けに戻る。
 * 買った商品や持ち物の忘れ物はセルフレジで起こりがちなので、二行目で注意を促す
 */
function thanks(): string {
  return `<div class="thanks"><div class="big">ありがとうございました</div><div>お忘れ物にご注意ください</div></div>`;
}

/** 日付を YYYY-MM-DD で（レジの日付として生年月日の判定に使う） */
export function isoDate(t: Date): string {
  return `${t.getFullYear()}-${pad(t.getMonth() + 1)}-${pad(t.getDate())}`;
}

/**
 * 店員側の年齢確認（例外：店員側の画面からしか押せない）。まず「する／スキップ」を選び、
 * 「する」なら身分証の生年月日を入力して判定する
 */
function idCard(reg: number, age: number, v: StaffView, a: (n: string) => ActionState): string {
  const choose = (idCheck: boolean, label: string, cls = "") =>
    btn(label, { type: "choose_age_check", register: reg, side: "staff", id_check: idCheck }, a("choose_age_check"), cls);
  if (v.age_choice_pending) {
    // ポイントカードの確認と同じ形：問いを一行、ボタンをその下の行に、どちらも中央に
    return `<div class="point-screen">
        <div class="big">${age}歳以上の年齢確認をしますか？</div>
        <div class="point-choices">${choose(true, "する", "primary")}${choose(false, "スキップ", "skip")}</div>
      </div>`;
  }
  // 生年月日の入力もポイントカードの確認と同じ形：問いを一行、入力欄、ボタンの行。どれも中央に
  const st = a("check_id");
  return `<div class="point-screen id-entry">
      <div class="big">身分証の生年月日を入力してください</div>
      <input type="text" inputmode="numeric" maxlength="8" placeholder="西暦8桁　例 19821002" autocomplete="off" data-key="birth-${reg}" data-birth="${reg}" aria-label="身分証の生年月日（西暦8桁）" />
      <div class="point-choices">
        <button class="primary" data-form="birth" data-reg="${reg}" ${st.allowed ? "" : `disabled title="${esc(st.reason ?? "")}"`}>判定</button>
        ${choose(false, "スキップ", "skip")}
      </div>
    </div>`;
}

/** 回収リストでチェックを入れた行（レジごと）。画面の中だけの状態で、コアには「回収しました」だけを送る */
export const collected = new Map<number, Set<number>>();

/** 年確で販売できず取り除いた商品。店員がお客様から回収する。全部にチェックを入れると「回収しました」が押せる */
function confiscatedCard(reg: number, v: StaffView): string {
  if (!v.confiscated.length) {
    collected.delete(reg);
    return "";
  }
  const done = collected.get(reg) ?? new Set<number>();
  const items = v.confiscated
    .map(
      (c, i) =>
        `<li><label><input type="checkbox" data-collect="${reg}:${i}"${done.has(i) ? " checked" : ""} aria-label="${esc(c.name)}を回収" /><b>${esc(c.name)}</b> × ${c.qty}</label></li>`,
    )
    .join("");
  const all = v.confiscated.every((_, i) => done.has(i));
  const st = { action: "dismiss_confiscated", allowed: all, reason: all ? null : "回収した商品にチェックを入れてください" };
  return `<div class="point-screen collect">
      <div class="big">販売できない商品について回収してください</div>
      <ul class="collect-list">${items}</ul>
      <div class="point-choices">${btn("回収しました", { type: "dismiss_confiscated", register: reg }, st, "primary")}</div>
    </div>`;
}

/** お渡し済みの FF を減らした・取り消した分の回収の案内（店員側）。「回収しました」で閉じる */
function ffCollectCard(reg: number, v: StaffView): string {
  if (!v.ff_collect.length) return "";
  const lines = v.ff_collect.map((c) => `<div class="big">${esc(c.name)}を${c.qty}点回収してください。</div>`).join("");
  return `<div class="point-screen collect">
      ${lines}
      <div class="point-choices">${btn("回収しました", { type: "dismiss_ff_collect", register: reg }, { action: "dismiss_ff_collect", allowed: true, reason: null }, "primary")}</div>
    </div>`;
}

/**
 * 代行収納の支払い内容・件数・金額の確認。受け付けたらすぐ、お客様本人がお客様側の画面で押す。
 * 手数料などで手持ちが足りないときは「スキップ」で代行収納の支払いを取りやめられる
 * （.customer-only：どちらが操作側でも、お客様側の画面でだけ押せる）
 */
export function billCard(reg: number, count: number, amount: number, st: ActionState): string {
  return `<div class="card bill-card customer-only">
      <div class="big">お支払い内容の確認</div>
      <p>お支払い　<b>${count}件</b>　合計 <b>${yen(amount)}</b><br>払込票の内容・件数・金額をご確認ください</p>
      <div class="point-choices">
        ${btn("確認しました", { type: "confirm_bill_payment", register: reg }, st, "primary")}
        ${btn("スキップ", { type: "confirm_bill_payment", register: reg, skip: true }, st, "skip")}
      </div>
      ${!st.allowed && st.reason ? `<p class="muted">${esc(st.reason)}</p>` : ""}
    </div>`;
}

/** 確認画面の入れ物：登録画面の領域いっぱいに広げ、中身を上下左右とも中央に置く */
const confirm = (body: string): string => `<div class="confirm">${body}</div>`;

/** 確認の最中に、操作しない側に出す対のメッセージ */
/**
 * 公共料金の払込票の読み取り。1 枚目を読んだら全部の枚数を尋ね、答えた後は残りの枚数を案内する
 * （読み終えたら、お客様がお支払い内容を確認する）。読み取っている側の画面に出す
 */
function billSlipsCard(reg: number, side: Side, s: BillSlips, st: ActionState): string {
  const off = st.allowed ? "" : `disabled title="${esc(st.reason ?? "")}"`;
  if (s.declared === null)
    return `<div class="point-screen bill-slips">
      <div class="big">払込票は全部で何枚ですか？</div>
      <p class="muted">1枚目を読み取りました</p>
      <div class="point-choices">
        <input type="number" inputmode="numeric" min="${s.scanned}" max="10" placeholder="枚数" autocomplete="off" data-key="slips-${reg}" data-slips="${reg}" aria-label="払込票の枚数" ${off} />
        <button class="primary" data-form="slips" data-reg="${reg}" data-side="${side}" ${off}>決定</button>
      </div>
    </div>`;
  return `<div class="point-screen bill-slips">
      <div class="big">残りの払込票を読み取ってください</div>
      <p>全 <b>${s.declared}枚</b> のうち <b>${s.scanned}枚</b> を読み取りました（あと ${s.declared - s.scanned}枚）</p>
      <div class="point-choices">
        ${btn("枚数を訂正", { type: "declare_bill_count", register: reg, side, count: 0 }, st, "neg")}
      </div>
    </div>`;
}

function pairNote(head: string, sub = ""): string {
  return `<div class="point-screen pair-note"><div class="big">${esc(head)}</div>${sub ? `<p>${esc(sub)}</p>` : ""}</div>`;
}

/** 確認のうち、出すべき最初の一つ（一つの画面で複数の確認をしない） */
const firstOf = (cards: string[]): string => cards.find((c) => c) ?? "";

/** 支払方法を選んだ後の案内 */
function payHint(method: string | null, blocker: string | null, inserted: number, due: number, cashlessDue = due): string {
  if (blocker) return `<p class="muted">${esc(blocker)}</p>`;
  // 現金のみの商品（切手・代行収納）があれば、電子マネーで払う額と現金で払う額をかざす前に見せる
  // （現金と併用できるのは電子マネーだけ。かざし方は入金ボックスの点滅する案内に任せ、ここは金額の内訳だけにする）
  if (method && method !== "cash" && cashlessDue < due)
    return `<p>${METHOD_LABEL[method]}で <b>${yen(cashlessDue)}</b>、切手・お支払いの <b>${yen(due - cashlessDue)}</b> はそのあと現金で</p>`;
  if (method === "points") return `<p>Wallet のポイントカードかポイントアプリを、入金ボックスにかざしてください</p>`;
  if (method && method !== "cash") return `<p>Wallet の${METHOD_LABEL[method]}を、入金ボックスにかざしてください</p>`;
  // 残りの金額はここだけに出す（合計は上の帯、投入額は入金ボックス）
  if (inserted && due > 0) return `<p>あと <b>${yen(due)}</b> 入れてください</p>`;
  if (inserted) return `<p>「投入おわり」を押すと、お支払いが確定します（入金ボックスで Enter でも同じです）</p>`;
  if (method === "cash") return `<p>現金を入金ボックスに入れ、「投入おわり」を押してください</p>`;
  return `<p>現金を入金ボックスに入れるか、支払方法を選んでください</p>`;
}

/**
 * 支払方法の選択肢。現金は入れただけで支払方法が現金に決まるので、入れた後は出さない
 * （キャッシュレスは電子マネーとバーコード決済を取り違える人がいるので、必ず選んでもらう）
 */
function methodRow(reg: number, side: Side, method: string | null, inserted: number, st: ActionState, cls = "", unavailable: Unavailable[] = []): string {
  if (method === "cash" && inserted) return "";
  // 決済の組み合わせの原則で使えない支払方法は押せなくする（理由は core が返す）
  const off = (m: string): ActionState => {
    const u = unavailable.find((x) => x.method === m);
    return u ? { action: st.action, allowed: false, reason: u.reason } : st;
  };
  return `<div class="row wrap ${cls}">${METHODS
    .map((m) => btn(METHOD_LABEL[m], { type: "select_payment", register: reg, side, method: m }, off(m), `seg ${method === m ? "on" : ""}`))
    .join("")}</div>`;
}

const CASH_ONLY_NOTE = "現金のみの商品（切手・お支払い）は現金でお支払いください";

/**
 * 有人モードのお客様側：支払方法の一覧を見せるだけ（選ぶのは店員）。電子マネーとバーコード決済、
 * 電子マネーとポイントを取り違えるお客様が多いので、現金以外は店員を介して決める
 */
function methodList(method: string | null, inserted: number, unavailable: Unavailable[] = []): string {
  if (method === "cash" && inserted) return "";
  return `<div class="row wrap method-list">${METHODS.map((m) => `<span class="seg ${method === m ? "on" : ""}${unavailable.some((u) => u.method === m) ? " off" : ""}">${METHOD_LABEL[m]}</span>`).join("")}</div>`;
}

/**
 * 明細の列：品目・数量・小計。数量はボタンの形で、押すと数量変更・取り消しの行が広がる。
 * 押せない画面でも同じ幅にし、店員側とお客様側で列の位置がずれないようにする
 */
const LINE_COLS = `<colgroup><col /><col class="c-qty" /><col class="c-amt" /></colgroup>`;

// 内税の表示（有人・セミセルフ共通）
function taxNote(v: { taxes: TaxTotal[] }): string {
  return `<span class="muted tax-note">内税 ${v.taxes.map((t) => `${esc(t.rate)} ${yen(t.tax)}`).join(" / ")}</span>`;
}

/**
 * 注意のしるし（年確・限定）。品目の横、FF のお渡し前・お渡し済と同じ位置に出す
 * （年確が要る FF はないので重ならず、専用の列を持たずに済む）
 */
function warnTags(l: { age: number | null; limit: number | null; cash_only?: boolean }): string {
  return [
    l.age ? `<span class="tag age">年確${l.age}</span>` : "",
    l.limit ? `<span class="tag age">限定${l.limit}</span>` : "",
    l.cash_only ? `<span class="tag age">現金のみ</span>` : "",
  ].filter(Boolean).join(" ");
}

function staffLine(reg: number, l: StaffLine, v: StaffView, side: Side): string {
  const tags: string[] = [];
  // 有人モードのお客様側は見るだけ（操作は店員側の画面で行う）
  const ro = side === "customer";
  if (l.kind === "ff") tags.push(ffTag(reg, side, l, find(v.actions, "check_ff"), ro));
  // 払込票（現金のみのサービス）は受付が要らないので、受付の札を出さない
  if (l.kind === "service" && !(l.done && l.cash_only)) tags.push(`<span class="tag ${l.done ? "ok" : "warn"}">${l.done ? "受付済" : "受付待ち"}</span>`);
  tags.push(warnTags(l));
  const remove = find(v.actions, "remove_line");
  let svc = "";
  if (l.kind === "service" && !l.done && !ro) {
    const a = find(v.actions, "complete_service");
    svc = `<div class="svc">
      <input type="number" inputmode="numeric" min="0" placeholder="金額" data-key="svc-${reg}-${l.id}" ${a.allowed ? "" : "disabled"} />
      <button data-form="service" data-reg="${reg}" data-line="${l.id}" ${a.allowed ? "" : "disabled"}>受付</button>
    </div>`;
  }
  return `<tr class="${flash(reg, l.id)} ${editingRow(reg, side, l.id)}">
    <td>${esc(l.name)} ${tags.join(" ")}${svc}</td>
    <td class="num">${ro ? l.qty : qtyBtn(reg, side, l.id, l.qty, remove)}</td>
    <td class="num">${l.kind === "service" && !l.done ? "—" : yen(l.amount)}</td>
  </tr>${lineEditor(reg, side, l, remove)}`;
}

/**
 * 個数変更・取り消しのために広げている行（レジ:画面 → 明細番号と変更後の個数）。
 * ポップアップは使わず、対象の行の下が一時的に広がって、登録画面の中で完結する
 */
export const editing = new Map<string, { id: number; qty: number }>();

const editingRow = (reg: number, side: Side, id: number): string => (editing.get(`${reg}:${side}`)?.id === id ? "editing" : "");

/** 数量をボタンの形で出す。押すと行が広がって数量変更・取り消しができる（もう一度押すと閉じる）。押せないときは数字だけ */
function qtyBtn(reg: number, side: Side, id: number, qty: number, st: ActionState): string {
  if (!st.allowed) return `${qty}`;
  const open = editing.get(`${reg}:${side}`)?.id === id;
  return `<button type="button" class="qty-btn ${open ? "on" : ""}" data-edit="${reg}:${side}:${id}:${qty}" title="数量変更・取り消し" aria-label="数量 ${qty}。押すと数量変更・取り消し">${qty}</button>`;
}

/** 広げた行の中身：個数の − ＋ と「個数変更」、それに「取り消し」 */
function lineEditor(reg: number, side: Side, l: { id: number; qty: number; kind: string; state?: string }, st: ActionState, semiSelf = false): string {
  const e = editing.get(`${reg}:${side}`);
  if (!e || e.id !== l.id || !st.allowed) return "";
  // お渡し済みの FF の数量変更・取り消しは店員が行う（増やせばお渡し前に戻り、減らせば回収の案内が出る）
  if (semiSelf && l.kind === "ff" && l.state === "received")
    return editorRow(reg, side, `<span class="muted">お渡し済みの FF の数量は係員が変更します</span>`);
  // サービス商品は数量を変えられない（取り消しだけ）
  const fixed = l.kind === "service";
  const qty = fixed
    ? `<span class="muted">この商品は数量を変えられません</span>`
    : `<span class="qty-edit">
        <button class="small neg" data-edit-step="${reg}:${side}:-1" ${e.qty <= 1 ? "disabled" : ""}>−</button>
        <b>${e.qty}</b>
        <button class="small neg" data-edit-step="${reg}:${side}:1">＋</button>
      </span>
      ${btn("数量変更", { type: "set_qty", register: reg, side, line_id: l.id, qty: e.qty }, { ...st, allowed: e.qty !== l.qty }, "small primary")}`;
  return editorRow(reg, side, `${qty}${btn("取り消し", { type: "remove_line", register: reg, side, line_id: l.id }, st, "small danger")}`);
}

function editorRow(reg: number, side: Side, body: string): string {
  return `<tr class="edit-row"><td colspan="3"><div class="edit-box">${body}<button class="small ghost" data-edit-close="${reg}:${side}">閉じる</button></div></td></tr>`;
}

/** 呼び出しの種類の短い表示（店員側画面） */
const CALL_SHORT: Record<string, string> = {
  fast_food: "FF",
  tobacco: "煙草",
  service: "サービス",
  customer_help: "呼び出し",
  age_check: "年確",
  change_left: "釣銭忘れ",
};

/**
 * 呼び出し中の表示。両画面同内容の原則の、表示上の例外：
 * 有人モードのお客様側には「店員が向かっています」を出す。店員側は Bell の不通だけを知らせる。
 */
export function callNotice(r: RegisterView, _now: number, side: Side): string {
  const v = r.staff;
  // 有人モードで年確の工程を出している間は、年確の呼び出しは表示しない（年確の画面そのものが知らせる）
  const ageOnScreen = r.mode === "attended" && (v.age_choice_pending || v.id_pending);
  const calls = ageOnScreen ? v.calls.filter((c) => c.kind !== "age_check") : v.calls;
  if (side === "customer") {
    // 排出口に釣り銭が残ったまま 1 分たったら知らせる（2 分で店員を呼ぶ）
    const change = r.change_left ? `<div class="banner warn">釣り銭が残っています</div>` : "";
    return (
      change +
      (r.mode === "attended" && r.customer.staff_called && (!ageOnScreen || calls.length)
        ? `<div class="banner call">店員が向かっています</div>`
        : "")
    );
  }
  const warn = v.mat_down.length
    ? `<div class="banner danger">${esc(v.mat_down.join("・"))} 不通：呼び出しが届きません</div>`
    : "";
  // 有人モードでは店員がレジにいるので、呼び出しの内容や経過時間は店員側に出さない
  return warn;
}

/** 呼び出しの経過時間。年確の呼び出しは数えない（経過時間を出さない） */
function elapsed(calls: { kind: string; created_at: number }[], now: number, before: string, after = ""): string {
  const timed = calls.filter((c) => c.kind !== "age_check");
  if (!timed.length) return "";
  const oldest = Math.min(...timed.map((c) => c.created_at));
  return `<span class="muted">${before}${secs(now, oldest)}経過${after}</span>`;
}

/** セミセルフの呼び出し中の店員側の画面：呼び出しの理由（「年確」「FF＆煙草」など）を大きく */
export function callScreen(r: RegisterView, now: number): string {
  const v = r.staff;
  const warn = v.mat_down.length
    ? `<div class="banner danger">${esc(v.mat_down.join("・"))} 不通：呼び出しが届きません</div>`
    : "";
  const kinds = [...new Set(v.calls.map((c) => CALL_SHORT[c.kind] ?? c.label))];
  const details = [...new Set(v.calls.map((c) => c.detail).filter(Boolean))];
  // 登録画面の領域いっぱいに広げて出す（最下段の FF・サービス・たばこボタンはそのまま）
  return `<div class="call-screen">${warn}
      <div class="call-reason">🔔 ${esc(kinds.join("＆"))}</div>
      ${details.length ? `<p>${esc(details.join("・"))}</p>` : ""}
      <p class="muted">${elapsed(v.calls, now, "", "　")}店員証でログインすると対応を始めます</p>
    </div>`;
}

/** セミセルフで年確の係員を待つ間のお客様側の案内（登録画面・支払画面の代わりに出す） */
function ageWait(age: number): string {
  return `<div class="point-screen">
      <div class="big">年齢確認が必要な商品が含まれています。</div>
      <p>${age}歳前後のお客様は身分証をご準備ください。</p>
    </div>`;
}

/** 会計時の年確。店員側の画面だけで行う（する：身分証の生年月日、スキップ：店員の判断で済む） */
function ageStep(reg: number, v: StaffView, side: Side, a: (n: string) => ActionState): string {
  const age = v.required_age;
  if (!age || side === "customer") return "";
  return v.age_choice_pending || v.id_pending ? idCard(reg, age, v, a) : "";
}

/** 登録した直後の行（レジ:明細番号）。一瞬目立つ色にする（main.ts が数量の変化から決める） */
export const flashing = new Set<string>();
const flash = (reg: number, id: number): string => (flashing.has(`${reg}:${id}`) ? "flash" : "");

/** 読み取ったクーポンの行。対象商品がまだなければ「対象商品待ち」（商品より先に読んでもよい）。数量 1 のボタンで取り消しを開く */
export function couponRows(reg: number, side: Side, coupons: Coupon[], st: ActionState, ro = false): Row[] {
  return coupons.map((c) => [
    c.id,
    `<tr class="coupon ${flash(reg, c.id)}"><td>${esc(c.name)} <span class="tag">クーポン</span></td><td class="num">${ro ? "1" : qtyBtn(reg, side, c.id, 1, st)}</td><td class="num">${
      c.discount === null ? `<span class="muted">対象商品待ち</span>` : `−${yen(c.discount)}`
    }</td></tr>${
      !ro && editing.get(`${reg}:${side}`)?.id === c.id && st.allowed ? editorRow(reg, side, btn("取り消し", { type: "remove_coupon", register: reg, side, coupon_id: c.id }, st, "small danger")) : ""
    }`,
  ]);
}

/** 明細の 1 行（最後に動きのあった順番, HTML）。クーポンの番号は読んだ順番 */
type Row = [number, string];

/**
 * 新しい動きのあった行ほど上に出す。同じ商品は続けて読んだときだけ一番上の行に足す（前の行にはまとめない）
 * （買い物が多いとき、最初の行の数量が表示エリアの外で増えて見えない、ということがない）
 */
function newestFirst(rows: Row[]): string {
  return [...rows]
    .sort((x, y) => y[0] - x[0])
    .map(([, html]) => html)
    .join("");
}

function removeCoupon(reg: number, side: Side, c: Coupon, st: ActionState): string {
  return btn("取消", { type: "remove_coupon", register: reg, side, coupon_id: c.id }, st, "small ghost");
}

/**
 * 会計に進んだのに、適用先（対象商品）のないクーポンが残っているときの注意。
 * クーポンで安く買うつもりの商品が似て非なる商品だった、という事故を防ぐため、
 * 対象商品を登録するかクーポンを取り消すまで決済できない
 */
export function unmatchedCard(reg: number, side: Side, coupons: Coupon[], st: ActionState, ro = false): string {
  const list = coupons
    .filter((c) => c.discount === null)
    .map((c) => `<li><b>${esc(c.name)}</b> ${ro ? "" : removeCoupon(reg, side, c, st)}</li>`)
    .join("");
  const [head, body] =
    side === "staff"
      ? ["適用先のないクーポンがあります", "対象商品と似た別の商品を通していないか確かめてください。対象商品を登録するか、クーポンを取り消すまで決済できません"]
      : ["ご利用になれないクーポンがあります", "クーポンの対象商品とお買い上げの商品が同じか、お確かめください。対象商品を追加するか、クーポンを取り消すとお支払いに進めます"];
  return `<div class="card coupon-card"><b>${head}</b><p>${body}</p><ul class="coupon-list">${list}</ul></div>`;
}

/**
 * 取り消したクーポンの返却。店員側には返却を促し、お客様側にはお返しする旨を案内する。
 * done はこの画面で案内を閉じるボタン（有人モードのお客様側は、店員の「返却しました」で閉じる）
 */
export function returnedCard(reg: number, side: Side, returned: Returned[], done: boolean): string {
  if (!returned.length) return "";
  const list = returned.map((c) => `<li><b>${esc(c.name)}</b></li>`).join("");
  const head =
    side === "staff"
      ? "取り消したクーポンをお客様に返却してください"
      : returned.every((c) => c.unmatched)
        ? "適用先のなかったクーポンをお返しします"
        : "取り消したクーポンをお返しします";
  const label = side === "staff" ? "返却しました" : "受け取りました";
  return `<div class="card coupon-card returned"><div class="id-head"><b>${head}</b>${
    done ? btn(label, { type: "dismiss_returned", register: reg }, undefined, "small") : ""
  }</div><ul class="coupon-list">${list}</ul></div>`;
}

/** 初期画面のボタンで選んだ案内（レジ:画面 → 種類）。選ばなくても、どのバーコードでも読める */
export const scanGuide = new Map<string, GuideKind>();
export type GuideKind = "item" | "bag" | "point" | "pay" | "coupon" | "shop" | "parcel" | "bill";
const GUIDES: [GuideKind, string, string][] = [
  ["item", "商品", "商品のバーコードをスキャンしてください"],
  ["bag", "レジ袋", "レジ袋のバーコードをスキャンしてください"],
  ["point", "ポイント", "ポイントカードまたはポイントアプリのバーコードをスキャンしてください"],
  ["pay", "お支払い", "支払い用紙のバーコードをスキャンしてください"],
  ["coupon", "クーポン", "引換券、割引券のバーコードをスキャンしてください"],
];

/**
 * 初期画面。慣れた人はそのままスキャンし、不慣れな人はボタンで何を読むか選んで案内を見る
 * （自由度がなさすぎても、ありすぎても人は迷う）
 */
export function welcome(reg: number, side: Side, emoney?: ActionState): string {
  const on = scanGuide.get(`${reg}:${side}`);
  const guide = GUIDES.find(([k]) => k === on);
  return `<div class="welcome${emoney ? " scan-welcome" : ""}">
      <div class="big">バーコードをスキャンしてください</div>
      <p>すべてのバーコードを受け付けます</p>
      <div class="guide-row">${GUIDES.map(
        ([k, label]) => `<button type="button" class="guide-btn ${k === on ? "on" : ""}" data-guide="${reg}:${side}:${k}">${label}</button>`,
      ).join("")}</div>
      ${guide ? `<p class="guide">${guide[2]}</p>` : ""}
      ${
        // 電子マネーはバーコードを読まない用件なので、案内ボタンから離して右下の明るい角に文字リンクで置く
        emoney ? btn("電子マネーのチャージ・残高確認", { type: "start_emoney", register: reg, side, kind: "menu" }, emoney, "emoney-link") : ""
      }
    </div>`;
}

/** 電子マネーのチャージ・残高確認を始めるボタン（支払方法の「電子マネー」と取り違えないよう二行目を添える） */
function emoneyEntry(reg: number, side: Side, st: ActionState, cls: string): string {
  return btn("", { type: "start_emoney", register: reg, side, kind: "menu" }, st, cls).replace("></button>", "><b>電子マネー</b><small>チャージ・残高確認</small></button>");
}

/**
 * 取引中止の確認（レジ番号 → 中止ボタンを押した画面）。登録画面を置き換えて出し、
 * 「中止する」は長押しでだけ決まる（大量に登録した後の誤操作で、登録をやり直させない）
 */
export const voidAsk = new Map<number, Side>();

/** 明細の上の帯に置く「取引中止」。会計ボタンから離した控えめなボタン */
function voidBtn(reg: number, side: Side, st: ActionState): string {
  const title = st.allowed ? "取引を中止する（確認があります）" : (st.reason ?? "");
  return `<button type="button" class="void-btn"${st.allowed ? ` data-void-ask="${reg}:${side}"` : " disabled"} title="${esc(title)}">取引中止</button>`;
}

function voidConfirm(r: RegisterView, side: Side, view: { lines: { qty: number }[]; total: number; inserted: number }): string {
  const reg = r.id;
  // 有人モードのお客様側には、店員側の確認と対になる知らせだけを出す
  if (r.mode === "attended" && side === "customer") return pairNote("取引の中止を確認しています");
  const st = find(r.mode === "attended" ? r.staff.actions : r.customer.actions, "void_transaction");
  const cmd = esc(JSON.stringify({ type: "void_transaction", register: reg }));
  const back = [
    view.inserted ? `入れたお金 ${yen(view.inserted)} は出金ボックスに戻ります。` : "",
    r.prepaid && r.prepaid_method === "電子マネー" ? `電子マネーで払った ${yen(r.prepaid)} は残高に戻ります。` : "",
    r.prepaid && r.prepaid_method !== "電子マネー" ? `${esc(r.prepaid_method ?? "")}で払った ${yen(r.prepaid)} は取り消します。` : "",
  ].join("");
  return `<div class="point-screen void-screen">
      <div class="big">取引を中止しますか？</div>
      <p>登録した ${itemCount(view.lines)}点（合計 ${yen(view.total)}）をすべて取り消します。${back}</p>
      <div class="void-choices">
        <button type="button" class="primary" data-void-cancel="${reg}">続ける</button>
        <button type="button" class="hold-confirm" data-hold-confirm="${cmd}"${st.allowed ? "" : " disabled"}><span class="hold-fill"></span><span class="hold-label">長押しで中止</span></button>
      </div>
    </div>`;
}

/** 有人モードの初期画面の案内ボタン（店員側で押す。お客様側には同じボタンを押せない形で映す） */
const ATTENDED_GUIDES: [GuideKind, string, string][] = [
  ["shop", "お買い物", "商品のバーコードをスキャンしてください"],
  ["parcel", "宅配便", "宅配便の伝票のバーコードをスキャンしてください"],
  ["bill", "お支払い", "支払い用紙のバーコードをスキャンしてください"],
];

/**
 * 有人モードの初期画面。あいさつと声かけの二行に、ご用件の案内ボタンを添える。
 * 一段目は お買い物・宅配便・お支払い（押すと読むバーコードを案内）、二段目は電子マネーのチャージ・残高確認。
 * 両方の画面に出し、操作できるのは店員側だけ（お客様側はどの用件を受け付けているかを見せる）
 */
function attendedWelcome(r: RegisterView, notice: string, side: Side): string {
  const reg = r.id;
  const on = scanGuide.get(`${reg}:staff`);
  const ro = side === "customer";
  const guide = ATTENDED_GUIDES.find(([k]) => k === on);
  const guides = ATTENDED_GUIDES.map(
    ([k, label]) =>
      `<button type="button" class="guide-btn ${k === on ? "on" : ""}${ro ? " mirror-only" : ""}"${ro ? ' tabindex="-1" aria-disabled="true"' : ` data-guide="${reg}:staff:${k}"`}>${label}</button>`,
  ).join("");
  // お買い物・宅配便・お支払いと同じ形と色（二行目に「チャージ・残高確認」）
  const emoney = ro
    ? `<button type="button" class="guide-btn emoney-guide mirror-only" tabindex="-1" aria-disabled="true"><b>電子マネー</b><small>チャージ・残高確認</small></button>`
    : emoneyEntry(reg, "staff", find(r.staff.actions, "emoney"), "guide-btn emoney-guide");
  const [head, sub] = ro ? ["いらっしゃいませ", "ご用件をお申し付けください"] : ["お辞儀とご挨拶をしましょう", "ご用件をお伺いしてください"];
  return `${notice}<div class="welcome attended-welcome"><div class="big">${head}</div><p>${sub}</p>
      <div class="guide-row">${guides}</div>
      <div class="guide-row">${emoney}</div>
      ${guide && !ro ? `<p class="guide">${guide[2]}</p>` : ""}
    </div>`;
}

/** 有人モードの登録画面（係員がいる）。どちらの画面にも同じものを出す */
export function attendedScreen(r: RegisterView, notice = "", side: Side = r.active_side ?? "staff"): string {
  // 店舗メニューは開いた側の画面にだけ出す。お客様側から開くときは、店員画面にパスワード・お客様画面に入力欄
  if (r.menu?.side === side) return notice + storeMenu(r);
  if (r.menu_code) return notice + menuCode(r, side);
  if (r.menu && side === "staff") return notice + pairNote("お客様側の画面で店舗メニューを操作しています");
  const v = r.staff;
  const reg = r.id;
  const a = (n: string) => find(v.actions, n);
  if (r.last_receipt && !v.lines.length) return notice + thanks();
  // 有人モードの初期画面（まだ何も登録していない）は、あいさつと声かけの二行だけ。店員側には対になる行動の案内
  if (!v.lines.length && !v.coupons.length) voidAsk.delete(reg);
  if (!v.lines.length && !v.coupons.length && !v.returned.length && !v.confiscated.length && !v.ff_collect.length) return attendedWelcome(r, notice, side);
  // お客様側に出すボタンは、お客様本人が押すもの（お支払い内容の確認・投入おわり）だけ。
  // ほかの操作（会計・変更・ポイントカード・支払方法など）は店員側の画面で行う
  const ro = side === "customer";
  // 新しい動きのあった行ほど上に出す
  const rows = newestFirst([...couponRows(reg, side, v.coupons, a("remove_coupon"), ro), ...v.lines.map((l): Row => [l.touched, staffLine(reg, l, v, side)])]);
  const paying = v.phase === "paying";

  // 会計ボタンの後は、年確 → ポイントカードの最終確認 → 決済手段の確認の順に進む
  const pointPending = paying && v.point_pending;
  const agePending = paying && (v.age_choice_pending || v.id_pending);
  // 決済手段の確認：登録画面と年確の状況は出さず、決済手段だけを示す（選ぶのは店員側の画面）。
  // 同じ情報を複数個所に出さない：点数・合計は上の帯、投入額は入金ボックス、選んだ手段はボタンの強調で示す
  const payStage = paying && !pointPending && !agePending;
  const ask = side === "staff" ? "決済手段をお伺いしてください" : "決済手段をお申しつけください";
  const pay = payStage
    ? `<div class="paybox pay-stage">
        ${v.method || v.inserted ? "" : `<div class="big">${ask}</div>`}
        ${side === "staff" ? methodRow(reg, side, v.method, v.inserted, a("select_payment"), "", r.unavailable) : methodList(v.method, v.inserted, r.unavailable)}
        ${r.prepaid && !v.inserted && !v.payment_blocker ? prepaidNote(r, v.due) : v.method || v.inserted || v.payment_blocker ? payHint(v.method, v.payment_blocker, v.inserted, v.due, r.cashless_due) : cashOnlyNote(r)}
      </div>`
    : agePending
      ? ageStep(reg, v, side, a)
      : "";

  // 合計と会計ボタンは登録画面の最上部に常に出す（明細が増えてもスクロールで隠れない）
  // 電子マネーのチャージ・残高確認は、お買い物の途中でも（会計中でも）店員側から始められる。
  // 「すべての操作はやりたいときにできる」：待ち受けに戻らなくてよいよう、会計ボタンの左に常に置く
  const emoney = ro ? "" : emoneyEntry(reg, side, a("emoney"), "emoney-top");
  const top = regTop(
    v.total,
    itemCount(v.lines),
    emoney +
    (paying && v.method === "cash"
      ? finishInsert(reg, v.inserted, v.due, a("insert_cash"))
      : ro
        ? ""
        : paying
          ? btn("登録に戻る", { type: "back_to_registering", register: reg, side }, a("back_to_registering"), "ghost")
          : btn("会計", { type: "checkout", register: reg, side }, a("checkout"), "primary")),
    r.max_items,
    !!v.point_card,
    ro || voidAsk.has(reg) ? "" : voidBtn(reg, side, a("void_transaction")),
  );
  // 一つの画面では一つの確認だけを、登録画面を置き換えて出す。優先順は
  // 代行収納の金額と件数（受付で即座に）＞年確（店員側だけ。お客様側は明細のまま）＞回収・クーポン＞ポイントカード＞決済手段
  const screen = (body: string) => `${top}${notice}${confirm(body)}`;
  // 取引中止の確認は、ほかのどの確認よりも先に出す（どの場面からでも中止できる）
  if (voidAsk.has(reg)) return screen(voidConfirm(r, side, v));
  // 確認の最中は、店員側とお客様側に対になるメッセージを出す（片方が操作し、もう片方はその旨を知る）
  if (v.bill_slips)
    return screen(side === "staff" ? billSlipsCard(reg, side, v.bill_slips, a("declare_bill_count")) : pairNote("払込票を読み取っています"));
  if (v.bill_check !== null)
    return screen(side === "customer" ? billCard(reg, v.bill_count, v.bill_check, a("confirm_bill_payment")) : pairNote("お客様がお支払い内容を確認しています"));
  if (agePending) return screen(side === "staff" ? pay : pairNote(v.id_pending ? "身分証を確認しています" : "年齢確認が必要な商品が含まれています"));
  const card = firstOf([
    side === "staff" ? confiscatedCard(reg, v) : v.confiscated.length ? pairNote("販売できない商品があります", "販売不可商品を回収いたします") : "",
    side === "staff" ? ffCollectCard(reg, v) : "",
    v.coupon_unmatched ? unmatchedCard(reg, side, v.coupons, a("remove_coupon"), ro) : "",
    returnedCard(reg, side, v.returned, side === "staff"),
  ]);
  if (card) return screen(card);
  if (pointPending) return screen(side === "staff" ? pointCheck(reg, side, a("confirm_point_card")) : customerPointAsk());
  scanGuide.delete(`${reg}:${side}`);
  if (payStage) return screen(pay);
  if (!ro && !paying && ffVerifying.has(`${reg}:${side}`) && ffVerifyRows(reg, side, v.lines).length)
    return screen(ffVerifyScreen(reg, side, v.lines, false));
  closeFfVerify(reg, side);
  return `${top}${notice}
    <div class="staff-head">
      <span>店員：<b>${esc(v.name)}</b>${v.role === "owner" ? "（オーナー）" : ""}</span>
      ${taxNote(v)}
    </div>
    <table class="lines cols3">${LINE_COLS}<thead><tr><th>品目</th><th class="num">数量</th><th class="num">小計</th></tr></thead>
      <tbody>${rows || `<tr><td colspan="3" class="muted">明細はありません。どのバーコードからでも読めます</td></tr>`}</tbody></table>
`;
}

/**
 * 会計ボタンの後のポイントカードの最終確認。有人モードの店員側と、セルフ・セミセルフのお客様側で同じ形。
 * 読み取りを促し、なければ「スキップ」。会計前に読み取り済みなら出ない
 */
function pointCheck(reg: number, side: Side, st: ActionState): string {
  // 年確と同じ形。「付与する」を押すと読み取りの案内を出す（押さずにスキャンしても読める）
  const reading = scanGuide.get(`${reg}:${side}`) === "point";
  return `<div class="point-screen">
      <div class="big">ポイントを付与しますか？</div>
      <div class="point-choices">
        <button type="button" class="primary ${reading ? "on" : ""}" data-guide="${reg}:${side}:point">付与する</button>
        ${btn("スキップ", { type: "confirm_point_card", register: reg, side }, st, "skip")}
      </div>
      ${reading ? `<p class="guide">ポイントサービスのバーコードを読み取ってください。</p>` : ""}
    </div>`;
}

/** このお店で使えるポイントサービス（お客様側の最終確認に並べるアイコン） */
const POINT_SERVICES: [string, string][] = [
  ["💳", "ポイントカード"],
  ["📱", "ポイントアプリ"],
];

/** 有人モード：ポイントカードの最終確認中のお客様側。案内文と使えるポイントサービスのアイコンだけ */
function customerPointAsk(): string {
  return `<div class="point-screen">
      <div class="big">ポイントをお付けします</div>
      <div class="point-services">${POINT_SERVICES.map(([icon, name]) => `<span class="point-service"><span class="icon">${icon}</span>${name}</span>`).join("")}</div>
    </div>`;
}

/**
 * 登録画面の最上部に固定する帯：左から買い物点数・合計金額・会計ボタン。
 * 商品登録が一つもないときは出さない（案内を広く見せる）。
 */
/**
 * 現金払いのときだけ、会計ボタンの位置が「投入おわり」に切り替わる。
 * 入金ボックスで Enter を押すのは、このボタンを押す近道
 */
function finishInsert(reg: number, inserted: number, due: number, st: ActionState): string {
  const ready: ActionState = !st.allowed
    ? st
    : !inserted
      ? { ...st, allowed: false, reason: "お金を入れてください" }
      : due > 0
        ? { ...st, allowed: false, reason: `あと ${yen(due)} 入れてください` }
        : st;
  return btn("投入おわり", { type: "finish_insert", register: reg }, ready, "primary");
}

export function regTop(total: number, count: number, action: string, max = Infinity, pointCard?: boolean, voidHtml = ""): string {
  if (count === 0) return "";
  const full = count >= max ? `<small class="full">上限</small>` : "";
  // 上の段にポイントカードの読み取り状況を控えめに、下の段に「合計 x点 XXX円」を出す
  // （ポイントカードの状況はほかの場所には出さない。ボタンは合計額の右に出し、有無で点数・合計額の位置は動かない）
  const point =
    pointCard === undefined ? "" : `<span class="point ${pointCard ? "read" : ""}">ポイント：${pointCard ? "読取済" : "未読取"}</span>`;
  // 取引中止は上の段の左寄り（会計ボタンから離す）。登録中・確認中・会計中のどこからでも押せる
  const info = point || voidHtml ? `<small class="info">${point}${voidHtml}</small>` : "";
  return `<div class="reg-top">${info}<span class="sum">合計 <span class="count"><b>${count}</b>点${full}</span> <b class="total">${yen(total)}</b></span><span class="action">${action}</span></div>`;
}

/** 買い物点数（数量の合計） */
export function itemCount(lines: { qty: number }[]): number {
  return lines.reduce((n, l) => n + l.qty, 0);
}

/**
 * 二つの画面は、同じ内容を映す（情報空間は一つ）。中身はモードで決まり、
 * どちらの画面もカラーで、その画面から触れない操作だけをグレーにする。
 * 例外は呼び出し中の表示（`callNotice`）と、有人モードのお客様側の画面。
 * 有人モードのお客様側には、お客様本人が押すボタン（お支払い内容の確認・投入おわり）だけを出す。
 */
export function sharedScreen(r: RegisterView, snap: Snapshot, side: Side): string {
  // レジ休止中：どちらの画面も休止の案内だけ（店員側には再開のしかた）
  if (r.paused)
    return confirm(`<div class="point-screen paused-screen"><div class="big">レジ休止中</div><p>${
      side === "customer" ? "お手数ですが、ほかのレジをご利用ください" : "スタッフバーコードを読むと再開します"
    }</p></div>`);
  // セミセルフの呼び出し中、店員側の画面は呼び出しの理由だけを大きく出す（年確の工程などは出さない：
  // お客様が店員側に回り込んで操作できないよう、店員がログインして有人モードになってから進める）
  if (r.mode === "semi_self" && side === "staff" && !r.menu && r.staff.calls.length) return callScreen(r, snap.now);
  const notice = callNotice(r, snap.now, side);
  if (!r.menu && (r.emoney || r.emoney_short !== null || r.emoney_partial !== null)) return notice + confirm(emoneyScreen(r, side));
  return r.mode === "attended" ? attendedScreen(r, notice, side) : semiSelfScreen(r, notice, side);
}

/**
 * 電子マネーの選択（チャージ・残高確認）・チャージ・残高確認・残高不足。登録画面を置き換えて一つだけ出す。
 * ボタンを押すのはセミセルフではお客様、有人モードでは店員（お客様側には案内だけ）
 */
function emoneyScreen(r: RegisterView, side: Side): string {
  const reg = r.id;
  const ro = r.mode === "attended" && side === "customer";
  const actions = r.mode === "semi_self" ? r.customer.actions : r.staff.actions;
  const close = (label: string) => (ro ? "" : btn(label, { type: "close_emoney", register: reg, side }, find(actions, "emoney"), "skip"));
  if (r.emoney_partial !== null) {
    // 「不足分を現金で払う」：残高を引き出し残りを現金で払うことを確かめてから、電子マネーを受け付ける
    // （かざしたままの電子マネーで、案内を見ないうちに決済が走らないように）
    const rest = Math.max(0, r.staff.due - r.emoney_partial);
    const st = find(actions, "resolve_emoney_short");
    const go = (label: string, c: string, cls: string) => btn(label, { type: "resolve_emoney_short", register: reg, side, choice: c }, st, cls);
    const note = `<p>電子マネーの残高 <b>${yen(r.emoney_partial)}</b> を引き出し、残り <b>${yen(rest)}</b> を現金でお支払いいただきます</p>`;
    if (!r.emoney_partial_ok)
      return `<div class="point-screen emoney-screen">
          <div class="big">お支払い方法の確認</div>${note}
          ${ro ? "" : `<div class="emoney-foot">${go("他の決済を使う", "cancel", "skip")}${go("確認しました", "cash_confirmed", "primary")}</div>`}
        </div>`;
    return `<div class="point-screen emoney-screen">
        <div class="big">電子マネーを入金ボックスにかざしてください</div>${note}
        ${ro ? "" : `<div class="emoney-foot">${go("他の決済を使う", "cancel", "skip")}</div>`}
      </div>`;
  }
  if (r.emoney_short !== null) {
    const st = find(actions, "resolve_emoney_short");
    const choice = (label: string, c: string, cls = "") => btn(label, { type: "resolve_emoney_short", register: reg, side, choice: c }, st, cls);
    return `<div class="point-screen emoney-screen">
        <div class="big">電子マネーの残高が足りません</div>
        <p>残高 <b>${yen(r.emoney_short)}</b>・お支払い <b>${yen(r.staff.due)}</b></p>
        ${ro ? "" : `<div class="emoney-choices">${choice("他の決済を使う", "cancel", "skip")}${choice("不足分を現金で払う", "cash", r.mode === "attended" ? "" : "primary")}${r.mode === "attended" ? choice("電子マネーをチャージする", "charge", "primary") : ""}</div>`}
      </div>`;
  }
  const e = r.emoney!;
  if (e.kind === "menu") {
    const go = (label: string, kind: string) => btn(label, { type: "start_emoney", register: reg, side, kind }, find(actions, "emoney"));
    return `<div class="point-screen emoney-screen">
        <div class="big">電子マネー</div>
        <p class="muted">選択してください</p>
        ${ro ? "" : `<div class="emoney-choices row">${go("チャージ", "charge")}${go("残高確認", "inquiry")}</div>${close("やめる")}`}
      </div>`;
  }
  if (e.kind === "inquiry")
    return e.balance === null
      ? `<div class="point-screen emoney-screen"><div class="big">電子マネーを入金ボックスにかざしてください</div>${close("やめる")}</div>`
      : `<div class="point-screen emoney-screen"><p>電子マネーの残高</p><div class="big emoney-balance">${yen(e.balance)}</div>${close("閉じる")}</div>`;
  return `<div class="point-screen emoney-screen">
      <div class="big">チャージする紙幣を入れてください</div>
      <p>${e.inserted ? "入れおわったら、電子マネーを入金ボックスにかざしてください" : "入金ボックスに入れます。1,000円札から使えます（残高は 20,000円まで）"}</p>
      ${close("やめる")}
    </div>`;
}

/** 残りを現金で払うとき、電子マネーで払った分を知らせる */
/** キャッシュレスで先に払ったあと、残りを現金で払う案内（一行に収める） */
function prepaidNote(r: RegisterView, due: number): string {
  return `<p>${esc(r.prepaid_method ?? "")}で ${yen(r.prepaid)} お支払い済み。残り <b>${yen(due)}</b> を現金で</p>`;
}

/** 現金のみの商品（切手・代行収納）があるとき、支払方法を選ぶ前に知らせる */
function cashOnlyNote(r: RegisterView): string {
  if (!r.cash_only || r.prepaid) return "";
  return r.cashless_due
    ? `<p>切手・お支払いの <b>${yen(r.cash_only)}</b> は現金のみ（併用できるのは電子マネー）</p>`
    : `<p>${CASH_ONLY_NOTE}</p>`;
}

// ---------------------------------------------------------------------------
// レジ画面のヘッダー：左上に稼働モードのバッジ、中央に日付と時刻、右上に呼び出しベル。
// バッジは両方の画面に出す。有人モードの店員側では、バッジを押すと店舗メニュー（返品・返金、自動釣銭機など）が開く。
// どちらが店員側・お客様側かは設置で自明なので画面には出さない
// ---------------------------------------------------------------------------

const pad = (n: number) => String(n).padStart(2, "0");

export function clockText(d: Date): string {
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日 ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function screenHead(r: RegisterView, now: Date, side: Side): string {
  // 有人モードでは、店員側のベルは仲間に SOS を出すのに使う（お客様側は店員がいるので押せない）
  const call = find(r.customer.actions, "call_staff");
  const sos = r.mode === "attended";
  const on = call.allowed && !(sos && side === "customer");
  const bellTip = sos ? "仲間の店員を呼ぶ（押している間鳴ります）" : "係員を呼ぶ（押している間鳴ります）";
  const reason = sos && side === "customer" ? "店員が対応中です" : (call.reason ?? "");
  const bell = `<button type="button" class="bell-btn" data-hold="${esc(JSON.stringify({ type: "call_staff", register: r.id }))}" aria-label="${bellTip}" title="${esc(on ? bellTip : reason)}"${on ? "" : " disabled"}>🔔</button>`;
  const label = r.mode === "attended" ? "有人モード" : "セミセルフモード";
  const cls = `mode-badge ${r.mode === "attended" ? "attended" : "semi"}`;
  // 店舗メニューは両方の画面のバッジから開ける。開いた側の画面にだけ、その側の項目を出す
  const open = r.menu?.side === side || (side === "customer" && !!r.menu_code);
  const what = side === "staff" ? "ジャーナル確認・廃棄登録・FF鮮度管理など" : "自動釣銭機へのお金の出し入れ。店員画面に出るパスワードで開く";
  const badge =
    r.mode === "attended" && r.staff.state === "ready"
      ? `<button type="button" class="${cls}" data-gear="${r.id}:${side}" aria-label="レジ${r.id}の店舗メニュー" title="${open ? "店舗メニューを閉じる" : `店舗メニュー（${what}）`}"${open ? ' aria-expanded="true"' : ""}>${label}<span class="caret">▾</span></button>`
      : `<span class="${cls}">${label}</span>`;
  // 休止中は画面の「レジ休止中」と重ねて出さない（稼働モードのバッジは出さない）
  return `<span class="head-l">${r.paused ? "" : badge}</span><time class="clock">${clockText(now)}</time><span class="head-r">${bell}</span>`;
}

// ---------------------------------------------------------------------------
// バーコードなしアイテム欄（レジ画面の下 1/3）
// - セミセルフ：店員呼び出しベルのショートカット（FF・サービス・たばこ）。触れている間だけ鳴る
// - 有人・セルフ：FF をタブ（フライヤー・ドリンク・コールド・中華まん・おでん）で選ぶ。
//   各タブはページを持ち、売れ筋順に並ぶので先頭ページの最上部に売れ筋が来る
// ---------------------------------------------------------------------------

export const FF_TABS = [
  ["fried", "フライヤー"],
  ["drink", "ドリンク"],
  ["cold", "コールド"],
  ["bun", "中華まん"],
  ["oden", "おでん"],
] as const;
export const FF_PER_PAGE = 4;
export type FfUi = { tab: string; page: number };

export function itemsPanel(r: RegisterView, snap: Snapshot, ui: FfUi, side: Side): string {
  const reg = r.id;
  // 店舗メニューと電子マネーの手続き（チャージ・残高不足）は登録画面を置き換えるので、最下段も出さない
  // 取引中止の確認中も、確認画面が登録画面を置き換えるので出さない
  if (r.paused || r.menu || r.menu_code || r.emoney || r.emoney_short !== null || r.emoney_partial !== null || voidAsk.has(reg)) return "";
  if (r.mode === "semi_self") {
    const a = (n: string) => find(r.customer.actions, n);
    const callBtn = (main: string, sub: string, cmd: Command, st: ActionState, cls: string) =>
      `<button class="call-btn ${cls}" data-hold="${esc(JSON.stringify(cmd))}"${st.allowed ? "" : ` disabled title="${esc(st.reason ?? "")}"`}><b>${esc(main)}</b>${sub ? `<small>${esc(sub)}</small>` : ""}</button>`;
    return `<div class="call-row">${[
      callBtn("FF", "揚げ物・中華まん", { type: "ff_button", register: reg }, a("ff_button"), "ff"),
      callBtn("サービス", "お支払い・荷物", { type: "call_staff", register: reg, topic: "service" }, a("call_staff"), "service"),
      callBtn("たばこ", "", { type: "call_staff", register: reg, topic: "tobacco" }, a("call_staff"), "tob"),
    ].join("")}</div>`;
  }
  // 有人モードの FF は店員側の画面から登録する（お客様側には出さない）
  if (r.mode === "attended" && side === "customer") return "";
  // 会計に進んだら FF は登録できない（登録に戻る）ので、タブも出さない
  if (r.mode === "attended" && r.staff.phase === "paying") return "";
  // 確認画面（代行収納の確認・クーポンの返却など）を出している間は、登録画面の一部である FF タブも出さない
  if (r.mode === "attended" && (r.staff.bill_check !== null || r.staff.returned.length || r.staff.confiscated.length || r.staff.ff_collect.length)) return "";
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
  journal: "ジャーナル確認",
  waste: "廃棄登録",
  ff_fresh: "FF鮮度管理",
};

/** 店舗メニューの項目：お客様側は自動釣銭機へのお金の出し入れ、店員側はそれ以外 */
const MENU_ITEMS: Record<Side, MenuScreen[]> = {
  customer: ["refund", "emergency", "deposit", "expense", "changer"],
  staff: ["journal", "waste", "ff_fresh"],
};

/**
 * お客様側から店舗メニューを開くときの使い捨てパスワード。店員画面にだけパスワードを出し、
 * お客様側の画面で入力する（パスワードを見た人でなければ開けない）
 */
function menuCode(r: RegisterView, side: Side): string {
  const reg = r.id;
  if (side === "staff")
    return confirm(`<div class="point-screen menu-code-show">
      <p>お客様側の画面から店舗メニューを開こうとしています</p>
      <div class="big menu-code-value" aria-label="パスワード">${esc(r.menu_code ?? "")}</div>
      <p class="muted">このパスワードをお客様側の画面で入力してください。一度きり有効です</p>
    </div>`);
  return confirm(`<div class="point-screen menu-code">
      <div class="big">店舗メニューのパスワード</div>
      <p>店員側の画面に出ている 4 桁を入力してください</p>
      <div class="row menu-code-form">
        <input type="text" maxlength="4" autocomplete="off" autocapitalize="characters" spellcheck="false" placeholder="例 U8X3" data-menucode="1" data-key="mc-${reg}" aria-label="パスワード" />
        <button class="primary" data-form="menucode" data-reg="${reg}">開く</button>
      </div>
      <div class="pay-choices">${btn("やめる", { type: "open_menu", register: reg, side: "customer", screen: null }, undefined, "ghost")}</div>
    </div>`);
}

/** 一度に一つの画面だけを出す：一覧 → 各項目の画面 */
export function storeMenu(r: RegisterView): string {
  const v = r.staff;
  const reg = r.id;
  const m = r.menu!;
  const side = m.side;
  const go = (screen: MenuScreen | null, extra: Record<string, unknown> = {}) => ({ type: "open_menu", register: reg, side, screen, ...extra });
  const head = (title: string) =>
    `<div class="menu-head">${btn("‹ 戻る", go("top"), undefined, "small ghost")}<b>${esc(title)}</b></div>`;
  const busy: ActionState | undefined = v.lines.length > 0 ? { action: "open_menu", allowed: false, reason: "取引中は使えません" } : undefined;
  switch (m.screen) {
    case "top":
      return `<div class="menu-head"><b>店舗メニュー</b><span class="muted">担当：${esc(v.name ?? "")}</span></div>
        <div class="menu-list">${MENU_ITEMS[side]
          .map((k) => btn(MENU_LABEL[k], go(k), side === "customer" ? busy : undefined, "menu-item"))
          .join("")}</div>
        <div class="menu-list">
          ${btn("ログアウト", { type: "end_staff_mode", register: reg }, find(v.actions, "end_staff_mode"), "menu-item")}
          ${btn("レジ休止中", { type: "pause", register: reg }, find(v.actions, "pause"), "menu-item")}
        </div>`;
    case "journal":
    case "waste":
    case "ff_fresh":
      return `${head(MENU_LABEL[m.screen])}
        <p class="muted">準備中です（今後つくりこみます）。</p>`;
    case "refund":
    case "emergency":
    case "expense": {
      const cout = find(v.actions, "cash_out");
      return `${head(MENU_LABEL[m.screen])}
        <p class="muted">お金を抜く操作です。お金は下の「出金」ボックスに出ます。記録は担当者名つきで残ります。</p>
        <div class="menu-form">
          <input type="number" inputmode="numeric" min="1" placeholder="金額" data-key="mo-amt-${reg}" />
          <button class="primary" data-form="cashout" data-reg="${reg}" data-kind="${m.screen}" ${cout.allowed ? "" : `disabled title="${esc(cout.reason ?? "")}"`}>出金</button>
        </div>`;
    }
    case "deposit":
      return `${head(MENU_LABEL.deposit)}
        <p>入金する金額を下の<b>「入金」ボックス</b>に入れて Enter を押してください。</p>
        <p class="muted">記録は担当者名つきで残ります。</p>`;
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
  if (r.menu?.side === side) return notice + storeMenu(r);
  const c: CustomerView = r.customer;
  const reg = r.id;
  const a = (n: string) => find(c.actions, n);
  if (r.last_receipt && !c.lines.length) return notice + thanks();


  // 新しい動きのあった行ほど上に出す
  const lineRows = c.lines.map((l): Row => {
      const tag =
        l.kind === "ff"
          ? ffTag(reg, side, l, a("check_ff"), false, { waiting: "お受け取り前", received: "お受け取り済" })
          : l.kind === "service" && !l.done
            ? `<span class="tag warn">係員が受付します</span>`
            : "";
      const qty = c.phase === "registering" ? qtyBtn(reg, side, l.id, l.qty, a("remove_line")) : `${l.qty}`;
      const editor = c.phase === "registering" ? lineEditor(reg, side, l, a("remove_line"), true) : "";
      return [l.touched, `<tr class="${flash(reg, l.id)} ${editingRow(reg, side, l.id)}"><td>${esc(l.name)} ${[tag, warnTags(l)].filter(Boolean).join(" ")}</td><td class="num">${qty}</td><td class="num">${l.kind === "service" && !l.done ? "—" : yen(l.amount)}</td></tr>${editor}`];
    });
  const rows = newestFirst([...couponRows(reg, side, c.coupons, a("remove_coupon")), ...lineRows]);

  // 合計とお会計ボタンは登録画面の最上部に常に出す
  const top = regTop(
    c.total,
    itemCount(c.lines),
    // セミセルフではお買い物の途中に電子マネーのチャージ・残高確認はできない（ボタンを出さない）
    (c.lines.length || c.coupons.length ? "" : emoneyEntry(reg, side, a("emoney"), "emoney-top")) +
    (c.phase === "registering"
      ? btn("お会計", { type: "checkout", register: reg, side }, a("checkout"), "primary")
      : c.method === "cash"
        ? finishInsert(reg, c.inserted, c.due, a("insert_cash"))
        : btn("商品登録に戻る", { type: "back_to_registering", register: reg, side }, a("back_to_registering"), "ghost")),
    r.max_items,
    !!c.point_card,
    voidAsk.has(reg) ? "" : voidBtn(reg, side, a("void_transaction")),
  );

  if (!c.lines.length && !c.coupons.length) voidAsk.delete(reg);
  if (!c.lines.length && !c.coupons.length && !c.returned.length) return `${top}${notice}${welcome(reg, side, a("emoney"))}`;
  // 一つの画面では一つの確認だけを、登録画面を置き換えて出す（優先順は有人モードと同じ）
  const screen = (body: string) => `${top}${notice}${confirm(body)}`;
  if (voidAsk.has(reg)) return screen(voidConfirm(r, side, c));
  if (c.bill_slips) return screen(billSlipsCard(reg, side, c.bill_slips, a("declare_bill_count")));
  if (c.bill_check !== null) return screen(billCard(reg, c.bill_count, c.bill_check, a("confirm_bill_payment")));
  if (c.phase === "paying" && c.age_wait) return screen(ageWait(c.age_wait));
  const card = firstOf([
    c.coupon_unmatched ? unmatchedCard(reg, side, c.coupons, a("remove_coupon")) : "",
    returnedCard(reg, side, c.returned, true),
  ]);
  if (card) return screen(card);
  if (c.phase === "paying" && c.point_pending) return screen(pointCheck(reg, side, a("confirm_point_card")));
  scanGuide.delete(`${reg}:${side}`);
  if (c.phase === "paying")
    return screen(`<div class="paybox center">
        ${methodRow(reg, side, c.method, c.inserted, a("select_payment"), "center", r.unavailable)}${r.prepaid && !c.inserted && !c.payment_blocker ? prepaidNote(r, c.due) : r.cash_only && !c.method && !c.inserted && !c.payment_blocker ? cashOnlyNote(r) : payHint(c.method, c.payment_blocker, c.inserted, c.due, r.cashless_due)}
      </div>`);
  if (ffVerifying.has(`${reg}:${side}`) && ffVerifyRows(reg, side, c.lines).length)
    return screen(ffVerifyScreen(reg, side, c.lines, true));
  closeFfVerify(reg, side);
  return `${top}${notice}
    <div class="staff-head">${taxNote(c)}</div>
    <table class="lines cols3">${LINE_COLS}<thead><tr><th>品目</th><th class="num">数量</th><th class="num">小計</th></tr></thead><tbody>${rows}</tbody></table>`;
}

// ---------------------------------------------------------------------------
// Bell（呼び鈴：据え付け型・携帯型の検品端末・ストアコンピューター）・店舗の設定
// ---------------------------------------------------------------------------

const BELL_ICON = { fixed: "🔔", handheld: "📱", store_computer: "🖥️" } as const;

export function matPanel(s: Snapshot): string {
  return s.mats
    .map(
      (m) => `<div class="mat ${m.buzzing ? "buzz" : ""} ${m.bell_on ? "" : "off"}">
        <span class="bell">${BELL_ICON[m.kind]}</span><span class="mat-id">${esc(m.id)}</span>${!m.bell_on ? `<span class="mat-state">呼び鈴OFF</span>` : m.buzzing ? `<span class="mat-state">鳴動中</span>` : ""}
      </div>`,
    )
    .join("");
}

/** Bell の設定：可搬性端末とストアコンピューターの呼び鈴機能の ON/OFF（据え付け端末は常に ON） */
export function bellSettings(s: Snapshot): string {
  return s.mats
    .map((m, i) =>
      m.kind === "fixed"
        ? `<label class="bell-setting"><span>${BELL_ICON[m.kind]} ${esc(m.id)}</span><input type="checkbox" checked disabled aria-label="${esc(m.id)}の呼び鈴機能" /><span class="muted">常に ON</span></label>`
        : `<label class="bell-setting"><span>${BELL_ICON[m.kind]} ${esc(m.id)}</span><input type="checkbox" data-bell-on="${i}"${m.bell_on ? " checked" : ""} aria-label="${esc(m.id)}の呼び鈴機能" /><span>${m.bell_on ? "ON" : "OFF"}</span></label>`,
    )
    .join("");
}

const AGE_CHOICES = [15, 18, 20];
const LIMIT_CHOICES = [1, 2, 3, 4, 5];

/** 販売個数の上限を選ぶ欄（1 回のお会計で何点まで） */
function limitSelect(p: Snapshot["catalog"][number]): string {
  const ns = LIMIT_CHOICES.slice();
  if (p.limit && !ns.includes(p.limit)) ns.push(p.limit);
  ns.sort((x, y) => x - y);
  const opts = [`<option value="" ${p.limit ? "" : "selected"}>制限なし</option>`]
    .concat(ns.map((n) => `<option value="${n}" ${p.limit === n ? "selected" : ""}>${n}点まで</option>`))
    .join("");
  return `<select data-limit="${esc(p.code)}" aria-label="${esc(p.name)}の販売個数の上限">${opts}</select>`;
}

export type RulesTab = "age" | "limit" | "tax";

const RULES_TABS: { id: RulesTab; label: string; note: string }[] = [
  {
    id: "age",
    label: "年確設定",
    note: "年確は会計ボタンを押した時点で、カゴの中で最も厳しい年齢で1回だけ行います。酒・たばこは法令どおり20歳以上で固定され、変更できません。",
  },
  { id: "limit", label: "制限設定", note: "1回のお会計で売れる販売個数の上限です。超える分はスキャンしても登録されません。" },
  {
    id: "tax",
    label: "税率設定",
    note: "税率は税区分ごとに持ちます。法改正で税率が変わったら区分の税率を変え、新しい税率ができたら区分を追加してください。価格は税込で、1回のお会計で税率ごとに合計してから1回だけ端数処理します。変更はこれから登録する商品から適用されます。",
  },
];

/** 売り場の棚ごとに、商品 1 行ずつの表を作る */
function shelfRows(s: Snapshot, cols: number, row: (p: Snapshot["catalog"][number]) => string): string {
  return SHELVES.map((g) => {
    const items = s.catalog.filter(g.match);
    if (!items.length) return "";
    return `<tr class="grp"><th colspan="${cols}">${esc(g.label)}</th></tr>${items.map(row).join("")}`;
  }).join("");
}

function ageRows(s: Snapshot): string {
  return shelfRows(s, 4, (p) => {
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
  });
}

const TAX_KINDS: { id: TaxKind; label: string }[] = [
  { id: "standard", label: "標準税率" },
  { id: "reduced", label: "軽減税率" },
  { id: "zero", label: "ゼロ税率" },
  { id: "exempt", label: "非課税・不課税" },
];
const ROUNDINGS: { id: Rounding; label: string }[] = [
  { id: "floor", label: "切り捨て" },
  { id: "half_up", label: "四捨五入" },
  { id: "ceil", label: "切り上げ" },
];
const fixedZero = (k: TaxKind) => k === "zero" || k === "exempt";

function taxPanel(s: Snapshot): string {
  const t = s.tax;
  const classes = t.classes
    .map(
      (c) => `<tr><td>${esc(c.name)}</td><td>${esc(c.kind_label)}</td>
        <td class="num">${
          fixedZero(c.kind)
            ? `<span class="muted">0%（固定）</span>`
            : `<input class="rate-in" type="number" min="0" max="100" step="0.01" value="${c.rate_bp / 100}" data-key="rate-${esc(c.id)}" data-tax-rate="${esc(c.id)}" aria-label="${esc(c.name)}の税率（%）" />%`
        }</td></tr>`,
    )
    .join("");
  const classOpts = (cur: string) => t.classes.map((c) => `<option value="${esc(c.id)}" ${c.id === cur ? "selected" : ""}>${esc(c.name)}（${esc(c.rate)}）</option>`).join("");
  const products = shelfRows(
    s,
    2,
    (p) => `<tr><td>${esc(p.name)}</td><td><select data-tax-class="${esc(p.code)}" aria-label="${esc(p.name)}の税区分">${classOpts(p.tax_class)}</select></td></tr>`,
  );
  return `<h4>税区分</h4>
    <table class="lines rules"><thead><tr><th>区分</th><th>種類</th><th class="num">税率</th></tr></thead><tbody>${classes}</tbody></table>
    <div class="row wrap tax-add">
      <input data-key="new-tax-name" data-new-tax="name" placeholder="区分名（例：新軽減税率）" aria-label="追加する税区分の名前" />
      <select data-key="new-tax-kind" data-new-tax="kind" aria-label="追加する税区分の種類">${TAX_KINDS.map((k) => `<option value="${k.id}">${k.label}</option>`).join("")}</select>
      <input class="rate-in" type="number" min="0" max="100" step="0.01" data-key="new-tax-rate" data-new-tax="rate" placeholder="税率" aria-label="追加する税区分の税率（%）" />%
      <button class="small" data-add-tax>区分を追加</button>
    </div>
    <h4>インボイス</h4>
    <div class="row wrap">
      <input data-key="invoice-no" data-invoice="no" value="${esc(t.invoice_no)}" placeholder="登録番号（T＋13桁）" aria-label="適格請求書発行事業者の登録番号" />
      <label>端数処理 <select data-key="invoice-rounding" data-invoice="rounding" aria-label="消費税の端数処理">${ROUNDINGS.map((r) => `<option value="${r.id}" ${r.id === t.rounding ? "selected" : ""}>${r.label}</option>`).join("")}</select></label>
      <button class="small" data-save-invoice>保存</button>
    </div>
    <h4>商品ごとの税区分</h4>
    <table class="lines rules"><thead><tr><th>商品</th><th>税区分</th></tr></thead><tbody>${products}</tbody></table>`;
}

/** 商品ルール設定：オーナー認証のあと、年確・販売個数の制限・税率をセレクタで切り替えて設定する */
export function ownerPanel(s: Snapshot, tab: RulesTab): string {
  const cur = RULES_TABS.find((x) => x.id === tab) ?? RULES_TABS[0];
  const body =
    cur.id === "age"
      ? `<table class="lines rules"><thead><tr><th>商品</th><th>法令</th><th>店舗の年確</th><th class="num">会計時の年確</th></tr></thead><tbody>${ageRows(s)}</tbody></table>`
      : cur.id === "limit"
        ? `<table class="lines rules"><thead><tr><th>商品</th><th>販売個数</th></tr></thead><tbody>${shelfRows(s, 2, (p) => `<tr><td>${esc(p.name)}</td><td>${limitSelect(p)}</td></tr>`)}</tbody></table>`
        : taxPanel(s);
  const hist = s.age_rules.history.length
    ? s.age_rules.history.map((h) => `<li>${esc(h.by)}：${esc(h.change)}</li>`).join("")
    : `<li class="muted">変更はまだありません</li>`;
  return `<label class="rules-tab">設定 <select data-rules-tab aria-label="設定する項目">${RULES_TABS.map((x) => `<option value="${x.id}" ${x.id === cur.id ? "selected" : ""}>${x.label}</option>`).join("")}</select></label>
    <p class="muted">${cur.note}</p>
    ${body}
    <h4>変更履歴</h4><ul>${hist}</ul>`;
}

// ---------------------------------------------------------------------------
// ハードウェア（スキャナー・現金・電子マネー端末）
// ---------------------------------------------------------------------------

type Shelf = { label: string; match: (p: Snapshot["catalog"][number]) => boolean };

/**
 * 売り場の棚。デモのため商品は抽象化し（将来 UI を英語にしても困らない一般名）、
 * コンビニの 4 大分類ごとに代表的な商品を置く（FF はレジ画面下のタブから登録するので置かない）
 */
const byCodes = (...codes: string[]) => (p: { code: string }) => codes.includes(p.code);
const SHELVES: Shelf[] = [
  { label: "日配食品", match: byCodes("4900000000028", "4900000000042", "4900000000011") },
  { label: "加工食品", match: byCodes("4900000000035", "4900000000103", "4900000000073", "4900000000066") },
  { label: "非食品", match: byCodes("4910000000014", "4910000000038", "4900000000127", "4900000000134", "4900000000141") },
  // 4 大分類の外：お客様が持ち込む払込票・送る荷物と受け取る荷物、レジに置いてあるカトラリー・レジ袋
  { label: "サービス", match: (p) => p.category === "service" },
  { label: "オプション", match: (p) => p.category === "bag" },
];

/** 使い方の説明（ボタンにカーソルを重ねると出る） */
const HINT_SCAN = "レジ画面にドラッグ＆ドロップ";
const HINT_CASH = "入金ボックスにドラッグ＆ドロップ";
const HINT_PAY = "支払方法を選んでから、入金ボックスにドラッグ＆ドロップ";
const HINT_EMONEY = "入金ボックスにドラッグ＆ドロップ";

function item(code: string, name: string, sub: string, cls = "", hint = HINT_SCAN): string {
  return `<button type="button" class="item ${cls}" data-code="${esc(code)}" data-hint="${esc(hint)}" aria-label="${esc(name)}をドラッグ。${esc(hint)}">
    <span class="item-name">${esc(name)}</span>${sub ? `<span class="item-sub">${sub}</span>` : ""}</button>`;
}

/** Item：4 大分類ごとに小さな見出しを付けて並べる */
export function shelfPanel(s: Snapshot): string {
  // たばこはカウンターの内側の商品。有人モードのレジがあるときだけ並べ、
  // たばこボタンで呼ばれて有人モードになったら（まだ登録していなければ）枠と文字を点滅させる
  const attended = s.registers.some((r) => r.mode === "attended");
  const asked = s.registers.some((r) => r.tobacco_asked);
  return SHELVES.map((g) => {
    // 棚の並びは SHELVES に書いた順
    const items = s.catalog.filter(g.match).filter((p) => p.category !== "tobacco" || attended);
    if (!items.length) return "";
    return `<div class="shelf"><h3>${esc(g.label)}</h3><div class="items uniform">${items
      .map((p) =>
        item(
          p.code,
          p.name,
          `${p.price ? yen(p.price) : "金額は受付時"}${p.age ? ` ・<b>年確${p.age}</b>` : ""}`,
          p.category === "tobacco" && asked ? "call-blink" : "",
        ),
      )
      .join("")}</div></div>`;
  }).join("");
}

// ---------------------------------------------------------------------------
// 財布（お客様の持ち物）。レジに落とすと、お金は投入口へ、カードは読み取り・端末へ
// 実在のサービス名は使わない
// ---------------------------------------------------------------------------

function walletItem(key: string, name: string, sub: string, cls: string, hint: string): string {
  return `<button type="button" class="item ${cls}" data-wallet="${esc(key)}" data-hint="${esc(hint)}" aria-label="${esc(name)}をドラッグ。${esc(hint)}">
    <span class="item-name">${esc(name)}</span>${sub ? `<span class="item-sub">${sub}</span>` : ""}</button>`;
}

export function walletPanel(emoneyBalance: number): string {
  const bills = [10000, 5000, 2000, 1000].map((d) => walletItem(`cash:${d}`, `${d.toLocaleString()}円`, "", "bill", HINT_CASH));
  const coins = [500, 100, 50, 10, 5, 1].map((d) => walletItem(`cash:${d}`, `${d}円`, "", "coin", HINT_CASH));
  const cards = [
    item("PC12345678", "ポイントカード", "", "card"),
    item("PC87654321", "ポイントアプリ", "", "card"),
    item("CP0001", "引換券", "ソフトドリンク", "coupon"),
    item("CP0002", "割引券", "サンドイッチ 50円引き", "coupon"),
    walletItem("pay:qr", "バーコード決済", "", "pay", HINT_PAY),
    walletItem("pay:credit", "クレジットカード", "", "pay", HINT_PAY),
    walletItem("pay:emoney", "電子マネー", `残高 ${yen(emoneyBalance)}`, "pay", HINT_EMONEY),
  ];
  // 見出しなし（表示スペースの節約）。お金以外は同じ大きさのボタンに揃える
  return `<div class="shelf"><div class="items">${[...bills, ...coins].join("")}</div><div class="items wallet-cards">${cards.join("")}</div></div>`;
}

/** コマンドがどちらの画面からの操作か（お知らせを出す場所を決める） */
export function sideOf(cmd: Command): "staff" | "customer" | "store" {
  if (cmd.side === "staff" || cmd.side === "customer") return cmd.side;
  if (["ff_button", "confirm_bill_payment", "insert_cash", "tap_emoney", "call_staff"].includes(cmd.type)) return "customer";
  if (typeof cmd.register === "number") return "staff";
  return "store";
}
