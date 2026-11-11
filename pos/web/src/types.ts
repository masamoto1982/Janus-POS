export type Side = "staff" | "customer";
export type Mode = "staff_operated" | "self_operated";
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

export interface Coupon {
  id: number;
  code: string;
  name: string;
  discount: number | null;
}

export interface Returned {
  name: string;
  unmatched: boolean;
}

export interface Parcel {
  sum_cm: number;
  weight_g: number;
  size: number | null;
  postal: string | null;
  region: string | null;
  slip: string;
  collect: boolean;
  fee: number;
}

export interface Pickup {
  no: string;
  amount: number;
  matched: boolean;
  wrong: string | null;
  missing: boolean;
}

export type StaffLine = LineKind & {
  id: number;
  touched: number;
  code: string;
  name: string;
  unit_price: number;
  qty: number;
  ff_base: number;
  category: string;
  cash_only: boolean;
  amount: number;
  age: number | null;
  age_checked: boolean;
  limit: number | null;
  parcel?: Parcel;
  pickup?: Pickup;
};

export type CustomerLine = LineKind & { id: number; touched: number; name: string; qty: number; ff_base: number; amount: number; age: number | null; age_checked: boolean; limit: number | null; cash_only: boolean; pickup: boolean; parcel: boolean };

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
  age_choice_pending: boolean;
  age_early: boolean;
  confiscated: { name: string; qty: number }[];
  ff_collect: { name: string; qty: number }[];
  coupon_unmatched: boolean;
  returned: Returned[];
  id_pending: boolean;
  bills: BillRow[];
  bill_unchecked: boolean;
  stamp_pending: boolean;
  point_pending: boolean;
  parcel_ask: boolean;
  method: Method | null;
  inserted: number;
  settle_in_ms: number | null;
  due: number;
  payment_blocker: string | null;
  calls: Call[];
  actions: ActionState[];
  drawer_total: number;
  drawer: [number, number][];
  bell_down: string[];
}

export interface TicketRow {
  line_id: number;
  code: string;
  name: string;
  issued: number;
  handed: number | null;
}

export interface BillRow {
  line_id: number;
  code: string;
  name: string;
  amount: number;
  slip_no: string;
  checked: boolean;
  /** 領収証に貼る収入印紙の額（要らない払込票は 0） */
  stamp: number;
  stamped: boolean;
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
  settle_in_ms: number | null;
  due: number;
  point_card: boolean;
  point_pending: boolean;
  parcel_ask: boolean;
  bills: BillRow[];
  bill_unchecked: boolean;
  stamp_pending: boolean;
  staff_called: boolean;
  age_wait: number | null;
  staff_present: boolean;
  method: Method | null;
  payment_blocker: string | null;
  actions: ActionState[];
}

export interface RegisterView {
  id: number;
  active_side: Side | null;
  max_items: number;
  lamp: boolean;
  call_waiting_ms: number | null;
  ring_volume: number;
  tobacco_asked: boolean;
  mode: Mode;
  mode_label: string;
  idle_revert_in: number | null;
  idle_voids: boolean;
  change_left: boolean;
  bell_dead: boolean;
  emergency: boolean;
  outlet: number;
  outlet_detail: [number, number][];
  menu: { screen: MenuScreen; exchange_to: number } | null;
  paused: boolean;
  staff: StaffView;
  customer: CustomerView;
  emoney: { kind: "menu" | "charge" | "inquiry"; inserted: number; balance: number | null } | null;
  emoney_short: number | null;
  emoney_partial: number | null;
  emoney_partial_ok: boolean;
  prepaid: number;
  prepaid_method: string | null;
  cash_only: number;
  cashless_due: number;
  unavailable: Unavailable[];
  last_receipt: Receipt | null;
  bill_handover: number | null;
  /** 決済後に発券したチケット。申込券ごとに発券数とお渡し数を照合する（全部合うと空） */
  ticket_handover: TicketRow[];
  /** 発券の列の先頭に来て発券した（false のあいだは発券待ち） */
  tickets_issued: boolean;
  /** 発券の列でこのレジより先に並んでいるレジの数 */
  tickets_ahead: number;
  /** 決済後、レシートに収入印紙を貼る確認待ち（stamp＝印紙の額、base＝税抜の受取金額） */
  receipt_stamp: { stamp: number; base: number } | null;
  pickup_handover: string | null;
}

export type MenuScreen = "top" | "refund" | "emergency" | "deposit" | "expense" | "changer" | "journal" | "waste" | "ff_fresh";

export interface AgeRule {
  id: number;
  target: { kind: "category"; category: string } | { kind: "product"; code: string };
  label: string;
  min_age: number;
  legal: boolean;
}

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
  bells: { id: string; kind: "fixed" | "handheld" | "store_computer"; buzzing: boolean; link_ok: boolean; rings: number; offline: boolean; bell_on: boolean }[];
  bell_links: { id: string; link_ok: boolean; acked_seq: number; sent: number; retries: number }[];
  signal: { seq: number; held: number; presses: number };
  calls: Call[];
  call_history: { id: number; register: number; label: string; created_at: number; resolved_at: number }[];
  age_rules: { rules: AgeRule[]; history: { at: number; by: string; change: string }[] };
  audit: { at: number; register: number; staff: string; operation: string; amount: number; result: string }[];
  ff_stock: { code: string; name: string; price: number; level: "made_to_order" | "plenty" | "low" | "out"; tab: string; tab_label: string }[];
  bell_loss: number;
  accounts: { id: string; barcode: string; name: string; role: string }[];
  catalog: { code: string; name: string; price: number; category: string; category_label: string; age: number | null; legal_age: number | null; store_age: number | null; limit: number | null; tax_class: string; tickets: number }[];
  tax: TaxView;
  categories: [string, string][];
  emoney_balance: number;
}

export type Command = { type: string; [k: string]: unknown };

export interface Unavailable {
  method: string;
  reason: string;
}
