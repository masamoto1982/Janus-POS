import type {
  Action,
  ActionState,
  EmoneyKind,
  LimitAlert,
  ShortChoice,
  BillRow,
  Parcel,
  PointsView,
  Receipt,
  Pickup,
  Unavailable,
  CouponView as Coupon,
  Dates,
  ReturnedCoupon as Returned,
  Command,
  PaymentMethod,
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
  TicketRow,
} from "./types";

type Gate = Pick<ActionState, "allowed" | "reason">;

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

function find(actions: ActionState[], name: Action): Gate {
  return actions.find((a) => a.action === name) ?? { action: name, allowed: false, reason: null };
}

export function btn(label: string, cmd: Command, a?: Gate, cls = ""): string {
  const disabled = a && !a.allowed;
  const title = disabled && a.reason ? ` title="${esc(a.reason)}"` : "";
  return `<button class="${cls}" data-cmd="${esc(JSON.stringify(cmd))}"${disabled ? " disabled" : ""}${title}>${esc(label)}</button>`;
}

export function backBtn(cmd: Command, a?: Gate): string {
  return btn("バック", cmd, a, "md ghost back-btn");
}
const backFootRow = (cmd: Command, a?: Gate): string => `<div class="screen-foot">${backBtn(cmd, a)}</div>`;
const backCorner = (cmd: Command, a?: Gate): string => `<div class="back-foot">${backBtn(cmd, a)}</div>`;
const cashCorner = (reg: number, side: Side, method: string | null, st: Gate): string =>
  `<div class="skip-foot">${btn("現金", { type: "select_payment", register: reg, side, method: "cash" }, st, `md skip${method === "cash" ? " on" : ""}`)}</div>`;

const skipCorner = (cmd: Command, a?: Gate): string => `<div class="skip-foot">${btn("スキップ", cmd, a, "md skip")}</div>`;

const flagSpan = (label: string): string => `<span class="flag">${esc(label)}</span>`;

function ffFlag(reg: number, side: Side, l: { kind: string; state?: FfState }, a: Gate, ro: boolean): string {
  if (l.kind !== "ff" || l.state !== "waiting" || ro) return "";
  const title = a.allowed ? "押してお渡しした個数を照合" : (a.reason ?? "");
  return `<button type="button" class="flag ff-verify" data-ffverify="${reg}:${side}" title="${esc(title)}"${a.allowed ? "" : " disabled"}>お渡し前</button>`;
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
      <div class="big">${customer ? "お受け取りした数を入力してください" : "お渡しした数を入力してください"}</div>
      <table class="lines ffv"><thead><tr><th>品目</th><th class="num">注文数</th><th class="num">${customer ? "お受け取り数" : "お渡し数"}</th><th class="ffv-judge">照合</th></tr></thead><tbody>${rows}</tbody></table>
      <div class="ffv-foot"><button type="button" class="md ghost back-btn" data-ffverify-close="${reg}:${side}" >バック</button></div>
    </div>`;
}

const METHOD_LABEL: Record<string, string> = { cash: "現金", emoney: "電子マネー", qr: "バーコード決済", credit: "クレジットカード", points: "ポイント利用" };
const METHODS = ["cash", "emoney", "qr", "credit", "points"] as const;

type Side = "staff" | "customer";

function thanks(rc: Receipt | null = null): string {
  return `<div class="thanks">${msg("ありがとうございました", "お忘れ物にご注意ください")}${pointResult(rc)}</div>`;
}

/** お会計のあと：今回のポイント（利用・付与）と残高。カードを読んでいなければ出さない */
function pointResult(rc: Receipt | null): string {
  if (!rc || rc.point_balance === null) return "";
  const used = rc.points_used ? `ご利用 ${pt(rc.points_used)}、` : "";
  return `<p class="small point-result">${used}付与 ${pt(rc.points)}、ポイント残高 ${pt(rc.point_balance)}</p>`;
}

// 釣り銭の取り忘れ：お釣りを受け取るべき人はもうそこにいないかもしれないので、お釣り・お礼の案内を両画面ともこれに置き換える
// 大きなテキストは警告の赤、背景はやわらかい黄色（小さなテキストを足すときはオレンジ）
function changeLeft(): string {
  return `<div class="confirm change-alert">${msg("釣り銭が残っています")}</div>`;
}

function changeDue(side: Side, amount: number): string {
  const msg = side === "staff" ? `お釣り ${yen(amount)} をお返ししてください` : `お釣り ${yen(amount)} と<br>レシートをお受け取りください`;
  return `<div class="thanks"><div class="big">${msg}</div></div>`;
}

export function isoDate(t: Date): string {
  return `${t.getFullYear()}-${pad(t.getMonth() + 1)}-${pad(t.getDate())}`;
}

function idCard(reg: number, age: number, v: StaffView, a: (n: Action) => Gate): string {
  const choose = (idCheck: boolean, label: string, cls = "") =>
    btn(label, { type: "choose_age_check", register: reg, side: "staff", id_check: idCheck }, a("choose_age_check"), cls);
  if (v.age_choice_pending) {
    return `<div class="point-screen">
        <div class="big">${age}歳以上の年齢確認をしますか？</div>
        <div class="point-choices">${choose(true, "する", "primary")}</div>
      </div>${skipCorner({ type: "choose_age_check", register: reg, side: "staff", id_check: false }, a("choose_age_check"))}`;
  }
  const st = a("check_id");
  // 法令の年齢制限（酒・たばこ）は免許証・マイナンバーカード、店舗が任意で決めた年齢制限だけなら学生証など顔写真付きの身分証も可。
  // デジタル身分証は偽造が容易なので、どちらでも認めない
  // 外国人の在留カード・特別永住者証明書・パスポートは法令の年確でも認める
  const ids = v.age_official_id ? "運転免許証またはマイナンバーカードで確認してください<br>在留カード・特別永住者証明書・パスポートも可" : "学生証など顔写真付きの身分証で確認できます<br>デジタル身分証は不可";
  return `<div class="point-screen id-entry">
      ${msg("身分証の生年月日を入力してください", ids)}
      ${padField(reg, "staff", "birth", padValue.get(padValueKey(reg, "birth")) ?? "", "西暦8桁　例 19821002", "身分証の生年月日（西暦8桁）", undefined, "birth-field", st)}
    </div>${skipCorner({ type: "choose_age_check", register: reg, side: "staff", id_check: false }, a("choose_age_check"))}`;
}

function pickupStep(reg: number, p: Pickup, st: Gate): string {
  const found = (v: boolean | null): Command => ({ type: "pickup_found", register: reg, found: v });
  const end: Command = { type: "pickup_handover", register: reg };
  if (p.missing) {
    return `<div class="point-screen pickup-step">
        <p class="pickup-no">${esc(p.no)} ${flagSpan("見つかりません")}</p>
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
      ${msg("受け取りのサインをお願いします", "レシートとお釣りをお受け取りください")}
    </div>`;
  return `<div class="point-screen pickup-step">
      <p class="pickup-no">${esc(no)}</p>
      <div class="big">店舗控えにサインをいただき、荷物をお渡しください</div>
      <div class="point-choices">${btn("お渡ししました", { type: "pickup_handover", register: reg }, undefined, "primary")}</div>
    </div>`;
}

function serviceFlag(l: { kind: string; done?: boolean; cash_only: boolean; parcel?: Parcel | boolean; pickup?: Pickup | boolean }): string {
  if (l.kind !== "service" || l.done) return "";
  if (l.pickup) return flagSpan(typeof l.pickup === "object" && l.pickup.missing ? "見つかりません" : "照合待ち");
  if (l.parcel) return flagSpan(typeof l.parcel === "object" && l.parcel.size ? "届け先未定" : "受付中");
  return flagSpan("受付待ち");
}

/** 個数制限のフラグ。販売個数の上限を超えて登録しようとした明細にだけ出す（押せない、お会計は止めない） */
function limitFlag(l: { limit: number | null; limit_hit: boolean }): string {
  return l.limit && l.limit_hit ? flagSpan(`限定${l.limit}`) : "";
}

/** 販売個数の上限があるとき、この明細にできる最大の数量（同じ商品のほかの明細の分を引く） */
type LimitLine = { id: number; name: string; qty: number; limit: number | null };
export function lineMax(l: LimitLine, lines: LimitLine[]): number | undefined {
  if (!l.limit) return undefined;
  return l.limit - lines.filter((x) => x.name === l.name && x.id !== l.id).reduce((n, x) => n + x.qty, 0);
}

function ageFlag(l: { age: number | null; age_checked?: boolean }, cmd?: Command, st?: Gate): string {
  if (!l.age || l.age_checked) return "";
  return cmd && st?.allowed ? btn(`年確${l.age}`, cmd, st, "flag age-start") : flagSpan(`年確${l.age}`);
}

// 販売期限・賞味期限：core はいまからの分で持つので、ページを開いた時刻を起点に日時へ直す
// （画面を描くたびに時刻がずれないよう起点は固定、日配食品は時刻まで・加工食品は日付まで）
const DATES_FROM = Date.now();
export function expiryLabel(min: number, fresh: boolean): string {
  const t = new Date(DATES_FROM + min * 60_000);
  if (!fresh) return `${t.getFullYear()}/${t.getMonth() + 1}/${t.getDate()}`;
  t.setMinutes(0, 0, 0);
  return `${t.getMonth() + 1}/${t.getDate()} ${pad(t.getHours())}:00`;
}

// 販売期限切れ：明細に「期限切れ」のフラグ。押すと案内の画面を開く（会計はグレーアウト）。
// 期限切れの品を読むとその場で自動的に開き、バックで閉じると、次に期限切れの品を読むまで開かない
export const expiredClosed = new Map<string, string>();
type ExpiredLine = { id: number; touched: number; name: string; qty: number; expired: boolean; fresh: boolean; dates: Dates | null };
const expiredSig = (lines: ExpiredLine[]): string =>
  lines
    .filter((l) => l.expired)
    .map((l) => `${l.id}x${l.qty}`)
    .join(",");
function expiredOpen(reg: number, side: Side, lines: ExpiredLine[]): boolean {
  const sig = expiredSig(lines);
  if (!sig) expiredClosed.delete(`${reg}:${side}`);
  return !!sig && expiredClosed.get(`${reg}:${side}`) !== sig;
}

function expiredFlag(reg: number, side: Side, l: { expired: boolean }, operable: boolean): string {
  if (!l.expired) return "";
  if (!operable) return flagSpan("期限切れ");
  return `<button type="button" class="flag" data-expiredview="${reg}:${side}" title="押して新しい商品との取り替えを案内">期限切れ</button>`;
}

/** 販売期限切れの案内（回収イベント）。期限切れの品はレジを通るたびに一点ずつ出てくるので、直近の一点だけを出す。
 *  操作できる側には「回収しました」と左下のバック（明細のフラグで開き直せる）、お客様が見るだけの側は案内だけ */
function expiredScreen(reg: number, side: Side, lines: ExpiredLine[], st: Gate | null, staff: boolean): string {
  const l = lines.filter((x) => x.expired).sort((a, b) => b.touched - a.touched)[0];
  if (!l) return "";
  const back = st ? `<div class="back-foot"><button type="button" class="md ghost back-btn" data-expired-close="${reg}:${side}" data-expired-sig="${expiredSig(lines)}">バック</button></div>` : "";
  return (
    collectScreen(
      "販売期限が切れています",
      // 期限切れの品はスキャンした瞬間に一点ずつ止まるので、回収は常に一点。点数は出さず、販売期限を出す
      [{ name: l.name, note: l.dates ? `販売期限 ${expiryLabel(l.dates.sell_by_min, l.fresh)}` : "" }],
      staff ? "お客様にお詫びし、販売期限切れ商品を回収してください" : "申し訳ございません、店員が新しい商品にお取替えいたします",
      st ? btn("回収しました", { type: "remove_line", register: reg, side, line_id: l.id }, st, "primary") : "",
    ) + back
  );
}

/** 回収イベント（年確で販売できない商品・お渡し済みFFの取り消し・販売期限切れ）の共通の画面。
 *  縦横中央に、大きなテキスト＝なぜ回収するか／回収する品の一覧（点数、販売期限切れは販売期限）／小さなテキスト＝スタッフへの指示またはお客様へのお詫び、
 *  スタッフ側だけ「回収しました」 */
function collectScreen(head: string, items: { name: string; qty?: number; note?: string }[], sub: string, action = ""): string {
  const list = items.length
    ? `<ul class="collect-list">${items.map((c) => `<li><b>${esc(c.name)}</b><span>${esc(c.note ?? `${c.qty}点`)}</span></li>`).join("")}</ul>`
    : "";
  return `<div class="point-screen collect">${msg(esc(head), esc(sub))}${list}${action ? `<div class="point-choices">${action}</div>` : ""}</div>`;
}

/** 照合イベント（年確・FFのお渡し）が済んだ。スタッフ側にはシステムのお墨付き（緑の照合OK）と「次へ」、
 *  お客様側には完了とご協力へのお礼。年確は責任の重い作業なので、手ごたえなしに次の画面へ進めない */
function passedCard(reg: number, side: Side, v: StaffView): string {
  const p = v.passed;
  if (!p) return "";
  const mark = `<div class="pass-mark" aria-hidden="true">✓</div>`;
  const next = `<div class="point-choices">${btn("次へ", { type: "dismiss_passed", register: reg }, { allowed: true, reason: null }, "primary")}</div>`;
  if (p.kind === "age")
    return side === "staff"
      ? `<div class="point-screen passed">${mark}${msg(`${p.age}歳以上であることを確認しました`, "身分証をお返しください")}${next}</div>`
      : `<div class="point-screen passed">${mark}${msg("年齢確認が完了しました", "ご協力ありがとうございました")}</div>`;
  // FF はスタッフが取り仕切るので、スタッフ操作モードのお客様側には出さない
  return side === "staff" ? `<div class="point-screen passed">${mark}${msg("FFのお渡しを確認しました", "お渡し数がすべて注文数と一致しています")}${next}</div>` : "";
}

/** 販売個数の上限を超えて登録を断ったときの案内（年齢確認と同じくレジの機能）。
 *  ほかの案内画面と同じ縦横中央の二行、操作できる側には左下のバック。次の登録でも閉じる */
function limitScreen(reg: number, side: Side, alert: LimitAlert, st: Gate | null, staff: boolean): string {
  const sub = staff
    ? `${alert.limit}点を超える分はお売りできないことをお客様にお伝えください`
    : `申し訳ございません、${alert.limit}点を超える分はお売りできません`;
  const back = st ? backCorner({ type: "close_limit_alert", register: reg, side }, st) : "";
  return `<div class="point-screen pair-note limit-screen">${msg(esc(`${alert.name}は1回のお会計で${alert.limit}点までです`), esc(sub))}</div>${back}`;
}

export function pendingParcel(lines: StaffLine[]): StaffLine | undefined {
  return lines.find((l) => l.kind === "service" && !l.done && l.parcel);
}


export const parcelError = new Map<number, string>();
const parcelErr = (reg: number): string => {
  const e = parcelError.get(reg);
  return e ? `<span class="parcel-error" role="alert">${esc(e)}</span>` : "";
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
      ${msg("スタッフがお伺いします", "伝票を貼り付けてお待ちください<br>伝票の貼り付け、梱包はお客様責となります")}
    </div>`;
}

