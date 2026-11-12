use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    StaffOperated,
    SelfOperated,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::StaffOperated => "スタッフ操作モード",
            Mode::SelfOperated => "セルフ操作モード",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Staff,
    Customer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Registering,
    Paying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    Cash,
    Emoney,
    Qr,
    Credit,
    Points,
}

impl PaymentMethod {
    pub fn label(self) -> &'static str {
        match self {
            PaymentMethod::Cash => "現金",
            PaymentMethod::Emoney => "電子マネー",
            PaymentMethod::Qr => "バーコード決済",
            PaymentMethod::Credit => "クレジットカード",
            PaymentMethod::Points => "ポイント利用",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum StaffState {
    Absent,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Scan,
    RegisterFf,
    FfButton,
    CheckFf,
    CompleteService,
    ParcelNext,
    RemoveLine,
    RemoveCoupon,
    Checkout,
    BackToRegistering,
    SelectPayment,
    ConfirmPointCard,
    ChooseAgeCheck,
    EarlyAgeCheck,
    CheckId,
    CheckBill,
    AffixStamp,
    AffixReceiptStamp,
    HandOverBills,
    ReceiveTickets,
    HandTickets,
    InsertCash,
    TapEmoney,
    CallStaff,
    VoidTransaction,
    EndStaffMode,
    Pause,
    CashOut,
    CashIn,
    Emoney,
    ResolveEmoneyShort,
}

pub const ACTIONS: &[Action] = &[
    Action::Scan,
    Action::RegisterFf,
    Action::FfButton,
    Action::CheckFf,
    Action::CompleteService,
    Action::ParcelNext,
    Action::RemoveLine,
    Action::RemoveCoupon,
    Action::Checkout,
    Action::BackToRegistering,
    Action::SelectPayment,
    Action::ConfirmPointCard,
    Action::ChooseAgeCheck,
    Action::EarlyAgeCheck,
    Action::CheckId,
    Action::CheckBill,
    Action::AffixStamp,
    Action::AffixReceiptStamp,
    Action::HandOverBills,
    Action::ReceiveTickets,
    Action::HandTickets,
    Action::InsertCash,
    Action::TapEmoney,
    Action::CallStaff,
    Action::VoidTransaction,
    Action::EndStaffMode,
    Action::Pause,
    Action::CashOut,
    Action::CashIn,
    Action::Emoney,
    Action::ResolveEmoneyShort,
];

#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub mode: Mode,
    pub phase: Phase,
    pub staff: StaffState,
    pub has_lines: bool,
    pub age_choice_pending: bool,
    pub id_pending: bool,
    pub age_unchecked: bool,
    pub age_early: bool,
    pub ff_waiting: bool,
    pub has_ff: bool,
    pub service_pending: bool,
    pub parcel_ask: bool,
    pub has_bills: bool,
    pub bill_unchecked: bool,
    /// 収入印紙が要る払込票がある
    pub has_stamps: bool,
    /// 収入印紙の貼り付けがまだ確認できていない払込票がある
    pub stamp_pending: bool,
    pub bill_handover: bool,
    /// 決済後、レシートに収入印紙を貼る確認を待っている
    pub receipt_stamp: bool,
    /// 決済後、発券したチケットのお渡し枚数の照合を待っている
    pub ticket_handover: bool,
    pub point_pending: bool,
    pub has_coupons: bool,
    pub coupon_unmatched: bool,
    pub method: Option<PaymentMethod>,
    pub customer_turn: bool,
    pub menu_open: bool,
    pub touching: Option<Side>,
    pub items_full: bool,
    pub emoney_short: bool,
    pub cash_only_goods: bool,
    /// 販売期限切れの明細がある
    pub expired: bool,
}

pub const MAX_ITEMS: u32 = 100;
pub const ITEMS_FULL: Denial = "買い物点数の上限（100点）に達しています";

pub const TOUCH_LOCK_MS: u64 = 1000;

impl Context {
    pub fn payment_blocker(&self) -> Option<&'static str> {
        if self.emoney_short {
            Some(EMONEY_SHORT)
        } else if self.expired {
            Some(EXPIRED)
        } else if self.bill_unchecked {
            Some(BILL_UNCHECKED)
        } else if self.stamp_pending {
            Some(STAMP_UNCHECKED)
        } else if self.mode == Mode::SelfOperated && self.cash_only_goods {
            Some(SELF_OP_CASH_ONLY_GOODS)
        } else if self.mode == Mode::SelfOperated && self.method == Some(PaymentMethod::Cash) {
            Some(SELF_OP_CASH_CHOSEN)
        } else if self.age_choice_pending {
            Some("スタッフが年齢確認をするか選んでいません")
        } else if self.id_pending {
            Some("スタッフが身分証の生年月日を入力していません")
        } else if self.point_pending {
            Some(POINT_UNCONFIRMED)
        } else if self.coupon_unmatched {
            Some(COUPON_UNMATCHED)
        } else if self.ff_waiting {
            Some(FF_UNCHECKED)
        } else if self.service_pending {
            Some("サービス商品の受付が済んでいません")
        } else if self.parcel_ask {
            Some(PARCEL_ASK)
        } else {
            None
        }
    }
}

