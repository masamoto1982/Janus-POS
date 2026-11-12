use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::age_rules::{AgeRules, RuleTarget};
use crate::auth::{self, Authenticator, Role, ACCOUNTS};
use crate::authority::{
    active_side, fixed_side, permitted, Action, Context, Mode, PaymentMethod, Phase, Side,
    StaffState, ACTIONS, ITEMS_FULL, MAX_ITEMS, TOUCH_LOCK_MS,
};
use crate::bell::{Bell, BellSignal};
use crate::calls::{Call, CallKind, Calls, ResolvedCall};
use crate::cash::{is_valid_denomination, Cash, Drawer};
use crate::catalog::{
    self, Category, Coupon, FfTab, Product, Scanned, CATALOG, FF_MENU, INITIAL_FF_STOCK,
};
use crate::parcel::{self, Parcel, Pickup};
use crate::rng::Rng;
use crate::tax::{self, Rounding, TaxKind, TaxSettings};

mod persist;

pub const MAX_REGISTERS: u8 = 10;
pub const MAX_BELLS: u8 = 5;

const FF_RECEIVED_SELF: &str =
    "お渡し済みの FF の数量はスタッフが変更します\nスタッフをお呼びください";
/// 日配食品は数量を増やせない。同じ商品でも一点ずつ販売期限が違うので、
/// 一つを読んで数量を増やすと、期限切れの品がお客様に渡ってしまう（ポカヨケ）
pub const FRESH_ONE_BY_ONE: &str = "日配食品は数量を増やせません\n一点ずつスキャンしてください";
const CASH_ONLY: &str = "現金のみ取り扱いいたします";
const NO_CASH_MIX: &str =
    "クレジットカード・バーコード決済は現金と併用できません";
const TICKETS_FIRST: &str = "チケットのお渡しが済むまで、次の取引は始められません";
const TICKET_ALONE: &str =
    "チケットは一件ずつのお取り扱いです（ほかのチケットや商品と一緒に会計できません）";
const BILL_ALONE: &str = "公共料金のお支払いは、ほかの商品と一緒に登録できません";
const PARCEL_CODE: &str = "2100000000014";
const PARCEL_ALONE: &str =
    "荷物の発送は、ほかの商品と一緒に会計できません（荷物どうしは何個でも一緒に会計できます）";
const PICKUP_STAFF: &str = "荷物の照合と引き渡しは、スタッフがログインしてから行います";
const PICKUP_ALONE: &str =
    "荷物のお受け取りは一件ずつ、ほかの商品と別にお取り扱いします（お受け取りが済んでから登録してください）";
const PICKUP_NO_REMOVE: &str = "荷物のお受け取りをやめるときは取引中止にしてください";
const PARCEL_SLIP_TO_REMOVE: &str =
    "宅配便を取り消すときは、取り消す荷物の伝票のバーコードを読み取ってください";
const BILL_NO_REMOVE: &str =
    "公共料金の払込票は取り消せません\nお支払いをやめるときは取引中止にしてください";
const NO_POINTS: &str =
    "ポイントを使える商品がありません（宅配便・代金引換・公共料金・チケット・切手・たばこにはポイントを使えません）";
const EMONEY_NO_CASH: &str =
    "セルフレジでは現金を使えません\nほかの支払方法を選ぶか、スタッフをお呼びください";