function parcelStep(reg: number, l: StaffLine, st: Gate, count: number): string {
  const p = l.parcel as Parcel;
  const no = count > 1 ? `<span class="parcel-no">${count}個目</span>` : "";
  const back = () => backCorner({ type: "parcel_back", register: reg, line_id: l.id }, st);
  const field = (target: PadTarget, placeholder: string, aria: string) =>
    padField(reg, "staff", target, padValue.get(padValueKey(reg, target, l.id)) ?? "", placeholder, aria, l.id, `parcel-${target}`, st);
  if (!p.size && !p.sum_cm) {
    return `<div class="point-screen parcel-entry">
        ${msg(`${no}取り扱いサイズを決定してください`, parcelErr(reg) || "三辺の長さの合計を入力してください")}
        <div class="parcel-fields"><label>長さ${field("pcm", "", "荷物の三辺の合計（cm）")}<span>cm</span></label></div>
      </div>${back()}`;
  }
  if (!p.size) {
    return `<div class="point-screen parcel-entry">
        ${msg(`${no}取り扱いサイズを決定してください`, parcelErr(reg) || "重量を入力してください")}
        <div class="parcel-fields"><label>重さ${field("pkg", "", "荷物の重さ（kg）")}<span>kg</span></label></div>
      </div>${back()}`;
  }
  return `<div class="point-screen parcel-entry">
      ${msg(`${no}お届け先の郵便番号を入力してください`, parcelErr(reg))}
      ${field("post", "例 100-0001", "お届け先の郵便番号（7桁）")}
    </div>${back()}`;
}

/** 複数口の伝票で、この荷物が何個口目か。同じお届け先へ続けて受け付けた複数口だけを数え、ほかの伝票は混ぜない */
function multiPiece(lines: StaffLine[], l: StaffLine): number {
  if (!l.parcel?.multi) return 0;
  const parcels = lines.filter((x) => x.parcel).sort((a, b) => a.id - b.id);
  let n = 1;
  for (let i = parcels.findIndex((x) => x.id === l.id) - 1; i >= 0; i--) {
    const q = parcels[i].parcel;
    if (!q?.multi || !l.parcel.postal || q.postal !== l.parcel.postal) break;
    n++;
  }
  return n;
}

function parcelAsk(reg: number, lines: StaffLine[], st: Gate): string {
  const done = lines.filter((l) => l.parcel && l.kind === "service" && l.done);
  const last = done.reduce<StaffLine | undefined>((a, l) => (!a || l.touched > a.touched ? l : a), undefined);
  const p = last?.parcel;
  const pieces = last ? multiPiece(lines, last) : 0;
  return `<div class="point-screen parcel-ask">
      ${msg("同じお届け先の荷物はありますか？", p && last ? `${p.size}サイズ・${esc(p.region ?? "")}あて（〒${esc(p.postal ?? "")}）<b class="parcel-fee">${p.collect ? "着払い" : yen(last.amount)}</b>` : "")}
      <div class="point-choices">
        ${btn("ある", { type: "parcel_next", register: reg, more: true }, st)}
        ${btn("ない", { type: "parcel_next", register: reg, more: false }, st, "primary")}
      </div>
      <p class="muted">受付済み ${pieces}個</p>
    </div>`;
}

function confiscatedCard(reg: number, side: Side, v: StaffView): string {
  if (!v.confiscated.length) return "";
  return side === "staff"
    ? collectScreen("年齢制限により販売できない商品があります", v.confiscated, "お客様にお詫びし、商品を回収してください", btn("回収しました", { type: "dismiss_confiscated", register: reg }, { allowed: true, reason: null }, "primary"))
    : collectScreen("年齢制限によりお売りできない商品があります", v.confiscated, "申し訳ございません、こちらの商品はお会計から外させていただきます");
}

function ffCollectCard(reg: number, v: StaffView): string {
  if (!v.ff_collect.length) return "";
  return collectScreen("お渡し済みのFFが取り消されました", v.ff_collect, "お客様からFFを回収してください", btn("回収しました", { type: "dismiss_ff_collect", register: reg }, { allowed: true, reason: null }, "primary"));
}

/** お客様側には回収の案内を出さない（FF はスタッフがリードする、user 2026-11-09） */
function ffReturnCard(v: StaffView): string {
  if (!v.ff_collect.length) return "";
  return collectScreen(v.ff_collect.map((c) => `${c.name}の数量を変更しました`).join("、"), [], "");
}

function backFoot(reg: number, side: Side, st: Gate): string {
  return backCorner({ type: "back_to_registering", register: reg, side }, st);
}

const billN = (n: number): string => `<span class="bill-n">${n}枚</span>`;