pub type Denial = &'static str;

const FF_UNCHECKED: Denial = "FF のお渡し確認（お渡し個数の照合）が済んでいません";
const PARCEL_ASK: Denial = "同じお届け先の荷物があるか選んでください";
pub const EXPIRED: Denial = "販売期限切れの商品があります\n取り消してから、お会計に進んでください";
const POINT_UNCONFIRMED: Denial = "ポイントカードの確認が済んでいません";
const COUPON_UNMATCHED: Denial =
    "適用先のないクーポンがあります\n対象商品を確かめるか、クーポンを取り消してください";
const BILL_UNCHECKED: Denial = "スタッフが公共料金の内容を確認していません";
const STAMP_UNCHECKED: Denial = "収入印紙の貼り付けが確認できていません";
const SELF_OP_CASH_ONLY_GOODS: Denial =
    "スタッフをお呼びしました";
const SELF_OP_NO_CASH: Denial =
    "セルフレジでは現金を使えません\n現金はスタッフがお預かりいたします";
const SELF_OP_CASH_CHOSEN: Denial = "現金はスタッフがお預かりいたします\nスタッフをお呼びしました";
const EMONEY_SHORT: Denial = "電子マネーの残高が足りません\nどうするか選んでください";

fn emoney(c: &Context) -> Result<(), Denial> {
    require(!c.emoney_short, EMONEY_SHORT)
}

fn void_transaction(c: &Context) -> Result<(), Denial> {
    require(c.has_lines || c.has_coupons, "中止する取引はありません")
}

fn resolve_emoney_short(c: &Context) -> Result<(), Denial> {
    require(c.emoney_short, "電子マネーの残高は足りています")
}

fn check_bill(c: &Context) -> Result<(), Denial> {
    require(c.has_bills, "公共料金の払込票はありません")?;
    require(c.phase == Phase::Paying, "会計ボタンを押してから確認します")
}

fn affix_stamp(c: &Context) -> Result<(), Denial> {
    require(c.has_stamps, "収入印紙が必要な払込票はありません")
}

fn affix_receipt_stamp(c: &Context) -> Result<(), Denial> {
    require(c.receipt_stamp, "収入印紙を貼るレシートはありません")
}

fn hand_over_bills(c: &Context) -> Result<(), Denial> {
    require(c.bill_handover, "お渡しする払込票はありません")
}

fn check_tickets(c: &Context) -> Result<(), Denial> {
    require(c.ticket_handover, "お渡しするチケットはありません")
}

pub fn is_staff_only(action: Action) -> bool {
    matches!(
        action,
        Action::ChooseAgeCheck
            | Action::EarlyAgeCheck
            | Action::CheckId
            | Action::CheckBill
            | Action::AffixStamp
            | Action::AffixReceiptStamp
            | Action::HandOverBills
            | Action::CheckFf
            | Action::HandTickets
    )
}

/// お客様側の画面でだけ行う操作（チケットの受け取り枚数の入力）
pub fn is_customer_only(action: Action) -> bool {
    action == Action::ReceiveTickets
}

pub fn fixed_side(action: Action) -> Option<Side> {
    if is_staff_only(action) {
        Some(Side::Staff)
    } else if is_customer_only(action) {
        Some(Side::Customer)
    } else {
        None
    }
}

fn choose_age_check(c: &Context) -> Result<(), Denial> {
    require(c.phase == Phase::Paying || c.age_early, "お会計前です")?;
    require(c.age_choice_pending || c.id_pending, "年齢確認は不要です")
}

fn early_age_check(c: &Context) -> Result<(), Denial> {
    require(
        c.phase == Phase::Registering,
        "お会計中は会計の流れで年齢確認をします",
    )?;
    require(
        c.age_unchecked || c.age_early,
        "年齢確認の済んでいない商品はありません",
    )
}

