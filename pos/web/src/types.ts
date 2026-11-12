// 生成物：手で編集しない（pos/core の型から `npm run gen:types` で作り直す）

export type AccountView = { id: string, barcode: string, name: string, role: Role, };

export type Action = "scan" | "register_ff" | "ff_button" | "check_ff" | "complete_service" | "parcel_next" | "remove_line" | "remove_coupon" | "checkout" | "back_to_registering" | "select_payment" | "confirm_point_card" | "choose_age_check" | "early_age_check" | "check_id" | "check_bill" | "affix_stamp" | "affix_receipt_stamp" | "hand_over_bills" | "receive_tickets" | "hand_tickets" | "insert_cash" | "tap_emoney" | "call_staff" | "void_transaction" | "end_staff_mode" | "pause" | "cash_out" | "cash_in" | "emoney" | "resolve_emoney_short";

export type ActionState = { action: Action, allowed: boolean, reason: string | null, };

export type AgeRule = { id: number, target: RuleTarget, label: string, min_age: number, legal: boolean, };

export type AgeRulesView = { rules: Array<AgeRule>, history: Array<RuleChange>, };

export type AuditEntry = { at: number, register: number, staff: string, operation: string, amount: number, result: string, };

export type BellKind = "fixed" | "handheld" | "store_computer";

export type BellLink = { id: string, link_ok: boolean, acked_seq: number, sent: number, retries: number, };

export type BellSignal = { seq: number, held: number, presses: number, };

export type BellView = { id: string, kind: BellKind, buzzing: boolean, link_ok: boolean, rings: number, offline: boolean, bell_on: boolean, };

export type BillRow = { line_id: number, code: string, name: string, amount: number, slip_no: string, checked: boolean, 
/**
 * 領収証に貼る収入印紙の額（要らない払込票は 0）
 */
stamp: number, 
/**
 * 収入印紙の貼り付けを確認した
 */
stamped: boolean, };

export type Call = { id: number, register: number, kind: CallKind, label: string, created_at: number, detail: string, };

export type CallKind = "age_check" | "fast_food" | "service" | "tobacco" | "voucher" | "customer_help" | "change_left" | "bill" | "emoney" | "cash_only" | "receipt_stamp" | "expired";

export type CallTopic = "service" | "voucher" | "tobacco" | "emoney" | "sos";

export type Cash = Array<[number, number]>;

export type CashInKind = "deposit" | "changer_refill";

export type CashOutKind = "refund" | "expense" | "emergency" | "pickup" | "changer_collect";

export type CatalogItem = { code: string, name: string, price: number, category: Category, category_label: string, age: number | null, legal_age: number | null, store_age: number | null, limit: number | null, tax_class: string, 
/**
 * チケットの申込券で発券する枚数（チケットでなければ 0）
 */
tickets: number, 
/**
 * 日配食品
 */
fresh: boolean, 
/**
 * 販売期限・賞味期限（いまからの分）
 */
dates: Dates | null, };

export type Category = "food" | "drink" | "daily" | "newspaper" | "magazine" | "alcohol" | "tobacco" | "smoking_goods" | "adult_book" | "adult_goods" | "lottery" | "fireworks" | "fast_food" | "bag" | "service" | "stamp";

