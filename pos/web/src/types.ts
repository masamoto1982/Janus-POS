// コアの Snapshot（core/src/store.rs）を TypeScript から読むための型。

export type Side = "staff" | "customer";
export type Mode = "attended" | "semi_self";
export type Phase = "registering" | "paying";
export type Method = "cash" | "emoney" | "qr" | "credit" | "points";
export type StaffState = "absent" | "ready";
export type FfState = "waiting" | "received";

export interface ActionState {
  action: string;
  allowed: boolean;
  reason: string | null;
}

export type LineKind =
  | { kind: "item" }
  | { kind: "ff"; state: FfState }
  | { kind: "service"; done: boolean; confirm_pending: boolean };

/** 読み取ったクーポン。discount が null なら対象商品の登録待ち */
export interface Coupon {
  /** 取消に使う番号 */
  id: number;
  code: string;
  name: string;
  discount: number | null;
}

/** 取り消して返すクーポン（unmatched は適用先がなかったもの） */
export interface Returned {
  name: string;
  unmatched: boolean;
}

export type StaffLine = LineKind & {
  id: number;
  /** 最後に動きのあった順番（クーポンの id と同じ通し番号） */
  touched: number;
  code: string;
  name: string;
  unit_price: number;
  qty: number;
  category: string;
  cash_only: boolean;
  amount: number;
  age: number | null;
  limit: number | null;
};

export type CustomerLine = LineKind & { id: number; touched: number; name: string; qty: number; amount: number; age: number | null; limit: number | null; cash_only: boolean };

export interface Call {
  id: number;
  register: number;
  kind: string;
  label: string;
  created_at: number;
  detail: string;
}

export interface Receipt {
  no: number;
  total: number;
  method: string;
  paid: number;
  change: [number, number][];
  change_total: number;
  items: number;
  point_card: string | null;
  points: number;
}

export interface StaffView {
  state: StaffState;
  name: string | null;
  role: "crew" | "owner" | null;
  phase: Phase;
  lines: StaffLine[];
  total: number;
  taxes: TaxTotal[];
  coupons: Coupon[];
  point_card: string | null;
  required_age: number | null;
  /** 店員の「年齢確認をする／しない」待ち */
  age_choice_pending: boolean;
  /** 年確で販売できず取り除いた商品（店員が回収する） */
  confiscated: { name: string; qty: number }[];
  /** お渡し済みの FF を減らした・取り消した分（店員がお客様から回収する） */
  ff_collect: { name: string; qty: number }[];
  /** 会計に進んだが、適用先のないクーポンが残っている（決済できない） */
  coupon_unmatched: boolean;
  /** 取り消して、お客様に返却するクーポン */
  returned: Returned[];
  /** 「する」を選び、店員の生年月日の入力待ち */
  id_pending: boolean;
  /** 公共料金の払込票（1 枚ずつ。なければ空） */
  bills: BillRow[];
  /** 店員がまだ内容を確認していない払込票がある */
  bill_unchecked: boolean;
  point_pending: boolean;
  method: Method | null;
  inserted: number;
  due: number;
  payment_blocker: string | null;
  calls: Call[];
  actions: ActionState[];
  drawer_total: number;
  drawer: [number, number][];
  mat_down: string[];
}

/** 公共料金の払込票 1 枚ごとの内容（同じ事業者でもまとめない） */
export interface BillRow {
  /** 明細番号（確認のチェックの宛先） */
  line_id: number;
  code: string;
  /** 事業者（電気料金など） */
  name: string;
  amount: number;
  /** 店員が事業者・金額を確認した（チェックを入れた） */
  checked: boolean;
}

export interface CustomerView {
  phase: Phase;
  lines: CustomerLine[];
  coupons: Coupon[];
  coupon_unmatched: boolean;
  returned: Returned[];
  total: number;
  taxes: TaxTotal[];
  inserted: number;
  due: number;
  point_card: boolean;
  point_pending: boolean;
  bills: BillRow[];
  bill_unchecked: boolean;
  staff_called: boolean;
  age_wait: number | null;
  staff_present: boolean;
  method: Method | null;
  payment_blocker: string | null;
  actions: ActionState[];
}