fn check_id(c: &Context) -> Result<(), Denial> {
    require(c.phase == Phase::Paying || c.age_early, "お会計前です")?;
    require(!c.age_choice_pending, "先に年齢確認をするか選んでください")?;
    require(c.id_pending, "生年月日の入力は不要です")
}

fn confirm_point(c: &Context) -> Result<(), Denial> {
    require(c.phase == Phase::Paying, "お会計前です")?;
    require(
        !(c.age_choice_pending || c.id_pending),
        "先に年齢確認をしてください",
    )?;
    require(c.point_pending, "ポイントカードの確認は済んでいます")
}

pub fn active_side(c: &Context) -> Option<Side> {
    if c.menu_open {
        return Some(Side::Staff);
    }
    if c.staff != StaffState::Ready {
        return Some(Side::Customer);
    }
    if c.customer_turn {
        return Some(Side::Customer);
    }
    c.touching
}

pub fn is_device_action(action: Action) -> bool {
    matches!(
        action,
        Action::Scan | Action::InsertCash | Action::TapEmoney | Action::EndStaffMode
    )
}

pub fn permitted(side: Side, action: Action, c: &Context) -> Result<(), Denial> {
    rules(action, c)?;
    if let Some(fixed) = fixed_side(action) {
        return if side == fixed {
            Ok(())
        } else if matches!(action, Action::CheckBill | Action::HandOverBills) {
            Err("払込票の確認・お渡しはスタッフ側の画面で行います")
        } else if matches!(action, Action::AffixStamp | Action::AffixReceiptStamp) {
            Err("収入印紙の貼り付け確認はスタッフ側の画面で行います")
        } else if action == Action::CheckFf {
            Err("FF のお渡し確認はスタッフ側の画面で行います")
        } else if action == Action::HandTickets {
            Err("チケットのお渡し枚数はスタッフ側の画面で入力します")
        } else if action == Action::ReceiveTickets {
            Err("チケットの受け取り枚数はお客様側の画面で入力していただきます")
        } else if is_staff_only(action) {
            Err("年齢確認はスタッフ側の画面で行います")
        } else {
            Err("お客様側の画面で押していただきます")
        };
    }
    if !is_device_action(action) && active_side(c).is_some_and(|s| s != side) {
        return Err("もう一方の画面で操作中です");
    }
    Ok(())
}

pub fn rules(action: Action, c: &Context) -> Result<(), Denial> {
    if c.mode == Mode::StaffOperated {
        staff_operated(action, c)
    } else {
        self_operated(action, c)
    }
}

fn check_ff(c: &Context) -> Result<(), Denial> {
    require(
        c.phase == Phase::Registering,
        "お会計中はお渡し確認を変えられません",
    )?;
    require(c.has_ff, "FF の明細がありません")
}

fn require(cond: bool, reason: Denial) -> Result<(), Denial> {
    if cond {
        Ok(())
    } else {
        Err(reason)
    }
}

fn pay_action(action: Action, c: &Context) -> Result<(), Denial> {
    require(c.phase == Phase::Paying, "お会計前です")?;
    if let Some(r) = c.payment_blocker() {
        return Err(r);
    }
    if action == Action::InsertCash {
        require(
            matches!(c.method, None | Some(PaymentMethod::Cash)),
            "現金以外のお支払いが選ばれています",
        )
    } else {
        require(
            matches!(
                c.method,
                Some(
                    PaymentMethod::Emoney
                        | PaymentMethod::Qr
                        | PaymentMethod::Credit
                        | PaymentMethod::Points
                )
            ),
            "キャッシュレスの支払方法が選ばれていません",
        )
    }
}