export type Command = { "type": "scan", register: number, side: Side, code: string, } | { "type": "register_ff", register: number, side?: Side, code: string, qty: number, force?: boolean, } | { "type": "ff_button", register: number, } | { "type": "check_ff", register: number, side: Side, line_id: number, handed: number, } | { "type": "complete_service", register: number, line_id: number, amount: number, } | { "type": "parcel_length", register: number, line_id: number, sum_cm: number, } | { "type": "parcel_size", register: number, line_id: number, sum_cm: number, weight_g: number, } | { "type": "parcel_postal", register: number, line_id: number, postal: string, } | { "type": "parcel_back", register: number, line_id: number, } | { "type": "parcel_next", register: number, more: boolean, } | { "type": "pickup_handover", register: number, } | { "type": "pickup_found", register: number, found?: boolean | null, } | { "type": "remove_line", register: number, side: Side, line_id: number, } | { "type": "set_qty", register: number, side: Side, line_id: number, qty: number, } | { "type": "remove_coupon", register: number, side: Side, coupon_id: number, } | { "type": "dismiss_returned", register: number, } | { "type": "checkout", register: number, side: Side, } | { "type": "back_to_registering", register: number, side: Side, } | { "type": "select_payment", register: number, side: Side, method: PaymentMethod, } | { "type": "choose_age_check", register: number, side?: Side, id_check: boolean, } | { "type": "early_age_check", register: number, side?: Side, start: boolean, } | { "type": "check_id", register: number, side?: Side, birth: string, today: string, } | { "type": "dismiss_confiscated", register: number, } | { "type": "dismiss_ff_collect", register: number, } | { "type": "check_bill", register: number, side?: Side, line_id: number, checked: boolean, } | { "type": "affix_receipt_stamp", register: number, side?: Side, } | { "type": "affix_stamp", register: number, side?: Side, line_id: number, affixed: boolean, } | { "type": "hand_over_bills", register: number, side?: Side, } | { "type": "hand_tickets", register: number, side?: Side, handed: number, } | { "type": "receive_tickets", register: number, side?: Side, received: number, } | { "type": "insert_cash", register: number, denomination: number, count: number, } | { "type": "finish_insert", register: number, } | { "type": "tap_emoney", register: number, method?: PaymentMethod, } | { "type": "start_emoney", register: number, side: Side, kind: EmoneyKind, } | { "type": "close_emoney", register: number, side: Side, } | { "type": "resolve_emoney_short", register: number, side: Side, choice: ShortChoice, } | { "type": "call_staff", register: number, topic?: CallTopic, } | { "type": "confirm_point_card", register: number, side: Side, } | { "type": "use_points", register: number, side: Side, how: PointsUse, 
/**
 * 使うポイント数（how が amount のときだけ）
 */
points?: number, } | { "type": "release_call", register: number, } | { "type": "void_transaction", register: number, } | { "type": "end_staff_mode", register: number, } | { "type": "pause", register: number, } | { "type": "cash_out", register: number, kind: CashOutKind, amount: number, password?: string, } | { "type": "cash_in", register: number, kind: CashInKind, amount: number, } | { "type": "open_menu", register: number, screen?: MenuScreen | null, exchange_to?: number, } | { "type": "insert", register: number, amount: number, } | { "type": "take_outlet", register: number, } | { "type": "claim", register: number, side: Side, } | { "type": "add_age_rule", owner_id: string, password: string, target: RuleTarget, min_age: number, } | { "type": "remove_age_rule", owner_id: string, password: string, rule_id: number, } | { "type": "set_product_age", owner_id: string, password: string, code: string, min_age?: number | null, } | { "type": "set_sale_limit", owner_id: string, password: string, code: string, limit?: number | null, } | { "type": "verify_owner", owner_id: string, password: string, } | { "type": "set_tax_rate", owner_id: string, password: string, class_id: string, rate_bp: number, } | { "type": "add_tax_class", owner_id: string, password: string, name: string, kind: TaxKind, rate_bp: number, } | { "type": "set_product_tax", owner_id: string, password: string, code: string, class_id: string, } | { "type": "set_invoice", owner_id: string, password: string, invoice_no: string, rounding: Rounding, } | { "type": "set_bell_loss", percent: number, } | { "type": "set_bell_offline", bell: number, offline: boolean, } | { "type": "set_bell_on", bell: number, on: boolean, };

export type Confiscated = { name: string, qty: number, };

export type CouponView = { id: number, code: string, name: string, discount: number | null, };

export type CustomerLine = { id: number, touched: number, name: string, qty: number, ff_base: number, amount: number, age: number | null, age_checked: boolean, limit: number | null, cash_only: boolean, pickup: boolean, parcel: boolean, fresh: boolean, expired: boolean, dates: Dates | null, tickets: number, } & ({ "kind": "item" } | { "kind": "ff", state: FfState, } | { "kind": "service", done: boolean, confirm_pending: boolean, });