export interface RegisterView {
  id: number;
  /** 今、画面を操作できる側（もう一方は表示だけ） */
  /** 操作できる側が片方に限られているなら、その側（null なら両方の画面が操作できる） */
  active_side: Side | null;
  /** 買い物点数の上限 */
  max_items: number;
  /** 有人モードで、グレーの画面にタッチして操作を引き継げる */
  /** レジ本体の呼び出しランプ（店員が来るまで点灯） */
  lamp: boolean;
  /** 店員が来ていない呼び出しの放置時間（ms） */
  call_waiting_ms: number | null;
  /** 鳴動の音量 0〜1（放置が長いほど大きい） */
  ring_volume: number;
  tobacco_asked: boolean;
  mode: Mode;
  mode_label: string;
  idle_revert_in: number | null;
  idle_voids: boolean;
  change_left: boolean;
  outlet: number;
  outlet_detail: [number, number][];
  menu: { side: Side; screen: MenuScreen; exchange_to: number } | null;
  /** お客様側から店舗メニューを開くための使い捨てパスワード（店員画面にだけ出す） */
  menu_code: string | null;
  /** レジ休止中（スタッフバーコードで再開） */
  paused: boolean;
  staff: StaffView;
  customer: CustomerView;
  /** 開いている電子マネーの手続き（チャージ・残高確認） */
  emoney: { kind: "menu" | "charge" | "inquiry"; inserted: number; balance: number | null } | null;
  /** 電子マネーの残高が足りないとき、その残高 */
  emoney_short: number | null;
  emoney_partial: number | null;
  emoney_partial_ok: boolean;
  /** このお会計で電子マネーから払った額 */
  prepaid: number;
  prepaid_method: string | null;
  cash_only: number;
  cashless_due: number;
  unavailable: Unavailable[];
  last_receipt: Receipt | null;
  /** 公共料金の決済の後、払込票のお渡し待ち（枚数）。店員の「お渡し済」で消える */
  bill_handover: number | null;
}

export type MenuScreen = "top" | "refund" | "emergency" | "deposit" | "expense" | "changer" | "journal" | "waste" | "ff_fresh";

export interface AgeRule {
  id: number;
  target: { kind: "category"; category: string } | { kind: "product"; code: string };
  label: string;
  min_age: number;
  legal: boolean;
}

/** 税率ごとの対象額（税込）と内税額 */
export interface TaxTotal {
  rate_bp: number;
  rate: string;
  base: number;
  tax: number;
}

export type TaxKind = "standard" | "reduced" | "zero" | "exempt";
export type Rounding = "floor" | "half_up" | "ceil";

export interface TaxView {
  classes: { id: string; name: string; kind: TaxKind; kind_label: string; rate_bp: number; rate: string }[];
  invoice_no: string;
  rounding: Rounding;
}

export interface Snapshot {
  now: number;
  registers: RegisterView[];
  mats: { id: string; kind: "fixed" | "handheld" | "store_computer"; buzzing: boolean; link_ok: boolean; rings: number; offline: boolean; bell_on: boolean }[];
  mat_links: { id: string; link_ok: boolean; acked_seq: number; sent: number; retries: number }[];
  bell: { seq: number; held: number; presses: number };
  calls: Call[];
  call_history: { id: number; register: number; label: string; created_at: number; resolved_at: number }[];
  age_rules: { rules: AgeRule[]; history: { at: number; by: string; change: string }[] };
  audit: { at: number; register: number; staff: string; operation: string; amount: number; result: string }[];
  ff_stock: { code: string; name: string; price: number; stock: number; tab: string; tab_label: string }[];
  mat_loss: number;
  accounts: { id: string; barcode: string; name: string; role: string }[];
  catalog: { code: string; name: string; price: number; category: string; category_label: string; age: number | null; legal_age: number | null; store_age: number | null; limit: number | null; tax_class: string }[];
  tax: TaxView;
  categories: [string, string][];
  /** 財布の電子マネーの残高 */
  emoney_balance: number;
}

export type Command = { type: string; [k: string]: unknown };

export interface Unavailable {
  method: string;
  reason: string;
}