fn staff_operated(action: Action, c: &Context) -> Result<(), Denial> {
    let paying = c.phase == Phase::Paying;
    match action {
        Action::Scan | Action::EndStaffMode => Ok(()),
        Action::RegisterFf => {
            require(!paying, "お会計中です")?;
            require(!c.items_full, ITEMS_FULL)
        }
        Action::Pause => require(
            !c.has_lines && !c.has_coupons,
            "取引中は休止できません（取引を終えるか中止してください）",
        ),
        Action::CashOut => require(!c.has_lines, "取引中は出金操作ができません"),
        Action::CashIn => require(!c.has_lines, "取引中は入金操作ができません"),
        Action::CheckFf => check_ff(c),
        Action::CompleteService => require(c.service_pending, "受付待ちのサービス商品はありません"),
        Action::ParcelNext => require(c.parcel_ask, "同じお届け先の荷物をお尋ねする場面ではありません"),
        Action::RemoveLine => {
            require(!paying, "お会計中です")?;
            require(c.has_lines, "明細がありません")
        }
        Action::RemoveCoupon => require(c.has_coupons, "クーポンはありません"),
        Action::VoidTransaction => void_transaction(c),
        Action::Checkout => {
            require(!paying, "すでに会計画面です")?;
            require(c.has_lines, "明細がありません")?;
            require(!c.expired, EXPIRED)?;
            require(!c.ff_waiting, FF_UNCHECKED)?;
            require(!c.service_pending, "サービス商品の受付が済んでいません")?;
            require(!c.parcel_ask, PARCEL_ASK)
        }
        Action::BackToRegistering => require(paying, "会計画面ではありません"),
        Action::SelectPayment => {
            require(paying, "会計画面ではありません")?;
            require(!c.point_pending, POINT_UNCONFIRMED)
        }
        Action::ConfirmPointCard => confirm_point(c),
        Action::ChooseAgeCheck => choose_age_check(c),
        Action::EarlyAgeCheck => early_age_check(c),
        Action::CheckId => check_id(c),
        Action::CheckBill => check_bill(c),
        Action::AffixStamp => affix_stamp(c),
        Action::AffixReceiptStamp => affix_receipt_stamp(c),
        Action::HandOverBills => hand_over_bills(c),
        Action::ReceiveTickets | Action::HandTickets => check_tickets(c),
        Action::InsertCash | Action::TapEmoney => pay_action(action, c),
        Action::FfButton => Err("FF はタブから登録します"),
        Action::CallStaff => Ok(()),
        Action::Emoney => emoney(c),
        Action::ResolveEmoneyShort => resolve_emoney_short(c),
    }
}