export type CustomerView = { phase: Phase, lines: Array<CustomerLine>, coupons: Array<CouponView>, coupon_unmatched: boolean, returned: Array<ReturnedCoupon>, total: number, taxes: Array<TaxTotal>, inserted: number, settle_in_ms: number | null, due: number, point_card: boolean, point_pending: boolean, parcel_ask: boolean, bills: Array<BillRow>, bill_unchecked: boolean, stamp_pending: boolean, staff_called: boolean, age_wait: number | null, staff_present: boolean, method: PaymentMethod | null, payment_blocker: string | null, actions: Array<ActionState>, };

/**
 * 販売期限と賞味期限。シミュレーターをいつ開いても同じ状態になるよう、いまからの分で持つ
 * （負の値は期限切れ）。販売期限は賞味期限より前に店が決める売り場から下げる時刻
 */
export type Dates = { sell_by_min: number, best_before_min: number, };

export type EmoneyKind = "menu" | "charge" | "inquiry";

export type EmoneyView = { kind: EmoneyKind, inserted: number, balance: number | null, };

export type FfMenuItem = { code: string, name: string, price: number, level: StockLevel, tab: FfTab, tab_label: string, };

export type FfState = "waiting" | "received";

export type FfTab = "fried" | "drink" | "cold" | "bun" | "oden";

export type Menu = { screen: MenuScreen, exchange_to: number, };

export type MenuScreen = "top" | "refund" | "emergency" | "deposit" | "expense" | "changer" | "journal" | "waste" | "ff_fresh";

export type Mode = "staff_operated" | "self_operated";

export type Parcel = { sum_cm: number, weight_g: number, size: number | null, postal: string | null, region: string | null, slip: string, collect: boolean, fee: number, };

export type PaymentMethod = "cash" | "emoney" | "qr" | "credit" | "points";

export type Phase = "registering" | "paying";

export type Pickup = { no: string, amount: number, matched: boolean, wrong: string | null, missing: boolean, };

/**
 * ポイントの使い方：使える分を全部・お支払いの端数（100円未満）・ポイント数を指定
 */
export type PointsUse = "all" | "fraction" | "amount";

export type PointsView = { 
/**
 * 付与対象の商品がある（ないときはポイントの確認も表示もしない）
 */
eligible: boolean, 
/**
 * 読み取ったカードの残高（ポイント）
 */
balance: number | null, 
/**
 * いま支払いに充てられるポイント（残高と、ポイントを使える商品の金額の小さいほう）
 */
usable: number, 
/**
 * このお会計で付与する見込みのポイント
 */
earn: number, 
/**
 * 支払いに使ったポイント
 */
used: number, 
/**
 * お支払い額の端数（100円未満）。ご利用可能なポイントを超えていれば端数では払えない
 */
fraction: number, };

export type Receipt = { no: number, at: number, total: number, method: string, paid: number, change: Cash, change_total: number, items: number, point_card: string | null, 
/**
 * 付与したポイント（付与対象の金額 200円につき1ポイント）
 */
points: number, 
/**
 * 支払いに使ったポイント
 */
points_used: number, 
/**
 * 付与・利用のあとのポイント残高（カードを読んでいなければなし）
 */
point_balance: number | null, };

/**
 * 通常のお買い物でレシート（金銭の受取書）に収入印紙が要るとき。
 * 税抜の受取金額が 5 万円以上で、クレジットカード払いでないとき
 */
export type ReceiptStamp = { stamp: number, 
/**
 * 印紙税の判定に使う税抜の金額
 */
base: number, };

export type RegisterView = { id: number, 
/**
 * ON の Bell がどれも通じず、呼び出しが誰にも届かない
 */
bell_dead: boolean, 
/**
 * エマージェンシーモード（セルフ操作モードで呼び鈴に応答がない）
 */
emergency: boolean, active_side: Side | null, max_items: number, lamp: boolean, call_waiting_ms: number | null, ring_volume: number, tobacco_asked: boolean, mode: Mode, mode_label: string, outlet: number, outlet_detail: Array<[number, number]>, menu: Menu | null, paused: boolean, idle_revert_in: number | null, idle_voids: boolean, change_left: boolean, staff: StaffView, customer: CustomerView, emoney: EmoneyView | null, emoney_short: number | null, emoney_partial: number | null, emoney_partial_ok: boolean, prepaid: number, prepaid_method: string | null, 
/**
 * ポイント：付与対象の商品があるか・カードの残高・いま充てられる分・このお会計で付与する分
 */
points: PointsView, cash_only: number, cashless_due: number, unavailable: Array<Unavailable>, last_receipt: Receipt | null, bill_handover: number | null, ticket_handover: Array<TicketRow>, 
/**
 * チケットを発券した（列の先頭に来た）。false のあいだは発券待ち
 */
tickets_issued: boolean, 
/**
 * お客様が入力した受け取り枚数
 */
tickets_received: number | null, 
/**
 * スタッフが入力したお渡し枚数
 */
tickets_handed: number | null, 
/**
 * 発券の列で、このレジより先に並んでいるレジの数
 */
tickets_ahead: number, pickup_handover: string | null, receipt_stamp: ReceiptStamp | null, };

