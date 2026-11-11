use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    bell_name, AgeMethod, AuditEntry, EmoneyTask, Line, LineKind, Menu, Receipt, ReceiptStamp,
    Register, StaffSession, Store, TicketRow, Txn,
};
use crate::age_rules::{AgeRule, AgeRules, RuleChange};
use crate::auth::{self, Authenticator, Guard};
use crate::authority::{Mode, PaymentMethod, Side};
use crate::bell::{Bell, BellSignal, Packet};
use crate::calls::{Call, CallKind, Calls, ResolvedCall};
use crate::cash::{Cash, Drawer};
use crate::catalog::{self, COUPONS};
use crate::parcel::{Parcel, Pickup};
use crate::rng::Rng;
use crate::tax::{Rounding, TaxClass, TaxSettings};

pub const SCHEMA: u32 = 2;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn product(code: &str) -> Result<&'static catalog::Product, String> {
    catalog::find(code).ok_or_else(|| format!("商品 {code} がありません"))
}

#[derive(Serialize, Deserialize)]
struct Saved {
    schema: u32,
    now: u64,
    rng: u64,
    registers: Vec<SavedRegister>,
    bells: Vec<SavedBell>,
    bell_loss: u8,
    signal: BellSignal,
    seen_raised: u64,
    presses: u64,
    calls: SavedCalls,
    age_rules: SavedAgeRules,
    tax: SavedTax,
    auth: [Guard; 2],
    ff_stock: Vec<(String, u32)>,
    audit: Vec<SavedAudit>,
    receipts: u32,
    emoney_balance: u32,
    /// チケットの発券の列（先頭のレジだけが発券済み）
    #[serde(default)]
    ticket_queue: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct SavedBell {
    received: BellSignal,
    acked_seq: u64,
    last_rx_at: u64,
    last_ack_at: u64,
    last_sent_at: Option<u64>,
    last_sent_seq: u64,
    in_flight: Vec<Packet>,
    sent: u64,
    retries: u64,
    offline: bool,
    buzz_until: u64,
    rings: u64,
    bell_on: bool,
}

#[derive(Serialize, Deserialize)]
struct SavedCalls {
    next_id: u32,
    open: Vec<SavedCall>,
    history: Vec<SavedResolved>,
    raised: u64,
}

#[derive(Serialize, Deserialize)]
struct SavedCall {
    id: u32,
    register: u8,
    kind: CallKind,
    created_at: u64,
    detail: String,
}

#[derive(Serialize, Deserialize)]
struct SavedResolved {
    id: u32,
    register: u8,
    label: String,
    created_at: u64,
    resolved_at: u64,
}

#[derive(Serialize, Deserialize)]
struct SavedAgeRules {
    rules: Vec<AgeRule>,
    history: Vec<RuleChange>,
    limits: BTreeMap<String, u32>,
    next_id: u32,
}

#[derive(Serialize, Deserialize)]
struct SavedTax {
    classes: Vec<TaxClass>,
    products: BTreeMap<String, String>,
    invoice_no: String,
    rounding: Rounding,
    next_id: u32,
}

#[derive(Serialize, Deserialize)]
struct SavedAudit {
    at: u64,
    register: u8,
    staff: String,
    operation: String,
    amount: u32,
    result: String,
}

#[derive(Serialize, Deserialize)]
struct SavedRegister {
    id: u8,
    staff: Option<String>,
    txn: SavedTxn,
    drawer: Cash,
    last_receipt: Option<SavedReceipt>,
    bill_handover: Option<u32>,
    pickup_handover: Option<String>,
    #[serde(default)]
    receipt_stamp: Option<ReceiptStamp>,
    /// 発券したチケットのお渡し照合（明細 id・商品コード・発券数・お渡し数）
    #[serde(default)]
    ticket_handover: Vec<(u32, String, u32, Option<u32>)>,
    /// お渡しの確認が済むまで留めているチケットのお釣り
    #[serde(default)]
    held_change: Cash,
    #[serde(default)]
    tickets_received: Option<u32>,
    last_op: u64,
    outlet: Cash,
    outlet_since: Option<u64>,
    menu: Option<Menu>,
    paused: bool,
    touch: Option<(Side, u64)>,
    emoney: Option<EmoneyTask>,
    #[serde(default)]
    press_log: Vec<u64>,
    #[serde(default)]
    emergency: bool,
}

#[derive(Serialize, Deserialize)]
struct SavedReceipt {
    no: u32,
    at: u64,
    total: u32,
    method: String,
    paid: u32,
    change: Cash,
    change_total: u32,
    items: u32,
    point_card: Option<String>,
    points: u32,
}

#[derive(Serialize, Deserialize)]
struct SavedTxn {
    parcel_ask: bool,
    tobacco_asked: bool,
    lines: Vec<SavedLine>,
    next_line: u32,
    paying: bool,
    point_card: Option<String>,
    required_age: Option<u8>,
    coupons: Vec<(u32, String)>,
    returned: Vec<(String, bool)>,
    age_method: Option<AgeMethod>,
    verified_age: Option<u8>,
    #[serde(default)]
    age_lines: Vec<u32>,
    #[serde(default)]
    age_early: bool,
    confiscated: Vec<(String, u32)>,
    ff_collect: Vec<(String, u32)>,
    method: Option<PaymentMethod>,
    point_checked: bool,
    #[serde(default)]
    point_declined: bool,
    inserted: Cash,
    inserted_at: u64,
    customer_turn: bool,
    prepaid: u32,
    prepaid_method: Option<PaymentMethod>,
    emoney_short: bool,
    emoney_partial: bool,
    emoney_partial_ok: bool,
}

#[derive(Serialize, Deserialize)]
struct SavedLine {
    id: u32,
    touched: u32,
    code: String,
    unit_price: u32,
    qty: u32,
    ff_base: u32,
    tax: String,
    rate_bp: u32,
    cash_only: bool,
    kind: LineKind,
    slip_no: Option<String>,
    #[serde(default)]
    stamped: bool,
    parcel: Option<SavedParcel>,
    pickup: Option<Pickup>,
}

#[derive(Serialize, Deserialize)]
struct SavedParcel {
    sum_cm: u32,
    weight_g: u32,
    size: Option<u32>,
    postal: Option<String>,
    region: Option<String>,
    slip: String,
    collect: bool,
    fee: u32,
}

fn named(v: &[(&'static str, u32)]) -> Vec<(String, u32)> {
    v.iter().map(|&(n, q)| (n.to_string(), q)).collect()
}

fn unnamed(v: Vec<(String, u32)>) -> Vec<(&'static str, u32)> {
    v.into_iter().map(|(n, q)| (leak(n), q)).collect()
}

impl SavedLine {
    fn from(l: &Line) -> Self {
        SavedLine {
            id: l.id,
            touched: l.touched,
            code: l.code.to_string(),
            unit_price: l.unit_price,
            qty: l.qty,
            ff_base: l.ff_base,
            tax: l.tax.clone(),
            rate_bp: l.rate_bp,
            cash_only: l.cash_only,
            kind: l.kind,
            slip_no: l.slip_no.clone(),
            stamped: l.stamped,
            parcel: l.parcel.as_ref().map(|p| SavedParcel {
                sum_cm: p.sum_cm,
                weight_g: p.weight_g,
                size: p.size,
                postal: p.postal.clone(),
                region: p.region.map(str::to_string),
                slip: p.slip.clone(),
                collect: p.collect,
                fee: p.fee,
            }),
            pickup: l.pickup.clone(),
        }
    }

    fn restore(self) -> Result<Line, String> {
        let p = product(&self.code)?;
        Ok(Line {
            id: self.id,
            touched: self.touched,
            code: p.code,
            name: p.name,
            unit_price: self.unit_price,
            qty: self.qty,
            ff_base: self.ff_base,
            tax: self.tax,
            rate_bp: self.rate_bp,
            category: p.category,
            cash_only: self.cash_only,
            kind: self.kind,
            slip_no: self.slip_no,
            stamped: self.stamped,
            parcel: self.parcel.map(|p| Parcel {
                sum_cm: p.sum_cm,
                weight_g: p.weight_g,
                size: p.size,
                postal: p.postal,
                region: p.region.map(leak),
                slip: p.slip,
                collect: p.collect,
                fee: p.fee,
            }),
            pickup: self.pickup,
        })
    }
}

impl SavedTxn {
    fn from(t: &Txn) -> Self {
        SavedTxn {
            parcel_ask: t.parcel_ask,
            tobacco_asked: t.tobacco_asked,
            lines: t.lines.iter().map(SavedLine::from).collect(),
            next_line: t.next_line,
            paying: t.paying,
            point_card: t.point_card.clone(),
            required_age: t.required_age,
            coupons: t
                .coupons
                .iter()
                .map(|&(id, c)| (id, c.code.to_string()))
                .collect(),
            returned: t
                .returned
                .iter()
                .map(|&(n, u)| (n.to_string(), u))
                .collect(),
            age_method: t.age_method,
            verified_age: t.verified_age,
            age_lines: t.age_lines.clone(),
            age_early: t.age_early,
            confiscated: named(&t.confiscated),
            ff_collect: named(&t.ff_collect),
            method: t.method,
            point_checked: t.point_checked,
            point_declined: t.point_declined,
            inserted: t.inserted.clone(),
            inserted_at: t.inserted_at,
            customer_turn: t.customer_turn,
            prepaid: t.prepaid,
            prepaid_method: t.prepaid_method,
            emoney_short: t.emoney_short,
            emoney_partial: t.emoney_partial,
            emoney_partial_ok: t.emoney_partial_ok,
        }
    }

    fn restore(self) -> Result<Txn, String> {
        let coupons = self
            .coupons
            .into_iter()
            .map(|(id, code)| {
                COUPONS
                    .iter()
                    .find(|c| c.code == code)
                    .map(|c| (id, c))
                    .ok_or_else(|| format!("クーポン {code} がありません"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Txn {
            parcel_ask: self.parcel_ask,
            tobacco_asked: self.tobacco_asked,
            lines: self
                .lines
                .into_iter()
                .map(SavedLine::restore)
                .collect::<Result<_, _>>()?,
            next_line: self.next_line,
            paying: self.paying,
            point_card: self.point_card,
            required_age: self.required_age,
            coupons,
            returned: self
                .returned
                .into_iter()
                .map(|(n, u)| (leak(n), u))
                .collect(),
            age_method: self.age_method,
            verified_age: self.verified_age,
            age_lines: self.age_lines,
            age_early: self.age_early,
            confiscated: unnamed(self.confiscated),
            ff_collect: unnamed(self.ff_collect),
            method: self.method,
            point_checked: self.point_checked,
            point_declined: self.point_declined,
            inserted: self.inserted,
            inserted_at: self.inserted_at,
            customer_turn: self.customer_turn,
            prepaid: self.prepaid,
            prepaid_method: self.prepaid_method,
            emoney_short: self.emoney_short,
            emoney_partial: self.emoney_partial,
            emoney_partial_ok: self.emoney_partial_ok,
        })
    }
}

impl SavedRegister {
    fn from(r: &Register) -> Self {
        SavedRegister {
            id: r.id,
            staff: r.staff.map(|s| s.account.id.to_string()),
            txn: SavedTxn::from(&r.txn),
            drawer: r.drawer.cash.clone(),
            last_receipt: r.last_receipt.as_ref().map(|rc| SavedReceipt {
                no: rc.no,
                at: rc.at,
                total: rc.total,
                method: rc.method.to_string(),
                paid: rc.paid,
                change: rc.change.clone(),
                change_total: rc.change_total,
                items: rc.items,
                point_card: rc.point_card.clone(),
                points: rc.points,
            }),
            bill_handover: r.bill_handover,
            pickup_handover: r.pickup_handover.clone(),
            receipt_stamp: r.receipt_stamp,
            ticket_handover: r
                .ticket_handover
                .iter()
                .map(|t| (t.line_id, t.code.to_string(), t.issued, t.handed))
                .collect(),
            held_change: r.held_change.clone(),
            tickets_received: r.tickets_received,
            last_op: r.last_op,
            outlet: r.outlet.clone(),
            outlet_since: r.outlet_since,
            menu: r.menu,
            paused: r.paused,
            touch: r.touch,
            emoney: r.emoney.clone(),
            press_log: r.press_log.clone(),
            emergency: r.emergency,
        }
    }

    fn restore(self) -> Result<Register, String> {
        let staff = match self.staff {
            Some(id) => Some(StaffSession {
                account: auth::account(&id).ok_or_else(|| format!("スタッフ {id} がいません"))?,
            }),
            None => None,
        };
        let mut r = Register {
            id: self.id,
            mode: Mode::SelfOperated,
            staff,
            txn: self.txn.restore()?,
            drawer: Drawer { cash: self.drawer },
            last_receipt: self.last_receipt.map(|rc| Receipt {
                no: rc.no,
                at: rc.at,
                total: rc.total,
                method: leak(rc.method),
                paid: rc.paid,
                change: rc.change,
                change_total: rc.change_total,
                items: rc.items,
                point_card: rc.point_card,
                points: rc.points,
            }),
            bill_handover: self.bill_handover,
            pickup_handover: self.pickup_handover,
            receipt_stamp: self.receipt_stamp,
            tickets_issued: false,
            held_change: self.held_change,
            tickets_received: self.tickets_received,
            ticket_handover: self
                .ticket_handover
                .into_iter()
                .filter_map(|(line_id, code, issued, handed)| {
                    let p = catalog::find(&code)?;
                    Some(TicketRow {
                        line_id,
                        code: p.code,
                        name: p.name,
                        issued,
                        handed,
                    })
                })
                .collect(),
            last_op: self.last_op,
            outlet: self.outlet,
            outlet_since: self.outlet_since,
            menu: self.menu,
            paused: self.paused,
            touch: self.touch,
            pressing: false,
            emoney: self.emoney,
            press_log: self.press_log,
            press_since: None,
            emergency: self.emergency,
        };
        r.sync_mode();
        Ok(r)
    }
}

impl Store {
    pub fn save_json(&self) -> String {
        let saved = Saved {
            schema: SCHEMA,
            now: self.now,
            rng: self.rng.0,
            registers: self.registers.iter().map(SavedRegister::from).collect(),
            bells: self
                .bells
                .iter()
                .map(|m| SavedBell {
                    received: m.received,
                    acked_seq: m.acked_seq,
                    last_rx_at: m.last_rx_at,
                    last_ack_at: m.last_ack_at,
                    last_sent_at: m.last_sent_at,
                    last_sent_seq: m.last_sent_seq,
                    in_flight: m.in_flight.clone(),
                    sent: m.sent,
                    retries: m.retries,
                    offline: m.offline,
                    buzz_until: m.buzz_until,
                    rings: m.rings,
                    bell_on: m.bell_on,
                })
                .collect(),
            bell_loss: self.bell_loss,
            signal: self.signal,
            seen_raised: self.seen_raised,
            presses: self.presses,
            calls: SavedCalls {
                next_id: self.calls.next_id,
                open: self
                    .calls
                    .open
                    .iter()
                    .map(|c| SavedCall {
                        id: c.id,
                        register: c.register,
                        kind: c.kind,
                        created_at: c.created_at,
                        detail: c.detail.clone(),
                    })
                    .collect(),
                history: self
                    .calls
                    .history
                    .iter()
                    .map(|c| SavedResolved {
                        id: c.id,
                        register: c.register,
                        label: c.label.to_string(),
                        created_at: c.created_at,
                        resolved_at: c.resolved_at,
                    })
                    .collect(),
                raised: self.calls.raised,
            },
            age_rules: SavedAgeRules {
                rules: self.age_rules.rules.clone(),
                history: self.age_rules.history.clone(),
                limits: self.age_rules.limits.clone(),
                next_id: self.age_rules.next_id,
            },
            tax: SavedTax {
                classes: self.tax.classes.clone(),
                products: self.tax.products.clone(),
                invoice_no: self.tax.invoice_no.clone(),
                rounding: self.tax.rounding,
                next_id: self.tax.next_id,
            },
            auth: [self.auth.owner.clone(), self.auth.store.clone()],
            ff_stock: named(&self.ff_stock),
            audit: self
                .audit
                .iter()
                .map(|a| SavedAudit {
                    at: a.at,
                    register: a.register,
                    staff: a.staff.to_string(),
                    operation: a.operation.to_string(),
                    amount: a.amount,
                    result: a.result.clone(),
                })
                .collect(),
            receipts: self.receipts,
            emoney_balance: self.emoney_balance,
            ticket_queue: self.ticket_queue.iter().copied().collect(),
        };
        serde_json::to_string(&saved).expect("saved state is always serializable")
    }

    pub fn load_json(json: &str) -> Result<Store, String> {
        let s: Saved =
            serde_json::from_str(json).map_err(|e| format!("保存した状態を読めません: {e}"))?;
        if s.schema != SCHEMA {
            return Err(format!("保存した状態の形式（{}）が古いです", s.schema));
        }
        let registers = s
            .registers
            .into_iter()
            .map(SavedRegister::restore)
            .collect::<Result<Vec<_>, _>>()?;
        if registers.is_empty() || s.bells.is_empty() {
            return Err("レジか BellSignal がありません".into());
        }
        let ff_stock = s
            .ff_stock
            .into_iter()
            .map(|(code, n)| product(&code).map(|p| (p.code, n)))
            .collect::<Result<Vec<_>, _>>()?;
        let bells = s
            .bells
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let mut bell = Bell::new(&bell_name(i as u8));
                bell.received = m.received;
                bell.acked_seq = m.acked_seq;
                bell.last_rx_at = m.last_rx_at;
                bell.last_ack_at = m.last_ack_at;
                bell.last_sent_at = m.last_sent_at;
                bell.last_sent_seq = m.last_sent_seq;
                bell.in_flight = m.in_flight.clone();
                bell.sent = m.sent;
                bell.retries = m.retries;
                bell.offline = m.offline;
                bell.buzz_until = m.buzz_until;
                bell.rings = m.rings;
                bell.bell_on = m.bell_on;
                bell
            })
            .collect();
        let mut signal = s.signal;
        signal.held = 0;
        let mut store = Store {
            now: s.now,
            rng: Rng(s.rng.max(1)),
            registers,
            bells,
            calls: Calls {
                next_id: s.calls.next_id,
                open: s
                    .calls
                    .open
                    .into_iter()
                    .map(|c| Call {
                        id: c.id,
                        register: c.register,
                        kind: c.kind,
                        label: c.kind.label(),
                        created_at: c.created_at,
                        detail: c.detail,
                    })
                    .collect(),
                history: s
                    .calls
                    .history
                    .into_iter()
                    .map(|c| ResolvedCall {
                        id: c.id,
                        register: c.register,
                        label: leak(c.label),
                        created_at: c.created_at,
                        resolved_at: c.resolved_at,
                    })
                    .collect(),
                raised: s.calls.raised,
            },
            age_rules: AgeRules {
                rules: s.age_rules.rules,
                history: s.age_rules.history,
                limits: s.age_rules.limits,
                next_id: s.age_rules.next_id,
            },
            tax: TaxSettings {
                classes: s.tax.classes,
                products: s.tax.products,
                invoice_no: s.tax.invoice_no,
                rounding: s.tax.rounding,
                next_id: s.tax.next_id,
            },
            auth: Authenticator {
                owner: s.auth[0].clone(),
                store: s.auth[1].clone(),
            },
            ff_stock,
            audit: s
                .audit
                .into_iter()
                .map(|a| AuditEntry {
                    at: a.at,
                    register: a.register,
                    staff: leak(a.staff),
                    operation: leak(a.operation),
                    amount: a.amount,
                    result: a.result,
                })
                .collect(),
            bell_loss: s.bell_loss,
            signal,
            seen_raised: s.seen_raised,
            presses: s.presses,
            receipts: s.receipts,
            emoney_balance: s.emoney_balance,
            ticket_queue: s.ticket_queue.into_iter().collect(),
        };
        store.issue_next_tickets();
        Ok(store)
    }
}