fn self_operated(action: Action, c: &Context) -> Result<(), Denial> {
    let paying = c.phase == Phase::Paying;
    match action {
        Action::Scan => Ok(()),
        Action::FfButton => Ok(()),
        Action::RegisterFf => Err("FF はスタッフにお申し付けください"),
        Action::CheckFf => {
            check_ff(c)?;
            Err("FF のお渡し確認はスタッフがログインしてから行います")
        }
        Action::RemoveLine => {
            require(!paying, "お会計中です")?;
            require(c.has_lines, "明細がありません")
        }
        Action::RemoveCoupon => require(c.has_coupons, "クーポンはありません"),
        Action::Checkout => {
            require(!paying, "すでにお会計中です")?;
            require(c.has_lines, "商品を登録してください")?;
            require(!c.expired, EXPIRED)?;
            require(!c.ff_waiting, FF_UNCHECKED)?;
            require(
                !c.service_pending,
                "スタッフがサービス商品の受付をするまでお待ちください",
            )
        }
        Action::BackToRegistering => require(paying, "お会計中ではありません"),
        Action::SelectPayment => {
            require(paying, "お会計前です")?;
            require(!c.point_pending, POINT_UNCONFIRMED)
        }
        Action::ConfirmPointCard => confirm_point(c),
        Action::ChooseAgeCheck | Action::CheckId | Action::EarlyAgeCheck => {
            if action == Action::EarlyAgeCheck {
                early_age_check(c)?;
            } else {
                choose_age_check(c)?;
            }
            Err("年齢確認はスタッフがログインしてから行います")
        }
        Action::CheckBill => {
            check_bill(c)?;
            Err("払込票の確認はスタッフがログインしてから行います")
        }
        Action::AffixReceiptStamp => {
            affix_receipt_stamp(c)?;
            Err("収入印紙の貼り付け確認はスタッフがログインしてから行います")
        }
        Action::AffixStamp => {
            affix_stamp(c)?;
            Err("収入印紙の貼り付け確認はスタッフがログインしてから行います")
        }
        Action::HandOverBills => hand_over_bills(c),
        Action::ReceiveTickets => check_tickets(c),
        Action::HandTickets => {
            check_tickets(c)?;
            Err("チケットのお渡し枚数はスタッフがログインしてから入力します")
        }
        Action::InsertCash => Err(SELF_OP_NO_CASH),
        Action::TapEmoney => pay_action(action, c),
        Action::CallStaff => Ok(()),
        Action::Emoney => emoney(c),
        Action::ResolveEmoneyShort => resolve_emoney_short(c),
        Action::VoidTransaction => void_transaction(c),
        Action::CompleteService
        | Action::ParcelNext
        | Action::EndStaffMode
        | Action::Pause
        | Action::CashOut
        | Action::CashIn => Err("スタッフの操作です（スタッフバーコードでログインしてください）"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contexts() -> Vec<Context> {
        let mut v = vec![];
        let b = [false, true];
        for mode in [Mode::StaffOperated, Mode::SelfOperated] {
            for phase in [Phase::Registering, Phase::Paying] {
                for has_lines in b {
                    for age_choice_pending in b {
                        for (ff_waiting, has_ff) in [(false, false), (false, true), (true, true)] {
                            for (service_pending, bill_unchecked) in
                                [(false, false), (true, false), (false, true)]
                            {
                                for method in
                                    [None, Some(PaymentMethod::Cash), Some(PaymentMethod::Emoney)]
                                {
                                    for customer_turn in b {
                                        v.push(Context {
                                            menu_open: false,
                                            point_pending: false,
                                            has_coupons: false,
                                            coupon_unmatched: false,
                                            touching: None,
                                            items_full: false,
                                            emoney_short: false,
                                            cash_only_goods: false,
                                            expired: false,
                                            customer_turn,
                                            mode,
                                            phase,
                                            staff: StaffState::Ready,
                                            has_lines,
                                            id_pending: false,
                                            age_unchecked: false,
                                            age_early: false,
                                            age_choice_pending: age_choice_pending
                                                && phase == Phase::Paying,
                                            ff_waiting,
                                            has_ff,
                                            service_pending,
                                            parcel_ask: false,
                                            has_bills: bill_unchecked,
                                            bill_unchecked,
                                            has_stamps: false,
                                            stamp_pending: false,
                                            bill_handover: false,
                                            receipt_stamp: false,
                                            ticket_handover: false,
                                            method,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        v
    }

    fn touch_states(base: Context) -> impl Iterator<Item = Context> {
        [None, Some(Side::Staff), Some(Side::Customer)]
            .into_iter()
            .map(move |touching| Context { touching, ..base })
    }

    #[test]
    fn touching_one_screen_locks_the_other() {
        for base in contexts() {
            for c in touch_states(base).filter(|c| c.touching.is_some()) {
                let any = |side: Side| {
                    ACTIONS.iter().any(|&a| {
                        !is_device_action(a)
                            && fixed_side(a).is_none()
                            && permitted(side, a, &c).is_ok()
                    })
                };
                assert!(!(any(Side::Staff) && any(Side::Customer)), "{c:?}");
            }
        }
    }

    #[test]
    fn both_screens_can_do_the_same_except_customer_only_buttons() {
        for base in contexts() {
            for c in touch_states(base) {
                let active = active_side(&c);
                for &a in ACTIONS {
                    for side in [Side::Staff, Side::Customer] {
                        let expect = rules(a, &c).is_ok()
                            && if let Some(fixed) = fixed_side(a) {
                                side == fixed
                            } else {
                                is_device_action(a) || active.is_none_or(|s| s == side)
                            };
                        assert_eq!(
                            permitted(side, a, &c).is_ok(),
                            expect,
                            "{a:?} {side:?} {c:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn staff_operated_ff_from_either_screen_when_nobody_touches() {
        for c in contexts().into_iter().filter(|c| {
            c.mode == Mode::StaffOperated && !c.customer_turn && c.phase == Phase::Registering
        }) {
            assert_eq!(active_side(&c), None);
            for side in [Side::Staff, Side::Customer] {
                assert!(permitted(side, Action::RegisterFf, &c).is_ok());
            }
        }
    }

    #[test]
    fn payment_is_gated_until_blockers_clear() {
        for c in contexts().into_iter().filter(|c| c.phase == Phase::Paying) {
            let blocked =
                c.age_choice_pending || c.bill_unchecked || c.ff_waiting || c.service_pending;
            for a in [Action::InsertCash, Action::TapEmoney] {
                if blocked {
                    assert!(permitted(Side::Customer, a, &c).is_err());
                }
            }
        }
    }

    #[test]
    fn bill_check_is_done_by_staff_on_the_staff_side() {
        let unchecked: Vec<_> = contexts()
            .into_iter()
            .filter(|c| c.bill_unchecked && c.phase == Phase::Paying)
            .collect();
        assert!(!unchecked.is_empty());
        for c in unchecked {
            assert!(permitted(Side::Customer, Action::CheckBill, &c).is_err());
            assert_eq!(
                permitted(Side::Staff, Action::CheckBill, &c).is_ok(),
                c.mode == Mode::StaffOperated,
                "{c:?}"
            );
            assert!(c.payment_blocker().is_some());
        }
        for c in contexts().into_iter().filter(|c| !c.has_bills) {
            assert!(permitted(Side::Staff, Action::CheckBill, &c).is_err());
        }
    }
}