export type ResolvedCall = { id: number, register: number, label: string, created_at: number, resolved_at: number, };

export type ReturnedCoupon = { name: string, unmatched: boolean, };

export type Role = "crew" | "owner";

export type Rounding = "floor" | "half_up" | "ceil";

export type RuleChange = { at: number, by: string, change: string, };

export type RuleTarget = { "kind": "category", category: Category, } | { "kind": "product", code: string, };

export type ShortChoice = "cancel" | "cash" | "cash_confirmed" | "charge" | "staff";

export type Side = "staff" | "customer";

export type Snapshot = { now: number, registers: Array<RegisterView>, bells: Array<BellView>, bell_links: Array<BellLink>, signal: BellSignal, calls: Array<Call>, call_history: Array<ResolvedCall>, age_rules: AgeRulesView, audit: Array<AuditEntry>, ff_stock: Array<FfMenuItem>, bell_loss: number, accounts: Array<AccountView>, catalog: Array<CatalogItem>, tax: TaxView, categories: Array<[Category, string]>, emoney_balance: number, };

export type StaffLine = { amount: number, age: number | null, age_checked: boolean, limit: number | null, 
/**
 * 日配食品（数量を増やせない）
 */
fresh: boolean, 
/**
 * 販売期限切れ（回収（取り消す）までお会計に進めない）
 */
expired: boolean, 
/**
 * 販売期限・賞味期限（いまからの分）
 */
dates: Dates | null, 
/**
 * チケットの発券枚数（チケットでなければ 0）
 */
tickets: number, id: number, touched: number, code: string, name: string, unit_price: number, qty: number, ff_base: number, tax: string, rate_bp: number, category: Category, cash_only: boolean, parcel?: Parcel, pickup?: Pickup, } & ({ "kind": "item" } | { "kind": "ff", state: FfState, } | { "kind": "service", done: boolean, confirm_pending: boolean, });

export type StaffState = "absent" | "ready";

export type StaffView = { state: StaffState, name: string | null, role: Role | null, phase: Phase, lines: Array<StaffLine>, total: number, taxes: Array<TaxTotal>, coupons: Array<CouponView>, point_card: string | null, required_age: number | null, age_choice_pending: boolean, age_early: boolean, confiscated: Array<Confiscated>, ff_collect: Array<Confiscated>, coupon_unmatched: boolean, returned: Array<ReturnedCoupon>, id_pending: boolean, bills: Array<BillRow>, bill_unchecked: boolean, stamp_pending: boolean, point_pending: boolean, parcel_ask: boolean, method: PaymentMethod | null, inserted: number, settle_in_ms: number | null, due: number, payment_blocker: string | null, calls: Array<Call>, actions: Array<ActionState>, drawer_total: number, drawer: Array<[number, number]>, bell_down: Array<string>, };

export type StockLevel = "made_to_order" | "plenty" | "low" | "out";

export type TaxClassView = { id: string, name: string, kind: TaxKind, kind_label: string, rate_bp: number, rate: string, };

export type TaxKind = "standard" | "reduced" | "zero" | "exempt";

export type TaxTotal = { rate_bp: number, rate: string, base: number, tax: number, };

export type TaxView = { classes: Array<TaxClassView>, invoice_no: string, rounding: Rounding, };

/**
 * 決済後に発券するチケット。申込券の明細ごとの発券枚数（＝申込数）
 */
export type TicketRow = { line_id: number, code: string, name: string, issued: number, };

export type Unavailable = { method: PaymentMethod, reason: string, };
