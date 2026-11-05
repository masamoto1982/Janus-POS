import type {
  ActionState,
  BillRow,
  Parcel,
  Pickup,
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

export function btn(label: string, cmd: Command, a?: ActionState, cls = ""): string {
  const disabled = a && !a.allowed;
  const title = disabled && a.reason ? ` title="${esc(a.reason)}"` : "";
  return `<button class="${cls}" data-cmd="${esc(JSON.stringify(cmd))}"${disabled ? " disabled" : ""}${title}>${esc(label)}</button>`;
}

export function backBtn(cmd: Command, a?: ActionState): string {
  return btn("バック", cmd, a, "md ghost back-btn");
}
const backFootRow = (cmd: Command, a?: ActionState): string => `<div class="screen-foot">${backBtn(cmd, a)}</div>`;
const backCorner = (cmd: Command, a?: ActionState): string => `<div class="back-foot">${backBtn(cmd, a)}</div>`;
const cashCorner = (reg: number, side: Side, method: string | null, st: ActionState): string =>
  `<div class="skip-foot">${btn("現金", { type: "select_payment", register: reg, side, method: "cash" }, st, `md skip${method === "cash" ? " on" : ""}`)}</div>`;

const skipCorner = (cmd: Command, a?: ActionState): string => `<div class="skip-foot">${btn("スキップ", cmd, a, "md skip")}</div>`;

const FF_LABEL: Record<FfState, string> = {
  waiting: "お渡し前",
  received: "お渡し済",
};

function ffTag(reg: number, side: Side, l: { kind: string; state?: FfState }, a: ActionState, ro: boolean, labels = FF_LABEL): string {
  if (l.kind !== "ff" || !l.state) return "";
  const text = labels[l.state];
  if (l.state === "received") return `<span class="ff-done">${text}</span>`;
  if (ro) return "";
  const title = a.allowed ? "押してお渡しした個数を照合" : (a.reason ?? "");
  return `<button type="button" class="ff-btn ff-waiting ff-verify" data-ffverify="${reg}:${side}" title="${esc(title)}"${a.allowed ? "" : " disabled"}>${text}</button>`;
}

export const ffVerifying = new Set<string>();
export const ffvInput = new Map<string, string>();

function closeFfVerify(reg: number, side: Side): void {
  ffVerifying.delete(`${reg}:${side}`);
  for (const k of [...ffvInput.keys()]) if (k.startsWith(`${reg}:${side}:`)) ffvInput.delete(k);
}

function ffVerifyRows<L extends { id: number; kind: string; state?: FfState }>(reg: number, side: Side, lines: L[]): L[] {
  return lines.filter((l) => l.kind === "ff" && (l.state === "waiting" || ffvInput.has(`${reg}:${side}:${l.id}`)));
}

function ffVerifyOpen<L extends { id: number; kind: string; state?: FfState }>(reg: number, side: Side, lines: L[]): boolean {
  if (!ffVerifying.has(`${reg}:${side}`)) return false;
  const rows = ffVerifyRows(reg, side, lines);
  return rows.length > 0 && !rows.every((l) => l.state === "received");
}

function ffVerifyScreen(reg: number, side: Side, lines: { id: number; name: string; kind: string; state?: FfState; touched: number; qty: number; ff_base?: number }[], customer: boolean): string {
  const ff = ffVerifyRows(reg, side, lines).sort((a, b) => b.touched - a.touched);
  const rows = ff
    .map((l) => {
      const key = `${reg}:${side}:${l.id}`;
      const base = l.ff_base ?? 0;
      const need = l.qty - base;
      const name = base > 0 ? `${l.name}（追加分）` : l.name;
      const value = ffvInput.get(key) ?? (l.state === "received" ? String(need) : "");
      const judge = value === "" ? "" : l.state === "received" ? `<span class="ffv-ok">OK</span>` : `<span class="ffv-ng">NG</span>`;
      if (need === 1) {
        const done = l.state === "received";
        return `<tr><td>${esc(name)}</td><td class="num">1</td><td class="num"><input type="checkbox" class="ffv-check" data-ffcheck="${l.id}" data-ffreg="${reg}:${side}" ${done ? "checked disabled" : ""} aria-label="${esc(l.name)}の${customer ? "お受け取り" : "お渡し"}を確認した" /></td><td class="ffv-judge">${done ? `<span class="ffv-ok">OK</span>` : ""}</td></tr>`;
      }
      return `<tr><td>${esc(name)}</td><td class="num">${need}</td><td class="num">${padField(reg, side, "ffv", value, "", `${l.name}の${customer ? "お受け取り" : "お渡し"}数`, l.id, "ffv-input")}</td><td class="ffv-judge">${judge}</td></tr>`;
    })
    .join("");
  return `<div class="ffv-screen">
      <div class="big">お渡しの確認</div>
      <table class="lines ffv"><thead><tr><th>品目</th><th class="num">注文数</th><th class="num">${customer ? "お受け取り数" : "お渡し数"}</th><th class="ffv-judge">照合</th></tr></thead><tbody>${rows}</tbody></table>
      <div class="ffv-foot"><button type="button" class="md ghost back-btn" data-ffverify-close="${reg}:${side}" >バック</button></div>
    </div>`;
}

const METHOD_LABEL: Record<string, string> = { cash: "現金", emoney: "電子マネー", qr: "バーコード決済", credit: "クレジットカード", points: "ポイント利用" };
const METHODS = ["cash", "emoney", "qr", "credit", "points"] as const;

type Side = "staff" | "customer";

function thanks(): string {
  return `<div class="thanks"><div class="big">ありがとうございました</div><div>お忘れ物にご注意ください</div></div>`;
}

function changeDue(side: Side, amount: number): string {
  const msg = side === "staff" ? `お釣り ${yen(amount)} をお返ししてください` : `お釣り ${yen(amount)} と<br>レシートをお受け取りください`;
  return `<div class="thanks"><div class="big">${msg}</div></div>`;
}

export function isoDate(t: Date): string {
  return `${t.getFullYear()}-${pad(t.getMonth() + 1)}-${pad(t.getDate())}`;
}

function idCard(reg: number, age: number, v: StaffView, a: (n: string) => ActionState): string {
  const choose = (idCheck: boolean, label: string, cls = "") =>
    btn(label, { type: "choose_age_check", register: reg, side: "staff", id_check: idCheck }, a("choose_age_check"), cls);
  if (v.age_choice_pending) {
    return `<div class="point-screen">
        <div class="big">${age}歳以上の年齢確認をしますか？</div>
        <div class="point-choices">${choose(true, "する", "primary")}</div>
      </div>${skipCorner({ type: "choose_age_check", register: reg, side: "staff", id_check: false }, a("choose_age_check"))}`;
  }
  const st = a("check_id");
  return `<div class="point-screen id-entry">
      <div class="big">身分証の生年月日を入力してください</div>
      ${padField(reg, "staff", "birth", padValue.get(padValueKey(reg, "birth")) ?? "", "西暦8桁　例 19821002", "身分証の生年月日（西暦8桁）", undefined, "birth-field")}
      <div class="point-choices">
        <button class="primary" data-form="birth" data-reg="${reg}" ${st.allowed ? "" : `disabled title="${esc(st.reason ?? "")}"`}>判定</button>
      </div>
    </div>${skipCorner({ type: "choose_age_check", register: reg, side: "staff", id_check: false }, a("choose_age_check"))}`;
}

function pickupTag(p: Pickup): string {
  if (p.missing) return `<span class="tag warn">見つかりません</span>`;
  return p.matched ? `<span class="tag ok">照合済</span>` : `<span class="tag warn">照合待ち</span>`;
}

function pickupStep(reg: number, p: Pickup, st: ActionState): string {
  const found = (v: boolean | null) => ({ type: "pickup_found", register: reg, found: v });
  const end = { type: "pickup_handover", register: reg };
  if (p.missing) {
    return `<div class="point-screen pickup-step">
        <p class="pickup-no">${esc(p.no)} <span class="tag warn">見つかりません</span></p>
        <div class="big">お客様にお詫びし、運送会社へお問い合わせください</div>
        <div class="point-choices">${btn("探し直す", found(null), st, "ghost")}${btn("お受け取りを終える", end, st, "primary")}</div>
      </div>`;
  }
  return `<div class="point-screen pickup-step">
      <p class="pickup-no">${esc(p.no)} <span class="tag">代金引換</span></p>
      <div class="big">保管場所に荷物はありましたか？</div>
      <div class="point-choices">${btn("無し", found(false), st, "ghost")}${btn("有り", found(true), st, "primary")}</div>
    </div>`;
}

function pickupHandover(reg: number, side: Side, no: string): string {
  if (side === "customer")
    return `<div class="point-screen pickup-step">
      <div class="big">受け取りのサインをお願いします</div>
      <p>レシートとお釣りをお受け取りください</p>
    </div>`;
  return `<div class="point-screen pickup-step">
      <p class="pickup-no">${esc(no)}</p>
      <div class="big">店舗控えにサインをいただき、荷物をお渡しください</div>
      <div class="point-choices">${btn("お渡ししました", { type: "pickup_handover", register: reg }, undefined, "primary")}</div>
    </div>`;
}

function parcelTag(p: Parcel, done: boolean): string {
  if (done && p.size && p.region) return `<span class="tag ok">${p.size}サイズ・${esc(p.region)}あて</span>`;
  return `<span class="tag warn">${p.size ? `${p.size}サイズ・届け先未定` : "受付中"}</span>`;
}

export function pendingParcel(lines: StaffLine[]): StaffLine | undefined {
  return lines.find((l) => l.kind === "service" && !l.done && l.parcel);
}


export const parcelError = new Map<number, string>();
const parcelErr = (reg: number): string => {
  const e = parcelError.get(reg);
  return e ? `<p class="parcel-error" role="alert">${esc(e)}</p>` : "";
};

function parcelNote(reg: number, l: StaffLine): string {
  const p = l.parcel as Parcel;
  const [head, sub] = p.size
    ? ["お届け先を入力しています", `${p.size}サイズでお取り扱いします`]
    : p.sum_cm
      ? ["重さを計測しています", "重さが30kg以内の荷物についてお取り扱いします"]
      : ["長さを計測しています", "三辺の合計が200cm以内の荷物についてお取り扱いします"];
  return pairNote(head, parcelError.get(reg) ?? sub);
}

function parcelWait(): string {
  return `<div class="point-screen pair-note parcel-wait">
      <div class="big">スタッフがお伺いします</div>
      <p class="lead">伝票を貼り付けてお待ちください</p>
      <p class="fine">伝票の貼り付け、梱包はお客様責となります</p>
    </div>`;
}

function parcelStep(reg: number, l: StaffLine, st: ActionState, count: number): string {
  const p = l.parcel as Parcel;
  const off = st.allowed ? "" : ` disabled title="${esc(st.reason ?? "")}"`;
  const no = count > 1 ? `<span class="parcel-no">${count}個目</span>` : "";
  const back = () => backCorner({ type: "parcel_back", register: reg, line_id: l.id }, st);
  const field = (target: PadTarget, placeholder: string, aria: string) =>
    padField(reg, "staff", target, padValue.get(padValueKey(reg, target, l.id)) ?? "", placeholder, aria, l.id, `parcel-${target}`);
  if (!p.size && !p.sum_cm) {
    return `<div class="point-screen parcel-entry">
        <div class="big">${no}取り扱いサイズを決定してください</div>
        ${parcelErr(reg) || `<p class="parcel-size">三辺の長さの合計を入力してください</p>`}
        <div class="parcel-fields"><label>長さ${field("pcm", "", "荷物の三辺の合計（cm）")}<span>cm</span></label></div>
        <div class="point-choices"><button class="primary" data-form="parcel-length" data-reg="${reg}" data-line="${l.id}"${off}>決定</button></div>
      </div>${back()}`;
  }
  if (!p.size) {
    return `<div class="point-screen parcel-entry">
        <div class="big">${no}取り扱いサイズを決定してください</div>
        ${parcelErr(reg) || `<p class="parcel-size">重量を入力してください</p>`}
        <div class="parcel-fields"><label>重さ${field("pkg", "", "荷物の重さ（kg）")}<span>kg</span></label></div>
        <div class="point-choices"><button class="primary" data-form="parcel-size" data-reg="${reg}" data-line="${l.id}" data-cm="${p.sum_cm}"${off}>決定</button></div>
      </div>${back()}`;
  }
  return `<div class="point-screen parcel-entry">
      <div class="big">${no}お届け先の郵便番号を入力してください</div>
      ${parcelErr(reg)}
      ${field("post", "例 100-0001", "お届け先の郵便番号（7桁）")}
      <div class="point-choices">
        <button class="primary" data-form="parcel-postal" data-reg="${reg}" data-line="${l.id}"${off}>決定</button>
      </div>
    </div>${back()}`;
}

function parcelAsk(reg: number, lines: StaffLine[], st: ActionState): string {
  const done = lines.filter((l) => l.parcel && l.kind === "service" && l.done);
  const last = done.reduce<StaffLine | undefined>((a, l) => (!a || l.touched > a.touched ? l : a), undefined);
  const p = last?.parcel;
  return `<div class="point-screen parcel-ask">
      ${p && last ? `<p class="parcel-fee">${p.size}サイズ・${esc(p.region ?? "")}あて（〒${esc(p.postal ?? "")}）<b>${p.collect ? "着払い" : yen(last.amount)}</b></p>` : ""}
      <div class="big">次の荷物はありますか？</div>
      <div class="point-choices">
        ${btn("ある", { type: "parcel_next", register: reg, more: true }, st)}
        ${btn("ない", { type: "parcel_next", register: reg, more: false }, st, "primary")}
      </div>
      <p class="muted">受付済み ${done.length}個</p>
    </div>`;
}

export const collected = new Map<number, Set<number>>();

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

function ffCollectCard(reg: number, v: StaffView): string {
  if (!v.ff_collect.length) return "";
  const lines = v.ff_collect.map((c) => `<div class="big">${esc(c.name)}を${c.qty}点回収してください</div>`).join("");
  return `<div class="point-screen collect">
      ${lines}
      <div class="point-choices">${btn("回収しました", { type: "dismiss_ff_collect", register: reg }, { action: "dismiss_ff_collect", allowed: true, reason: null }, "primary")}</div>
    </div>`;
}

function ffReturnCard(v: StaffView): string {
  if (!v.ff_collect.length) return "";
  const items = v.ff_collect.map((c) => `<li><b>${esc(c.name)}</b> × ${c.qty}</li>`).join("");
  return `<div class="ffv-screen ff-return">
      <div class="big">取り消した FF をお返しください</div>
      <ul class="ff-return-list">${items}</ul>
    </div>`;
}


function backFoot(reg: number, side: Side, st: ActionState): string {
  return backCorner({ type: "back_to_registering", register: reg, side }, st);
}

const billN = (n: number): string => `<span class="bill-n">${n}枚</span>`;

export function billVerifyScreen(reg: number, side: Side, bills: BillRow[], st: ActionState): string {
  const off = st.allowed ? "" : ` disabled title="${esc(st.reason ?? "")}"`;
  const rows = bills
    .map(
      (b) =>
        `<tr><td>${esc(b.name)}</td><td>${yen(b.amount)}</td><td class="bill-no">${esc(b.slip_no)}</td><td class="bill-tick"><input type="checkbox" ${b.checked ? "checked" : ""} data-bill-check="${reg}:${side}:${b.line_id}" aria-label="${esc(b.name)} ${yen(b.amount)}を確認した"${off} /></td></tr>`,
    )
    .join("");
  const total = bills.reduce((n, b) => n + b.amount, 0);
  return `<div class="ffv-screen bill-verify">
      <div class="big">受付 ${billN(bills.length)}<span class="bill-sum">合計 ${yen(total)}</span></div>
      <table class="lines ffv bill-table"><thead><tr><th>事業者</th><th>金額</th><th>バーコード番号</th><th class="bill-tick">確認</th></tr></thead><tbody>${rows}</tbody></table>
      <p class="muted">事業者・金額・バーコード番号が合っていれば、一枚ずつチェックを入れてください</p>
    </div>`;
}

export function billPayCard(count: number, selfOperated = false): string {
  return `<div class="point-screen bill-pay">
      <div class="big">公共料金${billN(count)}を受け付けます</div>
      <p>${selfOperated ? "現金はスタッフがお預かりします<br>スタッフをお呼びください" : "現金でお支払いください"}</p>
    </div>`;
}

export function billHandover(reg: number, side: Side, count: number, st: ActionState): string {
  if (side === "customer")
    return `<div class="point-screen bill-done">
      <div class="big">公共料金${billN(count)}　支払いが完了しました</div>
      <p>払込票とレシート、お釣りをお受け取りください</p>
    </div>`;
  return `<div class="point-screen bill-done">
      <div class="big">公共料金${billN(count)}</div>
      <p>払込票に収納印を押してお渡しください</p>
      <div class="point-choices">${btn("お渡し済", { type: "hand_over_bills", register: reg, side }, st, "primary")}</div>
    </div>`;
}

function billNote(count: number, sub: string): string {
  return `<div class="point-screen pair-note"><div class="big">公共料金${billN(count)}</div><p>${esc(sub)}</p></div>`;
}

const confirm = (body: string): string => `<div class="confirm">${body}</div>`;

function pairNote(head: string, sub = ""): string {
  return `<div class="point-screen pair-note"><div class="big">${esc(head)}</div>${sub ? `<p>${esc(sub)}</p>` : ""}</div>`;
}

const firstOf = (cards: string[]): string => cards.find((c) => c) ?? "";

const INSERT_DONE_MS = 30_000;

function payHint(method: string | null, blocker: string | null, inserted: number, due: number, cashlessDue = due, staff = false, settleIn: number | null = null, selfOperated = false): string {
  if (blocker) return `<p class="muted">${esc(blocker)}</p>`;
  if (method && method !== "cash" && cashlessDue < due)
    return `<p>${METHOD_LABEL[method]}で <b>${yen(cashlessDue)}</b>、切手・お支払いの <b>${yen(due - cashlessDue)}</b> はそのあと現金で</p>`;
  if (method === "points") return `<p>Wallet のポイントカードかポイントアプリを、決済端末にかざしてください</p>`;
  if (method && method !== "cash") return `<p>Wallet の${METHOD_LABEL[method]}を、決済端末にかざしてください</p>`;
  if (selfOperated) return `<p>キャッシュレスでお支払いください<br>お支払い方法を選んでください</p>`;
  if (inserted && due > 0) return staff ? `<p>あと <b>${yen(due)}</b> 不足しています</p>` : `<div class="big">不足額 ${yen(due)}</div>`;
  const sec = Math.ceil((settleIn ?? INSERT_DONE_MS) / 1000);
  if (inserted) return staff ? `<p>${sec}秒の無操作で、お支払いを確定します</p>` : `<div class="big">追加投入の受付 残り${sec}秒</div>`;
  if (method === "cash" && staff) return `<p>お預かりした現金を入金ボックスに入れ、「投入おわり」を押してください</p>`;
  if (method === "cash") return `<p>スタッフが現金をお預かりします</p>`;
  return staff ? `<p>お預かりした現金を入金ボックスに入れるか、支払方法を選んでください</p>` : `<p>お支払い方法をお申しつけください</p>`;
}

function methodRow(reg: number, side: Side, method: string | null, inserted: number, st: ActionState, cls = "", unavailable: Unavailable[] = [], noCash = false): string {
  if (method === "cash" && inserted) return "";
  const off = (m: string): ActionState => {
    const u = unavailable.find((x) => x.method === m);
    return u ? { action: st.action, allowed: false, reason: u.reason } : st;
  };
  return `<div class="row method-row ${cls}">${METHODS.filter((m) => !(noCash && m === "cash"))
    .map((m) => btn(METHOD_LABEL[m], { type: "select_payment", register: reg, side, method: m }, off(m), `seg ${method === m ? "on" : ""}`))
    .join("")}</div>`;
}

const CASH_ONLY_NOTE = "現金のみの商品（切手・お支払い）は現金でお支払いください";

function methodList(method: string | null, inserted: number, unavailable: Unavailable[] = []): string {
  if (method === "cash" && inserted) return "";
  return `<div class="row wrap method-list">${METHODS.map((m) => `<span class="seg ${method === m ? "on" : ""}${unavailable.some((u) => u.method === m) ? " off" : ""}">${METHOD_LABEL[m]}</span>`).join("")}</div>`;
}

const LINE_COLS = `<colgroup><col /><col class="c-qty" /><col class="c-amt" /></colgroup>`;

function taxNote(v: { taxes: TaxTotal[] }): string {
  return `<span class="muted tax-note">内税 ${v.taxes.map((t) => `${esc(t.rate)} ${yen(t.tax)}`).join(" / ")}</span>`;
}

function warnTags(l: { age: number | null; limit: number | null; cash_only?: boolean }): string {
  return [
    l.age ? `<span class="tag age">年確${l.age}</span>` : "",
    l.limit ? `<span class="tag age">限定${l.limit}</span>` : "",
    l.cash_only ? `<span class="tag age">現金のみ</span>` : "",
  ].filter(Boolean).join(" ");
}

function staffLine(reg: number, l: StaffLine, v: StaffView, side: Side): string {
  const tags: string[] = [];
  const ro = side === "customer";
  if (l.kind === "ff") tags.push(ffTag(reg, side, l, find(v.actions, "check_ff"), ro));
  if (l.kind === "service" && l.parcel) tags.push(parcelTag(l.parcel, l.done));
  else if (l.pickup) tags.push(pickupTag(l.pickup));
  else if (l.kind === "service" && !isBill(l)) tags.push(`<span class="tag ${l.done ? "ok" : "warn"}">${l.done ? "受付済" : "受付待ち"}</span>`);
  tags.push(warnTags(l));
  const remove = find(v.actions, "remove_line");
  let svc = "";
  if (l.kind === "service" && !l.done && !ro && !l.parcel && !l.pickup) {
    const a = find(v.actions, "complete_service");
    svc = `<div class="svc">
      <input type="number" inputmode="numeric" min="0" placeholder="金額" data-key="svc-${reg}-${l.id}" ${a.allowed ? "" : "disabled"} />
      <button data-form="service" data-reg="${reg}" data-line="${l.id}" ${a.allowed ? "" : "disabled"}>受付</button>
    </div>`;
  }
  const qty =
    ro || isBill(l) || l.pickup || l.parcel
      ? `${l.qty}`
      : qtyCell(reg, side, l, remove, { type: "remove_line", register: reg, side, line_id: l.id }, l.kind === "service");
  return `<tr class="${flash(reg, l.id)}">
    <td>${esc(l.name)} ${tags.join(" ")}${svc}</td>
    <td class="num">${qty}</td>
    <td class="num">${l.kind === "service" && !l.done && !l.amount ? "—" : yen(l.amount)}</td>
  </tr>`;
}

const isBill = (l: { kind: string; done?: boolean; cash_only: boolean }): boolean => l.kind === "service" && !!l.done && l.cash_only;

export const qtyEntry = new Map<string, { id: number; digits: string }>();

function qtyCell(reg: number, side: Side, l: { id: number; qty: number }, st: ActionState, remove: Command, fixed = false): string {
  if (!st.allowed) return `${l.qty}`;
  if (fixed) return `<span class="qty-step">${btn("削除", remove, st, "small neg")}<b class="qty-fixed">${l.qty}</b></span>`;
  const step = (d: number): Command => ({ type: "set_qty", register: reg, side, line_id: l.id, qty: l.qty + d });
  const minus = l.qty <= 1 ? btn("削除", remove, st, "small neg") : btn("−", step(-1), st, "small neg");
  const e = qtyEntry.get(`${reg}:${side}`);
  const typing = e?.id === l.id;
  const shown = typing ? e.digits || "　" : String(l.qty);
  const key = `<button type="button" class="pad-field qty-key${typing ? " typing" : ""}" data-qtyentry="${reg}:${side}:${l.id}" aria-label="数量 ${l.qty}、押すとテンキーで数量を入力">${esc(shown)}</button>`;
  return `<span class="qty-step">${minus}${key}${btn("＋", step(1), st, "small neg")}</span>`;
}

export function qtyPopup(r: RegisterView, side: Side): string {
  const reg = r.id;
  const e = qtyEntry.get(`${reg}:${side}`);
  const l = e && r.staff.lines.find((x) => x.id === e.id);
  if (!e || !l) return "";
  const k = (key: string, label = key, cls = "pad-key") => `<button type="button" class="${cls}" data-qtypad="${reg}:${side}:${key}">${label}</button>`;
  const ok = e.digits !== "" && Number(e.digits) > 0;
  return `<div class="qty-pop" data-qtypad-backdrop="${reg}:${side}">
      <div class="qty-pop-box" role="dialog" aria-label="${esc(l.name)}の数量">
        <div class="qty-pop-name">${esc(l.name)}<span class="muted">いまの数量 ${l.qty}</span></div>
        <div class="qty-pop-value">${esc(e.digits) || `<span class="muted">新しい数量</span>`}</div>
        <div class="qty-pop-keys">${["7", "8", "9", "4", "5", "6", "1", "2", "3"].map((d) => k(d)).join("")}${k("0", "0", "pad-key zero")}${k("back", "←", "pad-key neg")}</div>
        <div class="qty-pop-foot">${k("cancel", "バック", "md ghost back-btn")}<button type="button" class="primary" data-qtypad="${reg}:${side}:ok"${ok ? "" : " disabled"}>決定</button></div>
      </div>
    </div>`;
}

export type PadTarget = "pcm" | "pkg" | "post" | "birth" | "ffv" | "inlet" | "mamt" | "mpw";
export type PadEntry = { target: PadTarget; line?: number; digits: string };
export const padEntry = new Map<string, PadEntry>();
export const padValue = new Map<string, string>();
export const padValueKey = (reg: number, target: PadTarget, line?: number): string => `${reg}:${target}:${line ?? ""}`;
export const PAD: Record<PadTarget, { title: string; unit: string; max: number; decimal?: boolean; code?: boolean }> = {
  pcm: { title: "三辺の長さの合計", unit: "cm", max: 3 },
  pkg: { title: "荷物の重さ", unit: "kg", max: 3, decimal: true },
  post: { title: "お届け先の郵便番号（7桁）", unit: "", max: 7, code: true },
  birth: { title: "身分証の生年月日（西暦8桁）", unit: "", max: 8, code: true },
  ffv: { title: "お渡し数", unit: "", max: 3, code: true },
  inlet: { title: "入金", unit: "円", max: 6 },
  mamt: { title: "出金する金額", unit: "円", max: 6 },
  mpw: { title: "店舗パスワード", unit: "", max: 8, code: true },
};
export const padShow = (target: PadTarget, digits: string): string =>
  target === "mpw" ? "●".repeat(digits.length) : target === "post" && digits.length > 3 ? `${digits.slice(0, 3)}-${digits.slice(3)}` : digits;

function padField(reg: number, side: Side, target: PadTarget, value: string, placeholder: string, aria: string, line?: number, cls = ""): string {
  const typing = padEntry.get(`${reg}:${side}`);
  const on = typing?.target === target && typing.line === line;
  const shown = padShow(target, value);
  return `<button type="button" class="pad-field ${cls}${on ? " typing" : ""}" data-padopen="${reg}:${side}:${target}:${line ?? ""}" aria-label="${esc(aria)}（押すとテンキーで入力）">${
    shown ? esc(shown) : `<span class="placeholder">${esc(placeholder)}</span>`
  }</button>`;
}

export function padPopup(r: RegisterView, side: Side): string {
  const reg = r.id;
  const e = padEntry.get(`${reg}:${side}`);
  if (!e) return "";
  const d = PAD[e.target];
  const k = (key: string, label = key, cls = "pad-key") => `<button type="button" class="${cls}" data-pad="${reg}:${side}:${key}">${label}</button>`;
  const inserted = e.target === "inlet" ? (r.emoney && !r.menu ? r.emoney.inserted : !r.menu ? r.staff.inserted : 0) : 0;
  const ok = e.digits !== "" ? d.code || Number(e.digits) > 0 : e.target === "inlet" && !r.menu && inserted > 0;
  const now = e.target === "inlet" && inserted ? `<span class="muted">入金済み ${yen(inserted)}</span>` : "";
  const name = e.target === "ffv" ? r.staff.lines.find((l) => l.id === e.line)?.name : undefined;
  const title = name ? `${name}の${side === "customer" ? "お受け取り数" : "お渡し数"}` : d.title;
  const last = d.decimal ? `${k("0")}${k(".", ".")}` : k("0", "0", "pad-key zero");
  const shown = padShow(e.target, e.digits);
  return `<div class="qty-pop" data-pad-backdrop="${reg}:${side}">
      <div class="qty-pop-box" role="dialog" aria-label="${esc(title)}">
        <div class="qty-pop-name">${esc(title)}${now}</div>
        <div class="qty-pop-value">${shown ? `${esc(shown)}${d.unit ? `<small> ${d.unit}</small>` : ""}` : `<span class="muted">${d.unit || "　"}</span>`}</div>
        <div class="qty-pop-keys">${["7", "8", "9", "4", "5", "6", "1", "2", "3"].map((x) => k(x)).join("")}${last}${k("back", "←", "pad-key neg")}</div>
        <div class="qty-pop-foot">${k("cancel", "バック", "md ghost back-btn")}<button type="button" class="primary" data-pad="${reg}:${side}:ok"${ok ? "" : " disabled"}>決定</button></div>
      </div>
    </div>`;
}

const CALL_SHORT: Record<string, string> = {
  fast_food: "FF",
  tobacco: "煙草",
  service: "サービス",
  customer_help: "呼び出し",
  age_check: "年確",
  bill: "公共料金",
  change_left: "釣銭忘れ",
  emoney: "電子マネー",
};

export function callNotice(r: RegisterView, _now: number, side: Side): string {
  const v = r.staff;
  const ageOnScreen = r.mode === "staff_operated" && (v.age_choice_pending || v.id_pending);
  const calls = ageOnScreen ? v.calls.filter((c) => c.kind !== "age_check") : v.calls;
  if (side === "customer") {
    const change = r.change_left ? `<div class="banner warn">釣り銭が残っています</div>` : "";
    return (
      change +
      (r.mode === "staff_operated" && r.customer.staff_called && (!ageOnScreen || calls.length)
        ? `<div class="banner call">スタッフが向かっています</div>`
        : "")
    );
  }
  const warn = v.bell_down.length
    ? `<div class="banner danger">${esc(v.bell_down.join("・"))} 不通：呼び出しが届きません</div>`
    : "";
  return warn;
}

function elapsed(calls: { kind: string; created_at: number }[], now: number, before: string, after = ""): string {
  const timed = calls.filter((c) => c.kind !== "age_check");
  if (!timed.length) return "";
  const oldest = Math.min(...timed.map((c) => c.created_at));
  return `<span class="muted">${before}${secs(now, oldest)}経過${after}</span>`;
}

export function callScreen(r: RegisterView, now: number): string {
  const v = r.staff;
  const warn = v.bell_down.length
    ? `<div class="banner danger">${esc(v.bell_down.join("・"))} 不通：呼び出しが届きません</div>`
    : "";
  const kinds = [...new Set(v.calls.map((c) => CALL_SHORT[c.kind] ?? c.label))];
  const details = [...new Set(v.calls.filter((c) => c.kind !== "bill").map((c) => c.detail).filter(Boolean))];
  const bill = v.calls.find((c) => c.kind === "bill");
  return `<div class="call-screen">${warn}
      <div class="call-reason">🔔 ${esc(kinds.join("＆"))}</div>
      ${bill ? `<p class="bill-call"><span class="bill-n">${esc(bill.detail)}</span></p>` : ""}
      ${details.length ? `<p>${esc(details.join("・"))}</p>` : ""}
      <p class="muted">${elapsed(v.calls, now, "", "　")}スタッフバーコードでログインすると対応を始めます</p>
    </div>`;
}

function cashOnlyWait(): string {
  return `<div class="point-screen">
      <div class="big">切手は現金のみのお取り扱いです</div>
      <p>スタッフをお呼びしました<br>そのままお待ちください</p>
    </div>`;
}

function ageWait(age: number): string {
  return `<div class="point-screen">
      <div class="big">年齢確認が必要な商品が含まれています</div>
      <p>${age}歳前後のお客様は身分証をご準備ください</p>
    </div>`;
}

function ageStep(reg: number, v: StaffView, side: Side, a: (n: string) => ActionState): string {
  const age = v.required_age;
  if (!age || side === "customer") return "";
  return v.age_choice_pending || v.id_pending ? idCard(reg, age, v, a) : "";
}

export const flashing = new Set<string>();
const flash = (reg: number, id: number): string => (flashing.has(`${reg}:${id}`) ? "flash" : "");

export function couponRows(reg: number, side: Side, coupons: Coupon[], st: ActionState, ro = false): Row[] {
  return coupons.map((c) => [
    c.id,
    `<tr class="coupon ${flash(reg, c.id)}"><td>${esc(c.name)} <span class="tag">クーポン</span></td><td class="num">${
      ro ? "1" : qtyCell(reg, side, { id: c.id, qty: 1 }, st, { type: "remove_coupon", register: reg, side, coupon_id: c.id }, true)
    }</td><td class="num">${c.discount === null ? `<span class="muted">対象商品待ち</span>` : `−${yen(c.discount)}`}</td></tr>`,
  ]);
}

type Row = [number, string];

function newestFirst(rows: Row[]): string {
  return [...rows]
    .sort((x, y) => y[0] - x[0])
    .map(([, html]) => html)
    .join("");
}

function removeCoupon(reg: number, side: Side, c: Coupon, st: ActionState): string {
  return btn("取消", { type: "remove_coupon", register: reg, side, coupon_id: c.id }, st, "small ghost");
}

export function unmatchedCard(reg: number, side: Side, coupons: Coupon[], st: ActionState, ro = false): string {
  const list = coupons
    .filter((c) => c.discount === null)
    .map((c) => `<li><b>${esc(c.name)}</b> ${ro ? "" : removeCoupon(reg, side, c, st)}</li>`)
    .join("");
  const [head, body] =
    side === "staff"
      ? ["適用先のないクーポンがあります", "対象商品と似た別の商品を通していないか確かめてください<br>対象商品を登録するか、クーポンを取り消すまで決済できません"]
      : ["ご利用になれないクーポンがあります", "クーポンの対象商品とお買い上げの商品が同じか、お確かめください<br>対象商品を追加するか、クーポンを取り消すとお支払いに進めます"];
  return `<div class="card coupon-card"><b>${head}</b><p>${body}</p><ul class="coupon-list">${list}</ul></div>`;
}

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

export const scanGuide = new Map<string, GuideKind>();
export type GuideKind = "item" | "bag" | "point" | "pay" | "coupon" | "shop" | "parcel" | "bill";
const GUIDES: [GuideKind, string, string][] = [
  ["item", "商品", "商品のバーコードをスキャンしてください"],
  ["bag", "レジ袋", "レジ袋のバーコードをスキャンしてください"],
  ["point", "ポイント", "ポイントカードまたはポイントアプリのバーコードをスキャンしてください"],
  ["pay", "お支払い", "支払い用紙のバーコードをスキャンしてください"],
  ["coupon", "クーポン", "引換券、割引券のバーコードをスキャンしてください"],
];

const modeTitle = (staffOperated: boolean): string => `<span class="mode-title ${staffOperated ? "staff-operated" : "self-operated"}">${staffOperated ? "スタッフ操作モード" : "セルフ操作モード"}</span>`;

export function welcome(r: RegisterView, side: Side, emoney?: ActionState): string {
  const reg = r.id;
  const on = scanGuide.get(`${reg}:${side}`);
  const guide = GUIDES.find(([k]) => k === on);
  return `<div class="welcome${emoney ? " scan-welcome" : ""}">
      <div class="big">バーコードをスキャンしてください</div>
      <p>すべてのバーコードを受け付けます</p>
      <div class="guide-row">${GUIDES.map(
        ([k, label]) => `<button type="button" class="guide-btn ${k === on ? "on" : ""}" data-guide="${reg}:${side}:${k}">${label}</button>`,
      ).join("")}</div>
      ${guide ? `<p class="guide">${guide[2]}</p>` : ""}
    </div>`;
}

const EMONEY_HEAD_LABEL = `<span>電子マネーの</span><span>チャージ・残高確認</span>`;

function emoneyHead(r: RegisterView, side: Side): string {
  if (r.paused) return "";
  const staffOperated = r.mode === "staff_operated";
  const v = staffOperated ? r.staff : r.customer;
  const own: Side = staffOperated ? "staff" : "customer";
  const label = `<button type="button" class="emoney-head neg" disabled aria-disabled="true" title="電子マネーのチャージ・残高確認">${EMONEY_HEAD_LABEL}</button>`;
  if (side !== own) return label;
  const shopping = !staffOperated && (v.lines.length > 0 || v.coupons.length > 0 || v.returned.length > 0);
  if (shopping) {
    const st = find(r.customer.actions, "call_staff");
    const cmd = { type: "call_staff", register: r.id, topic: "emoney" };
    return `<button type="button" class="emoney-head neg" data-hold="${esc(JSON.stringify(cmd))}" title="${esc(st.allowed ? "スタッフを呼んで手続きします（押している間鳴ります）" : (st.reason ?? ""))}"${st.allowed ? "" : " disabled"}>${EMONEY_HEAD_LABEL}</button>`;
  }
  const st = find(v.actions, "emoney");
  const cmd = { type: "start_emoney", register: r.id, side, kind: "menu" };
  return `<button type="button" class="emoney-head neg" data-cmd="${esc(JSON.stringify(cmd))}" title="${esc(st.allowed ? "電子マネーのチャージ・残高確認" : (st.reason ?? ""))}"${st.allowed ? "" : " disabled"}>${EMONEY_HEAD_LABEL}</button>`;
}

export const voidAsk = new Map<number, Side>();

function voidBtn(reg: number, side: Side, st: ActionState): string {
  const title = st.allowed ? "取引を中止する（確認があります）" : (st.reason ?? "");
  return `<button type="button" class="void-btn"${st.allowed ? ` data-void-ask="${reg}:${side}"` : " disabled"} title="${esc(title)}">取引中止</button>`;
}

function voidConfirm(r: RegisterView, side: Side, view: { lines: { qty: number }[]; total: number; inserted: number }): string {
  const reg = r.id;
  if (r.mode === "staff_operated" && side === "customer") return pairNote("取引の中止を確認しています");
  const st = find(r.mode === "staff_operated" ? r.staff.actions : r.customer.actions, "void_transaction");
  const back = [
    view.inserted ? `入れたお金 ${yen(view.inserted)} は出金ボックスに戻ります` : "",
    r.prepaid && r.prepaid_method === "電子マネー" ? `電子マネーで払った ${yen(r.prepaid)} は残高に戻ります` : "",
    r.prepaid && r.prepaid_method !== "電子マネー" ? `${esc(r.prepaid_method ?? "")}で払った ${yen(r.prepaid)} は取り消します` : "",
  ]
    .filter(Boolean)
    .map((s) => `<br>${s}`)
    .join("");
  return `<div class="point-screen void-screen">
      <div class="big">取引を中止しますか？</div>
      <p>登録した ${itemCount(view.lines)}点（合計 ${yen(view.total)}）をすべて取り消します${back}</p>
      <div class="void-choices">
        ${btn("はい", { type: "void_transaction", register: reg }, st, "primary")}
      </div>
    </div>
    <div class="back-foot"><button type="button" class="md ghost back-btn" data-void-cancel="${reg}">バック</button></div>`;
}

const STAFF_OP_GUIDES: [GuideKind, string, string][] = [
  ["shop", "お買い物", "商品のバーコードをスキャンしてください"],
  ["parcel", "宅配便", "宅配便の伝票、または受取用のバーコードをスキャンしてください"],
  ["bill", "お支払い", "支払い用紙のバーコードをスキャンしてください"],
];

function staffOperatedWelcome(r: RegisterView, notice: string, side: Side): string {
  const reg = r.id;
  const on = scanGuide.get(`${reg}:staff`);
  const ro = side === "customer";
  const guide = STAFF_OP_GUIDES.find(([k]) => k === on);
  const guides = STAFF_OP_GUIDES.map(
    ([k, label]) =>
      `<button type="button" class="guide-btn ${k === on ? "on" : ""}${ro ? " mirror-only" : ""}"${ro ? ' tabindex="-1" aria-disabled="true"' : ` data-guide="${reg}:staff:${k}"`}>${label}</button>`,
  ).join("");
  const [head, sub] = ro ? ["いらっしゃいませ", "ご用件をお申し付けください"] : ["お辞儀とご挨拶をしましょう", "ご用件をお伺いしてください"];
  return `${notice}<div class="welcome staff-operated-welcome"><div class="big">${head}</div><p>${sub}</p>
      <div class="guide-row">${guides}</div>
      ${guide && !ro ? `<p class="guide">${guide[2]}</p>` : ""}
    </div>`;
}

export function staffOperatedScreen(r: RegisterView, notice = "", side: Side = r.active_side ?? "staff"): string {
  if (r.menu) return notice + (side === "staff" ? storeMenu(r) : confirm(pairNote("休止中", "空いているレジをお使い下さい")));
  const v = r.staff;
  const reg = r.id;
  const a = (n: string) => find(v.actions, n);
  if (r.bill_handover && !v.lines.length) return notice + confirm(billHandover(reg, side, r.bill_handover, a("hand_over_bills")));
  if (r.pickup_handover && !v.lines.length) return notice + confirm(pickupHandover(reg, side, r.pickup_handover));
  if (r.last_receipt && !v.lines.length) return notice + confirm(r.outlet ? changeDue(side, r.outlet) : thanks());
  if (!v.lines.length && !v.coupons.length) voidAsk.delete(reg);
  if (!v.lines.length && !v.coupons.length && !v.returned.length && !v.confiscated.length && !v.ff_collect.length) return staffOperatedWelcome(r, notice, side);
  const ro = side === "customer";
  const rows = newestFirst([...couponRows(reg, side, v.coupons, a("remove_coupon"), ro), ...v.lines.map((l): Row => [l.touched, staffLine(reg, l, v, side)])]);
  const paying = v.phase === "paying";

  const pointPending = paying && v.point_pending;
  const agePending = paying && (v.age_choice_pending || v.id_pending);
  const payStage = paying && !pointPending && !agePending;
  const ask = side === "staff" ? "決済手段をお伺いしてください" : "決済手段をお申しつけください";
  const pay = payStage
    ? `<div class="paybox pay-stage">
        ${v.method || v.inserted ? "" : `<div class="big">${ask}</div>`}
        ${side === "staff" ? methodRow(reg, side, v.method, v.inserted, a("select_payment"), "", r.unavailable) : methodList(v.method, v.inserted, r.unavailable)}
        ${r.prepaid && !v.inserted && !v.payment_blocker && (!v.method || v.method === "cash") ? prepaidNote(r, v.due) : v.method || v.inserted || v.payment_blocker ? payHint(v.method, v.payment_blocker, v.inserted, v.due, r.cashless_due, side === "staff", v.settle_in_ms) : cashOnlyNote(r)}
      </div>`
    : agePending
      ? ageStep(reg, v, side, a)
      : "";

  const top = regTop(
    v.total,
    itemCount(v.lines),
    (paying && v.method === "cash" && !ro
      ? finishInsert(reg, v.inserted, v.due, a("insert_cash"))
      : ro || paying
        ? ""
        : btn("会計", { type: "checkout", register: reg, side }, a("checkout"), "primary")),
    r.max_items,
    !!v.point_card,
    ro || voidAsk.has(reg) ? "" : voidBtn(reg, side, a("void_transaction")),
  );
  const back = paying && !ro ? backFoot(reg, side, a("back_to_registering")) : "";
  const screen = (body: string) => `${top}${notice}${confirm(body + (voidAsk.has(reg) ? "" : back))}`;
  if (voidAsk.has(reg)) return screen(voidConfirm(r, side, v));
  const pickup = v.lines.find((l) => l.pickup)?.pickup;
  if (!paying && pickup && !pickup.matched)
    return screen(
      side === "staff"
        ? pickupStep(reg, pickup, a("complete_service"))
        : pickup.missing
          ? pairNote("申し訳ございません", "お預かりの荷物が見つかりませんでした")
          : pairNote("荷物をお持ちします", "少々お待ちください"),
    );
  const parcel = pendingParcel(v.lines);
  if (!parcel) parcelError.delete(reg);
  if (!paying && parcel)
    return screen(
      side === "staff"
        ? parcelStep(reg, parcel, a("complete_service"), v.lines.filter((l) => l.parcel).length)
        : parcelNote(reg, parcel),
    );
  if (!paying && v.parcel_ask) return screen(side === "staff" ? parcelAsk(reg, v.lines, a("parcel_next")) : pairNote("ほかにお送りする荷物はございますか？"));
  const slips = v.bills.length;
  if (paying && slips && v.bill_unchecked)
    return screen(side === "staff" ? billVerifyScreen(reg, side, v.bills, a("check_bill")) : billNote(slips, "スタッフが払込票を確認しています"));
  if (paying && slips) return screen(side === "customer" ? billPayCard(slips) : billNote(slips, "お客様に受付点数、合計金額をお伝えください"));
  if (agePending) return screen(side === "staff" ? pay : pairNote(v.id_pending ? "身分証を確認しています" : "年齢確認が必要な商品が含まれています"));
  const card = firstOf([
    side === "staff" ? confiscatedCard(reg, v) : v.confiscated.length ? pairNote("販売できない商品があります", "販売不可商品を回収いたします") : "",
    side === "staff" ? ffCollectCard(reg, v) : ffReturnCard(v),
    v.coupon_unmatched ? unmatchedCard(reg, side, v.coupons, a("remove_coupon"), ro) : "",
    returnedCard(reg, side, v.returned, side === "staff"),
  ]);
  if (card) return screen(card);
  if (pointPending) return screen(side === "staff" ? pointCheck(reg, side, a("confirm_point_card")) : customerPointAsk());
  scanGuide.delete(`${reg}:${side}`);
  if (payStage) return screen(pay);
  if (!ro && !paying && ffVerifyOpen(reg, side, v.lines))
    return screen(ffVerifyScreen(reg, side, v.lines, false));
  closeFfVerify(reg, side);
  return `${top}${notice}
    <div class="staff-head">
      <span>スタッフ：<b>${esc(v.name)}</b>${v.role === "owner" ? "（オーナー）" : ""}</span>
      ${taxNote(v)}
    </div>
    <table class="lines cols3">${LINE_COLS}<thead><tr><th>品目</th><th class="num">数量</th><th class="num">小計</th></tr></thead>
      <tbody>${rows || `<tr><td colspan="3" class="muted">明細はありません<br>どのバーコードからでも読めます</td></tr>`}</tbody></table>
`;
}

function pointCheck(reg: number, side: Side, st: ActionState): string {
  const reading = scanGuide.get(`${reg}:${side}`) === "point";
  return `<div class="point-screen">
      <div class="big">ポイントを付与しますか？</div>
      <div class="point-choices">
        <button type="button" class="primary ${reading ? "on" : ""}" data-guide="${reg}:${side}:point">付与する</button>
              </div>
      ${reading ? `<p class="guide">ポイントサービスのバーコードを読み取ってください</p>` : ""}
    </div>${skipCorner({ type: "confirm_point_card", register: reg, side }, st)}`;
}

const POINT_SERVICES: [string, string][] = [
  ["💳", "ポイントカード"],
  ["📱", "ポイントアプリ"],
];

function customerPointAsk(): string {
  return `<div class="point-screen">
      <div class="big">ポイントをお付けします</div>
      <div class="point-services">${POINT_SERVICES.map(([icon, name]) => `<span class="point-service"><span class="icon">${icon}</span>${name}</span>`).join("")}</div>
    </div>`;
}

function finishInsert(reg: number, inserted: number, due: number, st: ActionState): string {
  const ready: ActionState = !st.allowed
    ? st
    : !inserted
      ? { ...st, allowed: false, reason: "お金を入れてください" }
      : due > 0
        ? { ...st, allowed: false, reason: `あと ${yen(due)} 入れてください` }
        : st;
  return `<span class="staff-only">${btn("投入おわり", { type: "finish_insert", register: reg }, ready, "primary")}</span>`;
}

export function regTop(total: number, count: number, action: string, max = Infinity, pointCard?: boolean, voidHtml = "", infoHtml = ""): string {
  if (count === 0) return "";
  const full = count >= max ? `<small class="full">上限</small>` : "";
  const point =
    pointCard === undefined ? "" : `<span class="point ${pointCard ? "read" : ""}">ポイント：${pointCard ? "読取済" : "未読取"}</span>`;
  const info = point ? `<small class="info">${point}</small>` : "";
  const below = infoHtml ? `<small class="info below">${infoHtml}</small>` : "";
  return `<div class="reg-top"><span class="void">${voidHtml}</span><span class="mid">${info}<span class="sum">合計 <span class="count"><b>${count}</b>点${full}</span> <b class="total">${total.toLocaleString("ja-JP")}</b><span class="yen">円</span></span>${below}</span><span class="action">${action}</span></div>`;
}

export function itemCount(lines: { qty: number }[]): number {
  return lines.reduce((n, l) => n + l.qty, 0);
}

export function sharedScreen(r: RegisterView, snap: Snapshot, side: Side): string {
  if (r.paused)
    return confirm(`<div class="point-screen paused-screen"><div class="big">レジ休止中</div><p>${
      side === "customer" ? "お手数ですが、ほかのレジをご利用ください" : "スタッフバーコードを読むと再開します"
    }</p></div>`);
  if (r.mode === "self_operated" && side === "staff" && !r.menu && r.staff.calls.length) return callScreen(r, snap.now);
  const notice = callNotice(r, snap.now, side);
  if (!r.menu && (r.emoney || r.emoney_short !== null || r.emoney_partial !== null)) return notice + confirm(emoneyScreen(r, side));
  return r.mode === "staff_operated" ? staffOperatedScreen(r, notice, side) : selfOperatedScreen(r, notice, side);
}

function emoneyScreen(r: RegisterView, side: Side): string {
  const reg = r.id;
  const ro = r.mode === "staff_operated" && side === "customer";
  const actions = r.mode === "self_operated" ? r.customer.actions : r.staff.actions;
  const close = (label: string) =>
    ro ? "" : label === "バック" ? backCorner({ type: "close_emoney", register: reg, side }, find(actions, "emoney")) : `<div class="screen-foot">${btn(label, { type: "close_emoney", register: reg, side }, find(actions, "emoney"), "skip")}</div>`;
  if (r.emoney_partial !== null) {
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
        <div class="big">電子マネーを決済端末にかざしてください</div>${note}
        ${ro ? "" : `<div class="emoney-foot">${go("他の決済を使う", "cancel", "skip")}</div>`}
      </div>`;
  }
  if (r.emoney_short !== null) {
    const st = find(actions, "resolve_emoney_short");
    const choice = (label: string, c: string, cls = "") => btn(label, { type: "resolve_emoney_short", register: reg, side, choice: c }, st, cls);
    const short = Math.max(0, r.staff.due - r.emoney_short);
    const buttons =
      r.mode === "staff_operated"
        ? `<div class="emoney-choices row pair">${choice("不足額を\n現金で払う", "cash", "primary")}${choice("電子マネーを\nチャージする", "charge", "primary")}</div>
           <div class="emoney-foot">${choice("他の決済を使う", "cancel", "skip")}</div>`
        : `<div class="emoney-choices">${choice("他の決済を使う", "cancel", "primary")}</div>`;
    return `<div class="point-screen emoney-screen">
        <div class="big">電子マネーの残高が足りません</div>
        <p>不足額 <b>${yen(short)}</b></p>
        ${ro ? "" : buttons}
      </div>`;
  }
  const e = r.emoney!;
  if (e.kind === "menu") {
    const go = (label: string, kind: string) => btn(label, { type: "start_emoney", register: reg, side, kind }, find(actions, "emoney"));
    return `<div class="point-screen emoney-screen">
        <div class="big">電子マネー</div>
        <p class="muted">選択してください</p>
        ${ro ? "" : `<div class="emoney-choices row">${go("チャージ", "charge")}${go("残高確認", "inquiry")}</div>`}
      </div>${close("バック")}`;
  }
  if (e.kind === "inquiry")
    return e.balance === null
      ? `<div class="point-screen emoney-screen"><div class="big">電子マネーを決済端末にかざしてください</div></div>${close("バック")}`
      : `<div class="point-screen emoney-screen"><p>電子マネーの残高</p><div class="big emoney-balance">${yen(e.balance)}</div>${close("閉じる")}</div>`;
  return `<div class="point-screen emoney-screen">
      <div class="big">チャージする紙幣を入れてください</div>
      <p>${e.inserted ? "入れおわったら、電子マネーを決済端末にかざしてください" : "入金ボックスに入れます<br>1,000円札から使えます（残高は 20,000円まで）"}</p>
    </div>${close("バック")}`;
}

function prepaidNote(r: RegisterView, due: number): string {
  const rest = r.cashless_due ? "のお支払い方法をお選びください" : "を現金で";
  return `<p>${esc(r.prepaid_method ?? "")}で ${yen(r.prepaid)} お支払い済み、残り <b>${yen(due)}</b> ${rest}</p>`;
}

function cashOnlyNote(r: RegisterView): string {
  if (!r.cash_only || r.prepaid) return "";
  return r.cashless_due
    ? `<p>切手・お支払いの <b>${yen(r.cash_only)}</b> は現金のみ（併用できるのは電子マネー）</p>`
    : `<p>${CASH_ONLY_NOTE}</p>`;
}

const pad = (n: number) => String(n).padStart(2, "0");

const WEEKDAY = ["日", "月", "火", "水", "木", "金", "土"];

export function clockText(d: Date): string {
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日 ${WEEKDAY[d.getDay()]}曜日 ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function screenHead(r: RegisterView, now: Date, side: Side): string {
  const call = find(r.customer.actions, "call_staff");
  const sos = side === "staff";
  const tip = sos ? "仲間のスタッフを呼ぶ（押している間鳴ります）" : "スタッフを呼ぶ（押している間鳴ります）";
  const cmd = sos ? { type: "call_staff", register: r.id, topic: "sos" } : { type: "call_staff", register: r.id };
  const bell =
    !sos && r.mode === "staff_operated"
      ? ""
      : `<button type="button" class="bell-btn" data-hold="${esc(JSON.stringify(cmd))}" aria-label="${tip}" title="${esc(call.allowed ? tip : (call.reason ?? ""))}"${call.allowed ? "" : " disabled"}>🔔</button>`;
  const mode = r.paused ? "" : r.menu ? `<span class="mode-title store-menu">店舗処理モード</span>` : modeTitle(r.mode === "staff_operated");
  return `<span class="head-l"><time class="clock">${clockText(now)}</time>${mode}</span><span class="head-r">${emoneyHead(r, side)}${bell}</span>`;
}

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
  if (r.paused || r.menu || r.emoney || r.emoney_short !== null || r.emoney_partial !== null || voidAsk.has(reg)) return "";
  if (r.mode === "self_operated") {
    const a = (n: string) => find(r.customer.actions, n);
    const callBtn = (main: string, sub: string, cmd: Command, st: ActionState, cls: string) =>
      `<button class="call-btn ${cls}" data-hold="${esc(JSON.stringify(cmd))}"${st.allowed ? "" : ` disabled title="${esc(st.reason ?? "")}"`}><b>${esc(main)}</b>${sub ? `<small>${esc(sub)}</small>` : ""}</button>`;
    return `<div class="call-row">${[
      callBtn("FF", "揚げ物・中華まん", { type: "ff_button", register: reg }, a("ff_button"), "ff"),
      callBtn("サービス", "お支払い・荷物", { type: "call_staff", register: reg, topic: "service" }, a("call_staff"), "service"),
      callBtn("たばこ", "", { type: "call_staff", register: reg, topic: "tobacco" }, a("call_staff"), "tob"),
    ].join("")}</div>`;
  }
  if (r.mode === "staff_operated" && side === "customer") return "";
  if (r.mode === "staff_operated" && r.staff.phase === "paying") return "";
  if (r.mode === "staff_operated" && (r.staff.returned.length || r.staff.confiscated.length || r.staff.ff_collect.length)) return "";
  if (r.mode === "staff_operated" && r.staff.lines.some((l) => l.parcel || l.pickup)) return "";
  if (r.mode === "staff_operated" && r.pickup_handover && !r.staff.lines.length) return "";
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
      ).replace("</button>", `<small>${f.stock > 0 ? (r.mode === "staff_operated" ? `残${f.stock}` : yen(f.price)) : "売り切れ"}</small></button>`),
    )
    .join("");
  const arrows =
    pages > 1
      ? `<div class="ff-arrows">${
          page > 0 ? `<button type="button" class="ff-arrow" data-ffpage="${reg}:-1" aria-label="前のページ">▲</button>` : "<span></span>"
        }${page < pages - 1 ? `<button type="button" class="ff-arrow" data-ffpage="${reg}:1" aria-label="次のページ">▼</button>` : "<span></span>"}</div>`
      : "";
  return `<div class="ff-tabs" role="tablist">${tabs}</div><div class="ff-page"><div class="ff-grid">${grid}</div>${arrows}</div>`;
}

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

const MENU_ITEMS: { screen: MenuScreen; cash: boolean }[] = [
  { screen: "refund", cash: true },
  { screen: "emergency", cash: true },
  { screen: "deposit", cash: true },
  { screen: "expense", cash: true },
  { screen: "changer", cash: true },
  { screen: "journal", cash: false },
  { screen: "waste", cash: false },
  { screen: "ff_fresh", cash: false },
];

export function storeMenuUsable(r: RegisterView): boolean {
  return r.mode === "staff_operated" && r.staff.state === "ready" && !r.paused;
}

export function storeMenu(r: RegisterView): string {
  const v = r.staff;
  const reg = r.id;
  const m = r.menu!;
  const go = (screen: MenuScreen | null, extra: Record<string, unknown> = {}) => ({ type: "open_menu", register: reg, screen, ...extra });
  const head = (title: string) => `<div class="menu-head"><b>${esc(title)}</b></div>`;
  const busy: ActionState | undefined = v.lines.length > 0 ? { action: "open_menu", allowed: false, reason: "取引中は使えません" } : undefined;
  const screen = (body: string) => `<div class="menu-screen">${body}${backFootRow(go("top"))}</div>`;
  switch (m.screen) {
    case "top":
      return `<div class="menu-head"><b>店舗メニュー</b><span class="muted">担当：${esc(v.name ?? "")}</span></div>
        <div class="menu-list">${MENU_ITEMS
          .map((k) => btn(MENU_LABEL[k.screen], go(k.screen), k.cash ? busy : undefined, "menu-item"))
          .join("")}</div>
        <div class="menu-list">
          ${btn("ログアウト", { type: "end_staff_mode", register: reg }, find(v.actions, "end_staff_mode"), "menu-item")}
          ${btn("レジ休止中", { type: "pause", register: reg }, find(v.actions, "pause"), "menu-item")}
        </div>`;
    case "journal":
    case "waste":
    case "ff_fresh":
      return screen(`${head(MENU_LABEL[m.screen])}
        <p class="muted">準備中です（今後つくりこみます）</p>`);
    case "refund":
    case "emergency":
    case "expense": {
      const cout = find(v.actions, "cash_out");
      const amt = padValue.get(padValueKey(reg, "mamt")) ?? "";
      const pw = padValue.get(padValueKey(reg, "mpw")) ?? "";
      const ready = cout.allowed && Number(amt) > 0 && pw !== "";
      return screen(`${head(MENU_LABEL[m.screen])}
        <p class="muted">お金を抜く操作です<br>操作のたびに店舗パスワードが要ります<br>お金は下の「出金」ボックスに出ます<br>記録は担当者名つきで残ります</p>
        <div class="menu-form">
          ${padField(reg, "staff", "mamt", amt, "金額（円）", "出金する金額")}
          ${padField(reg, "staff", "mpw", pw, "店舗パスワード", "店舗パスワード")}
          <button class="primary" data-form="cashout" data-reg="${reg}" data-kind="${m.screen}" ${ready ? "" : `disabled title="${esc(cout.allowed ? "金額と店舗パスワードを入れてください" : (cout.reason ?? ""))}"`}>出金</button>
        </div>`);
    }
    case "deposit":
      return screen(`${head(MENU_LABEL.deposit)}
        <p>入金する金額を下の<b>「入金」ボックス</b>に入れて Enter を押してください</p>
        <p class="muted">記録は担当者名つきで残ります</p>`);
    case "changer": {
      const have = (d: number) => v.drawer.find(([x]) => x === d)?.[1] ?? 0;
      const segs = [1000, 500, 100, 50, 10]
        .map((d) => btn(`${d.toLocaleString()}円`, go("changer", { exchange_to: d }), undefined, `seg small ${m.exchange_to === d ? "on" : ""}`))
        .join("");
      return screen(`${head(MENU_LABEL.changer)}
        <div class="row wrap">受け取る金種 ${segs}</div>
        <p>両替する金額を<b>「入金」ボックス</b>に入れて Enter を押してください<br>同じ額を ${m.exchange_to.toLocaleString()}円 で<b>「出金」ボックス</b>に出します</p>
        <p class="muted">釣銭機の ${m.exchange_to.toLocaleString()}円：残り ${have(m.exchange_to)} 枚　在高 ${yen(v.drawer_total)}</p>`);
    }
  }
}

export function selfOperatedScreen(r: RegisterView, notice = "", side: Side = r.active_side ?? "customer"): string {
  const c: CustomerView = r.customer;
  const reg = r.id;
  const a = (n: string) => find(c.actions, n);
  if (r.bill_handover && !c.lines.length) return notice + confirm(billHandover(reg, side, r.bill_handover, a("hand_over_bills")));
  if (r.last_receipt && !c.lines.length) return notice + confirm(thanks());

  const lineRows = c.lines.map((l): Row => {
      const tag =
        l.kind === "ff"
          ? ffTag(reg, side, l, a("check_ff"), true, { waiting: "お受け取り前", received: "お受け取り済" })
          : l.kind === "service" && !l.done
            ? `<span class="tag warn">スタッフが受付します</span>`
            : "";
      const fixedHere = isBill(l) || l.pickup || l.parcel || (l.kind === "ff" && l.state === "received");
      const qty =
        c.phase === "registering" && !fixedHere
          ? qtyCell(reg, side, l, a("remove_line"), { type: "remove_line", register: reg, side, line_id: l.id }, l.kind === "service")
          : `${l.qty}`;
      return [l.touched, `<tr class="${flash(reg, l.id)}"><td>${esc(l.name)} ${[tag, warnTags(l)].filter(Boolean).join(" ")}</td><td class="num">${qty}</td><td class="num">${l.kind === "service" && !l.done && !l.amount ? "—" : yen(l.amount)}</td></tr>`];
    });
  const rows = newestFirst([...couponRows(reg, side, c.coupons, a("remove_coupon")), ...lineRows]);

  const top = regTop(
    c.total,
    itemCount(c.lines),
    (c.phase === "registering"
      ? btn("お会計", { type: "checkout", register: reg, side }, a("checkout"), "primary")
      : ""),
    r.max_items,
    !!c.point_card,
    voidAsk.has(reg) ? "" : voidBtn(reg, side, a("void_transaction")),
  );

  if (!c.lines.length && !c.coupons.length) voidAsk.delete(reg);
  if (!c.lines.length && !c.coupons.length && !c.returned.length) return `${top}${notice}${welcome(r, side, a("emoney"))}`;
  const back = c.phase === "paying" ? backFoot(reg, side, a("back_to_registering")) : "";
  const screen = (body: string) => `${top}${notice}${confirm(body + (voidAsk.has(reg) ? "" : back))}`;
  if (voidAsk.has(reg)) return screen(voidConfirm(r, side, c));
  const slips = c.bills.length;
  if (c.phase === "paying" && slips) return screen(c.bill_unchecked ? billNote(slips, "払込票の内容を確認いたします") : billPayCard(slips, true));
  if (c.phase === "paying" && c.age_wait) return screen(ageWait(c.age_wait));
  if (c.phase === "paying" && r.cash_only) return screen(cashOnlyWait());
  if (c.lines.some((l) => l.parcel && l.kind === "service" && !l.done)) return screen(parcelWait());
  const card = firstOf([
    c.coupon_unmatched ? unmatchedCard(reg, side, c.coupons, a("remove_coupon")) : "",
    returnedCard(reg, side, c.returned, true),
  ]);
  if (card) return screen(card);
  if (c.phase === "paying" && c.point_pending) return screen(pointCheck(reg, side, a("confirm_point_card")));
  scanGuide.delete(`${reg}:${side}`);
  if (c.phase === "paying")
    return screen(`<div class="paybox center">
        ${methodRow(reg, side, c.method, c.inserted, a("select_payment"), "center", r.unavailable, true)}${r.prepaid && !c.inserted && !c.payment_blocker && (!c.method || c.method === "cash") ? prepaidNote(r, c.due) : r.cash_only && !c.method && !c.inserted && !c.payment_blocker ? cashOnlyNote(r) : payHint(c.method, c.payment_blocker, c.inserted, c.due, r.cashless_due, false, c.settle_in_ms, true)}
      </div>${cashCorner(reg, side, c.method, a("select_payment"))}`);
  if (ffVerifyOpen(reg, side, c.lines))
    return screen(ffVerifyScreen(reg, side, c.lines, true));
  closeFfVerify(reg, side);
  return `${top}${notice}
    <div class="staff-head">${taxNote(c)}</div>
    <table class="lines cols3">${LINE_COLS}<thead><tr><th>品目</th><th class="num">数量</th><th class="num">小計</th></tr></thead><tbody>${rows}</tbody></table>`;
}

const BELL_ICON = { fixed: "🔔", handheld: "📱", store_computer: "🖥️" } as const;

export function bellPanel(s: Snapshot): string {
  return s.bells
    .map(
      (m) => `<div class="bell-unit ${m.buzzing ? "buzz" : ""} ${m.bell_on ? "" : "off"}">
        <span class="bell-icon">${BELL_ICON[m.kind]}</span><span class="bell-id">${esc(m.id)}</span>${!m.bell_on ? `<span class="bell-state">呼び鈴OFF</span>` : m.buzzing ? `<span class="bell-state">鳴動中</span>` : ""}
      </div>`,
    )
    .join("");
}

export function bellSettings(s: Snapshot): string {
  return s.bells
    .map((m, i) =>
      m.kind === "fixed"
        ? `<label class="bell-setting"><span>${BELL_ICON[m.kind]} ${esc(m.id)}</span><input type="checkbox" checked disabled aria-label="${esc(m.id)}の呼び鈴機能" /><span class="muted">常に ON</span></label>`
        : `<label class="bell-setting"><span>${BELL_ICON[m.kind]} ${esc(m.id)}</span><input type="checkbox" data-bell-on="${i}"${m.bell_on ? " checked" : ""} aria-label="${esc(m.id)}の呼び鈴機能" /><span>${m.bell_on ? "ON" : "OFF"}</span></label>`,
    )
    .join("");
}

const AGE_CHOICES = [15, 18, 20];
const LIMIT_CHOICES = [1, 2, 3, 4, 5];

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
    note: "年確は会計ボタンを押した時点で、カゴの中で最も厳しい年齢で1回だけ行います<br>酒・たばこは法令どおり20歳以上で固定され、変更できません",
  },
  { id: "limit", label: "制限設定", note: "1回のお会計で売れる販売個数の上限です<br>超える分はスキャンしても登録されません" },
  {
    id: "tax",
    label: "税率設定",
    note: "税率は税区分ごとに持ちます<br>法改正で税率が変わったら区分の税率を変え、新しい税率ができたら区分を追加してください<br>価格は税込で、1回のお会計で税率ごとに合計してから1回だけ端数処理します<br>変更はこれから登録する商品から適用されます",
  },
];

function shelfRows(s: Snapshot, cols: number, row: (p: Snapshot["catalog"][number]) => string): string {
  return SHELVES.map((g) => {
    const items = s.catalog.filter(g.match);
    if (!items.length) return "";
    return `<tr class="grp"><th colspan="${cols}">${esc(g.label)}</th></tr>${items.map(row).join("")}`;
  }).join("");
}

function ageRows(s: Snapshot): string {
  return shelfRows(s, 4, (p) => {
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

type Shelf = { label: string; match: (p: Snapshot["catalog"][number]) => boolean };

const byCodes = (...codes: string[]) => (p: { code: string }) => codes.includes(p.code);
const SHELVES: Shelf[] = [
  { label: "日配食品", match: byCodes("4900000000028", "4900000000042", "4900000000011") },
  { label: "加工食品", match: byCodes("4900000000035", "4900000000103", "4900000000073", "4900000000066") },
  { label: "非食品", match: byCodes("4910000000014", "4910000000038", "4900000000127", "4900000000134", "4900000000141") },
  { label: "サービス", match: (p) => p.category === "service" },
  { label: "オプション", match: (p) => p.category === "bag" },
];

const PARCEL_CODES = ["2100000000014", "2100000000045"];

const PICKUP_CODE = "2100000000038";

const BILL_CODES = ["2100000000021", "2100000000043", "2100000000050"];

const HINT_SCAN = "レジ画面にドラッグ＆ドロップ";
const HINT_CASH = "入金ボックスにドラッグ＆ドロップ";
const HINT_PAY = "支払方法を選んでから、決済端末にドラッグ＆ドロップ";
const HINT_EMONEY = "決済端末にドラッグ＆ドロップ";

function item(code: string, name: string, sub: string, cls = "", hint = HINT_SCAN): string {
  return `<button type="button" class="item ${cls}" data-code="${esc(code)}" data-hint="${esc(hint)}" aria-label="${esc(name)}をドラッグ、${esc(hint)}">
    <span class="item-name">${esc(name)}</span>${sub ? `<span class="item-sub">${sub}</span>` : ""}</button>`;
}

export function shelfPanel(s: Snapshot, extra = ""): string {
  const staffOperated = s.registers.some((r) => r.mode === "staff_operated");
  const asked = s.registers.some((r) => r.tobacco_asked);
  return SHELVES.map((g) => {
    const items = s.catalog.filter(g.match).filter((p) => p.category !== "tobacco" || staffOperated);
    if (!items.length) return "";
    return `<div class="shelf"><h3>${esc(g.label)}</h3><div class="items uniform">${items
      .map((p) =>
        BILL_CODES.includes(p.code)
          ? item(p.code, "公共料金", `${esc(p.name.replace(/料金$/, ""))} ${yen(p.price)}`)
          :
            PARCEL_CODES.includes(p.code)
            ? item(p.code, p.name.split(" ")[0], esc(p.name.split(" ")[1] ?? ""))
            : item(
              p.code,
              p.name,
              `${p.code === PICKUP_CODE ? "受取用バーコード" : p.price ? yen(p.price) : "金額は受付時"}${p.age ? ` ・<b>年確${p.age}</b>` : ""}`,
              p.category === "tobacco" && asked ? "call-blink" : "",
            ),
      )
      .join("")}</div></div>`;
  }).join("") + extra + slipShelf(s);
}

function slipShelf(s: Snapshot): string {
  const slips = s.registers.flatMap((r) =>
    r.staff.lines
      .filter((l) => l.parcel?.slip && l.kind === "service" && l.done)
      .map((l) => item(`PS${l.parcel!.slip.replace(/-/g, "")}`, `伝票 レジ${r.id}`, esc(l.parcel!.slip), "", "スタッフ側に読むと、この荷物の受付を取り消します")),
  );
  return slips.length ? `<div class="shelf"><h3>荷物の伝票（取り消し用）</h3><div class="items uniform">${slips.join("")}</div></div>` : "";
}

function walletItem(key: string, name: string, sub: string, cls: string, hint: string): string {
  return `<button type="button" class="item ${cls}" data-wallet="${esc(key)}" data-hint="${esc(hint)}" aria-label="${esc(name)}をドラッグ、${esc(hint)}">
    <span class="item-name">${esc(name)}</span>${sub ? `<span class="item-sub">${sub}</span>` : ""}</button>`;
}

export function readableCardsShelves(): string {
  const shelf = (label: string, items: string[]) => `<div class="shelf"><h3>${label}</h3><div class="items uniform">${items.join("")}</div></div>`;
  return (
    shelf("ポイント", [item("PC12345678", "ポイントカード", "", "card"), item("PC87654321", "ポイントアプリ", "", "card")]) +
    shelf("クーポン", [item("CP0001", "引換券", "ソフトドリンク", "coupon"), item("CP0002", "割引券", "サンドイッチ 50円引き", "coupon")])
  );
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
  return `<div class="shelf"><div class="items">${[...bills, ...coins].join("")}</div><div class="items wallet-cards">${cards.join("")}</div></div>`;
}

export function sideOf(cmd: Command): "staff" | "customer" | "store" {
  if (cmd.side === "staff" || cmd.side === "customer") return cmd.side;
  if (["ff_button", "insert_cash", "tap_emoney", "call_staff"].includes(cmd.type)) return "customer";
  if (typeof cmd.register === "number") return "staff";
  return "store";
}