const EMONEY_NO_CHARGE: &str =
    "お買い物の途中は電子マネーのチャージ・残高確認はできません\nお会計のあとにどうぞ";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CashOutKind {
    Refund,
    Expense,
    Emergency,
    Pickup,
    ChangerCollect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum MenuScreen {
    Top,
    Refund,
    Emergency,
    Deposit,
    Expense,
    Changer,
    Journal,
    Waste,
    FfFresh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Menu {
    pub screen: MenuScreen,
    pub exchange_to: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CashInKind {
    Deposit,
    ChangerRefill,
}

impl CashInKind {
    pub fn label(self) -> &'static str {
        match self {
            CashInKind::Deposit => "取引外入金",
            CashInKind::ChangerRefill => "釣銭補充",
        }
    }
}

impl CashOutKind {
    pub fn label(self) -> &'static str {
        match self {
            CashOutKind::Refund => "返品返金",
            CashOutKind::Expense => "経費支出",
            CashOutKind::Emergency => "緊急出金",
            CashOutKind::Pickup => "途中集金",
            CashOutKind::ChangerCollect => "釣銭機回収",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum EmoneyKind {
    Menu,
    Charge,
    Inquiry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ShortChoice {
    Cancel,
    Cash,
    CashConfirmed,
    Charge,
    /// セルフ操作モード：「チャージまたは現金併用」＝スタッフを呼ぶ
    Staff,
}

pub const EMONEY_MAX: u32 = 20_000;
pub const EMONEY_INITIAL: u32 = 1_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
enum EmoneyTask {
    Menu,
    Inquiry { shown: bool },
    Charge { inserted: Cash },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CallTopic {
    Service,
    Voucher,
    Tobacco,
    Emoney,
    Sos,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Scan {
        register: u8,
        side: Side,
        code: String,
    },
    RegisterFf {
        register: u8,
        #[serde(default = "staff_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
        code: String,
        qty: u32,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        force: bool,
    },
    FfButton {
        register: u8,
    },
    CheckFf {
        register: u8,
        side: Side,
        line_id: u32,
        handed: u32,
    },
    CompleteService {
        register: u8,
        line_id: u32,
        amount: u32,
    },
    ParcelLength {
        register: u8,
        line_id: u32,
        sum_cm: u32,
    },
    ParcelSize {
        register: u8,
        line_id: u32,
        sum_cm: u32,
        weight_g: u32,
    },
    ParcelPostal {
        register: u8,
        line_id: u32,
        postal: String,
    },
    ParcelBack {
        register: u8,
        line_id: u32,
    },
    ParcelNext {
        register: u8,
        more: bool,
    },
    PickupHandover {
        register: u8,
    },
    PickupFound {
        register: u8,
        #[cfg_attr(feature = "ts", ts(optional = nullable))]
        found: Option<bool>,
    },
    RemoveLine {
        register: u8,
        side: Side,
        line_id: u32,
    },
    SetQty {
        register: u8,
        side: Side,
        line_id: u32,
        qty: u32,
    },
    RemoveCoupon {
        register: u8,
        side: Side,
        coupon_id: u32,
    },
    DismissReturned {
        register: u8,
    },
    Checkout {
        register: u8,
        side: Side,
    },
    BackToRegistering {
        register: u8,
        side: Side,
    },
    SelectPayment {
        register: u8,
        side: Side,
        method: PaymentMethod,
    },
    ChooseAgeCheck {
        register: u8,
        #[serde(default = "staff_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
        id_check: bool,
    },
    EarlyAgeCheck {
        register: u8,
        #[serde(default = "staff_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
        start: bool,
    },
    CheckId {
        register: u8,
        #[serde(default = "staff_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
        birth: String,
        today: String,
    },
    DismissConfiscated {
        register: u8,
    },
    DismissFfCollect {
        register: u8,
    },
    CheckBill {
        register: u8,
        #[serde(default = "staff_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
        line_id: u32,
        checked: bool,
    },
    AffixReceiptStamp {
        register: u8,
        #[serde(default = "staff_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
    },
    AffixStamp {
        register: u8,
        #[serde(default = "staff_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
        line_id: u32,
        affixed: bool,
    },
    HandOverBills {
        register: u8,
        #[serde(default = "staff_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
    },
    /// スタッフがお渡ししたチケットの枚数を入力する（発券枚数と照合）
    HandTickets {
        register: u8,
        #[serde(default = "staff_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
        handed: u32,
    },
    /// お客様が受け取ったチケットの枚数を入力する（発券枚数と照合）。
    /// 両方が発券枚数と一致したらお渡し完了
    ReceiveTickets {
        register: u8,
        #[serde(default = "customer_side")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Side>", optional))]
        side: Side,
        received: u32,
    },
    InsertCash {
        register: u8,
        denomination: u32,
        count: u32,
    },
    FinishInsert {
        register: u8,
    },
    TapEmoney {
        register: u8,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        method: Option<PaymentMethod>,
    },
    StartEmoney {
        register: u8,
        side: Side,
        kind: EmoneyKind,
    },
    CloseEmoney {
        register: u8,
        side: Side,
    },
    ResolveEmoneyShort {
        register: u8,
        side: Side,
        choice: ShortChoice,
    },
    CallStaff {
        register: u8,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        topic: Option<CallTopic>,
    },
    ConfirmPointCard {
        register: u8,
        side: Side,
    },
    ReleaseCall {
        register: u8,
    },
    VoidTransaction {
        register: u8,
    },
    EndStaffMode {
        register: u8,
    },
    Pause {
        register: u8,
    },
    CashOut {
        register: u8,
        kind: CashOutKind,
        amount: u32,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
        password: String,
    },
    CashIn {
        register: u8,
        kind: CashInKind,
        amount: u32,
    },
    OpenMenu {
        register: u8,
        #[cfg_attr(feature = "ts", ts(optional = nullable))]
        screen: Option<MenuScreen>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        exchange_to: Option<u32>,
    },
    Insert {
        register: u8,
        amount: u32,
    },
    TakeOutlet {
        register: u8,
    },
    Claim {
        register: u8,
        side: Side,
    },
    AddAgeRule {
        owner_id: String,
        password: String,
        target: RuleTarget,
        min_age: u8,
    },
    RemoveAgeRule {
        owner_id: String,
        password: String,
        rule_id: u32,
    },
    SetProductAge {
        owner_id: String,
        password: String,
        code: String,
        #[cfg_attr(feature = "ts", ts(optional = nullable))]
        min_age: Option<u8>,
    },
    SetSaleLimit {
        owner_id: String,
        password: String,
        code: String,
        #[cfg_attr(feature = "ts", ts(optional = nullable))]
        limit: Option<u32>,
    },
    VerifyOwner {
        owner_id: String,
        password: String,
    },
    SetTaxRate {
        owner_id: String,
        password: String,
        class_id: String,
        rate_bp: u32,
    },
    AddTaxClass {
        owner_id: String,
        password: String,
        name: String,
        kind: TaxKind,
        rate_bp: u32,
    },
    SetProductTax {
        owner_id: String,
        password: String,
        code: String,
        class_id: String,
    },
    SetInvoice {
        owner_id: String,
        password: String,
        invoice_no: String,
        rounding: Rounding,
    },
    SetBellLoss {
        percent: u8,
    },
    SetBellOffline {
        bell: usize,
        offline: bool,
    },
    SetBellOn {
        bell: usize,
        on: bool,
    },
}

impl Command {
    pub fn register(&self) -> Option<u8> {
        use Command as C;
        match self {
            C::Scan { register, .. }
            | C::RegisterFf { register, .. }
            | C::FfButton { register, .. }
            | C::CheckFf { register, .. }
            | C::CompleteService { register, .. }
            | C::ParcelLength { register, .. }
            | C::ParcelSize { register, .. }
            | C::ParcelPostal { register, .. }
            | C::ParcelBack { register, .. }
            | C::ParcelNext { register, .. }
            | C::PickupHandover { register }
            | C::PickupFound { register, .. }
            | C::RemoveLine { register, .. }
            | C::SetQty { register, .. }
            | C::RemoveCoupon { register, .. }
            | C::DismissReturned { register }
            | C::Checkout { register, .. }
            | C::BackToRegistering { register, .. }
            | C::SelectPayment { register, .. }
            | C::ChooseAgeCheck { register, .. }
            | C::EarlyAgeCheck { register, .. }
            | C::CheckId { register, .. }
            | C::DismissConfiscated { register }
            | C::DismissFfCollect { register }
            | C::CheckBill { register, .. }
            | C::AffixStamp { register, .. }
            | C::AffixReceiptStamp { register, .. }
            | C::HandOverBills { register, .. }
            | C::ReceiveTickets { register, .. }
            | C::HandTickets { register, .. }
            | C::InsertCash { register, .. }
            | C::FinishInsert { register }
            | C::TapEmoney { register, .. }
            | C::StartEmoney { register, .. }
            | C::CloseEmoney { register, .. }
            | C::ResolveEmoneyShort { register, .. }
            | C::CallStaff { register, .. }
            | C::ReleaseCall { register }
            | C::ConfirmPointCard { register, .. }
            | C::VoidTransaction { register, .. }
            | C::EndStaffMode { register, .. }
            | C::Pause { register }
            | C::CashOut { register, .. }
            | C::CashIn { register, .. }
            | C::OpenMenu { register, .. }
            | C::Insert { register, .. }
            | C::TakeOutlet { register, .. }
            | C::Claim { register, .. } => Some(*register),
            _ => None,
        }
    }

    fn screen(&self) -> Option<Side> {
        use Command as C;
        match self {
            C::RegisterFf { side, .. }
            | C::CheckFf { side, .. }
            | C::RemoveLine { side, .. }
            | C::SetQty { side, .. }
            | C::RemoveCoupon { side, .. }
            | C::Checkout { side, .. }
            | C::BackToRegistering { side, .. }
            | C::SelectPayment { side, .. }
            | C::ConfirmPointCard { side, .. }
            | C::CheckBill { side, .. }
            | C::AffixStamp { side, .. }
            | C::AffixReceiptStamp { side, .. }
            | C::HandOverBills { side, .. }
            | C::ReceiveTickets { side, .. }
            | C::HandTickets { side, .. }
            | C::StartEmoney { side, .. }
            | C::CloseEmoney { side, .. }
            | C::ResolveEmoneyShort { side, .. }
            | C::Claim { side, .. } => Some(*side),
            C::CallStaff {
                topic: Some(CallTopic::Sos),
                ..
            } => None,
            C::FfButton { .. } | C::CallStaff { .. } => Some(Side::Customer),
            C::CashOut { .. } | C::CashIn { .. } => Some(Side::Staff),
            C::OpenMenu { .. } => Some(Side::Staff),
            C::CompleteService { .. }
            | C::ParcelLength { .. }
            | C::ParcelSize { .. }
            | C::ParcelPostal { .. }
            | C::ParcelBack { .. }
            | C::ParcelNext { .. }
            | C::PickupHandover { .. }
            | C::PickupFound { .. }
            | C::VoidTransaction { .. } => Some(Side::Staff),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BillRow {
    pub line_id: u32,
    pub code: &'static str,
    pub name: &'static str,
    pub amount: u32,
    pub slip_no: String,
    pub checked: bool,
    /// 領収証に貼る収入印紙の額（要らない払込票は 0）
    pub stamp: u32,
    /// 収入印紙の貼り付けを確認した
    pub stamped: bool,
}

/// 決済後に発券するチケット。申込券の明細ごとの発券枚数（＝申込数）
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TicketRow {
    pub line_id: u32,
    pub code: &'static str,
    pub name: &'static str,
    pub issued: u32,
}

/// 払込票の明細に要る収入印紙の額。税金（公金）の領収証は非課税なので 0
fn line_stamp(l: &Line) -> u32 {
    if catalog::find(l.code).is_some_and(|p| p.stamp_exempt) {
        0
    } else {
        stamp_duty(l.amount())
    }
}

/// 領収証（払込票のお客様控え）に貼る収入印紙の額。受取金額 5 万円以上で要る
pub fn stamp_duty(amount: u32) -> u32 {
    match amount {
        0..=49_999 => 0,
        50_000..=1_000_000 => 200,
        1_000_001..=2_000_000 => 400,
        2_000_001..=3_000_000 => 600,
        3_000_001..=5_000_000 => 1_000,
        _ => 2_000,
    }
}

#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub message: Option<String>,
}

fn done(msg: impl Into<String>) -> Result<Outcome, String> {
    Ok(Outcome {
        message: Some(msg.into()),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FfState {
    Waiting,
    Received,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LineKind {
    Item,
    Ff { state: FfState },
    Service { done: bool, confirm_pending: bool },
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Line {
    pub id: u32,
    pub touched: u32,
    pub code: &'static str,
    pub name: &'static str,
    pub unit_price: u32,
    pub qty: u32,
    pub ff_base: u32,
    pub tax: String,
    pub rate_bp: u32,
    pub category: Category,
    pub cash_only: bool,
    #[serde(flatten)]
    pub kind: LineKind,
    #[serde(skip)]
    pub slip_no: Option<String>,
    /// 払込票の領収証に収入印紙を貼り付けたのを確認した
    #[serde(skip)]
    pub stamped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub parcel: Option<Parcel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub pickup: Option<Pickup>,
}

fn slip_no(seed: u64) -> String {
    let d = format!("91{:014}", mix(seed) % 100_000_000_000_000);
    format!("{} {} {} {}", &d[..4], &d[4..8], &d[8..12], &d[12..])
}

fn parcel_slip(seed: u64) -> String {
    let d = format!("{:012}", mix(seed ^ 0x534C_4950) % 1_000_000_000_000);
    format!("{}-{}-{}", &d[..4], &d[4..8], &d[8..])
}

fn pickup_no(seed: u64) -> String {
    let d = format!("{:012}", mix(seed) % 1_000_000_000_000);
    format!("{}-{}-{}", &d[..4], &d[4..8], &d[8..])
}

fn mix(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

impl Line {
    pub fn amount(&self) -> u32 {
        self.unit_price * self.qty
    }

    fn is_parcel(&self) -> bool {
        self.parcel.is_some()
    }

    /// 申込券なら、支払い後に発券するチケットの枚数（チケットでなければ 0）
    fn tickets(&self) -> u32 {
        catalog::find(self.code).map_or(0, |p| p.tickets) * self.qty
    }

    fn earns_points(&self) -> bool {
        !self.is_parcel()
            && !self.is_pickup()
            && !self.is_bill()
            && self.tickets() == 0
            && !matches!(self.category, Category::Stamp | Category::Tobacco)
    }

    fn is_pickup(&self) -> bool {
        self.pickup.is_some()
    }

    /// 日配食品（数量を増やせない）
    fn is_fresh(&self) -> bool {
        catalog::find(self.code).is_some_and(|p| p.fresh)
    }

    /// 販売期限が切れている
    fn is_expired(&self) -> bool {
        catalog::find(self.code).is_some_and(Product::expired)
    }

    fn set_pickup_found(&mut self, found: Option<bool>) {
        let Some(pk) = self.pickup.as_mut() else {
            return;
        };
        pk.matched = found == Some(true);
        pk.missing = found == Some(false);
        pk.wrong = None;
        self.unit_price = pk.amount;
        self.kind = LineKind::Service {
            done: pk.matched,
            confirm_pending: false,
        };
    }

    fn is_bill(&self) -> bool {
        catalog::find(self.code).is_some_and(|p| p.utility_bill)
    }
}

#[derive(Clone, Debug, Default)]
struct Txn {
    parcel_ask: bool,
    tobacco_asked: bool,
    lines: Vec<Line>,
    next_line: u32,
    paying: bool,
    point_card: Option<String>,
    required_age: Option<u8>,
    coupons: Vec<(u32, &'static Coupon)>,
    returned: Vec<(&'static str, bool)>,
    age_method: Option<AgeMethod>,
    verified_age: Option<u8>,
    /// 確認を済ませた年確の年齢（20・18 など）。同じ年齢の年確商品があとから加わっても
    /// やり直さず、まだ確認していない年齢の年確商品が加わったときだけやり直す
    age_done: Vec<u8>,
    /// スタッフが登録の途中で年齢確認を始めた
    age_early: bool,
    confiscated: Vec<(&'static str, u32)>,
    ff_collect: Vec<(&'static str, u32)>,
    method: Option<PaymentMethod>,
    point_checked: bool,
    /// 「ポイントカードなし」を選んだ。確認は一度の取引で一度きりなので、
    /// 登録画面や決済選択に引き返しても再び尋ねない（カードはいつでも読める）
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

impl Txn {
    fn coupon_uses(&self) -> Vec<(u32, &'static Coupon, Option<(u32, u32)>)> {
        let mut used: Vec<(&str, u32)> = vec![];
        self.coupons
            .iter()
            .map(|&(id, c)| {
                let have: u32 = self
                    .lines
                    .iter()
                    .filter(|l| l.code == c.target && l.kind == LineKind::Item)
                    .map(|l| l.qty)
                    .sum();
                let n = used.iter().find(|(t, _)| *t == c.target).map_or(0, |e| e.1);
                if n >= have {
                    return (id, c, None);
                }
                match used.iter_mut().find(|(t, _)| *t == c.target) {
                    Some(e) => e.1 += 1,
                    None => used.push((c.target, 1)),
                }
                let p = catalog::find(c.target);
                let rate = self
                    .lines
                    .iter()
                    .find(|l| l.code == c.target && l.kind == LineKind::Item)
                    .map_or(0, |l| l.rate_bp);
                (id, c, p.map(|p| (c.discount(p.price), rate)))
            })
            .collect()
    }

    fn discount_of(&self, rate: Option<u32>) -> u32 {
        self.coupon_uses()
            .iter()
            .filter_map(|(_, _, u)| *u)
            .filter(|(_, t)| rate.is_none_or(|r| r == *t))
            .map(|(yen, _)| yen)
            .sum()
    }

    fn total(&self) -> u32 {
        let gross: u32 = self.lines.iter().map(Line::amount).sum();
        gross.saturating_sub(self.discount_of(None))
    }

    fn due(&self) -> u32 {
        self.total().saturating_sub(self.prepaid)
    }

    fn cash_only(&self) -> u32 {
        self.lines
            .iter()
            .filter(|l| l.cash_only)
            .map(Line::amount)
            .sum()
    }

    fn cashless_due(&self) -> u32 {
        self.due().saturating_sub(self.cash_only())
    }

    fn points_due(&self) -> u32 {
        if self.prepaid > 0 {
            return 0;
        }
        let eligible: u32 = self
            .lines
            .iter()
            .filter(|l| l.earns_points())
            .map(Line::amount)
            .sum();
        eligible.min(self.due())
    }

    fn method_refusal(&self, m: PaymentMethod) -> Option<&'static str> {
        match m {
            PaymentMethod::Cash => None,
            PaymentMethod::Points if self.points_due() == 0 => Some(NO_POINTS),
            PaymentMethod::Points => None,
            _ if self.cashless_due() == 0 => Some(CASH_ONLY),
            PaymentMethod::Qr | PaymentMethod::Credit if self.cash_only() > 0 => Some(NO_CASH_MIX),
            _ => None,
        }
    }

    fn taxes(&self, rates: &[u32], rounding: Rounding) -> Vec<TaxTotal> {
        let mut all: Vec<u32> = rates
            .iter()
            .copied()
            .chain(self.lines.iter().map(|l| l.rate_bp).filter(|r| *r > 0))
            .collect();
        all.sort_unstable();
        all.dedup();
        all.into_iter()
            .map(|rate_bp| {
                let base = self
                    .lines
                    .iter()
                    .filter(|l| l.rate_bp == rate_bp)
                    .map(Line::amount)
                    .sum::<u32>()
                    .saturating_sub(self.discount_of(Some(rate_bp)));
                TaxTotal {
                    rate_bp,
                    rate: tax::rate_label(rate_bp),
                    base,
                    tax: tax::included_tax(base, rate_bp, rounding),
                }
            })
            .collect()
    }

    fn any_ff(&self, s: FfState) -> bool {
        self.lines
            .iter()
            .any(|l| matches!(l.kind, LineKind::Ff { state } if state == s))
    }

    fn service_pending(&self) -> bool {
        self.lines
            .iter()
            .any(|l| matches!(l.kind, LineKind::Service { done: false, .. }))
    }

    /// セルフ操作モードで現金のみの商品のためにスタッフを呼ぶときの呼び出し内容
    fn cash_only_label(&self) -> String {
        let mut names: Vec<&str> = vec![];
        for l in self.lines.iter().filter(|l| l.cash_only) {
            let n = if l.tickets() > 0 {
                "チケット"
            } else {
                l.name
            };
            if !names.contains(&n) {
                names.push(n);
            }
        }
        names.join("・")
    }

    fn bill_slips(&self) -> u32 {
        self.lines.iter().filter(|l| l.is_bill()).count() as u32
    }

    fn bill_unchecked(&self) -> bool {
        self.lines.iter().any(|l| {
            l.is_bill()
                && matches!(
                    l.kind,
                    LineKind::Service {
                        confirm_pending: true,
                        ..
                    }
                )
        })
    }

    fn needs_stamp(l: &Line) -> bool {
        l.is_bill() && line_stamp(l) > 0
    }

    fn has_stamps(&self) -> bool {
        self.lines.iter().any(Self::needs_stamp)
    }

    fn stamp_pending(&self) -> bool {
        self.lines
            .iter()
            .any(|l| Self::needs_stamp(l) && !l.stamped)
    }

    fn bill_rows(&self) -> Vec<BillRow> {
        self.lines
            .iter()
            .filter(|l| l.is_bill())
            .map(|l| BillRow {
                line_id: l.id,
                code: l.code,
                name: l.name,
                amount: l.amount(),
                slip_no: l.slip_no.clone().unwrap_or_default(),
                checked: !matches!(
                    l.kind,
                    LineKind::Service {
                        confirm_pending: true,
                        ..
                    }
                ),
                stamp: line_stamp(l),
                stamped: l.stamped,
            })
            .collect()
    }

    fn age_cleared(&self) -> bool {
        match (self.required_age, self.verified_age) {
            (None, _) => true,
            (Some(need), Some(ok)) => ok >= need,
            _ => false,
        }
    }

    fn age_choice_pending(&self) -> bool {
        (self.paying || self.age_early) && !self.age_cleared() && self.age_method.is_none()
    }

    fn id_pending(&self) -> bool {
        (self.paying || self.age_early)
            && !self.age_cleared()
            && self.age_method == Some(AgeMethod::Id)
    }

    fn line_ages(&self, rules: &AgeRules) -> Vec<u8> {
        let mut ages: Vec<u8> = self
            .lines
            .iter()
            .filter_map(|l| rules.age_for(l.code, l.category))
            .collect();
        ages.sort_unstable();
        ages.dedup();
        ages
    }

    /// 確認を済ませた年齢に、いま明細にある年確の年齢を加える
    fn mark_ages_done(&mut self, rules: &AgeRules) {
        for a in self.line_ages(rules) {
            if !self.age_done.contains(&a) {
                self.age_done.push(a);
            }
        }
    }

    /// 求める年齢：まだ確認していない年齢があればそのうち最も高いもの、なければ明細の最も高いもの
    fn age_to_check(&self, rules: &AgeRules) -> Option<u8> {
        self.line_ages(rules)
            .into_iter()
            .filter(|a| !self.age_done.contains(a))
            .max()
            .or_else(|| self.max_age(rules))
    }

    fn max_age(&self, rules: &AgeRules) -> Option<u8> {
        self.lines
            .iter()
            .filter_map(|l| rules.age_for(l.code, l.category))
            .max()
    }

    fn age_unchecked(&self, rules: &AgeRules) -> bool {
        self.line_ages(rules)
            .iter()
            .any(|a| !self.age_done.contains(a))
    }

    fn line_age_checked(&self, age: Option<u8>) -> bool {
        age.is_some_and(|a| self.age_done.contains(&a))
    }

    fn coupon_unmatched(&self) -> bool {
        self.coupon_uses().iter().any(|(_, _, u)| u.is_none())
    }

    fn item_count(&self) -> u32 {
        self.lines.iter().map(|l| l.qty).sum()
    }

    fn qty_of(&self, code: &str) -> u32 {
        self.lines
            .iter()
            .filter(|l| l.code == code)
            .map(|l| l.qty)
            .sum()
    }

    fn push(&mut self, p: &'static Product, qty: u32, kind: LineKind, tax: (String, u32)) {
        self.next_line += 1;
        let touched = self.next_line;
        if let Some(l) = self.lines.iter_mut().max_by_key(|l| l.touched).filter(|l| {
            // 日配食品は一点ずつ販売期限が違うので、続けて読んでもまとめず一行ずつ（期限切れの一点を見分ける）
            l.code == p.code
                && l.kind == kind
                && !matches!(kind, LineKind::Service { .. })
                && !p.fresh
        }) {
            l.qty += qty;
            l.touched = touched;
            return;
        }
        self.lines.push(Line {
            id: touched,
            touched,
            code: p.code,
            name: p.name,
            unit_price: p.price,
            qty,
            ff_base: 0,
            tax: tax.0,
            rate_bp: tax.1,
            category: p.category,
            cash_only: p.cash_only,
            kind,
            slip_no: None,
            stamped: false,
            parcel: p.parcel_send.then(|| Parcel {
                collect: p.parcel_collect,
                ..Parcel::default()
            }),
            pickup: None,
        });
    }
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Receipt {
    pub no: u32,
    pub at: u64,
    pub total: u32,
    pub method: &'static str,
    pub paid: u32,
    pub change: Cash,
    pub change_total: u32,
    pub items: u32,
    pub point_card: Option<String>,
    pub points: u32,
}

/// 通常のお買い物でレシート（金銭の受取書）に収入印紙が要るとき。
/// 税抜の受取金額が 5 万円以上で、クレジットカード払いでないとき
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReceiptStamp {
    pub stamp: u32,
    /// 印紙税の判定に使う税抜の金額
    pub base: u32,
}

#[derive(Clone, Copy, Debug)]
struct StaffSession {
    account: &'static auth::StaffAccount,
}

#[derive(Clone, Debug)]
struct Register {
    id: u8,
    mode: Mode,
    staff: Option<StaffSession>,
    txn: Txn,
    drawer: Drawer,
    last_receipt: Option<Receipt>,
    bill_handover: Option<u32>,
    pickup_handover: Option<String>,
    receipt_stamp: Option<ReceiptStamp>,
    /// 決済が済んだチケット。発券の順番が来たら、お渡し照合を待つ（全部合うと空になる）
    ticket_handover: Vec<TicketRow>,
    /// 発券の列（Store.ticket_queue）の先頭に来て、チケットを発券した
    tickets_issued: bool,
    /// チケットのお釣り。お渡しの確認が済むまで出金ボックスに出さずに留めておく
    held_change: Cash,
    /// お客様が入力した受け取り枚数（まだなら None）
    tickets_received: Option<u32>,
    /// スタッフが入力したお渡し枚数（まだなら None）。両方が発券枚数と一致してお渡し完了
    tickets_handed: Option<u32>,
    last_op: u64,
    outlet: Cash,
    outlet_since: Option<u64>,
    menu: Option<Menu>,
    paused: bool,
    touch: Option<(Side, u64)>,
    pressing: bool,
    emoney: Option<EmoneyTask>,
    /// 呼び鈴が鳴らされた時刻（エマージェンシーモードの判定用、直近 EMERGENCY_WINDOW_MS 分だけ持つ）
    press_log: Vec<u64>,
    /// 呼び鈴を押し始めた時刻（押している間だけ）。鳴らしっぱなしの判定用
    press_since: Option<u64>,
    /// エマージェンシーモード。一度入ったらスタッフがログインするまで続く
    emergency: bool,
}

impl Register {
    /// セルフ操作モードでスタッフがログインしないまま、呼び鈴が EMERGENCY_WINDOW_MS 以上鳴らしっぱなし、
    /// または EMERGENCY_WINDOW_MS 以内に EMERGENCY_PRESSES 回以上鳴らされた。
    /// 呼び鈴が届いているかどうかは問わない（不通でも、スタッフが応答しない状況には違いない）
    fn emergency_due(&self, now: u64) -> bool {
        let held = self
            .press_since
            .is_some_and(|t0| now.saturating_sub(t0) >= EMERGENCY_WINDOW_MS);
        let rung = self
            .press_log
            .iter()
            .filter(|&&t| now.saturating_sub(t) < EMERGENCY_WINDOW_MS)
            .count()
            >= EMERGENCY_PRESSES;
        self.mode == Mode::SelfOperated && self.staff.is_none() && (held || rung)
    }

    fn sync_mode(&mut self) {
        if self.staff_state() != StaffState::Ready {
            self.menu = None;
            if self.txn.age_early {
                self.txn.age_early = false;
                self.txn.age_method = None;
            }
        }
        self.mode = if self.staff.is_some() {
            Mode::StaffOperated
        } else {
            Mode::SelfOperated
        };
    }

    fn change_timers(&self) -> (u64, u64) {
        if self.mode == Mode::StaffOperated {
            (STAFF_OP_CHANGE_REMIND_MS, STAFF_OP_CHANGE_CALL_MS)
        } else {
            (CHANGE_REMIND_MS, CHANGE_CALL_MS)
        }
    }

    fn idle(&self) -> bool {
        self.txn.lines.is_empty()
            && self.txn.coupons.is_empty()
            && !self.txn.paying
            && self.emoney.is_none()
    }

    fn resume_idle_countdown(&mut self, now: u64) {
        if let Some(since) = self.outlet_since.take() {
            self.last_op = (self.last_op + now.saturating_sub(since)).min(now);
        }
    }

    fn idle_revert_in(&self, now: u64) -> u64 {
        let counted_until = self.outlet_since.unwrap_or(now).max(self.last_op);
        IDLE_REVERT_MS.saturating_sub(counted_until - self.last_op)
    }

    fn staff_state(&self) -> StaffState {
        match self.staff {
            None => StaffState::Absent,
            Some(_) => StaffState::Ready,
        }
    }

    fn context(&self, rules: &AgeRules) -> Context {
        let t = &self.txn;
        Context {
            mode: self.mode,
            phase: if t.paying {
                Phase::Paying
            } else {
                Phase::Registering
            },
            staff: self.staff_state(),
            has_lines: !t.lines.is_empty(),
            age_choice_pending: t.age_choice_pending(),
            id_pending: t.id_pending(),
            age_unchecked: t.age_unchecked(rules),
            age_early: t.age_early,
            ff_waiting: t.any_ff(FfState::Waiting),
            has_ff: t
                .lines
                .iter()
                .any(|l| matches!(l.kind, LineKind::Ff { .. })),
            service_pending: t.service_pending(),
            parcel_ask: t.parcel_ask,
            has_bills: t.bill_slips() > 0,
            bill_unchecked: t.bill_unchecked(),
            has_stamps: t.has_stamps(),
            stamp_pending: t.stamp_pending(),
            expired: t.lines.iter().any(Line::is_expired),
            bill_handover: self.bill_handover.is_some(),
            receipt_stamp: self.receipt_stamp.is_some(),
            ticket_handover: self.tickets_issued && !self.ticket_handover.is_empty(),
            point_pending: t.paying
                && !t.point_checked
                && !(t.age_choice_pending() || t.id_pending()),
            has_coupons: !t.coupons.is_empty(),
            coupon_unmatched: t.paying && t.coupon_unmatched(),
            method: t.method,
            customer_turn: t.customer_turn,
            menu_open: self.menu.is_some(),
            touching: self.touch.map(|(side, _)| side),
            items_full: t.item_count() >= MAX_ITEMS,
            emoney_short: t.emoney_short || t.emoney_partial,
            cash_only_goods: t.cash_only() > 0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AuditEntry {
    pub at: u64,
    pub register: u8,
    pub staff: &'static str,
    pub operation: &'static str,
    pub amount: u32,
    pub result: String,
}

pub struct Store {
    now: u64,
    rng: Rng,
    registers: Vec<Register>,
    bells: Vec<Bell>,
    calls: Calls,
    age_rules: AgeRules,
    tax: TaxSettings,
    auth: Authenticator,
    ff_stock: Vec<(&'static str, u32)>,
    audit: Vec<AuditEntry>,
    bell_loss: u8,
    signal: BellSignal,
    seen_raised: u64,
    presses: u64,
    receipts: u32,
    emoney_balance: u32,
    /// チケットの発券の列（先入れ先出し）。決済が済んだ順にレジ番号が並び、先頭のレジだけが発券する。
    /// 複数のレジで同時・僅差に申し込まれても、先のチケットの発券とお渡しが済むまで次は発券しない
    ticket_queue: VecDeque<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
enum AgeMethod {
    Skip,
    Id,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CouponView {
    pub id: u32,
    pub code: &'static str,
    pub name: &'static str,
    pub discount: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReturnedCoupon {
    pub name: &'static str,
    pub unmatched: bool,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Confiscated {
    pub name: &'static str,
    pub qty: u32,
}

fn ymd(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim();
    let (y, m, d) = if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        (
            s[..4].parse().ok()?,
            s[4..6].parse().ok()?,
            s[6..].parse().ok()?,
        )
    } else {
        let mut it = s.split('-').map(|x| x.parse::<u32>().ok());
        let ymd = (it.next()??, it.next()??, it.next()??);
        if it.next().is_some() {
            return None;
        }
        ymd
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    (y >= 1900 && (1..=days).contains(&d)).then_some((y, m, d))
}

pub fn age_on(birth: &str, today: &str) -> Result<u8, String> {
    let (by, bm, bd) = ymd(birth).ok_or("生年月日を西暦 8 桁で入力してください（例：19821002）")?;
    let (ty, tm, td) = ymd(today).ok_or("日付が正しくありません")?;
    if (by, bm, bd) > (ty, tm, td) {
        return Err("生年月日が今日より後です".into());
    }
    let age = ty - by - u32::from((tm, td) < (bm, bd));
    Ok(age.min(150) as u8)
}

fn staff_side() -> Side {
    Side::Staff
}

fn customer_side() -> Side {
    Side::Customer
}

pub const THANKS_MS: u64 = 3000;
pub const INSERT_DONE_MS: u64 = 30 * 1000;

pub const IDLE_REVERT_MS: u64 = 5 * 60 * 1000;
/// エマージェンシーモード：セルフ操作モードでスタッフがログインしないまま、
/// 呼び鈴が EMERGENCY_WINDOW_MS 以上鳴らしっぱなし、または EMERGENCY_WINDOW_MS 以内に EMERGENCY_PRESSES 回以上鳴らされた
pub const EMERGENCY_WINDOW_MS: u64 = 3 * 60 * 1000;
pub const EMERGENCY_PRESSES: usize = 10;

pub const CHANGE_REMIND_MS: u64 = 60 * 1000;
pub const CHANGE_CALL_MS: u64 = 2 * 60 * 1000;
pub const STAFF_OP_CHANGE_REMIND_MS: u64 = 30 * 1000;
pub const STAFF_OP_CHANGE_CALL_MS: u64 = 60 * 1000;
pub const RING_MIN: f32 = 0.25;
pub const RING_FULL_MS: u64 = 60 * 1000;

pub fn ring_volume(waiting_ms: u64) -> f32 {
    let t = waiting_ms.min(RING_FULL_MS) as f32 / RING_FULL_MS as f32;
    RING_MIN + (1.0 - RING_MIN) * t
}

impl Store {
    pub fn new(seed: u64, registers: u8, bells: u8) -> Store {
        let registers = registers.clamp(1, MAX_REGISTERS);
        let bells = bells.clamp(1, MAX_BELLS);
        let regs = (1..=registers)
            .map(|id| Register {
                id,
                mode: Mode::SelfOperated,
                staff: None,
                txn: Txn::default(),
                drawer: Drawer::default(),
                last_receipt: None,
                bill_handover: None,
                ticket_handover: vec![],
                tickets_issued: false,
                held_change: Cash::default(),
                tickets_received: None,
                tickets_handed: None,
                pickup_handover: None,
                receipt_stamp: None,
                last_op: 0,
                outlet: Cash::default(),
                outlet_since: None,
                menu: None,
                paused: false,
                touch: None,
                pressing: false,
                emoney: None,
                press_log: vec![],
                press_since: None,
                emergency: false,
            })
            .collect();
        Store {
            now: 0,
            rng: Rng::new(seed),
            registers: regs,
            bells: (0..bells).map(|i| Bell::new(&bell_name(i))).collect(),
            calls: Calls::default(),
            age_rules: AgeRules::default(),
            tax: TaxSettings::default(),
            auth: Authenticator::default(),
            ff_stock: INITIAL_FF_STOCK.to_vec(),
            audit: vec![],
            bell_loss: 0,
            signal: BellSignal::default(),
            seen_raised: 0,
            presses: 0,
            receipts: 0,
            emoney_balance: EMONEY_INITIAL,
            ticket_queue: VecDeque::new(),
        }
    }

    pub fn now(&self) -> u64 {
        self.now
    }

    pub fn tick(&mut self, now: u64) {
        self.now = self.now.max(now);
        let now = self.now;
        for r in &mut self.registers {
            if r.touch.is_some_and(|(_, until)| until <= now) {
                r.touch = None;
            }
            if r.outlet.total() == 0
                && r.bill_handover.is_none()
                && r.ticket_handover.is_empty()
                && r.pickup_handover.is_none()
                && r.receipt_stamp.is_none()
                && r.last_receipt
                    .as_ref()
                    .is_some_and(|rc| rc.at + THANKS_MS <= now)
            {
                r.last_receipt = None;
            }
        }
        let done_ids: Vec<u8> = self
            .registers
            .iter()
            .filter(|r| {
                let t = &r.txn;
                t.paying
                    && t.method == Some(PaymentMethod::Cash)
                    && r.emoney.is_none()
                    && t.inserted.total() > 0
                    && t.inserted.total() >= t.due()
                    && t.inserted_at + INSERT_DONE_MS <= now
            })
            .map(|r| r.id)
            .collect();
        for id in done_ids {
            let _ = self.finish_insert_auto(id);
        }
        if self.calls.raised > self.seen_raised {
            self.seen_raised = self.calls.raised;
            self.presses += 1;
        }
        let mut signal = self.signal;
        signal.held = self.registers.iter().filter(|r| r.pressing).count();
        signal.presses = self.presses;
        if signal != self.signal {
            signal.seq += 1;
            self.signal = signal;
            for m in &mut self.bells {
                m.kick();
            }
        }
        for m in self.bells.iter_mut().filter(|m| m.bell_on) {
            m.tick(now, self.signal, self.bell_loss, &mut self.rng);
        }
        for r in &mut self.registers {
            if !r.emergency && r.emergency_due(now) {
                r.emergency = true;
            }
            if r.outlet.total() > 0 {
                let since = *r.outlet_since.get_or_insert(now);
                if now.saturating_sub(since) >= r.change_timers().1 {
                    self.calls.raise(
                        r.id,
                        CallKind::ChangeLeft,
                        now,
                        "排出口に釣り銭が残っています",
                    );
                }
            } else {
                r.resume_idle_countdown(now);
                self.calls.resolve(r.id, CallKind::ChangeLeft, now);
            }
        }
        let due: Vec<u8> = self
            .registers
            .iter()
            .filter(|r| {
                (r.mode != Mode::SelfOperated || !r.idle())
                    && r.outlet_since.is_none()
                    && !self.calls.open.iter().any(|c| c.register == r.id)
                    && now.saturating_sub(r.last_op) >= IDLE_REVERT_MS
            })
            .map(|r| r.id)
            .collect();
        for id in due {
            if !self.reg(id).is_some_and(|r| r.idle()) {
                self.void_txn(id, now);
            }
            if let Ok(r) = self.reg_mut(id) {
                if r.mode != Mode::SelfOperated {
                    r.staff = None;
                    r.mode = Mode::SelfOperated;
                }
                r.menu = None;
                r.bill_handover = None;
                r.pickup_handover = None;
                r.receipt_stamp = None;
                r.last_op = now;
            }
        }
    }

    fn void_txn(&mut self, register: u8, now: u64) -> u32 {
        let Some(r) = self.registers.iter_mut().find(|r| r.id == register) else {
            return 0;
        };
        r.menu = None;
        let txn = std::mem::take(&mut r.txn);
        r.outlet.merge(&txn.inserted);
        let mut back = txn.inserted.total();
        if let Some(EmoneyTask::Charge { inserted }) = r.emoney.take() {
            back += inserted.total();
            r.outlet.merge(&inserted);
        }
        r.emoney = None;
        if txn.prepaid_method == Some(PaymentMethod::Emoney) {
            self.emoney_balance += txn.prepaid;
        }
        for l in &txn.lines {
            if l.kind
                == (LineKind::Ff {
                    state: FfState::Waiting,
                })
            {
                self.restock(l.code, l.qty);
            }
        }
        for k in [
            CallKind::AgeCheck,
            CallKind::FastFood,
            CallKind::Service,
            CallKind::Tobacco,
            CallKind::Voucher,
            CallKind::Bill,
            CallKind::Emoney,
            CallKind::CashOnly,
            CallKind::Expired,
        ] {
            self.calls.resolve(register, k, now);
        }
        back
    }

    fn reg(&self, id: u8) -> Option<&Register> {
        self.registers.iter().find(|r| r.id == id)
    }

    fn reg_mut(&mut self, id: u8) -> Result<&mut Register, String> {
        self.registers
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("レジ{id}はありません"))
    }

    fn check_active(&self, register: u8, action: Action) -> Result<(), String> {
        let r = self
            .reg(register)
            .ok_or_else(|| format!("レジ{register}はありません"))?;
        let c = r.context(&self.age_rules);
        permitted(active_side(&c).unwrap_or(Side::Customer), action, &c).map_err(String::from)
    }

    fn check(&self, register: u8, side: Side, action: Action) -> Result<(), String> {
        let r = self
            .reg(register)
            .ok_or_else(|| format!("レジ{register}はありません"))?;
        permitted(side, action, &r.context(&self.age_rules)).map_err(String::from)
    }

    pub fn dispatch(&mut self, cmd: Command) -> Result<Outcome, String> {
        if let Some(id) = cmd.register() {
            let resume = match &cmd {
                Command::Scan { code, .. } => {
                    matches!(catalog::classify(code), Scanned::Staff(_))
                }
                Command::TakeOutlet { .. } => true,
                _ => false,
            };
            if !resume && self.reg(id).is_some_and(|r| r.paused) {
                return Err("このレジは休止中です（スタッフバーコードで再開）".into());
            }
        }
        if let Some(id) = cmd.register() {
            let now = self.now;
            if let Ok(r) = self.reg_mut(id) {
                r.last_op = now;
            }
        }
        let screen = cmd.screen().zip(cmd.register());
        let res = self.apply(cmd);
        if let (Ok(_), Some((side, id))) = (&res, screen) {
            let now = self.now;
            if let Ok(r) = self.reg_mut(id) {
                r.touch = Some((side, now + TOUCH_LOCK_MS));
            }
        }
        for r in &mut self.registers {
            r.sync_mode();
        }
        res
    }

    fn apply(&mut self, cmd: Command) -> Result<Outcome, String> {
        use Command as C;
        match cmd {
            C::Scan {
                register,
                side,
                code,
            } => self.scan(register, side, &code),
            C::RegisterFf {
                register,
                side,
                code,
                qty,
                force,
            } => self.register_ff(register, side, &code, qty, force),
            C::FfButton { register } => {
                self.check_active(register, Action::FfButton)?;
                self.call(register, CallKind::FastFood, "FF のご注文");
                self.press(register)?;
                done("スタッフをお呼びしました\nFF のご注文を伺います")
            }
            C::CheckFf {
                register,
                side,
                line_id,
                handed,
            } => {
                self.check(register, side, Action::CheckFf)?;
                let r = self.reg_mut(register)?;
                let l = r
                    .txn
                    .lines
                    .iter_mut()
                    .find(|l| l.id == line_id && matches!(l.kind, LineKind::Ff { .. }))
                    .ok_or("FF の明細が見つかりません")?;
                let name = l.name;
                let matched = handed == l.qty - l.ff_base;
                l.kind = LineKind::Ff {
                    state: if matched {
                        FfState::Received
                    } else {
                        FfState::Waiting
                    },
                };
                done(if matched {
                    format!("{name}：照合 OK")
                } else {
                    format!("{name}：照合 NG（お渡し数 {handed} が注文数と合いません）")
                })
            }
            C::CompleteService {
                register,
                line_id,
                amount,
            } => {
                self.check_active(register, Action::CompleteService)?;
                let r = self.reg_mut(register)?;
                let l = r
                    .txn
                    .lines
                    .iter_mut()
                    .find(|l| {
                        l.id == line_id && matches!(l.kind, LineKind::Service { done: false, .. })
                    })
                    .ok_or("受付待ちのサービス商品が見つかりません")?;
                if l.is_parcel() {
                    return Err("荷物の料金は、サイズと届け先の郵便番号で決まります".into());
                }
                if l.is_pickup() {
                    return Err("荷物のバーコードを読んで照合してから、お渡しください".into());
                }
                l.unit_price = amount;
                l.kind = LineKind::Service {
                    done: true,
                    confirm_pending: catalog::find(l.code).is_some_and(|p| p.utility_bill),
                };
                let name = l.name;
                if !self.reg_mut(register)?.txn.service_pending() {
                    self.calls.resolve(register, CallKind::Service, self.now);
                }
                done(format!("{name}を受け付けました（{amount}円）"))
            }
            C::ParcelSize {
                register,
                line_id,
                sum_cm,
                weight_g,
            } => {
                self.check_active(register, Action::CompleteService)?;
                let size = parcel::size_for(sum_cm, weight_g)?;
                let pc = self.pending_parcel(register, line_id)?;
                pc.sum_cm = sum_cm;
                pc.weight_g = weight_g;
                pc.size = Some(size);
                done(format!(
                    "{size} サイズに決まりました\n届け先の郵便番号を入力してください"
                ))
            }
            C::ParcelLength {
                register,
                line_id,
                sum_cm,
            } => {
                self.check_active(register, Action::CompleteService)?;
                parcel::check_length(sum_cm)?;
                let pc = self.pending_parcel(register, line_id)?;
                pc.sum_cm = sum_cm;
                pc.weight_g = 0;
                pc.size = None;
                done("重さを入力してください".to_string())
            }
            C::ParcelBack { register, line_id } => {
                self.check_active(register, Action::CompleteService)?;
                let pc = self.pending_parcel(register, line_id)?;
                if pc.size.is_some() {
                    pc.size = None;
                    return done("重さを入力し直してください".to_string());
                }
                if pc.sum_cm > 0 {
                    pc.sum_cm = 0;
                    pc.weight_g = 0;
                    return done("三辺の合計を入力し直してください".to_string());
                }
                let now = self.now;
                let r = self.reg_mut(register)?;
                r.txn.lines.retain(|l| l.id != line_id);
                r.txn.parcel_ask = r.txn.lines.iter().any(|l| l.is_parcel());
                if !r.txn.service_pending() {
                    self.calls.resolve(register, CallKind::Service, now);
                }
                done("荷物の受付をやめました".to_string())
            }
            C::ParcelPostal {
                register,
                line_id,
                postal,
            } => {
                self.check_active(register, Action::CompleteService)?;
                let code = parcel::normalize_postal(&postal)?;
                let region = parcel::region(&code)?;
                let pc = self.pending_parcel(register, line_id)?;
                let size = pc.size.ok_or("先に取り扱いサイズを決めてください")?;
                let fee = parcel::fee(size, region);
                pc.postal = Some(format!("{}-{}", &code[..3], &code[3..]));
                pc.region = Some(region.label());
                pc.fee = fee;
                let charge = if pc.collect { 0 } else { fee };
                let now = self.now;
                let r = self.reg_mut(register)?;
                if let Some(l) = r.txn.lines.iter_mut().find(|l| l.id == line_id) {
                    l.unit_price = charge;
                    l.kind = LineKind::Service {
                        done: true,
                        confirm_pending: false,
                    };
                }
                r.txn.parcel_ask = true;
                if !r.txn.service_pending() {
                    self.calls.resolve(register, CallKind::Service, now);
                }
                done(format!(
                    "{size} サイズ・{}あて：料金 {fee}円",
                    region.label()
                ))
            }
            C::ParcelNext { register, more } => {
                self.check_active(register, Action::ParcelNext)?;
                let r = self.reg_mut(register)?;
                r.txn.parcel_ask = false;
                if !more {
                    return done("荷物の受付が済みました".to_string());
                }
                if r.txn.item_count() >= MAX_ITEMS {
                    return Err(ITEMS_FULL.into());
                }
                let code = r
                    .txn
                    .lines
                    .iter()
                    .filter(|l| l.is_parcel())
                    .max_by_key(|l| l.touched)
                    .map_or(PARCEL_CODE, |l| l.code);
                let p = catalog::find(code).ok_or("荷物の商品が見つかりません")?;
                let tax = self.tax_for(p.code);
                let now = self.now;
                let r = self.reg_mut(register)?;
                r.txn.push(
                    p,
                    1,
                    LineKind::Service {
                        done: false,
                        confirm_pending: false,
                    },
                    tax,
                );
                let seed = now ^ (u64::from(register) << 56) ^ (u64::from(r.txn.next_line) << 40);
                if let Some(pc) = r.txn.lines.last_mut().and_then(|l| l.parcel.as_mut()) {
                    pc.slip = parcel_slip(seed);
                }
                done("次の荷物：取り扱いサイズを決定してください".to_string())
            }
            C::PickupHandover { register } => {
                let now = self.now;
                let r = self.reg_mut(register)?;
                if r.staff_state() != StaffState::Ready {
                    return Err(PICKUP_STAFF.into());
                }
                if let Some(no) = r.pickup_handover.take() {
                    return done(format!("荷物（{no}）をお渡ししました"));
                }
                let pk = r
                    .txn
                    .lines
                    .iter()
                    .find_map(|l| l.pickup.as_ref())
                    .ok_or("お受け取りの手続き中ではありません")?;
                if !pk.missing {
                    return Err(if pk.matched {
                        "代金引換のお会計を済ませてから、荷物をお渡しください"
                    } else {
                        "先に保管場所の荷物を照合してください"
                    }
                    .into());
                }
                let no = pk.no.clone();
                r.txn = Txn::default();
                r.last_receipt = None;
                self.calls.resolve(register, CallKind::Service, now);
                done(format!(
                    "荷物（{no}）が見つからないため、お受け取りを終えました"
                ))
            }
            C::PickupFound { register, found } => {
                let r = self.reg_mut(register)?;
                if r.staff_state() != StaffState::Ready {
                    return Err(PICKUP_STAFF.into());
                }
                if r.txn.paying {
                    return Err("お会計中です（登録に戻ってから選び直してください）".into());
                }
                let l = r
                    .txn
                    .lines
                    .iter_mut()
                    .find(|l| l.is_pickup())
                    .ok_or("お受け取りの手続き中ではありません")?;
                if l.pickup.as_ref().is_some_and(|pk| pk.matched) {
                    return Err(PICKUP_NO_REMOVE.into());
                }
                l.set_pickup_found(found);
                let amount = l.unit_price;
                let now = self.now;
                if found == Some(true) {
                    self.calls.resolve(register, CallKind::Service, now);
                }
                done(match found {
                    Some(true) => format!("代金引換 {amount}円をお預かりしてください"),
                    Some(false) => "お客様にお詫びし、運送会社へお問い合わせください".into(),
                    None => "保管場所から荷物を探してください".into(),
                })
            }
            C::RemoveLine {
                register,
                side,
                line_id,
            } => {
                self.check(register, side, Action::RemoveLine)?;
                let r = self.reg_mut(register)?;
                let i = r
                    .txn
                    .lines
                    .iter()
                    .position(|l| l.id == line_id)
                    .ok_or("明細が見つかりません")?;
                if r.txn.lines[i].is_bill() {
                    return Err(BILL_NO_REMOVE.into());
                }
                if r.txn.lines[i].is_parcel() {
                    return Err(PARCEL_SLIP_TO_REMOVE.into());
                }
                if r.txn.lines[i].is_pickup() {
                    return Err(PICKUP_NO_REMOVE.into());
                }
                let received = r.txn.lines[i].kind
                    == LineKind::Ff {
                        state: FfState::Received,
                    };
                if received && r.staff_state() != StaffState::Ready {
                    return Err(FF_RECEIVED_SELF.into());
                }
                let l = r.txn.lines.remove(i);
                if received {
                    r.txn.ff_collect.push((l.name, l.qty));
                }
                if l.kind
                    == (LineKind::Ff {
                        state: FfState::Waiting,
                    })
                {
                    self.restock(l.code, l.qty);
                }
                let r = self.reg_mut(register)?;
                let service_pending = r.txn.service_pending();
                let expired = r.txn.lines.iter().any(Line::is_expired);
                if !service_pending {
                    self.calls.resolve(register, CallKind::Service, self.now);
                }
                if !expired {
                    self.calls.resolve(register, CallKind::Expired, self.now);
                }
                done(format!("{}を取り消しました", l.name))
            }
            C::SetQty {
                register,
                side,
                line_id,
                qty,
            } => self.set_qty(register, side, line_id, qty),
            C::Checkout { register, side } => self.checkout(register, side),
            C::BackToRegistering { register, side } => {
                self.check(register, side, Action::BackToRegistering)?;
                let r = self.reg_mut(register)?;
                if r.txn.inserted.total() > 0 {
                    let cash = std::mem::take(&mut r.txn.inserted);
                    let back = cash.total();
                    r.outlet.merge(&cash);
                    r.txn.method = None;
                    return done(format!(
                        "お支払い方法を選び直します\n{back}円を出金ボックスから取り出してください"
                    ));
                }
                if r.txn.prepaid > 0 {
                    return Err("一部お支払い済みです\n残りをお支払いください".into());
                }
                // セルフ操作モードで「現金」（店員呼び出し）を選んだあと引き返したら、呼び出しも取り下げる
                let undo_cash_call = r.mode == Mode::SelfOperated
                    && r.txn.method == Some(PaymentMethod::Cash)
                    && r.txn.cash_only() == 0;
                r.txn.paying = false;
                r.txn.method = None;
                r.txn.customer_turn = false;
                if undo_cash_call {
                    self.calls.resolve(register, CallKind::CashOnly, self.now);
                }
                done("登録画面に戻りました")
            }
            C::SelectPayment {
                register,
                side,
                method,
            } => {
                self.check(register, side, Action::SelectPayment)?;
                let r = self.reg_mut(register)?;
                if r.txn.inserted.total() > 0 && method != PaymentMethod::Cash {
                    return Err("現金が投入済みです\n現金でお支払いください".into());
                }
                let self_op = r.mode == Mode::SelfOperated;
                // 支払方法のボタンは待ち受けの案内ボタンと同じ挙動：選択中のボタンをもう一度押すと選択を外す
                // （セルフ操作モードの「現金」は店員呼び出しなので外さない）
                if r.txn.method == Some(method)
                    && r.txn.inserted.total() == 0
                    && !(self_op && method == PaymentMethod::Cash)
                {
                    r.txn.method = None;
                    return done(format!("{}の選択を外しました", method.label()));
                }
                if let Some(why) = r.txn.method_refusal(method) {
                    return Err(why.into());
                }
                r.txn.method = Some(method);
                let no_cash_goods = r.txn.cash_only() == 0;
                if self_op {
                    if method == PaymentMethod::Cash {
                        self.calls.raise(
                            register,
                            CallKind::CashOnly,
                            self.now,
                            "現金でのお支払い",
                        );
                        return done(
                            "スタッフをお呼びしました\n現金はスタッフがお預かりいたします",
                        );
                    }
                    if no_cash_goods {
                        self.calls.resolve(register, CallKind::CashOnly, self.now);
                    }
                }
                let r = self.reg_mut(register)?;
                if method == PaymentMethod::Points && r.txn.point_card.is_none() {
                    r.txn.point_checked = false;
                    return done(
                        "ポイントカードが読まれていません\nポイントカードを読み取ってください",
                    );
                }
                done(format!("{}を選びました", method.label()))
            }
            C::ChooseAgeCheck {
                register,
                side,
                id_check,
            } => {
                self.check(register, side, Action::ChooseAgeCheck)?;
                self.calls.resolve(register, CallKind::AgeCheck, self.now);
                let rules = &self.age_rules;
                let r = self
                    .registers
                    .iter_mut()
                    .find(|r| r.id == register)
                    .ok_or("レジがありません")?;
                if id_check {
                    r.txn.age_method = Some(AgeMethod::Id);
                    done("身分証の生年月日を入力してください")
                } else {
                    r.txn.required_age = r.txn.age_to_check(rules);
                    let age = r.txn.required_age.unwrap_or(0);
                    r.txn.age_method = Some(AgeMethod::Skip);
                    r.txn.verified_age = r.txn.required_age;
                    r.txn.mark_ages_done(rules);
                    r.txn.age_early = false;
                    done(format!("{age}歳以上の年齢確認を省略しました"))
                }
            }
            C::EarlyAgeCheck {
                register,
                side,
                start,
            } => {
                self.check(register, side, Action::EarlyAgeCheck)?;
                let rules = &self.age_rules;
                let r = self
                    .registers
                    .iter_mut()
                    .find(|r| r.id == register)
                    .ok_or("レジがありません")?;
                if !start {
                    r.txn.age_early = false;
                    r.txn.age_method = None;
                    return done("登録画面に戻りました");
                }
                let age = r.txn.age_to_check(rules);
                r.txn.required_age = age;
                r.txn.verified_age = None;
                r.txn.age_method = None;
                r.txn.age_early = true;
                done(format!("{}歳以上の年齢確認をします", age.unwrap_or(0)))
            }
            C::CheckId {
                register,
                side,
                birth,
                today,
            } => {
                self.check(register, side, Action::CheckId)?;
                let age = age_on(&birth, &today)?;
                self.verify_age(register, age)
            }
            C::RemoveCoupon {
                register,
                side,
                coupon_id,
            } => {
                self.check(register, side, Action::RemoveCoupon)?;
                let r = self.reg_mut(register)?;
                let unmatched = r
                    .txn
                    .coupon_uses()
                    .iter()
                    .find(|(id, _, _)| *id == coupon_id)
                    .ok_or("クーポンが見つかりません")?
                    .2
                    .is_none();
                let i = r
                    .txn
                    .coupons
                    .iter()
                    .position(|(id, _)| *id == coupon_id)
                    .unwrap();
                let (_, c) = r.txn.coupons.remove(i);
                r.txn.returned.push((c.name, unmatched));
                done(format!(
                    "{}を取り消しました\nお客様に返却してください",
                    c.name
                ))
            }
            C::DismissReturned { register } => {
                let r = self.reg_mut(register)?;
                r.txn.returned.clear();
                done("クーポンの返却の案内を閉じました")
            }
            C::DismissConfiscated { register } => {
                let r = self.reg_mut(register)?;
                if r.staff_state() != StaffState::Ready {
                    return Err("回収の確認はスタッフが行います".into());
                }
                r.txn.confiscated.clear();
                done("販売できない商品を回収しました")
            }
            C::DismissFfCollect { register } => {
                let r = self.reg_mut(register)?;
                if r.staff_state() != StaffState::Ready {
                    return Err("FF の回収の確認はスタッフが行います".into());
                }
                r.txn.ff_collect.clear();
                done("FF を回収しました")
            }
            C::CheckBill {
                register,
                side,
                line_id,
                checked,
            } => {
                self.check(register, side, Action::CheckBill)?;
                let now = self.now;
                let r = self.reg_mut(register)?;
                let l = r
                    .txn
                    .lines
                    .iter_mut()
                    .find(|l| l.is_bill() && l.id == line_id)
                    .ok_or("払込票が見つかりません")?;
                l.kind = LineKind::Service {
                    done: true,
                    confirm_pending: !checked,
                };
                let name = format!("{} {}円", l.name, l.amount());
                if !r.txn.bill_unchecked() {
                    self.calls.resolve(register, CallKind::Bill, now);
                }
                done(if checked {
                    format!("{name}：金額を確認しました")
                } else {
                    format!("{name}：確認を外しました")
                })
            }
            C::AffixReceiptStamp { register, side } => {
                self.check(register, side, Action::AffixReceiptStamp)?;
                let now = self.now;
                let r = self.reg_mut(register)?;
                let rs = r
                    .receipt_stamp
                    .take()
                    .ok_or("収入印紙を貼るレシートはありません")?;
                self.calls.resolve(register, CallKind::ReceiptStamp, now);
                done(format!(
                    "レシートに収入印紙 {}円の貼り付けを確認しました",
                    rs.stamp
                ))
            }
            C::AffixStamp {
                register,
                side,
                line_id,
                affixed,
            } => {
                self.check(register, side, Action::AffixStamp)?;
                let r = self.reg_mut(register)?;
                let l = r
                    .txn
                    .lines
                    .iter_mut()
                    .find(|l| Txn::needs_stamp(l) && l.id == line_id)
                    .ok_or("収入印紙が必要な払込票が見つかりません")?;
                l.stamped = affixed;
                let name = format!("{} {}円", l.name, l.amount());
                done(if affixed {
                    format!("{name}：収入印紙の貼り付けを確認しました")
                } else {
                    format!("{name}：収入印紙の確認を外しました")
                })
            }
            C::HandOverBills { register, side } => {
                self.check(register, side, Action::HandOverBills)?;
                let r = self.reg_mut(register)?;
                let n = r.bill_handover.take().unwrap_or(0);
                done(format!("払込票 {n}枚をお渡ししました"))
            }
            C::ReceiveTickets {
                register,
                side,
                received,
            } => {
                self.check(register, side, Action::ReceiveTickets)?;
                let now = self.now;
                let r = self.reg_mut(register)?;
                let n: u32 = r.ticket_handover.iter().map(|t| t.issued).sum();
                r.tickets_received = Some(received);
                if let Some(m) = self.finish_tickets(register, now) {
                    return done(m);
                }
                done(if received == n {
                    "受け取り枚数を入力しました"
                } else {
                    "申込数と受け取り数が一致しません\n再度ご確認ください"
                })
            }
            C::HandTickets {
                register,
                side,
                handed,
            } => {
                self.check(register, side, Action::HandTickets)?;
                let now = self.now;
                let r = self.reg_mut(register)?;
                let n: u32 = r.ticket_handover.iter().map(|t| t.issued).sum();
                r.tickets_handed = Some(handed);
                if let Some(m) = self.finish_tickets(register, now) {
                    return done(m);
                }
                done(if handed == n {
                    "お渡し枚数を入力しました"
                } else {
                    "発券枚数とお渡し枚数が一致しません\n再度ご確認ください"
                })
            }
            C::InsertCash {
                register,
                denomination,
                count,
            } => self.insert_cash(register, denomination, count),
            C::FinishInsert { register } => self.finish_insert(register),
            C::TapEmoney { register, method } => {
                if self.reg(register).is_some_and(|r| r.emoney.is_some()) {
                    return self.tap_emoney_task(register, method);
                }
                if self.reg(register).is_some_and(|r| r.txn.emoney_partial) {
                    if method.is_some_and(|m| m != PaymentMethod::Emoney) {
                        return Err("電子マネーをかざしてください".into());
                    }
                    let balance = self.emoney_balance;
                    let r = self.reg_mut(register)?;
                    if !r.txn.emoney_partial_ok {
                        return Err("案内を確認してから、電子マネーをかざしてください".into());
                    }
                    let drawn = balance.min(r.txn.cashless_due());
                    r.txn.emoney_partial = false;
                    r.txn.emoney_partial_ok = false;
                    r.txn.prepaid += drawn;
                    r.txn.prepaid_method = Some(PaymentMethod::Emoney);
                    r.txn.method = Some(PaymentMethod::Cash);
                    let due = r.txn.due();
                    self.emoney_balance -= drawn;
                    return done(format!(
                        "電子マネーで {drawn}円 お支払いしました\n残り {due}円を現金でお支払いください"
                    ));
                }
                self.check(register, Side::Customer, Action::TapEmoney)?;
                let balance = self.emoney_balance;
                let r = self.reg_mut(register)?;
                let chosen = r.txn.method.unwrap_or(PaymentMethod::Emoney);
                if let Some(m) = method.filter(|&m| m != chosen) {
                    return Err(format!(
                        "{}が選ばれています（{}は使えません）",
                        chosen.label(),
                        m.label()
                    ));
                }
                let method = chosen;
                let due = r.txn.due();
                if method == PaymentMethod::Points {
                    let part = r.txn.points_due();
                    if part == 0 {
                        return Err(NO_POINTS.into());
                    }
                    if part < due {
                        r.txn.prepaid += part;
                        r.txn.prepaid_method = Some(method);
                        let cash = r.txn.cashless_due() == 0;
                        r.txn.method = cash.then_some(PaymentMethod::Cash);
                        let rest = r.txn.due();
                        return done(if cash {
                            format!("ポイントで {part}円 お支払いしました\n残り {rest}円を現金でお支払いください")
                        } else {
                            format!("ポイントで {part}円 お支払いしました\n残り {rest}円のお支払い方法を選んでください")
                        });
                    }
                }
                let part = r.txn.cashless_due();
                if part == 0 {
                    return Err(CASH_ONLY.into());
                }
                if part < due {
                    if method != PaymentMethod::Emoney {
                        return Err(NO_CASH_MIX.into());
                    }
                    if balance < part {
                        r.txn.emoney_short = true;
                        return done(format!(
                            "電子マネーの残高が足りません（残高 {balance}円・お支払い {part}円）"
                        ));
                    }
                    self.emoney_balance -= part;
                    let r = self.reg_mut(register)?;
                    r.txn.prepaid += part;
                    r.txn.prepaid_method = Some(method);
                    r.txn.method = Some(PaymentMethod::Cash);
                    let rest = r.txn.due();
                    return done(format!(
                        "{}で {part}円 お支払いしました\n残り {rest}円は現金のみ取り扱いいたします",
                        method.label()
                    ));
                }
                if method == PaymentMethod::Emoney {
                    if balance < due {
                        r.txn.emoney_short = true;
                        return done(format!(
                            "電子マネーの残高が足りません（残高 {balance}円・お支払い {due}円）"
                        ));
                    }
                    self.emoney_balance -= due;
                }
                self.settle(register, method, due, Cash::default())
            }
            C::StartEmoney {
                register,
                side,
                kind,
            } => {
                self.check(register, side, Action::Emoney)?;
                let r = self.reg_mut(register)?;
                let shopping = !r.txn.lines.is_empty() || !r.txn.coupons.is_empty();
                if r.mode == Mode::SelfOperated && shopping {
                    return Err(EMONEY_NO_CHARGE.into());
                }
                if r.mode == Mode::SelfOperated && kind == EmoneyKind::Charge {
                    r.emoney = None;
                    self.call(register, CallKind::Emoney, "チャージ");
                    return done("チャージはスタッフがお手伝いします\nスタッフをお呼びしました");
                }
                if r.emoney
                    .as_ref()
                    .is_some_and(|e| !matches!(e, EmoneyTask::Menu))
                    || (r.emoney.is_some() && kind == EmoneyKind::Menu)
                {
                    return Err("電子マネーの手続き中です".into());
                }
                r.emoney = Some(match kind {
                    EmoneyKind::Menu => EmoneyTask::Menu,
                    EmoneyKind::Inquiry => EmoneyTask::Inquiry { shown: false },
                    EmoneyKind::Charge => EmoneyTask::Charge {
                        inserted: Cash::default(),
                    },
                });
                done(match kind {
                    EmoneyKind::Menu => "チャージか残高確認を選んでください",
                    EmoneyKind::Inquiry => "電子マネーを入金ボックスにかざしてください",
                    EmoneyKind::Charge => {
                        "チャージする金額の紙幣を入金ボックスに入れ、電子マネーをかざしてください"
                    }
                })
            }
            C::CloseEmoney { register, side } => {
                self.check(register, side, Action::Emoney)?;
                let r = self.reg_mut(register)?;
                match r.emoney.take() {
                    None => Err("電子マネーの手続きはしていません".into()),
                    Some(EmoneyTask::Charge { inserted }) if inserted.total() > 0 => {
                        let back = inserted.total();
                        r.outlet.merge(&inserted);
                        done(format!(
                            "チャージをやめました\n{back}円を出金ボックスから取り出してください"
                        ))
                    }
                    Some(EmoneyTask::Charge { .. }) => done("チャージをやめました"),
                    Some(EmoneyTask::Inquiry { .. }) => done("残高確認を閉じました"),
                    Some(EmoneyTask::Menu) => done("電子マネーの手続きを閉じました"),
                }
            }
            C::ResolveEmoneyShort {
                register,
                side,
                choice,
            } => {
                self.check(register, side, Action::ResolveEmoneyShort)?;
                let balance = self.emoney_balance;
                let r = self.reg_mut(register)?;
                if choice == ShortChoice::Staff {
                    if r.mode != Mode::SelfOperated {
                        return Err("スタッフ操作モードでは、その場でチャージ・現金併用を選べます".into());
                    }
                    self.call_emoney_short(register);
                    return done("スタッフをお呼びしました\nチャージまたは現金併用はスタッフがお手伝いします");
                }
                // スタッフを呼んだあとは、お客様は他の決済に切り替えられない（スタッフの無駄足になる）
                if r.mode == Mode::SelfOperated
                    && choice == ShortChoice::Cancel
                    && self
                        .calls
                        .open
                        .iter()
                        .any(|c| c.register == register && c.kind == CallKind::Emoney)
                {
                    return Err("スタッフが向かっています\nそのままお待ちください".into());
                }
                let r = self.reg_mut(register)?;
                if r.mode == Mode::SelfOperated && choice == ShortChoice::Charge {
                    return Err(EMONEY_NO_CHARGE.into());
                }
                if r.mode == Mode::SelfOperated
                    && matches!(choice, ShortChoice::Cash | ShortChoice::CashConfirmed)
                {
                    return Err(EMONEY_NO_CASH.into());
                }
                if choice == ShortChoice::CashConfirmed {
                    if !r.txn.emoney_partial {
                        return Err("「不足分を現金で払う」が選ばれていません".into());
                    }
                    r.txn.emoney_partial_ok = true;
                    return done("電子マネーを入金ボックスにかざしてください");
                }
                r.txn.emoney_short = false;
                r.txn.emoney_partial = false;
                r.txn.emoney_partial_ok = false;
                match choice {
                    ShortChoice::Cancel => {
                        r.txn.method = None;
                        // セルフ操作モードの残高不足で呼んだスタッフは、他の決済を選んだら取り下げる
                        if r.mode == Mode::SelfOperated {
                            self.calls.resolve(register, CallKind::Emoney, self.now);
                        }
                        done("電子マネーを使うのをやめました\nお支払い方法を選んでください")
                    }
                    ShortChoice::Cash => {
                        r.txn.emoney_partial = true;
                        let drawn = balance.min(r.txn.cashless_due());
                        let rest = r.txn.due().saturating_sub(drawn);
                        done(format!(
                            "電子マネーの残高 {drawn}円を引き出し、残り {rest}円を現金でお支払いいただきます"
                        ))
                    }
                    ShortChoice::CashConfirmed | ShortChoice::Staff => unreachable!(),
                    ShortChoice::Charge => {
                        r.emoney = Some(EmoneyTask::Charge {
                            inserted: Cash::default(),
                        });
                        done("チャージする金額の紙幣を入金ボックスに入れ、電子マネーをかざしてください")
                    }
                }
            }
            C::CallStaff { register, topic } => {
                self.check_active(register, Action::CallStaff)?;
                if topic == Some(CallTopic::Sos) {
                    self.press(register)?;
                    return done("仲間のスタッフを呼んでいます");
                }
                let (kind, detail) = match topic {
                    Some(CallTopic::Service) => (CallKind::Service, "お支払い・荷物"),
                    Some(CallTopic::Voucher) => (CallKind::Voucher, "ギフトカード・印紙"),
                    Some(CallTopic::Tobacco) => (CallKind::Tobacco, ""),
                    Some(CallTopic::Emoney) => (CallKind::Emoney, "チャージ・残高確認"),
                    Some(CallTopic::Sos) => unreachable!(),
                    None => (CallKind::CustomerHelp, "お客様からの呼び出し"),
                };
                self.call(register, kind, detail);
                self.press(register)?;
                done(match topic {
                    Some(CallTopic::Service) => "スタッフをお呼びしました（サービス）",
                    Some(CallTopic::Voucher) => "スタッフをお呼びしました（金券）",
                    Some(CallTopic::Tobacco) => "スタッフをお呼びしました（たばこ）",
                    Some(CallTopic::Emoney) => "スタッフをお呼びしました（電子マネー）",
                    Some(CallTopic::Sos) => unreachable!(),
                    None => "スタッフをお呼びしました",
                })
            }
            C::ConfirmPointCard { register, side } => {
                self.check(register, side, Action::ConfirmPointCard)?;
                let t = &mut self.reg_mut(register)?.txn;
                t.point_checked = true;
                t.point_declined = true;
                if t.method == Some(PaymentMethod::Points) {
                    t.method = None;
                }
                done("ポイントカードなしで進みます\nお支払い方法を選んでください")
            }
            C::ReleaseCall { register } => {
                let r = self.reg_mut(register)?;
                r.pressing = false;
                r.press_since = None;
                done("")
            }
            C::VoidTransaction { register } => {
                self.check_active(register, Action::VoidTransaction)?;
                let now = self.now;
                let r = self.reg_mut(register)?;
                r.menu = None;
                let back = self.void_txn(register, now);
                if back > 0 {
                    done(format!(
                        "取引を中止しました\n{back}円を出金ボックスから取り出してください"
                    ))
                } else {
                    done("取引を中止しました")
                }
            }
            C::Pause { register } => {
                self.check_active(register, Action::Pause)?;
                let r = self.reg_mut(register)?;
                r.staff = None;
                r.menu = None;
                r.touch = None;
                r.paused = true;
                done("レジ休止中にしました（スタッフバーコードで再開）")
            }
            C::EndStaffMode { register } => {
                self.check_active(register, Action::EndStaffMode)?;
                let r = self.reg_mut(register)?;
                r.staff = None;
                r.menu = None;
                done("ログアウトしました（セルフ操作モード）")
            }
            C::CashOut {
                register,
                kind,
                amount,
                password,
            } => self.cash_out(register, kind, amount, &password),
            C::CashIn {
                register,
                kind,
                amount,
            } => self.cash_in(register, kind, amount),
            C::OpenMenu {
                register,
                screen,
                exchange_to,
            } => self.open_menu(register, screen, exchange_to),
            C::Insert { register, amount } => self.insert(register, amount),
            C::TakeOutlet { register } => {
                let now = self.now;
                let r = self.reg_mut(register)?;
                let total = r.outlet.total();
                if total == 0 {
                    return Err("出金ボックスにお金はありません".into());
                }
                r.outlet = Cash::default();
                if let Some(rc) = r.last_receipt.as_mut() {
                    rc.at = now;
                }
                r.resume_idle_countdown(now);
                self.calls.resolve(register, CallKind::ChangeLeft, now);
                done(format!("{total}円を取り出しました"))
            }
            C::Claim { register, side } => self.claim(register, side),
            C::AddAgeRule {
                owner_id,
                password,
                target,
                min_age,
            } => {
                let by = self.owner(&owner_id, &password)?;
                self.age_rules.add(target, min_age, by, self.now)?;
                done("年確ルールを追加しました")
            }
            C::RemoveAgeRule {
                owner_id,
                password,
                rule_id,
            } => {
                let by = self.owner(&owner_id, &password)?;
                self.age_rules.remove(rule_id, by, self.now)?;
                done("年確ルールを削除しました")
            }
            C::SetProductAge {
                owner_id,
                password,
                code,
                min_age,
            } => {
                let by = self.owner(&owner_id, &password)?;
                self.age_rules.set_product(&code, min_age, by, self.now)?;
                done("年確ルールを変更しました")
            }
            C::SetSaleLimit {
                owner_id,
                password,
                code,
                limit,
            } => {
                let by = self.owner(&owner_id, &password)?;
                self.age_rules.set_limit(&code, limit, by, self.now)?;
                done("販売個数の上限を変更しました")
            }
            C::VerifyOwner { owner_id, password } => {
                self.owner(&owner_id, &password)?;
                Ok(Outcome::default())
            }
            C::SetTaxRate {
                owner_id,
                password,
                class_id,
                rate_bp,
            } => {
                let by = self.owner(&owner_id, &password)?;
                let change = self.tax.set_rate(&class_id, rate_bp)?;
                self.age_rules.record(by, change, self.now);
                done("税率を変更しました")
            }
            C::AddTaxClass {
                owner_id,
                password,
                name,
                kind,
                rate_bp,
            } => {
                let by = self.owner(&owner_id, &password)?;
                let change = self.tax.add_class(&name, kind, rate_bp)?;
                self.age_rules.record(by, change, self.now);
                done("税区分を追加しました")
            }
            C::SetProductTax {
                owner_id,
                password,
                code,
                class_id,
            } => {
                let by = self.owner(&owner_id, &password)?;
                let change = self.tax.set_product(&code, &class_id)?;
                self.age_rules.record(by, change, self.now);
                done("商品の税区分を変更しました")
            }
            C::SetInvoice {
                owner_id,
                password,
                invoice_no,
                rounding,
            } => {
                let by = self.owner(&owner_id, &password)?;
                let change = self.tax.set_invoice(&invoice_no, rounding)?;
                self.age_rules.record(by, change, self.now);
                done("インボイスの設定を変更しました")
            }
            C::SetBellLoss { percent } => {
                self.bell_loss = percent.min(100);
                done(format!("Bell の通信ロスを {}% にしました", self.bell_loss))
            }
            C::SetBellOn { bell, on } => {
                let signal = self.signal;
                let m = self.bells.get_mut(bell).ok_or("Bell が見つかりません")?;
                if bell_kind(bell as u8) == BellKind::Fixed {
                    return Err("据え付け端末は OFF にできません".into());
                }
                if on && !m.bell_on {
                    m.received.presses = signal.presses;
                    m.buzz_until = 0;
                }
                m.bell_on = on;
                done(format!(
                    "{}の呼び鈴を{}にしました",
                    m.id,
                    if on { "ON" } else { "OFF" }
                ))
            }
            C::SetBellOffline { bell, offline } => {
                let m = self.bells.get_mut(bell).ok_or("Bell が見つかりません")?;
                m.offline = offline;
                done(format!(
                    "{}を{}にしました",
                    m.id,
                    if offline { "不通" } else { "復旧" }
                ))
            }
        }
    }

    fn tax_for(&self, code: &str) -> (String, u32) {
        let c = self.tax.class_for(code);
        (c.id.clone(), c.rate_bp)
    }

    fn owner(&mut self, id: &str, password: &str) -> Result<&'static str, String> {
        self.auth.verify_owner(id, password, self.now)?;
        Ok(auth::account(id).map_or("?", |a| a.name))
    }

    fn pending_parcel(&mut self, register: u8, line_id: u32) -> Result<&mut Parcel, String> {
        self.reg_mut(register)?
            .txn
            .lines
            .iter_mut()
            .find(|l| l.id == line_id && matches!(l.kind, LineKind::Service { done: false, .. }))
            .and_then(|l| l.parcel.as_mut())
            .ok_or_else(|| "受付中の荷物が見つかりません".to_string())
    }

    fn check_limit(&self, register: u8, code: &str, add: u32) -> Result<(), String> {
        let Some(limit) = self.age_rules.limit_for(code) else {
            return Ok(());
        };
        let have = self.reg(register).map_or(0, |r| r.txn.qty_of(code));
        if have + add > limit {
            let name = catalog::find(code).map_or(code, |p| p.name);
            return Err(format!("{name}は1回のお会計で{limit}点までです"));
        }
        Ok(())
    }

    /// セルフ操作モードで電子マネーの残高が足りず、お客様が「チャージまたは現金併用」を選んだとき：
    /// セルフは現金を扱わずチャージもできないので、スタッフを呼ぶ（残高不足の時点では呼ばない。
    /// 他の決済を使うならスタッフの無駄足になるため）。スタッフがログインすると、スタッフ操作モードの
    /// 「電子マネーの残高が足りません」（不足額を現金で払う／チャージする）につながる
    fn call_emoney_short(&mut self, register: u8) {
        if self.reg(register).is_some_and(|r| r.mode == Mode::SelfOperated) {
            self.calls.raise(register, CallKind::Emoney, self.now, "残高不足");
        }
    }

    fn call(&mut self, register: u8, kind: CallKind, detail: &str) {
        if self
            .reg(register)
            .is_some_and(|r| r.staff_state() == StaffState::Ready)
        {
            return;
        }
        self.calls.raise(register, kind, self.now, detail);
    }

    fn press(&mut self, register: u8) -> Result<(), String> {
        let now = self.now;
        let r = self.reg_mut(register)?;
        if !r.pressing {
            r.press_since = Some(now);
        }
        r.pressing = true;
        if r.staff.is_none() {
            r.press_log
                .retain(|&t| now.saturating_sub(t) < EMERGENCY_WINDOW_MS);
            r.press_log.push(now);
        }
        self.presses += 1;
        Ok(())
    }

    fn set_qty(
        &mut self,
        register: u8,
        side: Side,
        line_id: u32,
        qty: u32,
    ) -> Result<Outcome, String> {
        self.check(register, side, Action::RemoveLine)?;
        if qty == 0 {
            return Err("取り消すときは「取り消し」を押してください".into());
        }
        let r = self.reg(register).ok_or("レジがありません")?;
        let l = r
            .txn
            .lines
            .iter()
            .find(|l| l.id == line_id)
            .ok_or("明細が見つかりません")?;
        let (code, name, old, kind) = (l.code, l.name, l.qty, l.kind);
        let received = kind
            == LineKind::Ff {
                state: FfState::Received,
            };
        match kind {
            LineKind::Service { .. } => return Err("サービス商品の個数は変えられません".into()),
            _ if received && r.staff_state() != StaffState::Ready => {
                return Err(FF_RECEIVED_SELF.into())
            }
            _ => {}
        }
        if qty > old && catalog::find(code).is_some_and(|p| p.fresh) {
            return Err(FRESH_ONE_BY_ONE.into());
        }
        if qty > old && r.txn.item_count() - old + qty > MAX_ITEMS {
            return Err(ITEMS_FULL.into());
        }
        if qty > old {
            self.check_limit(register, code, qty - old)?;
        }
        if matches!(kind, LineKind::Ff { .. }) && !catalog::is_made_to_order(code) {
            if qty > old {
                let stock = self
                    .ff_stock
                    .iter_mut()
                    .find(|(c, _)| *c == code)
                    .ok_or("在庫がありません")?;
                if stock.1 < qty - old {
                    return Err(format!("{name}{FF_OUT}"));
                }
                stock.1 -= qty - old;
            } else if !received {
                self.restock(code, old - qty);
            }
        }
        let t = &mut self.reg_mut(register)?.txn;
        let l = t.lines.iter_mut().find(|l| l.id == line_id).unwrap();
        l.qty = qty;
        if received && qty > old {
            l.ff_base = old;
            l.kind = LineKind::Ff {
                state: FfState::Waiting,
            };
        }
        if matches!(kind, LineKind::Ff { .. }) && !received && qty <= l.ff_base {
            let handed = l.ff_base;
            l.ff_base = qty;
            l.kind = LineKind::Ff {
                state: FfState::Received,
            };
            if handed > qty {
                t.ff_collect.push((name, handed - qty));
                return done(format!("{name}を{}点回収してください", handed - qty));
            }
        }
        if received && qty < old {
            t.ff_collect.push((name, old - qty));
            return done(format!("{name}を{}点回収してください", old - qty));
        }
        done(format!("{name}の個数を {qty} にしました"))
    }

    /// 発券の列の先頭のレジがまだ発券していなければ発券する（先のお渡しが済むまで次は発券しない）
    pub(super) fn issue_next_tickets(&mut self) {
        let Some(&id) = self.ticket_queue.front() else {
            return;
        };
        if let Some(r) = self.registers.iter_mut().find(|r| r.id == id) {
            r.tickets_issued = true;
        }
    }

    /// スタッフのお渡し枚数とお客様の受け取り枚数が、どちらも発券枚数と一致したらお渡しを終える
    /// （スタッフとお客様の入力どうしは比べない）。
    /// 留めていたお釣りとレシートを出し、発券の列の次のレジに発券させる
    fn finish_tickets(&mut self, register: u8, now: u64) -> Option<String> {
        let r = self.registers.iter_mut().find(|r| r.id == register)?;
        let n: u32 = r.ticket_handover.iter().map(|t| t.issued).sum();
        if r.ticket_handover.is_empty()
            || r.tickets_received != Some(n)
            || r.tickets_handed != Some(n)
        {
            return None;
        }
        r.ticket_handover.clear();
        r.tickets_issued = false;
        r.tickets_received = None;
        r.tickets_handed = None;
        let held = std::mem::take(&mut r.held_change);
        r.outlet.merge(&held);
        if let Some(rc) = r.last_receipt.as_mut() {
            rc.at = now;
        }
        self.ticket_queue.retain(|&id| id != register);
        self.issue_next_tickets();
        Some(format!("チケット {n}枚をお渡ししました"))
    }

    fn restock(&mut self, code: &str, qty: u32) {
        if let Some(e) = self.ff_stock.iter_mut().find(|(c, _)| *c == code) {
            e.1 += qty;
        }
    }

    fn scan(&mut self, register: u8, side: Side, code: &str) -> Result<Outcome, String> {
        let scanned = catalog::classify(code);
        if let Scanned::Staff(id) = &scanned {
            let acc = auth::account(id).ok_or("登録されていないスタッフバーコードです")?;
            let now = self.now;
            let r = self.reg_mut(register)?;
            let was = r.staff_state();
            let resumed = std::mem::take(&mut r.paused);
            let took_over = r.txn.customer_turn;
            r.staff = Some(StaffSession { account: acc });
            r.press_log.clear();
            r.emergency = false;
            r.touch = None;
            r.txn.customer_turn = false;
            r.pressing = false;
            r.press_since = None;
            let service_pending = r.txn.service_pending();
            self.calls.resolve(register, CallKind::CustomerHelp, now);
            if self.calls.is_open(register, CallKind::Tobacco) {
                self.reg_mut(register)?.txn.tobacco_asked = true;
            }
            self.calls.resolve(register, CallKind::Tobacco, now);
            self.calls.resolve(register, CallKind::Voucher, now);
            self.calls.resolve(register, CallKind::Emoney, now);
            if !service_pending {
                self.calls.resolve(register, CallKind::Service, now);
            }
            for k in [
                CallKind::FastFood,
                CallKind::Service,
                CallKind::AgeCheck,
                CallKind::Bill,
                CallKind::CashOnly,
                CallKind::Expired,
            ] {
                self.calls.resolve(register, k, now);
            }
            return done(match was {
                _ if resumed => {
                    format!("{}さんがレジを再開しました（スタッフ操作モード）", acc.name)
                }
                _ if took_over => format!("{}さんが操作を引き継ぎました", acc.name),
                _ => format!("{}さんがログインしました（スタッフ操作モード）", acc.name),
            });
        }
        self.check(register, side, Action::Scan)?;
        // チケットのお渡しが済むまで、そのレジは取引を終えられない（発券待ちのあいだも同じ）
        if self
            .reg(register)
            .is_some_and(|r| !r.ticket_handover.is_empty())
        {
            return Err(TICKETS_FIRST.into());
        }
        if let Scanned::Pickup(no) = &scanned {
            return self.match_pickup(register, side, no);
        }
        if let Scanned::ParcelSlip(no) = &scanned {
            return self.cancel_parcel(register, side, no);
        }
        if let Scanned::Product(p) = &scanned {
            self.check_limit(register, p.code, 1)?;
        }
        let tax = match &scanned {
            Scanned::Product(p) => self.tax_for(p.code),
            _ => (String::new(), 0),
        };
        let now = self.now;
        let r = self.reg_mut(register)?;
        r.last_receipt = None;
        r.bill_handover = None;
        r.pickup_handover = None;
        r.receipt_stamp = None;
        if let Scanned::PointCard(no) = &scanned {
            // 決済が終わるまではいつでも付与できる。支払い中に読んでも登録画面へは戻さない
            if r.txn.paying {
                let asked = !r.txn.point_checked;
                r.txn.point_card = Some(no.clone());
                r.txn.point_checked = true;
                return done(if asked && r.txn.method.is_none() {
                    "ポイントカードを読みました\nお支払い方法を選んでください"
                } else {
                    "ポイントカードを読みました"
                });
            }
        }
        if let Scanned::Product(p) = &scanned {
            if r.txn.lines.iter().any(Line::is_pickup) || p.parcel_pickup && !r.txn.lines.is_empty()
            {
                return Err(PICKUP_ALONE.into());
            }
            let slips = r.txn.bill_slips();
            if p.utility_bill && r.txn.lines.iter().any(|l| !l.is_bill())
                || !p.utility_bill && slips > 0
            {
                return Err(BILL_ALONE.into());
            }
            if r.txn.lines.iter().any(|l| l.is_parcel() != p.parcel_send) {
                return Err(PARCEL_ALONE.into());
            }
            // チケットは一件（申込券一枚）ずつ、ほかの商品ともほかのチケットとも一緒に会計しない
            if r.txn.lines.iter().any(|l| l.tickets() > 0)
                || p.tickets > 0 && !r.txn.lines.is_empty()
            {
                return Err(TICKET_ALONE.into());
            }
        }
        if matches!(scanned, Scanned::Product(_)) && r.txn.item_count() >= MAX_ITEMS {
            return Err(ITEMS_FULL.into());
        }
        let back = if r.txn.paying {
            r.txn.paying = false;
            r.txn.method = None;
            r.txn.customer_turn = false;
            "（登録画面に戻りました）"
        } else {
            ""
        };
        match scanned {
            Scanned::Staff(_) | Scanned::Pickup(_) | Scanned::ParcelSlip(_) => unreachable!(),
            Scanned::Unknown => Err("登録されていないバーコードです".into()),
            Scanned::PointCard(no) => {
                r.txn.point_card = Some(no);
                done(format!("ポイントカードを読みました{back}"))
            }
            Scanned::Coupon(c) => {
                r.txn.next_line += 1;
                let id = r.txn.next_line;
                r.txn.coupons.push((id, c));
                let waiting = r
                    .txn
                    .coupon_uses()
                    .last()
                    .is_some_and(|(_, _, u)| u.is_none());
                done(if waiting {
                    format!(
                        "{}を読みました\n対象商品を登録すると値引きします{back}",
                        c.name
                    )
                } else {
                    format!("{}を読みました{back}", c.name)
                })
            }
            Scanned::Product(p) if p.utility_bill => {
                r.txn.push(
                    p,
                    1,
                    LineKind::Service {
                        done: true,
                        confirm_pending: true,
                    },
                    tax,
                );
                let seed = now ^ (u64::from(register) << 56) ^ (u64::from(r.txn.next_line) << 40);
                let seed = seed ^ p.code.parse::<u64>().unwrap_or(0);
                if let Some(l) = r.txn.lines.last_mut() {
                    l.slip_no = Some(slip_no(seed));
                }
                done(format!("{} {}円{back}", p.name, p.price))
            }
            // チケットの申込券：お支払いのあとに発券する。申込券ごとに一行（重ねない）
            Scanned::Product(p) if p.tickets > 0 => {
                r.txn.push(
                    p,
                    1,
                    LineKind::Service {
                        done: true,
                        confirm_pending: false,
                    },
                    tax,
                );
                done(format!("{} {}枚 {}円{back}", p.name, p.tickets, p.price))
            }
            Scanned::Product(p) if p.needs_staff => {
                r.txn.push(
                    p,
                    1,
                    LineKind::Service {
                        done: false,
                        confirm_pending: false,
                    },
                    tax,
                );
                if p.parcel_send {
                    let seed =
                        now ^ (u64::from(register) << 56) ^ (u64::from(r.txn.next_line) << 40);
                    if let Some(pc) = r.txn.lines.last_mut().and_then(|l| l.parcel.as_mut()) {
                        pc.slip = parcel_slip(seed);
                    }
                }
                if p.parcel_pickup {
                    let seed = now ^ (u64::from(register) << 56) ^ 0x5049_434B;
                    if let Some(l) = r.txn.lines.last_mut() {
                        let pk = Pickup {
                            no: pickup_no(seed),
                            amount: 1_000 + (mix(seed ^ 0xC0D) % 90) as u32 * 100,
                            matched: false,
                            wrong: None,
                            missing: false,
                        };
                        l.unit_price = pk.amount;
                        l.pickup = Some(pk);
                    }
                }
                r.txn.parcel_ask = false;
                let ready = r.staff_state() == StaffState::Ready;
                if !ready {
                    self.calls.raise(register, CallKind::Service, now, p.name);
                }
                done(if ready && p.parcel_send {
                    format!("{}：取り扱いサイズを決定してください{back}", p.name)
                } else if ready && p.parcel_pickup {
                    format!(
                        "{}：保管場所から荷物を探し、バーコードを読み取ってください",
                        p.name
                    )
                } else if ready {
                    format!("{}：受付内容を入力してください{back}", p.name)
                } else {
                    format!("{}：スタッフをお呼びしました{back}", p.name)
                })
            }
            Scanned::Product(p) => {
                if p.category == Category::Tobacco {
                    r.txn.tobacco_asked = false;
                }
                r.txn.push(p, 1, LineKind::Item, tax);
                if p.expired() {
                    // 販売期限切れ：明細に「期限切れ」のフラグを出し、回収（取り消す）までお会計に進めない
                    if r.staff_state() != StaffState::Ready {
                        self.calls.raise(register, CallKind::Expired, now, p.name);
                        return done(format!(
                            "{}は販売期限が切れています\nスタッフをお呼びしました{back}",
                            p.name
                        ));
                    }
                    return done(format!(
                        "{}は販売期限が切れています\n販売期限切れ商品を回収してください{back}",
                        p.name
                    ));
                }
                done(format!("{} {}円{back}", p.name, p.price))
            }
        }
    }

    fn cancel_parcel(&mut self, register: u8, side: Side, no: &str) -> Result<Outcome, String> {
        let now = self.now;
        let r = self.reg_mut(register)?;
        if r.staff_state() != StaffState::Ready || side != Side::Staff {
            return Err(
                "宅配便の取り消しは、スタッフがスタッフ側で伝票を読み取って行います".into(),
            );
        }
        if r.txn.paying {
            return Err("お会計中です（登録に戻ってから伝票を読み取ってください）".into());
        }
        let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
        let i = r
            .txn
            .lines
            .iter()
            .position(|l| {
                l.parcel
                    .as_ref()
                    .is_some_and(|pc| digits(&pc.slip) == digits(no))
            })
            .ok_or("この取引で受け付けた荷物の伝票ではありません")?;
        let name = r.txn.lines[i].name;
        r.txn.lines.remove(i);
        if !r.txn.lines.iter().any(Line::is_parcel) {
            r.txn.parcel_ask = false;
        }
        if !r.txn.service_pending() {
            self.calls.resolve(register, CallKind::Service, now);
        }
        done(format!("{name}の受付を取り消しました"))
    }

    fn match_pickup(&mut self, register: u8, side: Side, no: &str) -> Result<Outcome, String> {
        let r = self.reg_mut(register)?;
        if r.staff_state() != StaffState::Ready || side != Side::Staff {
            return Err("お預かりの荷物のバーコードは、スタッフがスタッフ側で読み取ります".into());
        }
        let pk = r
            .txn
            .lines
            .iter_mut()
            .find_map(|l| l.pickup.as_mut())
            .ok_or("お受け取りの手続き中ではありません（先にお客様の受取用バーコードを読み取ってください）")?;
        if !pk.matches(no) {
            pk.wrong = Some(no.to_string());
            return Err(format!(
                "番号が違います\nこの荷物はお渡しできません（お受け取りの荷物は {}）",
                pk.no
            ));
        }
        let now = self.now;
        if let Some(l) = self
            .reg_mut(register)?
            .txn
            .lines
            .iter_mut()
            .find(|l| l.is_pickup())
        {
            l.set_pickup_found(Some(true));
        }
        self.calls.resolve(register, CallKind::Service, now);
        done("照合しました\n代金引換の金額をお預かりしてください")
    }

    fn register_ff(
        &mut self,
        register: u8,
        side: Side,
        code: &str,
        qty: u32,
        force: bool,
    ) -> Result<Outcome, String> {
        self.check(register, side, Action::RegisterFf)?;
        if self.reg(register).is_some_and(|r| r.txn.bill_slips() > 0) {
            return Err(BILL_ALONE.into());
        }
        if self
            .reg(register)
            .is_some_and(|r| r.txn.lines.iter().any(|l| l.tickets() > 0))
        {
            return Err(TICKET_ALONE.into());
        }
        if self
            .reg(register)
            .is_some_and(|r| r.txn.lines.iter().any(Line::is_parcel))
        {
            return Err(PARCEL_ALONE.into());
        }
        if self
            .reg(register)
            .is_some_and(|r| r.txn.lines.iter().any(Line::is_pickup))
        {
            return Err(PICKUP_ALONE.into());
        }
        let p = catalog::find(code)
            .filter(|p| p.category == Category::FastFood)
            .ok_or("FF の商品が見つかりません")?;
        let qty = qty.max(1);
        let count = self.reg(register).map_or(0, |r| r.txn.item_count());
        if count + qty > MAX_ITEMS {
            return Err(ITEMS_FULL.into());
        }
        self.check_limit(register, code, qty)?;
        let mut reserved = false;
        if !catalog::is_made_to_order(code) {
            let stock = self
                .ff_stock
                .iter_mut()
                .find(|(c, _)| *c == code)
                .ok_or("在庫がありません")?;
            reserved = stock.1 < qty;
            if reserved && !force {
                return Err(format!("{}{FF_OUT}", p.name));
            }
            stock.1 = stock.1.saturating_sub(qty);
        }
        let tax = self.tax_for(p.code);
        let now = self.now;
        let r = self.reg_mut(register)?;
        r.last_receipt = None;
        if r.txn.paying {
            r.txn.paying = false;
            r.txn.method = None;
            r.txn.customer_turn = false;
        }
        r.txn.push(
            p,
            qty,
            LineKind::Ff {
                state: FfState::Waiting,
            },
            tax,
        );
        self.calls.resolve(register, CallKind::FastFood, now);
        done(format!(
            "{} ×{qty} を{}しました\nお渡ししたらチェックを入れてください",
            p.name,
            if reserved {
                "予約注文として登録"
            } else {
                "登録"
            }
        ))
    }

    fn verify_age(&mut self, register: u8, age: u8) -> Result<Outcome, String> {
        let rules = &self.age_rules;
        let r = self
            .registers
            .iter_mut()
            .find(|r| r.id == register)
            .ok_or("レジがありません")?;
        let mut removed: Vec<(&'static str, u32)> = vec![];
        r.txn.lines.retain(|l| {
            let keep = rules.age_for(l.code, l.category).is_none_or(|a| a <= age);
            if !keep {
                match removed.iter_mut().find(|(n, _)| *n == l.name) {
                    Some(e) => e.1 += l.qty,
                    None => removed.push((l.name, l.qty)),
                }
            }
            keep
        });
        r.txn.verified_age = Some(age);
        r.txn.mark_ages_done(rules);
        r.txn.required_age = r.txn.age_to_check(rules);
        r.txn.age_early = false;
        if r.txn.lines.is_empty() {
            r.txn.paying = false;
            r.txn.method = None;
            r.txn.customer_turn = false;
        }
        if removed.is_empty() {
            return done(format!("{age}歳です\n年齢確認が済みました"));
        }
        let list = removed
            .iter()
            .map(|(n, q)| format!("{n}×{q}"))
            .collect::<Vec<_>>()
            .join("・");
        r.txn.confiscated.extend(removed);
        done(format!("{age}歳です\n販売できない{list}を取り除きました"))
    }

    fn checkout(&mut self, register: u8, side: Side) -> Result<Outcome, String> {
        self.check(register, side, Action::Checkout)?;
        let rules = &self.age_rules;
        let r = self
            .registers
            .iter_mut()
            .find(|r| r.id == register)
            .ok_or("レジがありません")?;
        let age = r.txn.age_to_check(rules);
        r.txn.paying = true;
        r.txn.age_early = false;
        r.txn.required_age = age;
        if r.txn.age_unchecked(rules) {
            r.txn.verified_age = None;
        }
        if !r.txn.age_cleared() {
            r.txn.age_method = None;
        }
        r.txn.point_checked = r.txn.point_card.is_some()
            || r.txn.point_declined
            || r.txn.bill_slips() > 0
            || !r.txn.lines.iter().any(Line::earns_points);
        r.txn.customer_turn = r.mode == Mode::SelfOperated;
        let ready = r.staff_state() == StaffState::Ready;
        let pending = r.txn.age_choice_pending();
        let mode = r.mode;
        let slips = r.txn.bill_slips();
        if slips > 0 {
            r.txn.method = Some(PaymentMethod::Cash);
            if !r.txn.bill_unchecked() && r.txn.stamp_pending() {
                return done(format!(
                    "公共料金 {slips}枚：領収証に収入印紙を貼り付けてください"
                ));
            }
            if !r.txn.bill_unchecked() {
                return done(format!("公共料金 {slips}枚：現金のみ取り扱いいたします"));
            }
            if mode != Mode::StaffOperated && !ready {
                self.calls
                    .raise(register, CallKind::Bill, self.now, format!("{slips}枚"));
                return done("公共料金のお支払いのためスタッフをお呼びしました");
            }
            return done(format!(
                "公共料金 {slips}枚：スタッフ側の画面で払込票の内容を確認してください"
            ));
        }
        if mode != Mode::StaffOperated && !ready && r.txn.cash_only() > 0 {
            let label = r.txn.cash_only_label();
            self.calls
                .raise(register, CallKind::CashOnly, self.now, label.clone());
            if !pending {
                return done(format!("{label}は現金のみのためスタッフをお呼びしました"));
            }
        }
        if pending && mode != Mode::StaffOperated && !ready {
            self.calls.raise(
                register,
                CallKind::AgeCheck,
                self.now,
                format!("{}歳以上", age.unwrap_or(0)),
            );
            return done("年齢確認のためスタッフをお呼びしました");
        }
        if !pending {
            self.calls.resolve(register, CallKind::AgeCheck, self.now);
        }
        if pending {
            done(format!(
                "{}歳以上の年齢確認があります\nスタッフ側の画面で年齢確認をするか選んでください",
                age.unwrap_or(0)
            ))
        } else {
            done("お会計に進みました")
        }
    }

    fn insert_cash(&mut self, register: u8, d: u32, count: u32) -> Result<Outcome, String> {
        if !is_valid_denomination(d) {
            return Err("使えない金種です".into());
        }
        let mut c = Cash::default();
        c.add(d, count.max(1));
        // 二千円札は取り扱わない。自動釣銭機は受け付けず、そのまま出金ボックスに吐き出す
        // （テンキーで 2000 と打った場合は千円札 2 枚とみなすので、これに当たらない）
        if d == 2_000 {
            self.reg_mut(register)?.outlet.merge(&c);
            return done("二千円札はお取り扱いしていません\n出金ボックスからお受け取りください");
        }
        if self.reg(register).is_some_and(|r| r.emoney.is_some()) {
            return self.charge_cash(register, c);
        }
        self.check(register, Side::Customer, Action::InsertCash)?;
        self.pay_cash(register, c)
    }

    fn charge_cash(&mut self, register: u8, c: Cash) -> Result<Outcome, String> {
        let balance = self.emoney_balance;
        let r = self.reg_mut(register)?;
        let Some(EmoneyTask::Charge { inserted }) = &mut r.emoney else {
            return Err("チャージを選ぶと、お金を受け付けます".into());
        };
        if c.0.iter().any(|&(d, n)| n > 0 && d < 1000) {
            return Err("チャージは千円札・紙幣で入れてください".into());
        }
        if balance + inserted.total() + c.total() > EMONEY_MAX {
            return Err(format!(
                "チャージすると残高の上限（{EMONEY_MAX}円）を超えます"
            ));
        }
        inserted.merge(&c);
        done(format!(
            "チャージ {}円\n電子マネーをかざしてください",
            inserted.total()
        ))
    }

    fn tap_emoney_task(
        &mut self,
        register: u8,
        method: Option<PaymentMethod>,
    ) -> Result<Outcome, String> {
        if method.is_some_and(|m| m != PaymentMethod::Emoney) {
            return Err("電子マネーをかざしてください".into());
        }
        let balance = self.emoney_balance;
        let r = self.reg_mut(register)?;
        let paying_emoney = r.txn.paying && r.txn.method == Some(PaymentMethod::Emoney);
        match &mut r.emoney {
            Some(EmoneyTask::Inquiry { shown }) => {
                *shown = true;
                done(format!("電子マネーの残高は {balance}円です"))
            }
            Some(EmoneyTask::Charge { inserted }) => {
                let amount = inserted.total();
                if amount == 0 {
                    return Err("先にチャージする紙幣を入金ボックスに入れてください".into());
                }
                let cash = std::mem::take(inserted);
                r.drawer.cash.merge(&cash);
                r.emoney = None;
                self.emoney_balance += amount;
                let now = self.emoney_balance;
                done(if paying_emoney {
                    format!("{amount}円チャージしました（残高 {now}円）\nもう一度電子マネーをかざしてください")
                } else {
                    format!("{amount}円チャージしました（残高 {now}円）")
                })
            }
            Some(EmoneyTask::Menu) => Err("チャージか残高確認を選んでください".into()),
            None => Err("電子マネーの手続きはしていません".into()),
        }
    }

    fn claim(&mut self, register: u8, side: Side) -> Result<Outcome, String> {
        let r = self.reg(register).ok_or("レジがありません")?;
        let c = r.context(&self.age_rules);
        match active_side(&c) {
            Some(s) if s != side => Err(if c.menu_open {
                "もう一方の画面で店舗メニューを操作中です"
            } else if c.staff != StaffState::Ready {
                "スタッフバーコードでログインしてください"
            } else if c.customer_turn {
                "お客様がお支払い中です"
            } else {
                "もう一方の画面で操作中です"
            }
            .into()),
            _ => Ok(Outcome::default()),
        }
    }

    fn insert(&mut self, register: u8, amount: u32) -> Result<Outcome, String> {
        if amount == 0 {
            return Err("金額を入力してください".into());
        }
        let menu = self.reg(register).ok_or("レジがありません")?.menu;
        match menu.map(|m| (m.screen, m.exchange_to)) {
            Some((MenuScreen::Deposit, _)) => self.cash_in(register, CashInKind::Deposit, amount),
            Some((MenuScreen::Changer, to)) => self.exchange(register, amount, to),
            Some(_) => Err("この画面ではお金を受け付けません".into()),
            None if self.reg(register).is_some_and(|r| r.emoney.is_some()) => {
                self.charge_cash(register, Cash::from_amount(amount))
            }
            None => {
                self.check(register, Side::Customer, Action::InsertCash)?;
                self.pay_cash(register, Cash::from_amount(amount))
            }
        }
    }

    fn exchange(&mut self, register: u8, amount: u32, to: u32) -> Result<Outcome, String> {
        self.check_active(register, Action::CashIn)?;
        if amount % to != 0 {
            return Err(format!("{to}円単位で入れてください"));
        }
        let now = self.now;
        let r = self.reg_mut(register)?;
        let staff = r.staff.map_or("?", |s| s.account.name);
        let out = r.drawer.take_exact(to, amount / to)?;
        r.drawer.put(amount);
        r.outlet.merge(&out);
        self.audit.push(AuditEntry {
            at: now,
            register,
            staff,
            operation: "両替",
            amount,
            result: "実行".into(),
        });
        done(format!(
            "{amount}円を {to}円×{} に両替しました\n出金ボックスから取り出してください",
            amount / to
        ))
    }

    fn pay_cash(&mut self, register: u8, c: Cash) -> Result<Outcome, String> {
        let now = self.now;
        let r = self.reg_mut(register)?;
        r.txn.method = Some(PaymentMethod::Cash);
        r.txn.inserted.merge(&c);
        r.txn.inserted_at = now;
        let paid = r.txn.inserted.total();
        let total = r.txn.due();
        if paid < total {
            return done(format!("{paid}円 投入（あと {}円）", total - paid));
        }
        done(format!(
            "{paid}円 投入\n{}秒の無操作で、お支払いを確定します",
            INSERT_DONE_MS / 1000
        ))
    }

    fn finish_insert(&mut self, register: u8) -> Result<Outcome, String> {
        self.check(register, Side::Customer, Action::InsertCash)?;
        self.finish_insert_auto(register)
    }

    fn finish_insert_auto(&mut self, register: u8) -> Result<Outcome, String> {
        let r = self.reg_mut(register)?;
        let paid = r.txn.inserted.total();
        if paid == 0 {
            return Err("お金を入れてください".into());
        }
        let total = r.txn.due();
        if paid < total {
            return Err(format!("あと {}円 入れてください", total - paid));
        }
        let inserted = r.txn.inserted.clone();
        self.settle(register, PaymentMethod::Cash, paid, inserted)
    }

    fn settle(
        &mut self,
        register: u8,
        method: PaymentMethod,
        paid: u32,
        inserted: Cash,
    ) -> Result<Outcome, String> {
        self.receipts += 1;
        let no = self.receipts;
        let now = self.now;
        let rates = self.tax.display_rates();
        let rounding = self.tax.rounding;
        let r = self.reg_mut(register)?;
        let total = r.txn.total();
        let due = r.txn.due();
        for (d, n) in &inserted.0 {
            r.drawer.cash.add(*d, *n);
        }
        let change = match r.drawer.take(paid - due, true) {
            Ok(c) => c,
            Err(e) => {
                for (d, n) in &inserted.0 {
                    if let Some(x) = r.drawer.cash.0.iter_mut().find(|(x, _)| x == d) {
                        x.1 -= n;
                    }
                }
                self.receipts -= 1;
                return Err(format!("{e}\nスタッフをお呼びください"));
            }
        };
        // チケットはお渡しの確認が済むまでレシートもお釣りも出さない
        if r.txn.lines.iter().any(|l| l.tickets() > 0) {
            r.held_change.merge(&change);
        } else {
            r.outlet.merge(&change);
        }
        let txn = std::mem::take(&mut r.txn);
        let slips = txn.bill_slips();
        r.bill_handover = (slips > 0).then_some(slips);
        // チケットのお渡しが済むまで次の取引は始められないので、ここに残っているチケットはない
        r.ticket_handover.extend(
            txn.lines
                .iter()
                .filter(|l| l.tickets() > 0)
                .map(|l| TicketRow {
                    line_id: no * 1000 + l.id,
                    code: l.code,
                    name: l.name,
                    issued: l.tickets(),
                }),
        );
        let has_tickets = !r.ticket_handover.is_empty();
        // 払込票は払込票ごとに確認済み。通常のお買い物はレシートの税抜金額で判定する
        let base = total.saturating_sub(txn.taxes(&rates, rounding).iter().map(|t| t.tax).sum());
        let credit = method == PaymentMethod::Credit && txn.prepaid == 0;
        r.receipt_stamp = (slips == 0 && !credit && stamp_duty(base) > 0).then_some(ReceiptStamp {
            stamp: stamp_duty(base),
            base,
        });
        let stamp_call = r.receipt_stamp.is_some() && r.staff_state() != StaffState::Ready;
        r.pickup_handover = txn
            .lines
            .iter()
            .find_map(|l| l.pickup.as_ref())
            .map(|p| p.no.clone());
        let receipt = Receipt {
            no,
            at: now,
            total,
            method: match (txn.prepaid_method.filter(|_| txn.prepaid > 0), method) {
                (Some(PaymentMethod::Points), PaymentMethod::Emoney) => "ポイント利用・電子マネー",
                (Some(PaymentMethod::Points), PaymentMethod::Qr) => "ポイント利用・バーコード決済",
                (Some(PaymentMethod::Points), PaymentMethod::Credit) => {
                    "ポイント利用・クレジットカード"
                }
                (Some(PaymentMethod::Points), _) => "ポイント利用・現金",
                (Some(PaymentMethod::Emoney), _) => "電子マネー・現金",
                (Some(PaymentMethod::Qr), _) => "バーコード決済・現金",
                (Some(PaymentMethod::Credit), _) => "クレジットカード・現金",
                _ => method.label(),
            },
            paid,
            change_total: change.total(),
            change,
            items: txn.lines.iter().map(|l| l.qty).sum(),
            points: if txn.point_card.is_some() {
                txn.lines
                    .iter()
                    .filter(|l| l.earns_points())
                    .map(Line::amount)
                    .sum::<u32>()
                    .min(total)
                    / 100
            } else {
                0
            },
            point_card: txn.point_card,
        };
        let msg = if receipt.change_total > 0 {
            format!("お支払いが完了しました\nおつり {}円", receipt.change_total)
        } else {
            "お支払いが完了しました".to_string()
        };
        r.last_receipt = Some(receipt);
        for k in [
            CallKind::AgeCheck,
            CallKind::FastFood,
            CallKind::Service,
            CallKind::Tobacco,
            CallKind::Voucher,
            CallKind::Bill,
            CallKind::Emoney,
            CallKind::CashOnly,
        ] {
            self.calls.resolve(register, k, now);
        }
        if has_tickets {
            self.ticket_queue.push_back(register);
            self.issue_next_tickets();
        }
        if stamp_call {
            self.calls
                .raise(register, CallKind::ReceiptStamp, now, "レシート");
            return done(format!(
                "{msg}\nレシートに収入印紙を貼るためスタッフをお呼びしました"
            ));
        }
        done(msg)
    }

    fn open_menu(
        &mut self,
        register: u8,
        screen: Option<MenuScreen>,
        exchange_to: Option<u32>,
    ) -> Result<Outcome, String> {
        let r = self.reg(register).ok_or("レジがありません")?;
        if r.staff_state() != StaffState::Ready {
            return Err("店舗メニューはログインしたスタッフだけが使えます".into());
        }
        let Some(screen) = screen else {
            self.reg_mut(register)?.menu = None;
            return Ok(Outcome::default());
        };
        let r = self.reg_mut(register)?;
        if !r.idle() {
            return Err("取引中は店舗メニューを開けません".into());
        }
        let to = exchange_to
            .or(r.menu.map(|m| m.exchange_to))
            .unwrap_or(1_000);
        if !is_valid_denomination(to) {
            return Err("使えない金種です".into());
        }
        r.menu = Some(Menu {
            screen,
            exchange_to: to,
        });
        Ok(Outcome::default())
    }

    fn require_cash_menu(&self, register: u8) -> Result<(), String> {
        let r = self.reg(register).ok_or("レジがありません")?;
        if r.menu.is_some() {
            Ok(())
        } else {
            Err("お金の出し入れは、スタッフ側の画面の店舗メニューで行います".into())
        }
    }

    fn cash_out(
        &mut self,
        register: u8,
        kind: CashOutKind,
        amount: u32,
        password: &str,
    ) -> Result<Outcome, String> {
        self.check_active(register, Action::CashOut)?;
        self.require_cash_menu(register)?;
        if amount == 0 {
            return Err("金額を入力してください".into());
        }
        self.auth.verify_store(password, self.now)?;
        let now = self.now;
        let staff = self
            .reg(register)
            .and_then(|r| r.staff)
            .map_or("?", |s| s.account.name);
        let log = |s: &mut Store, result: String| {
            s.audit.push(AuditEntry {
                at: now,
                register,
                staff,
                operation: kind.label(),
                amount,
                result,
            });
        };
        let r = self.reg_mut(register)?;
        match r.drawer.take(amount, false) {
            Ok(c) => {
                r.outlet.merge(&c);
                log(self, "実行".into());
                done(format!(
                    "{} {amount}円を出金しました\n出金ボックスから取り出してください",
                    kind.label()
                ))
            }
            Err(e) => {
                log(self, format!("失敗：{e}"));
                Err(e)
            }
        }
    }

    fn cash_in(&mut self, register: u8, kind: CashInKind, amount: u32) -> Result<Outcome, String> {
        self.check_active(register, Action::CashIn)?;
        self.require_cash_menu(register)?;
        if amount == 0 {
            return Err("金額を入力してください".into());
        }
        let now = self.now;
        let r = self.reg_mut(register)?;
        let staff = r.staff.map_or("?", |s| s.account.name);
        r.drawer.put(amount);
        self.audit.push(AuditEntry {
            at: now,
            register,
            staff,
            operation: kind.label(),
            amount,
            result: "実行".into(),
        });
        done(format!("{} {amount}円を入金しました", kind.label()))
    }

    pub fn snapshot(&self) -> Snapshot {
        let now = self.now;
        Snapshot {
            now,
            registers: self
                .registers
                .iter()
                .map(|r| self.register_view(r))
                .collect(),
            bells: self
                .bells
                .iter()
                .enumerate()
                .map(|(i, m)| BellView {
                    id: m.id.clone(),
                    kind: bell_kind(i as u8),
                    buzzing: m.buzzing(now),
                    link_ok: m.rx_ok(now),
                    rings: m.rings,
                    offline: m.offline,
                    bell_on: m.bell_on,
                })
                .collect(),
            bell_links: self
                .bells
                .iter()
                .map(|m| BellLink {
                    id: m.id.clone(),
                    link_ok: m.link_ok(now),
                    acked_seq: m.acked_seq,
                    sent: m.sent,
                    retries: m.retries,
                })
                .collect(),
            signal: self.signal,
            calls: self.calls.open.clone(),
            call_history: self.calls.history.clone(),
            age_rules: AgeRulesView {
                rules: self.age_rules.rules.clone(),
                history: self.age_rules.history.clone(),
            },
            audit: self.audit.clone(),
            ff_stock: self.ff_menu(),
            bell_loss: self.bell_loss,
            accounts: ACCOUNTS
                .iter()
                .map(|a| AccountView {
                    barcode: format!("{}{}", catalog::STAFF_PREFIX, a.id),
                    id: a.id,
                    name: a.name,
                    role: a.role,
                })
                .collect(),
            catalog: CATALOG
                .iter()
                .filter(|p| p.category != Category::FastFood)
                .map(|p| CatalogItem {
                    code: p.code,
                    name: p.name,
                    price: p.price,
                    category: p.category,
                    category_label: p.category.label(),
                    age: self.age_rules.age_for(p.code, p.category),
                    legal_age: self.age_rules.legal_age_for(p.code, p.category),
                    store_age: self.age_rules.store_age_for(p.code),
                    limit: self.age_rules.limit_for(p.code),
                    tax_class: self.tax.class_for(p.code).id.clone(),
                    tickets: p.tickets,
                    fresh: p.fresh,
                    dates: p.dates,
                })
                .collect(),
            tax: TaxView {
                classes: self
                    .tax
                    .classes
                    .iter()
                    .map(|c| TaxClassView {
                        id: c.id.clone(),
                        name: c.name.clone(),
                        kind: c.kind,
                        kind_label: c.kind.label(),
                        rate_bp: c.rate_bp,
                        rate: tax::rate_label(c.rate_bp),
                    })
                    .collect(),
                invoice_no: self.tax.invoice_no.clone(),
                rounding: self.tax.rounding,
            },
            categories: Category::ALL.iter().map(|c| (*c, c.label())).collect(),
            emoney_balance: self.emoney_balance,
        }
    }

    fn ff_menu(&self) -> Vec<FfMenuItem> {
        FF_MENU
            .iter()
            .filter_map(|&(code, tab)| {
                let p = catalog::find(code)?;
                let level = if catalog::is_made_to_order(code) {
                    StockLevel::MadeToOrder
                } else {
                    StockLevel::of(self.ff_stock.iter().find(|(c, _)| *c == code)?.1)
                };
                Some(FfMenuItem {
                    code: p.code,
                    name: p.name,
                    price: p.price,
                    level,
                    tab,
                    tab_label: tab.label(),
                })
            })
            .collect()
    }

    fn register_view(&self, r: &Register) -> RegisterView {
        let c = r.context(&self.age_rules);
        let t = &r.txn;
        let taxes = t.taxes(&self.tax.display_rates(), self.tax.rounding);
        let c_side = active_side(&c).unwrap_or(Side::Customer);
        let actions = |side: Side, list: &[Action]| {
            list.iter()
                .map(|&a| {
                    let side = fixed_side(a).unwrap_or(side);
                    let res = permitted(side, a, &c);
                    ActionState {
                        action: a,
                        allowed: res.is_ok(),
                        reason: res.err(),
                    }
                })
                .collect::<Vec<_>>()
        };
        let total = t.total();
        let inserted = t.inserted.total();
        let settle_in_ms = (t.paying
            && t.method == Some(PaymentMethod::Cash)
            && r.emoney.is_none()
            && inserted > 0
            && inserted >= t.due())
        .then(|| (t.inserted_at + INSERT_DONE_MS).saturating_sub(self.now));
        let coupons: Vec<CouponView> = t
            .coupon_uses()
            .into_iter()
            .map(|(id, c, u)| CouponView {
                id,
                code: c.code,
                name: c.name,
                discount: u.map(|(yen, _)| yen),
            })
            .collect();
        let returned: Vec<ReturnedCoupon> = t
            .returned
            .iter()
            .map(|&(name, unmatched)| ReturnedCoupon { name, unmatched })
            .collect();
        let calls = self.calls.for_register(r.id);
        let staff_called = !calls.is_empty() && c.staff != StaffState::Ready;
        let waiting = staff_called
            .then(|| calls.iter().map(|c| c.created_at).min())
            .flatten()
            .map(|t0| self.now.saturating_sub(t0));
        let staff = r.staff.map(|s| s.account);
        let bell_down = self
            .bells
            .iter()
            .filter(|m| m.bell_on && !m.link_ok(self.now))
            .map(|m| m.id.clone())
            .collect();

        let lines: Vec<StaffLine> = t
            .lines
            .iter()
            .map(|l| StaffLine {
                line: l.clone(),
                amount: l.amount(),
                age: self.age_rules.age_for(l.code, l.category),
                age_checked: t.line_age_checked(self.age_rules.age_for(l.code, l.category)),
                limit: self.age_rules.limit_for(l.code),
                fresh: l.is_fresh(),
                expired: l.is_expired(),
                dates: catalog::find(l.code).and_then(|p| p.dates),
                tickets: l.tickets(),
            })
            .collect();

        let customer_lines = t
            .lines
            .iter()
            .map(|l| CustomerLine {
                id: l.id,
                touched: l.touched,
                name: l.name,
                qty: l.qty,
                ff_base: l.ff_base,
                amount: l.amount(),
                age: self.age_rules.age_for(l.code, l.category),
                age_checked: t.line_age_checked(self.age_rules.age_for(l.code, l.category)),
                limit: self.age_rules.limit_for(l.code),
                cash_only: l.cash_only,
                pickup: l.is_pickup(),
                parcel: l.is_parcel(),
                fresh: l.is_fresh(),
                expired: l.is_expired(),
                dates: catalog::find(l.code).and_then(|p| p.dates),
                tickets: l.tickets(),
                kind: l.kind,
            })
            .collect();

        let bell_dead = !self.bells.iter().any(|m| m.bell_on && m.link_ok(self.now));
        RegisterView {
            id: r.id,
            bell_dead,
            emergency: r.emergency,
            active_side: active_side(&c),
            max_items: MAX_ITEMS,
            lamp: waiting.is_some(),
            call_waiting_ms: waiting,
            ring_volume: waiting.map_or(0.0, ring_volume),
            tobacco_asked: r.mode == Mode::StaffOperated && t.tobacco_asked,
            mode: r.mode,
            mode_label: r.mode.label(),
            outlet: r.outlet.total(),
            outlet_detail: r.outlet.0.clone(),
            menu: r.menu,
            paused: r.paused,
            idle_voids: !r.idle(),
            idle_revert_in: (r.mode != Mode::SelfOperated || !r.idle())
                .then(|| r.idle_revert_in(self.now)),
            change_left: r
                .outlet_since
                .is_some_and(|t| self.now.saturating_sub(t) >= r.change_timers().0),
            staff: StaffView {
                state: c.staff,
                name: staff.map(|a| a.name),
                role: staff.map(|a| a.role),
                phase: c.phase,
                lines,
                total,
                taxes: taxes.clone(),
                coupons: coupons.clone(),
                point_card: t.point_card.clone(),
                required_age: t.required_age.filter(|_| t.paying || t.age_early),
                age_choice_pending: c.age_choice_pending,
                age_early: t.age_early,
                confiscated: t
                    .confiscated
                    .iter()
                    .map(|&(name, qty)| Confiscated { name, qty })
                    .collect(),
                ff_collect: t
                    .ff_collect
                    .iter()
                    .map(|&(name, qty)| Confiscated { name, qty })
                    .collect(),
                id_pending: c.id_pending,
                coupon_unmatched: c.coupon_unmatched,
                returned: returned.clone(),
                bills: t.bill_rows(),
                bill_unchecked: c.bill_unchecked,
                stamp_pending: c.stamp_pending,
                point_pending: c.point_pending,
                parcel_ask: c.parcel_ask,
                method: t.method,
                inserted,
                settle_in_ms,
                due: t.due().saturating_sub(inserted),
                payment_blocker: c.payment_blocker(),
                calls,
                actions: actions(c_side, ACTIONS),
                drawer_total: r.drawer.cash.total(),
                drawer: r.drawer.cash.0.clone(),
                bell_down,
            },
            customer: CustomerView {
                phase: c.phase,
                lines: customer_lines,
                coupons,
                coupon_unmatched: c.coupon_unmatched,
                returned,
                total,
                taxes: taxes.clone(),
                inserted,
                settle_in_ms,
                due: t.due().saturating_sub(inserted),
                point_card: t.point_card.is_some(),
                point_pending: c.point_pending,
                parcel_ask: c.parcel_ask,
                bills: t.bill_rows(),
                bill_unchecked: c.bill_unchecked,
                stamp_pending: c.stamp_pending,
                staff_called,
                age_wait: t
                    .required_age
                    .filter(|_| c.age_choice_pending && r.mode != Mode::StaffOperated),
                staff_present: c.staff == StaffState::Ready,
                method: t.method,
                payment_blocker: c.payment_blocker(),
                actions: actions(c_side, ACTIONS),
            },
            emoney: r.emoney.as_ref().map(|e| match e {
                EmoneyTask::Menu => EmoneyView {
                    kind: EmoneyKind::Menu,
                    inserted: 0,
                    balance: None,
                },
                EmoneyTask::Inquiry { shown } => EmoneyView {
                    kind: EmoneyKind::Inquiry,
                    inserted: 0,
                    balance: shown.then_some(self.emoney_balance),
                },
                EmoneyTask::Charge { inserted } => EmoneyView {
                    kind: EmoneyKind::Charge,
                    inserted: inserted.total(),
                    balance: None,
                },
            }),
            emoney_short: t.emoney_short.then_some(self.emoney_balance),
            emoney_partial: t
                .emoney_partial
                .then_some(self.emoney_balance.min(t.cashless_due())),
            emoney_partial_ok: t.emoney_partial_ok,
            prepaid: t.prepaid,
            prepaid_method: t
                .prepaid_method
                .filter(|_| t.prepaid > 0)
                .map(PaymentMethod::label),
            cash_only: t.cash_only(),
            cashless_due: t.cashless_due(),
            unavailable: [
                PaymentMethod::Emoney,
                PaymentMethod::Qr,
                PaymentMethod::Credit,
                PaymentMethod::Points,
            ]
            .into_iter()
            .filter_map(|m| {
                t.method_refusal(m)
                    .map(|reason| Unavailable { method: m, reason })
            })
            .collect(),
            last_receipt: r.last_receipt.clone(),
            bill_handover: r.bill_handover,
            ticket_handover: r.ticket_handover.clone(),
            tickets_issued: r.tickets_issued,
            tickets_received: r.tickets_received,
            tickets_handed: r.tickets_handed,
            tickets_ahead: self
                .ticket_queue
                .iter()
                .position(|&id| id == r.id)
                .unwrap_or(0),
            pickup_handover: r.pickup_handover.clone(),
            receipt_stamp: r.receipt_stamp,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ActionState {
    pub action: Action,
    pub allowed: bool,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StaffLine {
    #[serde(flatten)]
    pub line: Line,
    pub amount: u32,
    pub age: Option<u8>,
    pub age_checked: bool,
    pub limit: Option<u32>,
    /// 日配食品（数量を増やせない）
    pub fresh: bool,
    /// 販売期限切れ（回収（取り消す）までお会計に進めない）
    pub expired: bool,
    /// 販売期限・賞味期限（いまからの分）
    pub dates: Option<catalog::Dates>,
    /// チケットの発券枚数（チケットでなければ 0）
    pub tickets: u32,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CustomerLine {
    pub id: u32,
    pub touched: u32,
    pub name: &'static str,
    pub qty: u32,
    pub ff_base: u32,
    pub amount: u32,
    pub age: Option<u8>,
    pub age_checked: bool,
    pub limit: Option<u32>,
    pub cash_only: bool,
    pub pickup: bool,
    pub parcel: bool,
    pub fresh: bool,
    pub expired: bool,
    pub dates: Option<catalog::Dates>,
    pub tickets: u32,
    #[serde(flatten)]
    pub kind: LineKind,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StaffView {
    pub state: StaffState,
    pub name: Option<&'static str>,
    pub role: Option<Role>,
    pub phase: Phase,
    pub lines: Vec<StaffLine>,
    pub total: u32,
    pub taxes: Vec<TaxTotal>,
    pub coupons: Vec<CouponView>,
    pub point_card: Option<String>,
    pub required_age: Option<u8>,
    pub age_choice_pending: bool,
    pub age_early: bool,
    pub confiscated: Vec<Confiscated>,
    pub ff_collect: Vec<Confiscated>,
    pub coupon_unmatched: bool,
    pub returned: Vec<ReturnedCoupon>,
    pub id_pending: bool,
    pub bills: Vec<BillRow>,
    pub bill_unchecked: bool,
    pub stamp_pending: bool,
    pub point_pending: bool,
    pub parcel_ask: bool,
    pub method: Option<PaymentMethod>,
    pub inserted: u32,
    pub settle_in_ms: Option<u64>,
    pub due: u32,
    pub payment_blocker: Option<&'static str>,
    pub calls: Vec<Call>,
    pub actions: Vec<ActionState>,
    pub drawer_total: u32,
    pub drawer: Vec<(u32, u32)>,
    pub bell_down: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CustomerView {
    pub phase: Phase,
    pub lines: Vec<CustomerLine>,
    pub coupons: Vec<CouponView>,
    pub coupon_unmatched: bool,
    pub returned: Vec<ReturnedCoupon>,
    pub total: u32,
    pub taxes: Vec<TaxTotal>,
    pub inserted: u32,
    pub settle_in_ms: Option<u64>,
    pub due: u32,
    pub point_card: bool,
    pub point_pending: bool,
    pub parcel_ask: bool,
    pub bills: Vec<BillRow>,
    pub bill_unchecked: bool,
    pub stamp_pending: bool,
    pub staff_called: bool,
    pub age_wait: Option<u8>,
    pub staff_present: bool,
    pub method: Option<PaymentMethod>,
    pub payment_blocker: Option<&'static str>,
    pub actions: Vec<ActionState>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RegisterView {
    pub id: u8,
    /// ON の Bell がどれも通じず、呼び出しが誰にも届かない
    pub bell_dead: bool,
    /// エマージェンシーモード（セルフ操作モードで呼び鈴に応答がない）
    pub emergency: bool,
    pub active_side: Option<Side>,
    pub max_items: u32,
    pub lamp: bool,
    pub call_waiting_ms: Option<u64>,
    pub ring_volume: f32,
    pub tobacco_asked: bool,
    pub mode: Mode,
    pub mode_label: &'static str,
    pub outlet: u32,
    pub outlet_detail: Vec<(u32, u32)>,
    pub menu: Option<Menu>,
    pub paused: bool,
    pub idle_revert_in: Option<u64>,
    pub idle_voids: bool,
    pub change_left: bool,
    pub staff: StaffView,
    pub customer: CustomerView,
    pub emoney: Option<EmoneyView>,
    pub emoney_short: Option<u32>,
    pub emoney_partial: Option<u32>,
    pub emoney_partial_ok: bool,
    pub prepaid: u32,
    pub prepaid_method: Option<&'static str>,
    pub cash_only: u32,
    pub cashless_due: u32,
    pub unavailable: Vec<Unavailable>,
    pub last_receipt: Option<Receipt>,
    pub bill_handover: Option<u32>,
    pub ticket_handover: Vec<TicketRow>,
    /// チケットを発券した（列の先頭に来た）。false のあいだは発券待ち
    pub tickets_issued: bool,
    /// お客様が入力した受け取り枚数
    pub tickets_received: Option<u32>,
    /// スタッフが入力したお渡し枚数
    pub tickets_handed: Option<u32>,
    /// 発券の列で、このレジより先に並んでいるレジの数
    pub tickets_ahead: usize,
    pub pickup_handover: Option<String>,
    pub receipt_stamp: Option<ReceiptStamp>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Unavailable {
    pub method: PaymentMethod,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EmoneyView {
    pub kind: EmoneyKind,
    pub inserted: u32,
    pub balance: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum BellKind {
    Fixed,
    Handheld,
    StoreComputer,
}

fn bell_kind(i: u8) -> BellKind {
    match i {
        0 => BellKind::Fixed,
        2 => BellKind::StoreComputer,
        _ => BellKind::Handheld,
    }
}

fn bell_name(i: u8) -> String {
    match i {
        0 => "据え付け端末".into(),
        1 => "可搬性端末".into(),
        2 => "ストアコンピューター".into(),
        n => format!("可搬性端末{}", n - 1),
    }
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BellView {
    pub id: String,
    pub kind: BellKind,
    pub buzzing: bool,
    pub link_ok: bool,
    pub rings: u64,
    pub offline: bool,
    pub bell_on: bool,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BellLink {
    pub id: String,
    pub link_ok: bool,
    pub acked_seq: u64,
    pub sent: u64,
    pub retries: u64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AgeRulesView {
    pub rules: Vec<crate::age_rules::AgeRule>,
    pub history: Vec<crate::age_rules::RuleChange>,
}

/// 作り置きがこの数（二桁）以上なら豊富、1〜9 は希薄
pub const FF_PLENTY_STOCK: u32 = 10;
const FF_OUT: &str = "は欠品です（FF ボタンの長押しで予約注文として登録できます）";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum StockLevel {
    /// 作り置きの概念が無い（注文を受けてから作る）
    MadeToOrder,
    Plenty,
    Low,
    Out,
}

impl StockLevel {
    pub fn of(stock: u32) -> Self {
        match stock {
            0 => Self::Out,
            n if n < FF_PLENTY_STOCK => Self::Low,
            _ => Self::Plenty,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FfMenuItem {
    pub code: &'static str,
    pub name: &'static str,
    pub price: u32,
    pub level: StockLevel,
    pub tab: FfTab,
    pub tab_label: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AccountView {
    pub id: &'static str,
    pub barcode: String,
    pub name: &'static str,
    pub role: Role,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CatalogItem {
    pub code: &'static str,
    pub name: &'static str,
    pub price: u32,
    pub category: Category,
    pub category_label: &'static str,
    pub age: Option<u8>,
    pub legal_age: Option<u8>,
    pub store_age: Option<u8>,
    pub limit: Option<u32>,
    pub tax_class: String,
    /// チケットの申込券で発券する枚数（チケットでなければ 0）
    pub tickets: u32,
    /// 日配食品
    pub fresh: bool,
    /// 販売期限・賞味期限（いまからの分）
    pub dates: Option<catalog::Dates>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TaxTotal {
    pub rate_bp: u32,
    pub rate: String,
    pub base: u32,
    pub tax: u32,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TaxClassView {
    pub id: String,
    pub name: String,
    pub kind: TaxKind,
    pub kind_label: &'static str,
    pub rate_bp: u32,
    pub rate: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TaxView {
    pub classes: Vec<TaxClassView>,
    pub invoice_no: String,
    pub rounding: Rounding,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Snapshot {
    pub now: u64,
    pub registers: Vec<RegisterView>,
    pub bells: Vec<BellView>,
    pub bell_links: Vec<BellLink>,
    pub signal: BellSignal,
    pub calls: Vec<Call>,
    pub call_history: Vec<ResolvedCall>,
    pub age_rules: AgeRulesView,
    pub audit: Vec<AuditEntry>,
    pub ff_stock: Vec<FfMenuItem>,
    pub bell_loss: u8,
    pub accounts: Vec<AccountView>,
    pub catalog: Vec<CatalogItem>,
    pub tax: TaxView,
    pub categories: Vec<(Category, &'static str)>,
    pub emoney_balance: u32,
}