export function billVerifyScreen(reg: number, side: Side, bills: BillRow[], st: Gate): string {
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

// 収入印紙の貼り付け確認：FF のお渡し確認と同じく、明細のフラグ「印紙貼付前」から開き、
// お会計では払込票の確認のあとに自動で出る。確認が済むまで決済できない
export const stampVerifying = new Set<string>();

const stampRows = (bills: BillRow[]): BillRow[] => bills.filter((b) => b.stamp > 0);
const stampPending = (bills: BillRow[]): boolean => stampRows(bills).some((b) => !b.stamped);

function stampFlag(reg: number, side: Side, b: BillRow | undefined, st: Gate, ro: boolean): string {
  if (!b || !b.stamp || b.stamped) return "";
  if (ro) return flagSpan("印紙貼付前");
  const title = st.allowed ? "押して収入印紙の貼り付けを確認" : (st.reason ?? "");
  return `<button type="button" class="flag ff-verify" data-stampverify="${reg}:${side}" title="${esc(title)}"${st.allowed ? "" : " disabled"}>印紙貼付前</button>`;
}

export function stampVerifyScreen(reg: number, side: Side, bills: BillRow[], st: Gate, close: boolean): string {
  const off = st.allowed ? "" : ` disabled title="${esc(st.reason ?? "")}"`;
  const rows = stampRows(bills)
    .map(
      (b) =>
        `<tr><td>${esc(b.name)}</td><td>${yen(b.amount)}</td><td class="bill-no">${esc(b.slip_no)}</td><td>${yen(b.stamp)}</td><td class="bill-tick"><input type="checkbox" ${b.stamped ? "checked" : ""} data-stamp-check="${reg}:${side}:${b.line_id}" aria-label="${esc(b.name)} ${yen(b.amount)}の領収証に収入印紙${yen(b.stamp)}を貼り付けた"${off} /></td></tr>`,
    )
    .join("");
  return `<div class="ffv-screen bill-verify stamp-verify">
      <div class="big">収入印紙の貼り付け確認</div>
      <table class="lines ffv bill-table stamp-table"><thead><tr><th>事業者</th><th>金額</th><th>バーコード番号</th><th>印紙</th><th class="bill-tick">貼付</th></tr></thead><tbody>${rows}</tbody></table>
      <p class="muted">領収証に収入印紙を貼り付けたら、一枚ずつチェックを入れてください</p>
      ${close ? `<div class="ffv-foot"><button type="button" class="md ghost back-btn" data-stampverify-close="${reg}:${side}">バック</button></div>` : ""}
    </div>`;
}

// 通常のお買い物：税抜 5 万円以上をクレジットカード以外で受け取ると、レシートに収入印紙が要る。
// レシートは決済のあとに出るので、確認も決済のあと（お釣り・お礼の案内の前）
export function receiptStampScreen(reg: number, side: Side, rs: { stamp: number; base: number }, receiptNo: number | undefined, st: Gate, staffPresent = true): string {
  if (side === "customer")
    return `<div class="point-screen pair-note">${staffPresent ? msg("レシートに収入印紙を貼り付けています", "少々お待ちください") : msg("レシートに収入印紙を貼り付けます", "スタッフが向かっています")}</div>`;
  const off = st.allowed ? "" : ` disabled title="${esc(st.reason ?? "")}"`;
  return `<div class="ffv-screen bill-verify stamp-verify">
      <div class="big">収入印紙の貼り付け確認</div>
      <table class="lines ffv bill-table receipt-stamp-table"><thead><tr><th>レシート</th><th>金額（税抜）</th><th>印紙</th><th class="bill-tick">貼付</th></tr></thead><tbody><tr><td>No.${receiptNo ?? ""}</td><td>${yen(rs.base)}</td><td>${yen(rs.stamp)}</td><td class="bill-tick"><input type="checkbox" data-receipt-stamp="${reg}:${side}" aria-label="レシートに収入印紙${yen(rs.stamp)}を貼り付けた"${off} /></td></tr></tbody></table>
      <p class="muted">レシートに収入印紙を貼り付けたら、チェックを入れてください</p>
    </div>`;
}

export function billPayCard(count: number, selfOperated = false): string {
  return `<div class="point-screen bill-pay">
      ${msg(`公共料金${billN(count)}を受け付けます`, selfOperated ? "現金はスタッフがお預かりします<br>スタッフをお呼びください" : "現金でお支払いください")}
    </div>`;
}

export function billHandover(reg: number, side: Side, count: number, st: Gate): string {
  if (side === "customer")
    return `<div class="point-screen bill-done">
      ${msg(`公共料金${billN(count)}　支払いが完了しました`, "払込票とレシート、お釣りをお受け取りください")}
    </div>`;
  return `<div class="point-screen bill-done">
      ${msg(`公共料金${billN(count)}`, "払込票に収納印を押してお渡しください")}
      <div class="point-choices">${btn("お渡し済", { type: "hand_over_bills", register: reg, side }, st, "primary")}</div>
    </div>`;
}

/** 宅配便の決済のあと：お客様控えの伝票のお渡し。伝票ごとに1枚、複数口は何個口でも1枚。枚数は両画面に出す */
export function parcelHandover(reg: number, side: Side, count: number, st: Gate): string {
  if (side === "customer")
    return `<div class="point-screen bill-done">
      ${msg(`控え伝票${billN(count)}`, "控え伝票をお受け取りください")}
    </div>`;
  return `<div class="point-screen bill-done">
      ${msg(`控え伝票${billN(count)}`, "お客様控えの伝票をお渡しください")}
      <div class="point-choices">${btn("お渡し済", { type: "hand_over_bills", register: reg, side }, st, "primary")}</div>
    </div>`;
}

/** 決済後のチケットのお渡し。スタッフはお渡しした枚数、お客様は受け取った枚数を、それぞれ入力欄に入力する（発券枚数は見せない）。
 *  どちらの入力も発券枚数とだけ比べ（スタッフとお客様の入力どうしは比べない）、一致しない側にだけ警告を出す。
 *  両方が一致したらお渡し完了。それまでレシートもお釣りも出さず、完了するとお釣り・お礼の画面に進む */
export function ticketHandover(reg: number, side: Side, rows: TicketRow[], issued = true, received: number | null = null, handed: number | null = null, st?: Gate): string {
  const total = rows.reduce((n, t) => n + t.issued, 0);
  // 発券はお店全体で先入れ先出し。先に支払ったレジのお渡しが済むまで、このレジは発券を待つ
  if (!issued)
    return `<div class="point-screen ticket-wait">
      ${side === "customer" ? msg(`チケット${billN(total)}`, "発券の順番をお待ちください") : msg("チケットの発券待ち", "ほかのレジのチケットのお渡しが済むと発券します")}
    </div>`;
  const staff = side === "staff";
  const value = staff ? handed : received;
  const field = st?.allowed === false
    ? `<button type="button" class="pad-field" disabled>${value ?? ""}</button>`
    : padField(reg, side, staff ? "tkh" : "tkr", value === null ? "" : String(value), "", staff ? "お渡し枚数" : "受け取り枚数");
  const note = value === null || value === total
    ? ""
    : `<p class="ticket-alert">${staff ? "発券枚数とお渡し枚数が一致しません" : "申込数と受け取り数が一致しません"}<br>再度ご確認ください</p>`;
  return `<div class="point-screen ticket-done ${staff ? "staff-only" : "customer-only"}">
      ${msg(staff ? "お渡し枚数を入力してください" : "受け取り枚数をご入力ください")}
      <div class="parcel-fields"><label>${field}<span>枚</span></label></div>
      ${note}
    </div>`;
}

function billNote(count: number, sub: string): string {
  return `<div class="point-screen pair-note">${msg(`公共料金${billN(count)}`, esc(sub))}</div>`;
}

const confirm = (body: string): string => `<div class="confirm">${body}</div>`;

/** 大きなテキストと小さなテキスト（アプリ全体で共通、待ち受け画面が基準）
 *  大きなテキスト＝濃いグレー・太字、小さなテキスト＝薄いグレー。大きなテキストは必ず小さなテキストより上に置く
 *  引数は HTML（呼び出し側でエスケープする） */
export function msg(big: string, small = ""): string {
  return `<div class="msg"><div class="big">${big}</div>${small ? `<p class="small">${small}</p>` : ""}</div>`;
}

function pairNote(head: string, sub = ""): string {
  return `<div class="point-screen pair-note">${msg(esc(head), esc(sub))}</div>`;
}

const firstOf = (cards: string[]): string => cards.find((c) => c) ?? "";

const INSERT_DONE_MS = 30_000;

/** お客様がかざすもの（レジ画面にデモ用の「Wallet」という言葉は出さない） */
const TAP_THING: Record<string, string> = { emoney: "電子マネー", qr: "決済用バーコード", credit: "クレジットカード", points: "ポイントカード" };
/** かざす機器：電子マネー・クレジットカードは決済端末、バーコード・ポイントカードはスキャナ（同じボックスの表示が切り替わる） */
const TAP_DEVICE = (method: string): string => (method === "emoney" || method === "credit" ? "決済端末" : "スキャナ");

/** 現金のみの商品があるとき：キャッシュレスで払う分と、そのあと現金で払う分 */
const splitNote = (method: string, cashlessDue: number, due: number): string =>
  `${METHOD_LABEL[method]}で <b>${yen(cashlessDue)}</b>、切手・チケット・お支払いの <b>${yen(due - cashlessDue)}</b> はそのあと現金で`;

/** 支払方法を選んだときの案内。選んだ側には青文字（p.guide）で出す。
 *  inert＝その画面からは操作できない（セルフ操作モードのスタッフ側）。青文字にせず、操作できない部品と同じグレー */
function payHint(method: string | null, blocker: string | null, inserted: number, due: number, cashlessDue = due, staff = false, settleIn: number | null = null, selfOperated = false, inert = false): string {
  if (blocker) return `<p class="muted">${esc(blocker)}</p>`;
  const guide = inert ? `<p class="guide off">` : `<p class="guide">`;
  if (method && method !== "cash") {
    // スタッフ操作モードのスタッフ側は、キャッシュレスでスタッフのなすべき作業を出す（かざすのはお客様、バーコード決済はスタッフがスキャン）
    const op = staff ? (method === "qr" ? "決済用バーコードをスキャンしてください" : `お客様に${TAP_DEVICE(method)}の位置を案内してください`) : `${TAP_THING[method]}を${TAP_DEVICE(method)}にかざしてください`;
    const split = cashlessDue < due ? `<br>${splitNote(method, cashlessDue, due)}` : "";
    return `${guide}${op}${split}</p>`;
  }
  if (selfOperated) return "";
  if (inserted && due > 0) return staff ? `<p>あと <b>${yen(due)}</b> 不足しています</p>` : `<div class="big">不足額 ${yen(due)}</div>`;
  const sec = Math.ceil((settleIn ?? INSERT_DONE_MS) / 1000);
  if (inserted) return staff ? `<p>${sec}秒の無操作で、お支払いを確定します</p>` : `<div class="big">追加投入の受付 残り${sec}秒</div>`;
  if (method === "cash" && staff) return `${guide}お預かりした現金を入金ボックスに入れ、「投入おわり」を押してください</p>`;
  if (method === "cash") return `${guide}スタッフが現金をお預かりします</p>`;
  return staff ? `<p>お預かりした現金を入金ボックスに入れるか、支払方法を選んでください</p>` : `<p>お支払い方法をお申しつけください</p>`;
}

/** スタッフ操作モードのお客様側：スタッフが支払方法を選んだら、その決済の操作を案内する。
 *  お客様が押したボタンの案内ではないので青文字にせず、大きなテキスト／小さなテキストで出す */
function customerPayGuide(method: string, cashlessDue: number, due: number): string {
  if (method === "cash") return msg("スタッフが現金をお預かりします");
  const split = cashlessDue < due ? splitNote(method, cashlessDue, due) : "";
  // バーコード決済はスタッフがスキャンするので、お客様には提示をお願いする
  const big = method === "qr" ? "決済用バーコードをご提示ください" : `${TAP_THING[method]}を${TAP_DEVICE(method)}にかざしてください`;
  return msg(big, split || (method === "points" ? "ポイントアプリもお使いいただけます" : ""));
}

function methodRow(reg: number, side: Side, method: string | null, inserted: number, st: Gate, cls = "", unavailable: Unavailable[] = [], noCash = false): string {
  if (method === "cash" && inserted) return "";
  const off = (m: string): Gate => {
    const u = unavailable.find((x) => x.method === m);
    return u ? { allowed: false, reason: u.reason } : st;
  };
  return `<div class="row method-row ${cls}">${METHODS.filter((m) => !(noCash && m === "cash"))
    .map((m) => btn(METHOD_LABEL[m], { type: "select_payment", register: reg, side, method: m }, off(m), `seg ${method === m ? "on" : ""}`))
    .join("")}</div>`;
}

const CASH_ONLY_NOTE = "現金のみ取り扱いいたします";

function methodList(method: string | null, inserted: number, unavailable: Unavailable[] = []): string {
  if (method === "cash" && inserted) return "";
  return `<div class="row wrap method-list">${METHODS.map((m) => `<span class="seg ${method === m ? "on" : ""}${unavailable.some((u) => u.method === m) ? " off" : ""}">${METHOD_LABEL[m]}</span>`).join("")}</div>`;
}

const LINE_COLS = `<colgroup><col /><col class="c-qty" /><col class="c-amt" /></colgroup>`;

function taxNote(v: { taxes: TaxTotal[] }): string {
  return `<span class="muted tax-note">内税 ${v.taxes.map((t) => `${esc(t.rate)} ${yen(t.tax)}`).join(" / ")}</span>`;
}

function staffLine(reg: number, l: StaffLine, v: StaffView, side: Side): string {
  const ro = side === "customer";
  const tags = [
    expiredFlag(reg, side, l, !ro),
    ffFlag(reg, side, l, find(v.actions, "check_ff"), ro),
    stampFlag(reg, side, v.bills.find((b) => b.line_id === l.id), find(v.actions, "affix_stamp"), ro),
    serviceFlag(l),
    ro ? ageFlag(l) : ageFlag(l, { type: "early_age_check", register: reg, side: "staff", start: true }, find(v.actions, "early_age_check")),
    limitFlag(l),
  ].filter(Boolean);
  const remove = find(v.actions, "remove_line");
  let svc = "";
  if (l.kind === "service" && !l.done && !ro && !l.parcel && !l.pickup) {
    const a = find(v.actions, "complete_service");
    svc = `<div class="svc">
      <input type="number" inputmode="numeric" min="0" placeholder="金額" data-key="svc-${reg}-${l.id}" ${a.allowed ? "" : "disabled"} />
      <button data-form="service" data-reg="${reg}" data-line="${l.id}" ${a.allowed ? "" : "disabled"}>受付</button>
    </div>`;
  }
  // チケットは一件ずつの取り扱いで数量に意味がないので、数量の代わりに発券枚数を出す
  const n = l.tickets || l.qty;
  const qty =
    ro || isBill(l) || l.pickup || l.parcel
      ? `${n}`
      : qtyCell(reg, side, { id: l.id, qty: n }, remove, { type: "remove_line", register: reg, side, line_id: l.id }, l.kind === "service", l.fresh, lineMax(l, v.lines));
  return `<tr class="${flash(reg, l.id)}${tags.length ? " flagged" : ""}">
    <td>${esc(l.name)} ${tags.join(" ")}${svc}</td>
    <td class="num">${qty}</td>
    <td class="num">${l.kind === "service" && !l.done && !l.amount ? "—" : yen(l.amount)}</td>
  </tr>`;
}

/** チケットはほかの商品と同じ会計にしないので、チケットの明細があれば数量の列は「発券枚数」 */
const qtyHead = (lines: { tickets: number }[]): string => (lines.some((l) => l.tickets > 0) ? "発券枚数" : "数量");

const isBill = (l: { kind: string; done?: boolean; cash_only: boolean }): boolean => l.kind === "service" && !!l.done && l.cash_only;

export const qtyEntry = new Map<string, { id: number; digits: string }>();

/** 日配食品は数量を増やせない。同じ商品でも一点ずつ販売期限が違うので、一つを読んで数量を増やすと
 *  期限切れの品がお客様に渡ってしまう（ポカヨケ）。続けて読んでも一点ずつ別の行になるので、
 *  数量の欄は「削除」と数字だけ（サービス商品と同じ固定の形） */

/** 数量の欄。販売個数の上限（max）に達した明細は＋を出さない（幅は残して列をそろえる） */
function qtyCell(reg: number, side: Side, l: { id: number; qty: number }, st: Gate, remove: Command, fixed = false, fresh = false, max?: number): string {
  if (!st.allowed) return `${l.qty}`;
  if (fresh) return qtyCell(reg, side, l, st, remove, true);
  if (fixed) return `<span class="qty-step">${btn("削除", remove, st, "small neg")}<b class="qty-fixed">${l.qty}</b><i class="qty-pad" aria-hidden="true"></i></span>`;
  const step = (d: number): Command => ({ type: "set_qty", register: reg, side, line_id: l.id, qty: l.qty + d });
  const minus = l.qty <= 1 ? btn("削除", remove, st, "small neg") : btn("−", step(-1), st, "small neg");
  const e = qtyEntry.get(`${reg}:${side}`);
  const typing = e?.id === l.id;
  const shown = typing ? e.digits || "　" : String(l.qty);
  const key = `<button type="button" class="pad-field qty-key${typing ? " typing" : ""}" data-qtyentry="${reg}:${side}:${l.id}" aria-label="数量 ${l.qty}、押すとテンキーで数量を入力">${esc(shown)}</button>`;
  const plus = max !== undefined && l.qty >= max ? `<i class="qty-plus-gone" aria-hidden="true"></i>` : btn("＋", step(1), st, "small neg");
  return `<span class="qty-step">${minus}${key}${plus}</span>`;
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

export type PadTarget = "pcm" | "pkg" | "post" | "birth" | "ffv" | "tkh" | "tkr" | "inlet" | "mamt" | "mpw" | "ppt";
/** error：テンキーの「決定」で送った入力が受け付けられなかった理由。テンキーを開いたまま表示し、次のキーで消す */
export type PadEntry = { target: PadTarget; line?: number; digits: string; error?: string };
export const padEntry = new Map<string, PadEntry>();
export const padValue = new Map<string, string>();
export const padValueKey = (reg: number, target: PadTarget, line?: number): string => `${reg}:${target}:${line ?? ""}`;
export const PAD: Record<PadTarget, { title: string; unit: string; max: number; decimal?: boolean; code?: boolean }> = {
  pcm: { title: "三辺の長さの合計", unit: "cm", max: 3 },
  pkg: { title: "荷物の重さ", unit: "kg", max: 3, decimal: true },
  post: { title: "お届け先の郵便番号（7桁）", unit: "", max: 7, code: true },
  birth: { title: "身分証の生年月日（西暦8桁）", unit: "", max: 8, code: true },
  ffv: { title: "お渡し数", unit: "", max: 3, code: true },
  tkh: { title: "お渡し枚数", unit: "枚", max: 3, code: true },
  tkr: { title: "受け取り枚数", unit: "枚", max: 3, code: true },
  inlet: { title: "入金", unit: "円", max: 6 },
  mamt: { title: "出金する金額", unit: "円", max: 6 },
  mpw: { title: "店舗パスワード", unit: "", max: 8, code: true },
  ppt: { title: "使うポイント数", unit: "P", max: 5 },
};
export const padShow = (target: PadTarget, digits: string): string =>
  target === "mpw" ? "●".repeat(digits.length) : target === "post" && digits.length > 3 ? `${digits.slice(0, 3)}-${digits.slice(3)}` : digits;

/** 入力欄。st を渡した欄はテンキーの「決定」でそのまま送信する（画面側に決定ボタンを置かない）ので、操作できないときは欄ごと押せなくする */
function padField(reg: number, side: Side, target: PadTarget, value: string, placeholder: string, aria: string, line?: number, cls = "", st?: Gate): string {
  const typing = padEntry.get(`${reg}:${side}`);
  const on = typing?.target === target && typing.line === line;
  const shown = padShow(target, value);
  const off = st && !st.allowed ? ` disabled title="${esc(st.reason ?? "")}"` : "";
  return `<button type="button" class="pad-field ${cls}${on ? " typing" : ""}" data-padopen="${reg}:${side}:${target}:${line ?? ""}" aria-label="${esc(aria)}（押すとテンキーで入力）"${off}>${
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
  // チケットの枚数はお客様側・スタッフ側それぞれ専用の入力。その側では操作可能部品として青で描く
  const only = e.target === "tkr" ? " customer-only" : e.target === "tkh" ? " staff-only" : "";
  return `<div class="qty-pop${only}" data-pad-backdrop="${reg}:${side}">
      <div class="qty-pop-box" role="dialog" aria-label="${esc(title)}">
        <div class="qty-pop-name">${esc(title)}${now}</div>
        <div class="qty-pop-value">${shown ? `${esc(shown)}${d.unit ? `<small> ${d.unit}</small>` : ""}` : `<span class="muted">${d.unit || "　"}</span>`}</div>
        ${e.error ? `<div class="pad-error" role="alert">${esc(e.error)}</div>` : ""}
        <div class="qty-pop-keys">${["7", "8", "9", "4", "5", "6", "1", "2", "3"].map((x) => k(x)).join("")}${last}${k("back", "←", "pad-key neg")}</div>
        <div class="qty-pop-foot">${k("cancel", "バック", "md ghost back-btn")}<button type="button" class="primary" data-pad="${reg}:${side}:ok"${ok ? "" : " disabled"}>決定</button></div>
      </div>
    </div>`;
}

/** 二千円札は取り扱わないので、自動釣銭機がそのまま出金ボックスに返す。なぜ戻ってきたかを両画面に出し、出金ボックスから取り出すと消える。
 *  現金は自動釣銭機のあるスタッフ側で扱うので、お客様にはスタッフが返す */
function returned2000(side: Side): string {
  return confirm(
    side === "staff"
      ? msg("二千円札を返却しました", "取り扱い対象外の紙幣です<br>出金ボックスからお客様にお返しください")
      : msg("二千円札はご利用いただけません", "スタッフがお返しいたします"),
  );
}

/** 呼び鈴が不通（セルフ操作モード）：呼び出しが届かないので、お客様に店員へ直接声をかけてもらう */
function bellDeadScreen(): string {
  return confirm(msg("呼び出し端末が動作していません", "お手数ですが、店員に直接お声かけください"));
}

/** 店舗の緊急連絡先：オーナーと担当スーパーバイザー（シミュレーターの架空の番号） */
const STORE_CONTACTS = [
  ["オーナー：", "090-0000-0000"],
  ["担当スーパーバイザー：", "090-0000-0001"],
] as const;

/** シミュレーターの時刻（snap.now の時間軸）を壁時計の時刻にして「11月14日 13:52:10」 */
function simClock(snap: Snapshot, t: number): string {
  const d = new Date(Date.now() - (snap.now - t));
  return `${d.getMonth() + 1}月${d.getDate()}日 ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** 非常事態モード（旧 エマージェンシーモード）：セルフ操作モードでスタッフがログインしないまま、呼び鈴が3分以上鳴らしっぱなし、
 *  または3分以内に10回以上鳴らされた。両画面を警告（赤の大きなテキスト・オレンジの小さなテキスト・やわらかい黄色）に置き換える。
 *  スタッフ側は帯（二段目）を出さず、「非常事態モードを解除します」の案内と解除ボタンだけ。オーナーと担当スーパーバイザーの連絡先は最下段
 *  （emergencyContactsRow）。解除はログインなしで誰でも押せる（駆け付けた警察官・救急隊が解除する、user 2026-11-14）。ログインだけでは解除しない */
function emergencyScreen(r: RegisterView, side: Side): string {
  const body = `${msg(
    "スタッフの応答がありません",
    "事件または事故の可能性があります<br>ご自身の安全を確保の上、通報をお願いいたします",
  )}<div class="sos-contact"><p class="sos-tel"><span>警察：<b>110</b></span><span>消防救急：<b>119</b></span></p><p class="small">緊急連絡先と店舗住所を記載したレシートを出力します</p></div>`;
  if (side === "customer") return `<div class="confirm change-alert emergency">${body}</div>`;
  // 解除はログインなしで誰でも押せる（駆け付けた警察官・救急隊も、user 2026-11-14）。表示のみの側でも押せるよう staff-only に入れる
  const end = `<span class="staff-only">${btn("解除", { type: "end_emergency", register: r.id }, undefined, "primary")}</span>`;
  return `<div class="confirm change-alert emergency">${msg("非常事態モードを解除します")}${end}</div>`;
}

/** 非常事態モード中のスタッフ側の最下段：オーナーと担当スーパーバイザーの連絡先（ヘッダーと同じ赤で点滅） */
function emergencyContactsRow(): string {
  return `<div class="call-row calling"><p class="sos-store">${STORE_CONTACTS.map(([who, tel]) => `<span>${who}<b>${tel}</b></span>`).join("")}</p></div>`;
}

/** 非常事態モードを解除した直後のスタッフ側：発動時刻と解除時刻を印字したレシートを出した案内（次の操作か10秒で閉じる） */
function emergencyEnded(snap: Snapshot, log: { started_at: number; ended_at: number }): string {
  return confirm(`${msg("非常事態モードを解除しました", "発動時刻と解除時刻を記載したレシートを出力しました")}<p class="sos-log"><span>発動：${simClock(snap, log.started_at)}</span><span>解除：${simClock(snap, log.ended_at)}</span></p>`);
}

export function callScreen(r: RegisterView): string {
  const v = r.staff;
  // 用件は最初の呼び出しの一件だけ（「＆」で繋がない）。スタッフがレジに着けば、残りはお客様との対話でわかる（user 2026-11-14）。
  // 用件はコアが決める：ボタンの呼び出しはボタンの文言、商品登録・お会計からの呼び出しは「お支払い」「宅配便」など具体的に
  const first = [...v.calls].sort((a, b) => a.created_at - b.created_at || a.id - b.id)[0];
  const reason = first?.topic ?? "";
  const bill = first?.kind === "bill" ? first : undefined;
  // 用件だけを第一待ち受けの「セルフレジ」と同じ特大の文字で（呼び鈴の絵・詳細・経過時間は出さない。経過秒数は最下段に出す、user 2026-11-14）。
  // 公共料金の枚数だけは対応の準備に要るので残す。長い用件は画面の幅に収まるよう文字を小さくする
  return `<div class="call-screen">
      <b class="call-reason mode-big" style="--len:${[...reason].length}">${esc(reason)}</b>
      ${bill ? `<p class="bill-call"><span class="bill-n">${esc(bill.detail)}</span></p>` : ""}
      <p class="small">鳴動を止めるにはログインしてください</p>
    </div>`;
}

function cashOnlyWait(names: string[]): string {
  return `<div class="point-screen">
      ${msg(`${esc(names.join("・"))}は現金のみのお取り扱いです`, "スタッフをお呼びしました<br>そのままお待ちください")}
    </div>`;
}

function cashCallWait(): string {
  return `<div class="point-screen">
      ${msg("現金は店員がお預かりいたします", "店員が向かっています")}
    </div>`;
}

function ageWait(age: number): string {
  return `<div class="point-screen">
      ${msg("年齢確認が必要な商品が含まれています", `${age}歳前後のお客様は身分証をご準備ください`)}
    </div>`;
}

/** 外国人の身分証（法令の年確でも認める）。並べると日本人にも外国人にも冗長なので、日本語の案内の下に英語で添える。
 *  店舗が任意で決めた年確（花火など）には添えない：日本語を介さない外国人の小中高生が買うのはまれ */
const FOREIGN_IDS = `<span lang="en" data-i18n-skip>Also OK: Residence Card, Special Permanent Resident Certificate, Passport</span>`;

/** 年確のあいだのお客様側。スタッフが「する」を選んだら身分証の提示をお願いし、
 *  スタッフが生年月日を打ち始めたら（テンキーを開いたら）判定中であることと、個人情報を残さないことを伝える */
function ageNote(reg: number, v: StaffView): string {
  if (!v.id_pending) return pairNote("年齢確認が必要な商品が含まれています");
  const typing = padEntry.get(`${reg}:staff`)?.target === "birth" || !!padValue.get(padValueKey(reg, "birth"));
  return typing
    ? pairNote("生年月日から年齢を判定しています", "端末は個人情報を保存しません")
    : `<div class="point-screen pair-note">${msg(
        "年齢確認が必要な商品が含まれています",
        v.age_official_id
          ? `免許証またはマイナンバーカードの提示をお願いいたします<br>${FOREIGN_IDS}`
          : "学生証など顔写真付きの身分証の提示をお願いいたします<br>デジタル身分証はご利用いただけません",
      )}</div>`;
}

function ageStep(reg: number, v: StaffView, side: Side, a: (n: Action) => Gate): string {
  const age = v.required_age;
  if (!age || side === "customer") return "";
  return v.age_choice_pending || v.id_pending ? idCard(reg, age, v, a) : "";
}

export const flashing = new Set<string>();
const flash = (reg: number, id: number): string => (flashing.has(`${reg}:${id}`) ? "flash" : "");

export function couponRows(reg: number, side: Side, coupons: Coupon[], st: Gate, ro = false): Row[] {
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

function removeCoupon(reg: number, side: Side, c: Coupon, st: Gate): string {
  return btn("取消", { type: "remove_coupon", register: reg, side, coupon_id: c.id }, st, "small ghost");
}

export function unmatchedCard(reg: number, side: Side, coupons: Coupon[], st: Gate, ro = false): string {
  const list = coupons
    .filter((c) => c.discount === null)
    .map((c) => `<li><b>${esc(c.name)}</b> ${ro ? "" : removeCoupon(reg, side, c, st)}</li>`)
    .join("");
  const [head, body] =
    side === "staff"
      ? ["適用先のないクーポンがあります", "対象商品と似た別の商品を通していないか確かめてください<br>対象商品を登録するか、クーポンを取り消すまで決済できません"]
      : ["ご利用になれないクーポンがあります", "クーポンの対象商品とお買い上げの商品が同じか、お確かめください<br>対象商品を追加するか、クーポンを取り消すとお支払いに進めます"];
  return `<div class="point-screen coupon-screen">${msg(head, body)}<ul class="coupon-list">${list}</ul></div>`;
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
  return `<div class="point-screen coupon-screen">${msg(head)}<ul class="coupon-list">${list}</ul>${
    done ? `<div class="point-choices">${btn(label, { type: "dismiss_returned", register: reg }, undefined, "primary")}</div>` : ""
  }</div>`;
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

/** 待ち受けの帯：会計ボタンを含む帯（.reg-top）と同じ縦幅。呼び鈴（大きめ）を中央に、左に言語切り替え、右に電子マネー（チャージ・残高照会）を寄せる
 *  言語切り替えは押した側の画面だけを切り替える（日本語・英語、標準は英語）。セルフ操作モードのスタッフ側は操作不可。
 *  電子マネーはスタッフ操作モードのお客様側では表示のみ。その側は呼び鈴がないので、言語と電子マネーを中央に詰める。商品登録が始まると呼び鈴はヘッダーに戻る */
function welcomeBand(r: RegisterView, side: Side, emoney: Gate, emoneyRo: boolean): string {
  const mirror = ' tabindex="-1" aria-disabled="true"';
  const big = (cls: string, main: string, sub: string, ro: boolean, attrs: string) =>
    `<button type="button" class="band-btn neg ${cls}${ro ? " mirror-only" : ""}"${cls === "lang-btn" ? " data-i18n-skip" : ""}${ro ? mirror : attrs}><b>${main}</b><small>${sub}</small></button>`;
  const langRo = r.mode === "self_operated" && side === "staff";
  const cmd = { type: "start_emoney", register: r.id, side, kind: "menu" };
  const lang = big("lang-btn", "Language", "言語", langRo, ` data-lang="${r.id}:${side}" title="この画面の言語を切り替える／Switch this screen's language"`);
  const money = big("emoney-btn", "電子マネー", "チャージ・残高照会", emoneyRo, ` data-cmd="${esc(JSON.stringify(cmd))}"${emoney.allowed ? "" : ` disabled title="${esc(emoney.reason ?? "")}"`}`);
  // 呼び鈴のない側（スタッフ操作モードのお客様側）は空きを作らず、両ボタンを中央に詰める
  const bell = bellBtn(r, side);
  return `<div class="reg-top welcome-band"><div class="band-row">${lang}${bell ? `<span class="bell-slot">${bell}</span>` : ""}${money}</div></div>`;
}

export function welcome(r: RegisterView, side: Side, emoney?: Gate): string {
  const reg = r.id;
  const on = scanGuide.get(`${reg}:${side}`);
  const guide = GUIDES.find(([k]) => k === on);
  return `<div class="welcome${emoney ? " scan-welcome" : ""}">
      ${msg("バーコードをスキャンしてください", "すべてのバーコードを受け付けます")}
      <div class="guide-row">${GUIDES.map(
        ([k, label]) => `<button type="button" class="guide-btn ${k === on ? "on" : ""}" data-guide="${reg}:${side}:${k}">${label}</button>`,
      ).join("")}</div>
      ${guide ? `<p class="guide">${guide[2]}</p>` : ""}
    </div>`;
}

export const voidAsk = new Map<number, Side>();

function voidBtn(reg: number, side: Side, st: Gate): string {
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
    r.prepaid && r.prepaid_method === "ポイント利用" ? `ポイントで払った ${pt(r.prepaid)} は残高に戻ります` : "",
    r.prepaid && r.prepaid_method !== "電子マネー" && r.prepaid_method !== "ポイント利用" ? `${esc(r.prepaid_method ?? "")}で払った ${yen(r.prepaid)} は取り消します` : "",
  ]
    .filter(Boolean)
    .map((s) => `<br>${s}`)
    .join("");
  return `<div class="point-screen void-screen">
      ${msg("取引を中止しますか？", `登録した ${itemCount(view.lines)}点（合計 ${yen(view.total)}）をすべて取り消します${back}`)}
      <div class="point-choices">${btn("はい", { type: "void_transaction", register: reg }, st, "primary")}</div>
    </div>
    <div class="back-foot"><button type="button" class="md ghost back-btn" data-void-cancel="${reg}">バック</button></div>`;
}

const STAFF_OP_GUIDES: [GuideKind, string, string][] = [
  ["shop", "お買い物", "商品のバーコードをスキャンしてください"],
  ["parcel", "宅配便", "宅配便の伝票、または受取用のバーコードをスキャンしてください"],
  ["bill", "お支払い", "支払い用紙のバーコードをスキャンしてください"],
];

function staffOperatedWelcome(r: RegisterView, side: Side): string {
  const reg = r.id;
  const on = scanGuide.get(`${reg}:staff`);
  const ro = side === "customer";
  const guide = STAFF_OP_GUIDES.find(([k]) => k === on);
  const guides = STAFF_OP_GUIDES.map(
    ([k, label]) =>
      `<button type="button" class="guide-btn ${k === on ? "on" : ""}${ro ? " mirror-only" : ""}"${ro ? ' tabindex="-1" aria-disabled="true"' : ` data-guide="${reg}:staff:${k}"`}>${label}</button>`,
  ).join("");
  const [head, sub] = ro ? ["いらっしゃいませ", "ご用件をお申し付けください"] : ["お辞儀とご挨拶をしましょう", "ご用件をお伺いしてください"];
  return `${welcomeBand(r, side, find(r.staff.actions, "emoney"), ro)}<div class="welcome staff-operated-welcome">${msg(head, sub)}
      <div class="guide-row">${guides}</div>
      ${guide && !ro ? `<p class="guide">${guide[2]}</p>` : ""}
    </div>`;
}

export function staffOperatedScreen(r: RegisterView, side: Side = r.active_side ?? "staff"): string {
  if (r.menu) return side === "staff" ? storeMenu(r) : menuPaused();
  const v = r.staff;
  const reg = r.id;
  const a = (n: Action) => find(v.actions, n);
  if (r.bill_handover && !v.lines.length) return confirm(billHandover(reg, side, r.bill_handover, a("hand_over_bills")));
  if (r.parcel_handover && !v.lines.length) return confirm(parcelHandover(reg, side, r.parcel_handover, a("hand_over_bills")));
  if (r.ticket_handover.length && !v.lines.length) return confirm(ticketHandover(reg, side, r.ticket_handover, r.tickets_issued, r.tickets_received, r.tickets_handed, side === "staff" ? find(r.staff.actions, "hand_tickets") : find(r.customer.actions, "receive_tickets")));
  if (r.pickup_handover && !v.lines.length) return confirm(pickupHandover(reg, side, r.pickup_handover));
  if (r.receipt_stamp && !v.lines.length) return confirm(receiptStampScreen(reg, side, r.receipt_stamp, r.last_receipt?.no, a("affix_receipt_stamp")));
  if (r.last_receipt && !v.lines.length) return (r.change_left ? changeLeft() : confirm(r.outlet ? changeDue(side, r.outlet) : thanks(r.last_receipt)));
  if (!v.lines.length && !v.coupons.length && !v.point_card) voidAsk.delete(reg);
  // ポイントカード（アプリ）だけ読んだときも取引は始まっているので、登録画面に進む
  if (!v.lines.length && !v.coupons.length && !v.point_card && !v.returned.length && !v.confiscated.length && !v.ff_collect.length) return staffOperatedWelcome(r, side);
  const ro = side === "customer";
  const rows = newestFirst([...couponRows(reg, side, v.coupons, a("remove_coupon"), ro), ...v.lines.map((l): Row => [l.touched, staffLine(reg, l, v, side)])]);
  const paying = v.phase === "paying";

  const pointPending = paying && v.point_pending;
  const agePending = (paying || v.age_early) && (v.age_choice_pending || v.id_pending);
  const payStage = paying && !pointPending && !agePending;
  const ask = side === "staff" ? "決済手段をお伺いしてください" : "決済手段をお申しつけください";
  // 「ポイント利用」を選んだら、ポイントの使い方（全部・端数・指定）を選ぶ画面に切り替わる
  const pointsUsing = payStage && v.method === "points" && r.points.balance !== null && !r.prepaid;
  const pay = pointsUsing
    ? pointsUse(r, side, a("select_payment"), side === "staff" ? "ポイントのご利用方法をお伺いしてください" : null)
    : payStage
    ? `<div class="paybox pay-stage">
        ${v.method || v.inserted ? "" : `<div class="big">${ask}</div>`}
        ${side === "staff" ? methodRow(reg, side, v.method, v.inserted, a("select_payment"), "", r.unavailable) : v.method ? "" : methodList(v.method, v.inserted, r.unavailable)}
        ${r.outlet && !v.inserted && !v.method ? cashBackNote(side, r.outlet) : r.prepaid && !v.inserted && !v.payment_blocker && (!v.method || v.method === "cash") ? prepaidNote(r, v.due, v.method) : side === "customer" && v.method && !v.inserted && !v.payment_blocker ? customerPayGuide(v.method, r.cashless_due, v.due) : v.method || v.inserted || v.payment_blocker ? payHint(v.method, v.payment_blocker, v.inserted, v.due, r.cashless_due, side === "staff", v.settle_in_ms) : cashOnlyNote(r) + pointNote(r)}
      </div>`
    : agePending
      ? ageStep(reg, v, side, a)
      : "";

  const ffFirst = !paying && v.lines.some((l) => l.kind === "ff" && l.state === "waiting") && a("check_ff").allowed;
  // 販売期限切れの品を回収するまで会計は押せないので、会計ボタンはグレーアウト（core の checkout が理由つきで拒む）
  const expiredAny = v.lines.some((l) => l.expired);
  // 一部を払ったあと（電子マネーの残高を使い切って残りを現金で払うときなど）は、合計を残りのお支払い額にする
  const top = regTop(
    v.total - r.prepaid,
    itemCount(v.lines),
    (paying && v.method === "cash" && !ro
      ? finishInsert(reg, v.inserted, v.due, a("insert_cash"))
      : ro || paying
        ? ""
        : ffFirst && !expiredAny
          ? `<button type="button" class="primary" data-ffverify="${reg}:${side}">会計</button>`
          : btn("会計", { type: "checkout", register: reg, side }, a("checkout"), "primary")),
    r.max_items,
    r.points,
    ro || voidAsk.has(reg) ? "" : voidBtn(reg, side, a("void_transaction")),
  );
  const back = ro
    ? ""
    : pointsUsing
      ? backCorner({ type: "select_payment", register: reg, side, method: "points" }, a("select_payment"))
      : paying
      ? backFoot(reg, side, a("back_to_registering"))
      : agePending
        ? backCorner({ type: "early_age_check", register: reg, side: "staff", start: false }, a("early_age_check"))
        : "";
  const screen = (body: string) => `${top}${confirm(body + (voidAsk.has(reg) ? "" : back))}`;
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
        ? parcelStep(reg, parcel, a("complete_service"), multiPiece(v.lines, parcel))
        : parcelNote(reg, parcel),
    );
  if (!paying && v.parcel_ask) return screen(side === "staff" ? parcelAsk(reg, v.lines, a("parcel_next")) : pairNote("同じお届け先にお送りする荷物はございますか？"));
  const slips = v.bills.length;
  if (paying && slips && v.bill_unchecked)
    return screen(side === "staff" ? billVerifyScreen(reg, side, v.bills, a("check_bill")) : billNote(slips, "スタッフが払込票を確認しています"));
  if (paying && slips && v.stamp_pending)
    return screen(side === "staff" ? stampVerifyScreen(reg, side, v.bills, a("affix_stamp"), false) : billNote(slips, "スタッフが領収証に収入印紙を貼り付けています"));
  if (paying && slips) return screen(side === "customer" ? billPayCard(slips) : billNote(slips, "お客様に受付点数、合計金額をお伝えください"));
  if (agePending) return screen(side === "staff" ? pay : ageNote(reg, v));
  // 回収イベント・照合の済んだ案内は、それぞれのボタン（回収しました・次へ）だけで閉じる（バックで登録画面に戻っても同じ案内が残るだけなので出さない）
  const held = firstOf([confiscatedCard(reg, side, v), passedCard(reg, side, v), side === "staff" ? ffCollectCard(reg, v) : ffReturnCard(v)]);
  if (held) return `${top}${confirm(held)}`;
  const card = firstOf([
    v.coupon_unmatched ? unmatchedCard(reg, side, v.coupons, a("remove_coupon"), ro) : "",
    returnedCard(reg, side, v.returned, side === "staff"),
  ]);
  if (card) return screen(card);
  if (pointPending) return screen(side === "staff" ? pointCheck(reg, side, a("confirm_point_card")) : customerPointAsk());
  scanGuide.delete(`${reg}:${side}`);
  if (payStage) return screen(pay);
  if (!paying && r.limit_alert)
    return screen(limitScreen(reg, side, r.limit_alert, ro || !a("remove_line").allowed ? null : a("remove_line"), side === "staff"));
  if (!ro && !paying && a("remove_line").allowed && expiredOpen(reg, side, v.lines))
    return screen(expiredScreen(reg, side, v.lines, a("remove_line"), true));
  if (ro && !paying && v.lines.some((l) => l.expired)) return screen(expiredScreen(reg, side, v.lines, null, false));
  if (!ro && !paying && ffVerifyOpen(reg, side, v.lines))
    return screen(ffVerifyScreen(reg, side, v.lines, false));
  closeFfVerify(reg, side);
  if (!ro && !paying && stampVerifying.has(`${reg}:${side}`) && stampPending(v.bills))
    return screen(stampVerifyScreen(reg, side, v.bills, a("affix_stamp"), true));
  stampVerifying.delete(`${reg}:${side}`);
  return `${top}
    <div class="staff-head">
      <span>スタッフ：<b>${esc(v.name)}</b>${v.role === "owner" ? "（オーナー）" : ""}${answeredTopic(r, side)}</span>
      ${taxNote(v)}
    </div>
    <table class="lines cols3">${LINE_COLS}<thead><tr><th>品目</th><th class="num">${qtyHead(v.lines)}</th><th class="num">小計</th></tr></thead>
      <tbody>${rows || `<tr><td colspan="3" class="muted">明細はありません<br>どのバーコードからでも読めます</td></tr>`}</tbody></table>
`;
}

function pointCheck(reg: number, side: Side, st: Gate): string {
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

function finishInsert(reg: number, inserted: number, due: number, st: Gate): string {
  const ready: Gate = !st.allowed
    ? st
    : !inserted
      ? { ...st, allowed: false, reason: "お金を入れてください" }
      : due > 0
        ? { ...st, allowed: false, reason: `あと ${yen(due)} 入れてください` }
        : st;
  return `<span class="staff-only">${btn("投入おわり", { type: "finish_insert", register: reg }, ready, "primary")}</span>`;
}

/** 合計の下のポイント表示。付与対象の商品がなく、カードも読んでいなければ出さない */
function pointInfo(p: PointsView | undefined): string {
  if (!p || (!p.eligible && p.balance === null)) return "";
  if (p.balance === null) return `<span class="point">ポイント：未読取</span>`;
  const earn = p.eligible ? `（今回 +${pt(p.earn)}）` : "";
  return `<span class="point read">ポイント ${pt(p.balance)}${earn}</span>`;
}

const pt = (n: number): string => `${n.toLocaleString("ja-JP")}P`;

export function regTop(total: number, count: number, action: string, max = Infinity, points?: PointsView, voidHtml = "", infoHtml = ""): string {
  // ポイントカードだけ読んだときも、取引中止と読んだポイントを出す
  if (count === 0 && (points?.balance ?? null) === null) return "";
  const full = count >= max ? `<small class="full">上限</small>` : "";
  const point = pointInfo(points);
  const info = point ? `<small class="info">${point}</small>` : "";
  const below = infoHtml ? `<small class="info below">${infoHtml}</small>` : "";
  return `<div class="reg-top"><span class="void">${voidHtml}</span><span class="mid"><span class="sum">合計 <span class="count"><b>${count}</b>点${full}</span> <b class="total">${total.toLocaleString("ja-JP")}</b><span class="yen">円</span></span>${info}${below}</span><span class="action">${action}</span></div>`;
}

export function itemCount(lines: { qty: number }[]): number {
  return lines.reduce((n, l) => n + l.qty, 0);
}

export function sharedScreen(r: RegisterView, snap: Snapshot, side: Side): string {
  if (r.paused)
    return confirm(`<div class="point-screen paused-screen">${msg(
      "レジ休止中",
      side === "customer" ? "お手数ですが、ほかのレジをご利用ください" : "スタッフバーコードを読むと再開します",
    )}</div>`);
  if (r.emergency) return emergencyScreen(r, side);
  if (r.emergency_log && side === "staff") return emergencyEnded(snap, r.emergency_log);
  if (!r.menu && r.outlet_detail.some(([d]) => d === 2000)) return returned2000(side);
  if (r.mode === "self_operated" && side === "staff" && !r.menu && r.staff.calls.length) return callScreen(r);
  if (r.mode === "self_operated" && side === "customer" && r.bell_dead && r.staff.calls.length) return bellDeadScreen();
  if (!r.menu && (r.emoney || r.emoney_short !== null || r.emoney_partial !== null)) return confirm(emoneyScreen(r, side));
  return r.mode === "staff_operated" ? staffOperatedScreen(r, side) : selfOperatedScreen(r, side);
}

function emoneyScreen(r: RegisterView, side: Side): string {
  const reg = r.id;
  const ro = r.mode === "staff_operated" && side === "customer";
  const actions = r.mode === "self_operated" ? r.customer.actions : r.staff.actions;
  const close = (label: string) =>
    ro ? "" : `<div class="back-foot">${btn(label, { type: "close_emoney", register: reg, side }, find(actions, "emoney"), "md ghost back-btn")}</div>`;
  if (r.emoney_partial !== null) {
    const rest = Math.max(0, r.staff.due - r.emoney_partial);
    const st = find(actions, "resolve_emoney_short");
    const go = (label: string, c: ShortChoice, cls: string) => btn(label, { type: "resolve_emoney_short", register: reg, side, choice: c }, st, cls);
    const other = ro ? "" : `<div class="back-foot">${go("他の決済を使う", "cancel", "md ghost back-btn wide")}</div>`;
    const note = `電子マネーの残高 <b>${yen(r.emoney_partial)}</b> を引き出し、残り <b>${yen(rest)}</b> を現金でお支払いいただきます`;
    if (!r.emoney_partial_ok)
      return `<div class="point-screen emoney-screen">
          ${msg("お支払い方法の確認", note)}
          ${ro ? "" : `<div class="point-choices">${go("確認しました", "cash_confirmed", "primary")}</div>`}
        </div>${other}`;
    return `<div class="point-screen emoney-screen">
        ${msg("電子マネーを決済端末にかざしてください", note)}
      </div>${other}`;
  }
  if (r.emoney_short !== null) {
    const st = find(actions, "resolve_emoney_short");
    const choice = (label: string, c: ShortChoice, cls = "") => btn(label, { type: "resolve_emoney_short", register: reg, side, choice: c }, st, cls);
    const short = Math.max(0, r.staff.due - r.emoney_short);
    const staffOp = r.mode === "staff_operated";
    if (!staffOp) {
      // セルフ操作モード：残高不足の時点ではスタッフを呼ばない（他の決済を使うならスタッフの無駄足になる）。
      // 「チャージまたは現金併用」（左・消極）でスタッフを呼び、「他の決済を使う」（右・積極）で支払方法の選択へ。
      const called = r.staff.calls.some((c) => c.kind === "emoney");
      // スタッフを呼んだあとは「他の決済を使う」も出さない（押されるとスタッフの無駄足になる）
      const buttons = called
        ? ""
        : `<div class="point-choices pair">${choice("チャージまたは\n現金併用", "staff", "ghost")}${choice("他の決済を使う", "cancel", "primary")}</div>`;
      return `<div class="point-screen emoney-screen emoney-short">
        ${msg("電子マネーの残高が足りません", `不足額 <b>${yen(short)}</b>${called ? "<br>スタッフが向かっています" : ""}`)}
        ${ro ? "" : buttons}
      </div>`;
    }
    const buttons = `<div class="point-choices pair">${choice("不足額を\n現金で払う", "cash", "primary")}${choice("電子マネーを\nチャージする", "charge", "primary")}</div>`;
    return `<div class="point-screen emoney-screen emoney-short">
        ${msg("電子マネーの残高が足りません", `不足額 <b>${yen(short)}</b>`)}
        ${ro ? "" : buttons}
      </div>${ro ? "" : `<div class="back-foot">${choice("他の決済を使う", "cancel", "md ghost back-btn wide")}</div>`}`;
  }
  const e = r.emoney!;
  if (e.kind === "menu") {
    const go = (label: string, kind: EmoneyKind) => btn(label, { type: "start_emoney", register: reg, side, kind }, find(actions, "emoney"));
    return `<div class="point-screen emoney-screen">
        ${msg("電子マネー", "選択してください")}
        ${ro ? "" : `<div class="point-choices">${go("チャージ", "charge")}${go("残高確認", "inquiry")}</div>`}
      </div>${close("バック")}`;
  }
  if (e.kind === "inquiry")
    return e.balance === null
      ? `<div class="point-screen emoney-screen"><div class="big">電子マネーを決済端末にかざしてください</div></div>${close("バック")}`
      : `<div class="point-screen emoney-screen">${msg(`電子マネーの残高<span class="emoney-balance">${yen(e.balance)}</span>`)}</div>${close("閉じる")}`;
  return `<div class="point-screen emoney-screen">
      ${msg("チャージする紙幣を入れてください", e.inserted ? "入れおわったら、電子マネーを決済端末にかざしてください" : "入金ボックスに入れます<br>1,000円札から使えます（残高は 20,000円まで）")}
    </div>${close("バック")}`;
}

/** 一部を払ったあとの残り。現金が選ばれている（電子マネーの残高不足で「不足額を現金で払う」を選んだときなど）か、
 *  残りが現金でしか払えないときは「現金で」、まだ選べるときは支払方法を選んでもらう。スタッフ側で選んだ支払方法とお客様側の案内を揃える */
function prepaidNote(r: RegisterView, due: number, method: PaymentMethod | null): string {
  const rest = r.cashless_due && method !== "cash" ? "のお支払い方法をお選びください" : "を現金でお支払いいただきます";
  // ポイントは「ポイントで 80P」と、ポイント数で出す（1ポイント＝1円）
  const [by, amount] = r.prepaid_method === "ポイント利用" ? ["ポイント", pt(r.prepaid)] : [esc(r.prepaid_method ?? ""), yen(r.prepaid)];
  return `<p>${by}で ${amount} お支払い済み<br>残り <b>${yen(due)}</b> ${rest}</p>`;
}

function cashBackNote(side: Side, amount: number): string {
  return side === "staff" ? `<p>現金 <b>${yen(amount)}</b> を出金ボックスからお返しください</p>` : `<p>現金 <b>${yen(amount)}</b> をお返しします</p>`;
}

/** ポイントの使い方：使える分を全部・お支払いの端数（100円未満）・ポイント数を指定。
 *  head が null の側（スタッフ操作モードのお客様側）は操作せず、案内だけを出す */
function pointsUse(r: RegisterView, side: Side, st: Gate, head: string | null): string {
  const p = r.points;
  const reg = r.id;
  const sub = `ポイント残高 ${pt(p.balance ?? 0)}、ご利用可能 ${pt(p.usable)}`;
  if (head === null) return `<div class="point-screen pair-note">${msg("ポイントのご利用方法をお申しつけください", sub)}</div>`;
  const use = (how: "all" | "fraction"): Command => ({ type: "use_points", register: reg, side, how });
  const amount = padValue.get(padValueKey(reg, "ppt")) ?? "";
  // 端数がない、またはご利用可能なポイントが端数に満たないときは「端数を払う」を出さない。
  // ご利用可能なポイントが 1P だけなら、指定しても「全部使う」と同じなので指定欄を出さない
  const fraction = p.fraction > 0 && p.fraction <= p.usable ? btn(`端数を払う\n${pt(p.fraction)}`, use("fraction"), st, "primary") : "";
  const specify =
    p.usable > 1
      ? `<div class="parcel-fields points-amount"><label>ポイント数を指定${padField(reg, side, "ppt", amount, "", "使うポイント数", undefined, "", st)}<span>P</span></label></div>`
      : "";
  return `<div class="point-screen points-use">
      ${msg(head, sub)}
      <div class="point-choices pair">${btn(`全部使う\n${pt(p.usable)}`, use("all"), st, "primary")}${fraction}</div>
      ${specify}
    </div>`;
}

/** 支払方法を選ぶ前に、読み取ったカードで使えるポイントを出す（スタッフ側・お客様側で同じ言い回し） */
function pointNote(r: RegisterView): string {
  const p = r.points;
  if (p.balance === null || r.prepaid || !p.usable) return "";
  return `<p>ポイント残高 ${pt(p.balance)}、ご利用可能 <b>${pt(p.usable)}</b></p>`;
}

function cashOnlyNote(r: RegisterView): string {
  if (!r.cash_only || r.prepaid) return "";
  return r.cashless_due
    ? `<p>うち <b>${yen(r.cash_only)}</b> は現金のみ取り扱いいたします</p>`
    : `<p>${CASH_ONLY_NOTE}</p>`;
}

const pad = (n: number) => String(n).padStart(2, "0");

const WEEKDAY = ["日", "月", "火", "水", "木", "金", "土"];

export function clockText(d: Date): string {
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日 ${WEEKDAY[d.getDay()]}曜日 ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** 呼び鈴の絵（色は CSS の currentColor：押せるときは青、押せないときはグレー） */
const BELL_SVG = `<svg class="bell-svg" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="currentColor" d="M12 2.5a1.5 1.5 0 0 0-1.5 1.5v.6A6.5 6.5 0 0 0 5.5 11v4.2L3.8 17.4a.9.9 0 0 0 .7 1.5h15a.9.9 0 0 0 .7-1.5l-1.7-2.2V11a6.5 6.5 0 0 0-5-6.4V4A1.5 1.5 0 0 0 12 2.5Zm-2.6 17.6a2.6 2.6 0 0 0 5.2 0Z"/></svg>`;

/** 呼び鈴。スタッフ側は応援を呼ぶ（SOS）、お客様側はスタッフを呼ぶ。スタッフ操作モードのお客様側には出さない */
function bellBtn(r: RegisterView, side: Side): string {
  const call = find(r.customer.actions, "call_staff");
  const sos = side === "staff";
  const tip = sos ? "仲間のスタッフを呼ぶ（押している間鳴ります）" : "スタッフを呼ぶ（押している間鳴ります）";
  const cmd = sos ? { type: "call_staff", register: r.id, topic: "sos" } : { type: "call_staff", register: r.id };
  // 呼び鈴が不通ならグレーアウト。セルフ操作モードのお客様側だけは押せるままにし、押すと店員へ直接声をかける案内を出す
  const dead = r.bell_dead && (sos || r.mode === "staff_operated");
  const ok = call.allowed && !dead;
  const title = dead ? "呼び出し端末が動作していません" : call.allowed ? tip : (call.reason ?? "");
  // 呼び出し中も青のまま：鳴らし続け・繰り返しでエマージェンシーモードに入るので、押す意味がある
  const bell =
    !sos && r.mode === "staff_operated"
      ? ""
      : `<button type="button" class="bell-btn" data-hold="${esc(JSON.stringify(cmd))}" aria-label="${tip}" title="${esc(title)}"${ok ? "" : " disabled"}>${BELL_SVG}</button>`;
  return bell;
}

export function screenHead(r: RegisterView, now: Date, side: Side): string {
  const bell = bellBtn(r, side);
  const mode = r.emergency ? `<span class="mode-title emergency">非常事態モード</span>` : r.paused ? "" : r.menu ? `<span class="mode-title store-menu">店舗処理モード</span>` : modeTitle(r.mode === "staff_operated");
  return `<span class="head-l">${mode}</span><span class="head-c">${bell}</span><span class="head-r"><time class="clock">${clockText(now)}</time></span>`;
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

/** スタッフ操作モードのお客様側の最下段（セルフ操作モードの FF・サービス・たばこの段と同じ位置・高さ）に流すサイネージ。
 *  セール情報や詐欺防止の呼びかけを順番に表示する（シミュレーターの見本）。最下段を両モードで揃え、モード切り替え時の画面の変化を抑える。
 *  現物では広告を画像で流す想定なので、どのモードでも言語切り替えの対象外（data-i18n-skip） */
export const SIGNAGE: readonly (readonly [string, string])[] = [
  // 値引きを約束する広告は出さない（シミュレーターに値引きの仕組みがなく、会計と食い違うため）。販促は告知の枠だけを見本にする
  ["販促キャンペーン告知", "揚げ物・中華まんなど、店舗のキャンペーンをお知らせする枠です"],
  ["電話で電子マネーの購入を求められたら詐欺です", "ご心配なときはスタッフにお声がけください"],
  ["販促キャンペーン告知", "新商品・季節の商品をお知らせする枠です"],
];
export const SIGNAGE_MS = 8000;

export function signage(now = Date.now()): string {
  const [big, small] = SIGNAGE[Math.floor(now / SIGNAGE_MS) % SIGNAGE.length];
  return `<div class="signage" aria-label="お知らせ" data-i18n-skip>${msg(esc(big), esc(small))}</div>`;
}

/** 第一待ち受け画面：レジ前に人がいない間の待ち受け。ヘッダーと待ち受けの帯はそのまま、三段目に稼働モードを特大の文字で示し、最下段はサイネージ。
 *  現物ではレジ画面のカメラがレジ前の人を認識すると解除され（第二待ち受け＝従来の待ち受けへ）、シミュレーターではレジ画面にカーソルが重なると解除される */
export function firstWelcome(r: RegisterView, side: Side): string {
  const staffOp = r.mode === "staff_operated";
  const band = staffOp
    ? welcomeBand(r, side, find(r.staff.actions, "emoney"), side === "customer")
    : welcomeBand(r, side, find(r.customer.actions, "emoney"), false);
  return `${band}<div class="first-welcome"><b class="mode-big">${staffOp ? "有人レジ" : "セルフレジ"}</b></div>`;
}

/** スタッフ側で店舗メニューを開いている間のお客様側：第一待ち受けと同じ形で「休止中」を特大の文字で示す。
 *  帯には部品を置かず英語の「Please use another register」、「休止中」の下に添え字「空いているレジをお使いください」。言語切り替えの影響を受けない（data-i18n-skip）。最下段はサイネージ。レジ前に人が立っても解除されず、スタッフが店舗メニューを閉じたときに解除される */
export function menuPaused(): string {
  return `<div class="reg-top welcome-band" data-i18n-skip><div class="band-row">${msg("Please use another register")}</div></div><div class="first-welcome" data-i18n-skip><b class="mode-big">休止中</b><small class="mode-sub">空いているレジをお使いください</small></div>`;
}

/** 待ち受け（第二待ち受け）を表示しているか。第一待ち受けに切り替えてよいのはこのときだけ */
export const isWelcome = (main: string): boolean => main.includes("welcome-band");


/** セルフ操作モードの最下段：お客様の呼び出しに応答があるまで。お客様側は「スタッフを呼び出しています」（添え字なし）、
 *  スタッフ側は呼び出しからの経過秒数（取り消しボタンは置かない、user 2026-11-14） */
function callingRow(r: RegisterView, side: Side, now: number): string {
  if (side === "customer") return `<div class="call-row calling">${msg("スタッフを呼び出しています")}</div>`;
  const at = r.staff.calls.filter((c) => r.waiting_calls.includes(c.kind)).map((c) => c.created_at);
  return `<div class="call-row calling">${msg(at.length ? `${secs(now, Math.min(...at))}経過` : "")}</div>`;
}

/** スタッフがログインして応答した呼び出しの用件（最初の一件）。取引が終わるまでスタッフ側の担当スタッフ名の右に出す */
export function answeredTopic(r: RegisterView, side: Side): string {
  const topics = side === "staff" ? (r.answered ?? "") : "";
  return topics ? `<span class="answered-topic" title="お客様の呼び出しの用件">用件：${esc(topics)}</span>` : "";
}

export function itemsPanel(r: RegisterView, snap: Snapshot, ui: FfUi, side: Side, now = Date.now(), first = false): string {
  const reg = r.id;
  if (r.menu && side === "customer" && !r.paused) return signage(now);
  // 非常事態モード中のお客様側の最下段：英語で「警戒してください」「通報をお願いします」（ヘッダーと同じ赤で点滅）
  if (r.emergency && side === "customer") return `<div class="call-row calling" data-i18n-skip>${msg("Stay alert", "Please call emergency services")}</div>`;
  if (r.emergency) return emergencyContactsRow();
  // セルフ操作モードで応答のない呼び出しがあるあいだは、種類を問わず（年確・公共料金・宅配便・電子マネー・釣銭忘れなど）
  // 最下段を呼び出し中の案内にする（user 2026-11-14：あらゆる呼び出しで挙動を揃える）。電子マネーの手続き中なども同じ
  if (r.mode === "self_operated" && !r.paused && !r.menu && r.waiting_calls.length) return callingRow(r, side, snap.now);
  if (r.paused || r.menu || r.emoney || r.emoney_short !== null || r.emoney_partial !== null || voidAsk.has(reg)) return "";
  if (first) return signage(now);
  if (r.mode === "self_operated") {
    const a = (n: Action) => find(r.customer.actions, n);
    const callBtn = (main: string, sub: string, cmd: Command, st: Gate, cls: string) =>
      `<button class="call-btn ${cls}" data-hold="${esc(JSON.stringify(cmd))}"${st.allowed ? "" : ` disabled title="${esc(st.reason ?? "")}"`}><b>${esc(main)}</b>${sub ? `<small>${esc(sub)}</small>` : ""}</button>`;
    return `<div class="call-row">${[
      callBtn("FF", "揚げ物・中華まん", { type: "ff_button", register: reg }, a("ff_button"), "ff"),
      callBtn("サービス", "お支払い・荷物", { type: "call_staff", register: reg, topic: "service" }, a("call_staff"), "service"),
      callBtn("金券", "ギフトカード・印紙", { type: "call_staff", register: reg, topic: "voucher" }, a("call_staff"), "voucher"),
      callBtn("たばこ", "紙巻き・電子", { type: "call_staff", register: reg, topic: "tobacco" }, a("call_staff"), "tob"),
    ].join("")}</div>`;
  }
  if (r.mode === "staff_operated" && side === "customer") return signage(now);
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
    ([k, label]) => `<button type="button" role="tab" class="tab ff-tab ${k === ui.tab ? "on" : ""}" data-fftab="${reg}:${k}" aria-selected="${k === ui.tab}">${label}</button>`,
  ).join("");
  const grid = items
    .slice(page * FF_PER_PAGE, (page + 1) * FF_PER_PAGE)
    .map((f) => {
      const cmd: Command = { type: "register_ff", register: reg, side, code: f.code, qty: 1 };
      if (f.level !== "out" || !can.allowed) return btn(f.name, cmd, can, `ff-item ${f.level}`);
      return `<button type="button" class="ff-item out" aria-disabled="true" data-ffforce="${esc(JSON.stringify({ ...cmd, force: true }))}" title="欠品：長押しで予約注文として登録します">${esc(f.name)}</button>`;
    })
    .join("");
  const arrows =
    pages > 1
      ? `<div class="ff-arrows">${
          page > 0 ? `<button type="button" class="ff-arrow" data-ffpage="${reg}:-1" aria-label="前のページ">▲</button>` : "<span></span>"
        }${page < pages - 1 ? `<button type="button" class="ff-arrow" data-ffpage="${reg}:1" aria-label="次のページ">▼</button>` : "<span></span>"}</div>`
      : "";
  return `<div class="tabs ff-tabs" role="tablist" aria-label="FF">${tabs}</div><div class="ff-page"><div class="ff-grid">${grid}</div>${arrows}</div>`;
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

const MENU_ITEMS: MenuScreen[] = ["refund", "emergency", "deposit", "expense", "changer", "journal", "waste", "ff_fresh"];

export function storeMenuUsable(r: RegisterView): boolean {
  return r.mode === "staff_operated" && r.staff.state === "ready" && !r.paused && (r.menu !== null || !inTransaction(r));
}

/** 取引中（明細・クーポンがある、または電子マネーの手続き中）は店舗メニューを開けない */
function inTransaction(r: RegisterView): boolean {
  return r.staff.lines.length > 0 || r.staff.coupons.length > 0 || r.staff.phase === "paying" || r.emoney !== null;
}

export function storeMenu(r: RegisterView): string {
  const v = r.staff;
  const reg = r.id;
  const m = r.menu!;
  const go = (screen: MenuScreen | null, extra: { exchange_to?: number } = {}): Command => ({ type: "open_menu", register: reg, screen, ...extra });
  const head = (title: string) => `<div class="menu-head"><b class="big">${esc(title)}</b></div>`;
  const screen = (body: string) => `<div class="menu-screen">${body}${backFootRow(go("top"))}</div>`;
  switch (m.screen) {
    case "top":
      return `<div class="menu-head"><b class="big">店舗メニュー</b><span class="muted">担当：${esc(v.name ?? "")}</span></div>
        <div class="menu-list">${MENU_ITEMS
          .map((k) => btn(MENU_LABEL[k], go(k), undefined, "menu-item"))
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
        <p class="muted">入金する金額を下の<b>「入金」ボックス</b>に入れて Enter を押してください</p>
        <p class="muted">記録は担当者名つきで残ります</p>`);
    case "changer": {
      const have = (d: number) => v.drawer.find(([x]) => x === d)?.[1] ?? 0;
      const segs = [1000, 500, 100, 50, 10]
        .map((d) => btn(`${d.toLocaleString()}円`, go("changer", { exchange_to: d }), undefined, `seg small ${m.exchange_to === d ? "on" : ""}`))
        .join("");
      return screen(`${head(MENU_LABEL.changer)}
        <div class="row wrap">受け取る金種 ${segs}</div>
        <p class="muted">両替する金額を<b>「入金」ボックス</b>に入れて Enter を押してください<br>同じ額を ${m.exchange_to.toLocaleString()}円 で<b>「出金」ボックス</b>に出します</p>
        <p class="muted">釣銭機の ${m.exchange_to.toLocaleString()}円：残り ${have(m.exchange_to)} 枚　在高 ${yen(v.drawer_total)}</p>`);
    }
  }
}

export function selfOperatedScreen(r: RegisterView, side: Side = r.active_side ?? "customer"): string {
  const c: CustomerView = r.customer;
  const reg = r.id;
  const a = (n: Action) => find(c.actions, n);
  if (r.bill_handover && !c.lines.length) return confirm(billHandover(reg, side, r.bill_handover, a("hand_over_bills")));
  if (r.parcel_handover && !c.lines.length) return confirm(parcelHandover(reg, side, r.parcel_handover, a("hand_over_bills")));
  if (r.ticket_handover.length && !c.lines.length) return confirm(ticketHandover(reg, side, r.ticket_handover, r.tickets_issued, r.tickets_received, r.tickets_handed, side === "staff" ? find(r.staff.actions, "hand_tickets") : find(r.customer.actions, "receive_tickets")));
  if (r.receipt_stamp && !c.lines.length) return confirm(receiptStampScreen(reg, "customer", r.receipt_stamp, r.last_receipt?.no, a("affix_receipt_stamp"), false));
  if (r.last_receipt && !c.lines.length) return (r.change_left ? changeLeft() : confirm(thanks(r.last_receipt)));

  const lineRows = c.lines.map((l): Row => {
      const tags = [expiredFlag(reg, side, l, a("remove_line").allowed), stampFlag(reg, side, c.bills.find((b) => b.line_id === l.id), a("affix_stamp"), true), serviceFlag(l), ageFlag(l), limitFlag(l)].filter(Boolean);
      const fixedHere = isBill(l) || l.pickup || l.parcel || (l.kind === "ff" && l.state === "received");
      const n = l.tickets || l.qty;
      const qty =
        c.phase === "registering" && !fixedHere
          ? qtyCell(reg, side, { id: l.id, qty: n }, a("remove_line"), { type: "remove_line", register: reg, side, line_id: l.id }, l.kind === "service", l.fresh, lineMax(l, c.lines))
          : `${n}`;
      return [l.touched, `<tr class="${flash(reg, l.id)}${tags.length ? " flagged" : ""}"><td>${esc(l.name)} ${tags.join(" ")}</td><td class="num">${qty}</td><td class="num">${l.kind === "service" && !l.done && !l.amount ? "—" : yen(l.amount)}</td></tr>`];
    });
  const rows = newestFirst([...couponRows(reg, side, c.coupons, a("remove_coupon")), ...lineRows]);

  const top = regTop(
    c.total - r.prepaid,
    itemCount(c.lines),
    (c.phase === "registering"
      ? btn("お会計", { type: "checkout", register: reg, side }, a("checkout"), "primary")
      : ""),
    r.max_items,
    r.points,
    voidAsk.has(reg) ? "" : voidBtn(reg, side, a("void_transaction")),
  );

  if (!c.lines.length && !c.coupons.length && !c.point_card) voidAsk.delete(reg);
  if (!c.lines.length && !c.coupons.length && !c.point_card && !c.returned.length) return `${welcomeBand(r, side, a("emoney"), false)}${welcome(r, side, a("emoney"))}`;
  const back = c.phase === "paying" ? backFoot(reg, side, a("back_to_registering")) : "";
  const screen = (body: string) => `${top}${confirm(body + (voidAsk.has(reg) ? "" : back))}`;
  if (voidAsk.has(reg)) return screen(voidConfirm(r, side, c));
  const slips = c.bills.length;
  if (c.phase === "paying" && slips) return screen(c.bill_unchecked ? billNote(slips, "払込票の内容を確認いたします") : c.stamp_pending ? billNote(slips, "スタッフが領収証に収入印紙を貼り付けます") : billPayCard(slips, true));
  if (c.phase === "paying" && c.age_wait) return screen(ageWait(c.age_wait));
  if (c.phase === "paying" && r.cash_only) return screen(cashOnlyWait([...new Set(c.lines.filter((l) => l.cash_only).map((l) => l.name))]));
  // セルフ操作モードで「現金」を選ぶと店員呼び出し、お客様側は支払方法の選択から案内画面に切り替わる
  if (c.phase === "paying" && c.method === "cash" && side === "customer") return screen(cashCallWait());
  if (c.lines.some((l) => l.parcel && l.kind === "service" && !l.done)) return screen(parcelWait());
  if (c.phase === "registering" && r.limit_alert)
    return screen(limitScreen(reg, side, r.limit_alert, a("remove_line").allowed ? a("remove_line") : null, side === "staff"));
  if (c.phase === "registering" && a("remove_line").allowed && expiredOpen(reg, side, c.lines))
    return screen(expiredScreen(reg, side, c.lines, side === "staff" ? a("remove_line") : null, side === "staff"));
  const card = firstOf([
    c.coupon_unmatched ? unmatchedCard(reg, side, c.coupons, a("remove_coupon")) : "",
    returnedCard(reg, side, c.returned, true),
  ]);
  if (card) return screen(card);
  if (c.phase === "paying" && c.point_pending) return screen(pointCheck(reg, side, a("confirm_point_card")));
  scanGuide.delete(`${reg}:${side}`);
  if (c.phase === "paying" && c.method === "points" && r.points.balance !== null && !r.prepaid)
    return `${top}${confirm(pointsUse(r, side, a("select_payment"), "ポイントの使い方を選んでください") + (voidAsk.has(reg) ? "" : backCorner({ type: "select_payment", register: reg, side, method: "points" }, a("select_payment"))))}`;
  if (c.phase === "paying")
    return screen(`<div class="paybox pay-stage">
        <div class="big">お支払い方法を選択してください</div>
        ${methodRow(reg, side, c.method, c.inserted, a("select_payment"), "center", r.unavailable, true)}${r.prepaid && !c.inserted && !c.payment_blocker && (!c.method || c.method === "cash") ? prepaidNote(r, c.due, c.method) : !c.method && !c.inserted && !c.payment_blocker && (r.cash_only || r.points.usable) ? cashOnlyNote(r) + pointNote(r) : payHint(c.method, c.payment_blocker, c.inserted, c.due, r.cashless_due, false, c.settle_in_ms, true, side === "staff")}
      </div>${cashCorner(reg, side, c.method, a("select_payment"))}`);
  if (ffVerifyOpen(reg, side, c.lines))
    return screen(ffVerifyScreen(reg, side, c.lines, true));
  closeFfVerify(reg, side);
  return `${top}
    <div class="staff-head">${taxNote(c)}</div>
    <table class="lines cols3">${LINE_COLS}<thead><tr><th>品目</th><th class="num">${qtyHead(c.lines)}</th><th class="num">小計</th></tr></thead><tbody>${rows}</tbody></table>`;
}

const BELL_ICON = { fixed: "🔔", handheld: "📱", store_computer: "🖥️" } as const;

export function bellPanel(s: Snapshot): string {
  return s.bells
    .map(
      // 呼び鈴機能が OFF、または不通の端末は鳴らない＝操作不可の部品と同じグレーアウト
      (m) => `<div class="bell-unit ${m.buzzing ? "buzz" : ""} ${m.bell_on && !m.offline ? "" : "off"}">
        <span class="bell-icon">${BELL_ICON[m.kind]}</span><span class="bell-id">${esc(m.id)}</span>${!m.bell_on ? `<span class="bell-state">呼び鈴OFF</span>` : m.offline ? `<span class="bell-state">不通</span>` : m.buzzing ? `<span class="bell-state">鳴動中</span>` : ""}
      </div>`,
    )
    .join("");
}

export function bellSettings(s: Snapshot): string {
  const rows = s.bells
    .map((m, i) =>
      m.kind === "fixed"
        ? `<label class="bell-setting"><span>${BELL_ICON[m.kind]} ${esc(m.id)}</span><input type="checkbox" checked disabled aria-label="${esc(m.id)}の呼び鈴機能" /><span class="muted">常に ON</span></label>`
        : `<label class="bell-setting"><span>${BELL_ICON[m.kind]} ${esc(m.id)}</span><input type="checkbox" data-bell-on="${i}"${m.bell_on ? " checked" : ""} aria-label="${esc(m.id)}の呼び鈴機能" /><span>${m.bell_on ? "ON" : "OFF"}</span></label>`,
    )
    .join("");
  const off = s.bells.every((m) => m.offline);
  const test = `<label class="bell-setting"><span>呼び鈴を不通にする</span><input type="checkbox" data-bells-offline${off ? " checked" : ""} aria-label="呼び鈴を不通にする" /><span>${off ? "不通" : "通常"}</span></label>`;
  return `${rows}
    <h3 class="bell-sub">エマージェンシーモードの確認</h3><p class="muted">セルフ操作モードで、呼び鈴を3分以上鳴らしっぱなしにする、または3分以内に10回以上鳴らすと、両画面に通報をお願いする案内を出します<br>スタッフがログインすると解除されます<br>呼び鈴を不通にして、スタッフに届かない状況で試せます</p>${test}`;
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
    id: "tax",
    label: "税率",
    note: "税率は税区分ごとに持ちます<br>法改正で税率が変わったら区分の税率を変え、新しい税率ができたら区分を追加してください<br>価格は税込で、1回のお会計で税率ごとに合計してから1回だけ端数処理します<br>変更はこれから登録する商品から適用されます",
  },
  {
    id: "age",
    label: "年齢制限",
    note: "年確は会計ボタンを押した時点で、カゴの中で最も厳しい年齢で1回だけ行います<br>酒・たばこは法令どおり20歳以上で固定され、変更できません",
  },
  { id: "limit", label: "個数制限", note: "1回のお会計で売れる販売個数の上限です<br>超える分はスキャンしても登録されません" },
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
  const tabs = RULES_TABS.map(
    (x) => `<button type="button" role="tab" class="tab ${x.id === cur.id ? "on" : ""}" data-rules-tab="${x.id}" aria-selected="${x.id === cur.id}">${x.label}</button>`,
  ).join("");
  return `<div class="tabs rules-tabs" role="tablist" aria-label="設定する項目">${tabs}</div>
    <p class="muted">${cur.note}</p>
    ${body}
    <h4>変更履歴</h4><ul>${hist}</ul>`;
}

type Shelf = { label: string; match: (p: Snapshot["catalog"][number]) => boolean };

const byCodes = (...codes: string[]) => (p: { code: string }) => codes.includes(p.code);
const SHELVES: Shelf[] = [
  { label: "日配食品", match: byCodes("4900000000028", "4900000000042", "4900000000011", "4900000000080") },
  { label: "加工食品", match: byCodes("4900000000035", "4900000000103", "4900000000073", "4900000000066") },
  { label: "非食品", match: byCodes("4910000000014", "4910000000038", "4900000000127", "4900000000158", "4900000000134", "4900000000141", "4910000000076", "4910000000069", "4910000000090") },
  { label: "サービス", match: (p) => p.category === "service" },
  { label: "オプション", match: (p) => p.category === "bag" },
];

const PARCEL_CODES = ["2100000000014", "2100000000045", "2100000000081"];

const PICKUP_CODE = "2100000000038";

const BILL_CODES = ["2100000000021", "2100000000043", "2100000000050"];
const TAX_CODES = ["2100000000067"];
const TICKET_CODES = ["2100000000074"];

/** Item の商品ボタンのホバー：その商品が何のデモンストレーションのために置いてあるか。
 *  登録のしかたは Item の見出しのホバーにまとめてあるので、ここには書かない */
const PURPOSE: Record<string, string> = {
  "4900000000028": "割引券（50円引き）の対象商品、クーポンの読み取りと値引きを試す",
  "4900000000042": "軽減税率8%の日配食品、レジ画面では数量を増やせず、一点ずつスキャンすることを確かめる",
  "4900000000011": "軽減税率8%の日配食品、基本の登録と、＋で数量を増やせないことを確かめる",
  "4900000000035": "引換券（1個無料）の対象商品、クーポンの読み取りと引き換えを試す",
  "4900000000103": "法令による年齢確認（20歳）、標準税率10%なので、食品と一緒に買うと税率ごとの内税表示も確かめられる",
  "4900000000080": "販売期限が切れた日配食品、期限切れの商品を登録したときの警告と取り替えを試す",
  "4900000000073": "規則のない軽減税率8%の加工食品、数量の変更や販売規則の個数制限を試す",
  "4900000000066": "規則のない軽減税率8%の加工食品、数量の変更や販売規則の個数制限を試す",
  "4910000000014": "店頭で買う新聞は軽減税率の対象外（標準税率10%）、税区分の違いを確かめる",
  "4910000000038": "標準税率10%の非食品、販売規則で店舗独自の年齢確認を付けるなどの設定を試す",
  "4900000000127": "法令による年齢確認（20歳）、スタッフがログインしているレジでのみ扱える",
  "4900000000158": "コンビニでは高額な品、まとめ買いで税抜5万円を超え、レシートの収入印紙の確認を再現する",
  "4910000000076": "年齢確認（18歳）、20歳の年齢確認を済ませたあとに登録すると、18歳の確認をやり直すことを確かめる",
  "4910000000069": "コンビニでは高額な品、たばこカートンと合わせて、またはセルフ操作モードでも25個で税抜5万円になり、収入印紙の確認を再現する",
  "4910000000090": "コンビニの販売制限商品の代表格（金券の扱い）、スタッフがログインしているレジでのみ扱え、1回のお会計3パックまで、4パック目はスキャンしても登録されず、＋も消えることを確かめる",
  "4900000000134": "法令ではなく店舗の判断で年齢確認（20歳）を行う例",
  "4900000000141": "現金でしか買えない商品、スタッフがログインしているレジでのみ扱える",
  "2000000000015": "有料のレジ袋、待ち受けの「レジ袋」案内と対になる登録",
  "2000000000022": "有料のレジ袋、待ち受けの「レジ袋」案内と対になる登録",
  "2000000000039": "お弁当などに添える付属品、オプションの登録を試す",
  "2000000000046": "お弁当などに添える付属品、オプションの登録を試す",
  "2000000000053": "お弁当などに添える付属品、オプションの登録を試す",
  "2100000000014": "宅配便の受付（サイズ・お届け先の郵便番号・送料）、ほかの商品と一緒にお会計できない",
  "2100000000045": "送料をお届け先が払う宅配便の受付、代金0円で受け付けだけを行う",
  "2100000000081": "同じお届け先に複数の荷物を送る受付、2個目からは届け先を引き継ぎ、サイズを決めるだけで送料が決まる",
  "2100000000038": "荷物の受け取り、受取番号の表示、保管場所の荷物の照合、サインからお渡しまで",
  "2100000000021": "5万円以上の払込票、現金のみで、収入印紙の貼り付け確認が済むまでお会計に進めない",
  "2100000000043": "公共料金の受付、スタッフが払込先と金額を確かめ、現金で受け取る",
  "2100000000050": "公共料金の受付、スタッフが払込先と金額を確かめ、現金で受け取る",
  "2100000000067": "電気料金と同じ5万円でも、税金の領収証には収入印紙が要らないことを確かめる",
  "2100000000074": "申込券でチケットを買う、現金のみで一件ずつ会計、支払い後に発券し、スタッフのお渡し枚数とお客様の受け取り枚数をそれぞれ発券枚数と照合する",
};

/** Item・Wallet のボタン。back（裏面）があれば、表は商品名と価格だけにして、それ以外の情報は裏に回す。
 *  クリック（モバイルのピッカーではタップ）でカードを裏返すように表と裏を入れ替える */
function item(code: string, name: string, sub: string, cls = "", hint = PURPOSE[code] ?? "", back = ""): string {
  const face = `<span class="item-name">${esc(name)}</span>${sub ? `<span class="item-sub">${sub}</span>` : ""}`;
  const body = back ? `<span class="item-face item-front">${face}</span><span class="item-face item-back">${back}</span>` : face;
  const flip = back ? ` data-flip="${esc(code)}"` : "";
  if (cls.split(" ").includes("off"))
    return `<button type="button" class="item ${cls}"${flip} aria-disabled="true" title="${esc(hint)}" aria-label="${esc(name)}（${esc(hint)}）">
    ${body}</button>`;
  const tip = hint ? ` data-hint="${esc(hint)}"` : "";
  return `<button type="button" class="item ${cls}" data-code="${esc(code)}"${flip}${tip} aria-label="${esc(name)}${hint ? `、${esc(hint)}` : ""}">
    ${body}</button>`;
}

/** スタッフがログインしているレジ（スタッフ操作モード）でだけ扱えるカテゴリ（たばこ・切手・金券）。
 *  どのレジにもスタッフがいないあいだは Item エリアでグレーアウトし、ドラッグできない */
const STAFF_ONLY_SHELF = ["tobacco", "stamp", "voucher"];
const STAFF_ONLY_OFF = "スタッフがログインしているレジでのみ扱えます";

export function shelfPanel(s: Snapshot, extra = ""): string {
  const staffOperated = s.registers.some((r) => r.mode === "staff_operated");
  const asked = s.registers.some((r) => r.tobacco_asked);
  return SHELVES.map((g) => {
    const items = s.catalog.filter(g.match);
    if (!items.length) return "";
    return `<div class="shelf"><h3>${esc(g.label)}</h3><div class="items uniform">${items
      .map((p) =>
        PARCEL_CODES.includes(p.code)
          ? item(p.code, p.name.split(" ")[0], esc(p.name.split(" ")[1] ?? ""))
          : item(
              p.code,
              p.name,
              p.code === PICKUP_CODE ? "" : p.price ? yen(p.price) : "金額は受付時",
              STAFF_ONLY_SHELF.includes(p.category) && !staffOperated ? "off" : p.category === "tobacco" && asked ? "call-blink" : "",
              STAFF_ONLY_SHELF.includes(p.category) && !staffOperated ? STAFF_ONLY_OFF : undefined,
              backOf(p),
            ),
      )
      .join("")}</div></div>`;
  }).join("") + extra + slipShelf(s);
}

/** Item の商品ボタンの裏面：商品名と価格のほかの情報（年確・限定・販売期限・種別・枚数など）。何もなければ空（裏返らない）。
 *  販売期限切れは警告色。賞味期限はボタンには載せず、期限切れの案内にだけ使う */
export function backOf(p: { code: string; age: number | null; limit: number | null; fresh: boolean; dates: Dates | null; tickets: number }): string {
  const marks = [p.age ? `<b>年確${p.age}</b>` : "", p.limit ? `<b>限定${p.limit}</b>` : ""].filter(Boolean).join(" ・");
  const kind = BILL_CODES.includes(p.code) ? "公共料金" : TAX_CODES.includes(p.code) ? "税金" : p.code === PICKUP_CODE ? "受取用バーコード" : "";
  const lines = [
    kind,
    TICKET_CODES.includes(p.code) ? `${p.tickets}枚` : "",
    marks,
    p.dates ? (p.dates.sell_by_min < 0 ? `<b>販売 ${expiryLabel(p.dates.sell_by_min, p.fresh)}</b>` : `販売 ${expiryLabel(p.dates.sell_by_min, p.fresh)}`) : "",
  ].filter(Boolean);
  return lines.map((l) => `<span class="item-sub">${l}</span>`).join("");
}

function slipShelf(s: Snapshot): string {
  const slips = s.registers.flatMap((r) =>
    r.staff.lines
      .filter((l) => l.parcel?.slip && l.kind === "service" && l.done)
      .map((l) => item(`PS${l.parcel!.slip.replace(/-/g, "")}`, `伝票 レジ${r.id}`, esc(l.parcel!.slip), "", "スタッフ側に読むと、この荷物の受付を取り消します")),
  );
  return slips.length ? `<div class="shelf"><h3>荷物の伝票（取り消し用）</h3><div class="items uniform">${slips.join("")}</div></div>` : "";
}

/** Wallet のボタンのホバー：そのお金・カードが何のデモンストレーションのために置いてあるか。
 *  使い方（どこに落とすか）は Wallet の見出しのホバーにまとめてあるので、ここには書かない */
const WALLET_PURPOSE: Record<string, string> = {
  "cash:10000": "高額紙幣、少額の買い物で大きなお釣りが出る場面を確かめる",
  "cash:5000": "五千円札、お釣りに紙幣が混じる場面を確かめる",
  "cash:2000": "取り扱わない二千円札、入金ボックスに入れてもすぐ出金ボックスに吐き出されることを確かめる",
  "cash:1000": "千円札、電子マネーのチャージは紙幣だけを受け付けることも確かめる",
  "cash:500": "硬貨、少しずつ入れて不足額が減っていく案内を確かめる",
  "cash:100": "硬貨、少しずつ入れて不足額が減っていく案内を確かめる",
  "cash:50": "硬貨、ちょうどの金額で払いお釣りが出ない場面を確かめる",
  "cash:10": "硬貨、ちょうどの金額で払いお釣りが出ない場面を確かめる",
  "cash:5": "硬貨、レジ袋など1円単位の端数をちょうど払う場面を確かめる",
  "cash:1": "硬貨、レジ袋など1円単位の端数をちょうど払う場面を確かめる",
  PC12345678: "ポイントカード（はじめの残高 1,250P）、付与対象の商品 200円につき1P付き、ポイント利用で対象の商品の分を払い切れることを確かめる",
  PC87654321: "スマートフォンのポイントアプリ（はじめの残高 1P）、使えるのが 1P だけのときは「全部使う」だけになり、残りを別の支払方法で払うことを確かめる",
  CP0001: "ソフトドリンク1個無料の引換券、対象商品がないときの扱いも確かめる",
  CP0002: "サンドイッチ50円引きの割引券、明細に値引きが付くことを確かめる",
  "pay:qr": "バーコード決済、選んだ支払方法と違うものは受け付けないことを確かめる",
  "pay:credit": "クレジットカード、選んだ支払方法と違うものは受け付けないことを確かめる",
  "pay:emoney": "電子マネー、残高不足のときの案内や現金との併用、チャージ・残高照会を確かめる",
};

function walletItem(key: string, name: string, sub: string, cls: string): string {
  const hint = WALLET_PURPOSE[key] ?? "";
  return `<button type="button" class="item ${cls}" data-wallet="${esc(key)}" data-hint="${esc(hint)}" aria-label="${esc(name)}、${esc(hint)}">
    <span class="item-name">${esc(name)}</span>${sub ? `<span class="item-sub">${sub}</span>` : ""}</button>`;
}

export function walletPanel(emoneyBalance: number): string {
  const bills = [10000, 5000, 2000, 1000].map((d) => walletItem(`cash:${d}`, `${d.toLocaleString()}円`, "", "bill"));
  const coins = [500, 100, 50, 10, 5, 1].map((d) => walletItem(`cash:${d}`, `${d}円`, "", "coin"));
  const cards = [
    item("PC12345678", "ポイントカード", "", "card", WALLET_PURPOSE.PC12345678),
    item("PC87654321", "ポイントアプリ", "", "card", WALLET_PURPOSE.PC87654321),
    item("CP0001", "引換券", "ソフトドリンク", "coupon", WALLET_PURPOSE.CP0001),
    item("CP0002", "割引券", "サンドイッチ 50円引き", "coupon", WALLET_PURPOSE.CP0002),
    walletItem("pay:qr", "バーコード決済", "", "pay"),
    walletItem("pay:credit", "クレジットカード", "", "pay"),
    walletItem("pay:emoney", "電子マネー", `残高 ${yen(emoneyBalance)}`, "pay"),
  ];
  return `<div class="shelf"><div class="items">${[...bills, ...coins].join("")}</div><div class="items wallet-cards">${cards.join("")}</div></div>`;
}

export function sideOf(cmd: Command): "staff" | "customer" | "store" {
  if ("side" in cmd && cmd.side) return cmd.side;
  if (["ff_button", "insert_cash", "tap_emoney", "call_staff"].includes(cmd.type)) return "customer";
  if ("register" in cmd) return "staff";
  return "store";
}
