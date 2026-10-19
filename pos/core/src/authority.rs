//! 操作権のモデル（問題3・5への対策）。
//!
//! レジには画面が二つ（店員側・お客様側）あるが、**画面ごとの役割の区別はない**。
//! 二つの画面は一つの買い物画面（情報空間）を同じ内容で映し、どちらからでも操作できる。
//! 排他は触れている一瞬だけ：どちらかが操作した直後のわずかな間（[`TOUCH_LOCK_MS`]）、
//! もう一方からの操作は利かない。どちらも触れていなければ、両方の画面が操作できる。
//!
//! 何ができるかは **モード** だけで決まる。名札でログインした係員がいれば有人モード、
//! いなければセミセルフモード（[`rules`]）。どちらの画面から操作するかは関係ない。
//! 例外はお客様本人が押すボタン（年齢確認・代行収納の内容確認）で、お客様側の
//! 画面からしか押せない（問題5、[`is_customer_only`]）。
//!
//! 片方の画面だけが操作できる状態は純粋関数 [`active_side`] で決まる。店舗メニューは
//! お客様側、セミセルフ（係員なし）もお客様側。有人モードでは、触れている一瞬だけ
//! その側に限られ、それ以外は両方（`None`）。
//!
//! スキャナー・現金投入口・電子マネー端末・名札は画面ではない（機器）ので排他の対象外。
//!
//! 画面のボタンも [`permitted`] の結果から作り、コアは同じ関数で各操作を検証する。
//! 「画面に見えているから操作できる」という状態は生まれない。

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// 有人：名札でログインした係員がいる
    Attended,
    /// セミセルフ：係員なし。お客様が登録して支払う（FF・サービス・たばこは係員を呼ぶ）
    SemiSelf,
    /// セルフ：お客様が登録から支払いまで行い、店員は MAT の呼び出しで駆けつける
    FullSelf,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Attended => "有人",
            Mode::SemiSelf => "セミセルフ",
            Mode::FullSelf => "セルフ",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Staff,
    Customer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// 商品登録中（明細が空なら待機中）
    Registering,
    /// 会計画面
    Paying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    Cash,
    Emoney,
    Qr,
    Credit,
}

impl PaymentMethod {
    pub fn label(self) -> &'static str {
        match self {
            PaymentMethod::Cash => "現金",
            PaymentMethod::Emoney => "電子マネー",
            PaymentMethod::Qr => "バーコード決済",
            PaymentMethod::Credit => "クレジットカード",
        }
    }
}

/// 店員側の状態。`Ready` のときが「店員操作モード」。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaffState {
    Absent,
    Locked,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Scan,
    RegisterFf,
    FfButton,
    /// FF の明細のお渡し確認チェック（入れる・外す）
    CheckFf,
    CompleteService,
    RemoveLine,
    Checkout,
    BackToRegistering,
    SelectPayment,
    /// 会計時のポイントカードの最終確認（「ポイントカードなしで進む」）
    ConfirmPointCard,
    ConfirmAge,
    /// 年確商品があるとき、店員が「年齢確認をする／しない」を選ぶ
    ChooseAgeCheck,
    /// 「する」を選んだ後、店員が身分証の生年月日を入力して判定する
    CheckId,
    /// 代行収納の支払い内容・金額の確認（お客様の承認）
    ConfirmBillPayment,
    InsertCash,
    TapEmoney,
    CallStaff,
    VoidTransaction,
    SetMode,
    Lock,
    EndStaffMode,
    CashOut,
    CashIn,
}

/// 画面のボタンになる操作。どちらの画面にも同じボタンが並ぶ（店員画面・お客様画面の区別はない）
pub const ACTIONS: &[Action] = &[
    Action::Scan,
    Action::RegisterFf,
    Action::FfButton,
    Action::CheckFf,
    Action::CompleteService,
    Action::RemoveLine,
    Action::Checkout,
    Action::BackToRegistering,
    Action::SelectPayment,
    Action::ConfirmPointCard,
    Action::ConfirmAge,
    Action::ChooseAgeCheck,
    Action::CheckId,
    Action::ConfirmBillPayment,
    Action::InsertCash,
    Action::TapEmoney,
    Action::CallStaff,
    Action::VoidTransaction,
    Action::SetMode,
    Action::Lock,
    Action::EndStaffMode,
    Action::CashOut,
    Action::CashIn,
];

/// 判定に必要な取引の状態。
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub mode: Mode,
    pub phase: Phase,
    pub staff: StaffState,
    pub has_lines: bool,
    /// 会計時の年確で、店員がまだ「年齢確認をする／しない」を選んでいない
    pub age_choice_pending: bool,
    /// 「しない」を選び、お客様の「X歳以上です」待ち
    pub age_pending: bool,
    /// 「する」を選び、店員の生年月日の入力待ち
    pub id_pending: bool,
    /// お渡し確認のチェックが入っていない FF がある
    pub ff_waiting: bool,
    /// FF の明細がある
    pub has_ff: bool,
    /// 店員の受付が済んでいないサービス商品がある
    pub service_pending: bool,
    /// 代行収納の支払い内容・金額を、お客様がまだ確認していない
    pub bill_pending: bool,
    /// 会計に進んだが、ポイントカードの最終確認がまだ
    pub point_pending: bool,
    pub method: Option<PaymentMethod>,
    /// セミセルフで、店員が会計に進めてお客様に支払いを任せている
    pub customer_turn: bool,
    /// 店員がお客様画面で店舗メニュー（入出金・両替）を開いている
    pub menu_open: bool,
    /// いま触れている側（操作した直後の一瞬だけ。その間はもう一方からの操作が利かない）
    pub touching: Option<Side>,
    /// 買い物点数が上限（[`MAX_ITEMS`]）に達している
    pub items_full: bool,
}

/// 一回のお買い物で登録できる点数（数量の合計）の上限。
/// Janus POS はセミセルフ POS の模範的なありかたを探るシミュレーターなので、
/// 画面の表示エリアに収まる点数に絞る
pub const MAX_ITEMS: u32 = 10;
pub const ITEMS_FULL: Denial = "買い物点数の上限（10点）に達しています";

/// 操作した側が「触れている」とみなす時間。この間だけ、もう一方の画面は操作できない
pub const TOUCH_LOCK_MS: u64 = 1000;

impl Context {
    /// 決済を止めている理由（なければ None）。決済の関門はここに集める。
    pub fn payment_blocker(&self) -> Option<&'static str> {
        if self.point_pending {
            Some(POINT_UNCONFIRMED)
        } else if self.age_choice_pending {
            Some("店員が年齢確認をするか選んでいません")
        } else if self.age_pending {
            Some("年齢確認が済んでいません")
        } else if self.id_pending {
            Some("店員が身分証の生年月日を入力していません")
        } else if self.bill_pending {
            Some(BILL_UNCONFIRMED)
        } else if self.ff_waiting {
            Some(FF_UNCHECKED)
        } else if self.service_pending {
            Some("サービス商品の受付が済んでいません")
        } else {
            None
        }
    }
}

pub type Denial = &'static str;

const FF_UNCHECKED: Denial = "FF のお渡し確認にチェックが入っていません";
const POINT_UNCONFIRMED: Denial = "ポイントカードの確認が済んでいません";
const BILL_UNCONFIRMED: Denial = "お客様が代行収納の支払い内容・金額を確認していません";

/// 会計時の代行収納の内容・金額の確認（どちらのモードでも、お客様が押す）
fn confirm_bill(c: &Context) -> Result<(), Denial> {
    require(
        c.phase == Phase::Paying && c.bill_pending,
        "確認が必要な代行収納はありません",
    )
}

/// お客様本人の意思表示・申告にあたる操作。お客様側の画面からしか押せず、
/// どちらの画面が操作側でも押せる（画面の排他の例外）。
///
/// いまは年齢確認と代行収納の内容・金額の確認の二つ。同じ性質の操作として、
/// クレジット・デビットの暗証番号入力（現実には専用端末で行うため扱わない）や
/// 宅配便の危険物・禁制品がないことの申告（ラベル・ステッカーで行うため扱わない）がある。
pub fn is_customer_only(action: Action) -> bool {
    matches!(action, Action::ConfirmAge | Action::ConfirmBillPayment)
}

/// 店員本人が判断・確認する操作。店員側の画面からしか押せず、どちらの画面が操作側でも押せる。
/// いまは年確の「する／しない」の選択と、身分証の生年月日の入力。
pub fn is_staff_only(action: Action) -> bool {
    matches!(action, Action::ChooseAgeCheck | Action::CheckId)
}

/// その操作を押せる画面が決まっているなら、その側（画面の排他の例外）
pub fn fixed_side(action: Action) -> Option<Side> {
    if is_customer_only(action) {
        Some(Side::Customer)
    } else if is_staff_only(action) {
        Some(Side::Staff)
    } else {
        None
    }
}

/// 年確をするか選ぶ（年確が済むまでは選び直せる）
fn choose_age_check(c: &Context) -> Result<(), Denial> {
    require(c.phase == Phase::Paying, "お会計前です")?;
    require(
        c.age_choice_pending || c.age_pending || c.id_pending,
        "年齢確認は不要です",
    )
}

/// 身分証の生年月日の入力（「する」を選んだ後）
fn check_id(c: &Context) -> Result<(), Denial> {
    require(c.phase == Phase::Paying, "お会計前です")?;
    require(!c.age_choice_pending, "先に年齢確認をするか選んでください")?;
    require(c.id_pending, "生年月日の入力は不要です")
}

/// 会計ボタンの後のポイントカードの最終確認（どちらの画面からでも）
fn confirm_point(c: &Context) -> Result<(), Denial> {
    require(c.phase == Phase::Paying, "お会計前です")?;
    require(c.point_pending, "ポイントカードの確認は済んでいます")
}

/// 今、操作できる側が片方に限られているなら、その側（`None` なら両方の画面が操作できる）。
pub fn active_side(c: &Context) -> Option<Side> {
    if c.menu_open {
        // 店舗メニューはお客様画面に出す（釣銭機の出し入れ口が売り場側にあるため）
        return Some(Side::Customer);
    }
    if c.staff != StaffState::Ready {
        // 店員が操作できない（未ログイン・ロック中）ならお客様側
        return Some(Side::Customer);
    }
    if c.customer_turn {
        // セミセルフで店員が会計に進めた後はお客様の支払い。
        // 店員が名札を読む（ログインする）と両方に戻る
        return Some(Side::Customer);
    }
    // 有人モードでは、どちらかが触れている一瞬だけその側に限る
    c.touching
}

/// 画面ではなく機器の操作（排他の対象外）
pub fn is_device_action(action: Action) -> bool {
    matches!(
        action,
        // ログアウト（名札を外す）も画面の操作ではない。ログイン直後はお客様画面が
        // 操作側なので、画面の排他で止めるとセミセルフに戻れなくなる
        Action::Scan | Action::InsertCash | Action::TapEmoney | Action::EndStaffMode
    )
}

/// 操作できるかどうか。
///
/// 何ができるかは **モード（有人・セミセルフ）** だけで決まり、どちらの画面かは関係ない。
/// 二つの画面は同じ内容を映し、操作は排他（操作側の画面だけが押せる）。
/// 例外はお客様本人が押すボタン（年齢確認・代行収納の内容確認。`is_customer_only`）で、
/// これはお客様側の画面からしか押せない。
pub fn permitted(side: Side, action: Action, c: &Context) -> Result<(), Denial> {
    rules(action, c)?;
    if let Some(fixed) = fixed_side(action) {
        // 例外：お客様本人・店員本人が押すもの。決まった側の画面からだけ、
        // どちらの画面が操作側でも押せる
        return if side == fixed {
            Ok(())
        } else if is_staff_only(action) {
            Err("年齢確認は店員側の画面で行います")
        } else if action == Action::ConfirmAge {
            Err("年齢確認はお客様側の画面で押していただきます")
        } else {
            Err("支払い内容・金額の確認はお客様側の画面で押していただきます")
        };
    }
    if !is_device_action(action) && active_side(c).is_some_and(|s| s != side) {
        return Err("もう一方の画面で操作中です");
    }
    Ok(())
}

/// モードごとの操作の可否（画面の区別なし）
pub fn rules(action: Action, c: &Context) -> Result<(), Denial> {
    if c.mode == Mode::Attended {
        attended(action, c)
    } else {
        semi_self(action, c)
    }
}

/// お渡し確認のチェックは登録画面で付け外しする（どちらの画面からでも）
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

/// 支払いの操作（現金投入・カードをかざす）の共通の関門
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
                Some(PaymentMethod::Emoney | PaymentMethod::Qr | PaymentMethod::Credit)
            ),
            "キャッシュレスの支払方法が選ばれていません",
        )
    }
}

/// 有人モード（名札でログインした係員がいる）
fn attended(action: Action, c: &Context) -> Result<(), Denial> {
    let paying = c.phase == Phase::Paying;
    if c.staff == StaffState::Locked
        && !matches!(
            action,
            Action::Scan | Action::EndStaffMode | Action::InsertCash | Action::TapEmoney
        )
    {
        return Err("画面ロック中です（スタッフバーコードで解除）");
    }
    match action {
        Action::Scan | Action::SetMode | Action::Lock | Action::EndStaffMode => Ok(()),
        Action::RegisterFf => {
            require(!paying, "お会計中です")?;
            require(!c.items_full, ITEMS_FULL)
        }
        Action::CashOut => require(!c.has_lines, "取引中は出金操作ができません"),
        Action::CashIn => require(!c.has_lines, "取引中は入金操作ができません"),
        Action::CheckFf => check_ff(c),
        Action::CompleteService => require(c.service_pending, "受付待ちのサービス商品はありません"),
        Action::RemoveLine => {
            require(!paying, "お会計中です")?;
            require(c.has_lines, "明細がありません")
        }
        Action::VoidTransaction => require(c.has_lines, "明細がありません"),
        Action::Checkout => {
            require(!paying, "すでに会計画面です")?;
            require(c.has_lines, "明細がありません")?;
            require(!c.ff_waiting, FF_UNCHECKED)
        }
        Action::BackToRegistering => require(paying, "会計画面ではありません"),
        Action::SelectPayment => {
            require(paying, "会計画面ではありません")?;
            require(!c.point_pending, POINT_UNCONFIRMED)
        }
        Action::ConfirmPointCard => confirm_point(c),
        Action::ConfirmAge => {
            require(paying, "年齢確認は不要です")?;
            require(!c.age_choice_pending, "店員が年齢確認の方法を選んでいます")?;
            require(c.age_pending, "年齢確認は不要です")
        }
        Action::ChooseAgeCheck => choose_age_check(c),
        Action::CheckId => check_id(c),
        Action::ConfirmBillPayment => confirm_bill(c),
        Action::InsertCash | Action::TapEmoney => pay_action(action, c),
        Action::FfButton => Err("FF はタブから登録します"),
        Action::CallStaff => Err("係員が対応中です"),
    }
}

/// セミセルフモード（名札なし。お客様が登録して支払う）。セルフ（将来）は FF も自分で登録する
fn semi_self(action: Action, c: &Context) -> Result<(), Denial> {
    let paying = c.phase == Phase::Paying;
    match action {
        // 会計画面でも受け付け、登録画面に引き返して追加する
        Action::Scan => Ok(()),
        Action::FfButton => {
            require(c.mode == Mode::SemiSelf, "FF はタブから登録してください")?;
            require(!paying, "お会計中です")
        }
        Action::RegisterFf => {
            require(c.mode == Mode::FullSelf, "FF は係員にお申し付けください")?;
            require(!paying, "お会計中です")?;
            require(!c.items_full, ITEMS_FULL)
        }
        Action::CheckFf => check_ff(c),
        Action::RemoveLine => {
            require(!paying, "お会計中です")?;
            require(c.has_lines, "明細がありません")
        }
        Action::Checkout => {
            require(!paying, "すでにお会計中です")?;
            require(c.has_lines, "商品を登録してください")?;
            require(!c.ff_waiting, FF_UNCHECKED)
        }
        Action::BackToRegistering => require(paying, "お会計中ではありません"),
        Action::SelectPayment => {
            require(paying, "お会計前です")?;
            require(!c.point_pending, POINT_UNCONFIRMED)
        }
        Action::ConfirmPointCard => confirm_point(c),
        // 年確は係員が来て（名札でログインして有人モードになって）から
        Action::ConfirmAge => {
            require(
                paying && (c.age_choice_pending || c.age_pending),
                "年齢確認は不要です",
            )?;
            Err("係員が向かっています。しばらくお待ちください")
        }
        Action::ChooseAgeCheck | Action::CheckId => {
            choose_age_check(c)?;
            Err("年齢確認は係員が名札でログインしてから行います")
        }
        Action::ConfirmBillPayment => confirm_bill(c),
        Action::InsertCash | Action::TapEmoney => pay_action(action, c),
        Action::CallStaff => Ok(()),
        Action::CompleteService
        | Action::VoidTransaction
        | Action::SetMode
        | Action::Lock
        | Action::EndStaffMode
        | Action::CashOut
        | Action::CashIn => Err("係員の操作です（名札でログインしてください）"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contexts() -> Vec<Context> {
        let mut v = vec![];
        let b = [false, true];
        for mode in [Mode::Attended, Mode::SemiSelf, Mode::FullSelf] {
            for phase in [Phase::Registering, Phase::Paying] {
                for has_lines in b {
                    for age_pending in b {
                        for (ff_waiting, has_ff) in [(false, false), (false, true), (true, true)] {
                            for (service_pending, bill_pending) in
                                [(false, false), (true, false), (false, true)]
                            {
                                for method in
                                    [None, Some(PaymentMethod::Cash), Some(PaymentMethod::Emoney)]
                                {
                                    for customer_turn in b {
                                        v.push(Context {
                                            menu_open: false,
                                            point_pending: false,
                                            touching: None,
                                            items_full: false,
                                            customer_turn,
                                            mode,
                                            phase,
                                            staff: StaffState::Ready,
                                            has_lines,
                                            age_pending: age_pending && phase == Phase::Paying,
                                            id_pending: false,
                                            age_choice_pending: false,
                                            ff_waiting,
                                            has_ff,
                                            service_pending,
                                            bill_pending: bill_pending && phase == Phase::Paying,
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

    /// 触れている一瞬は排他：その間、もう一方の画面は操作できない（お客様だけが押すボタンを除く）。
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

    /// 有人モードで年確が出ても、年確ボタンはお客様側の画面で押す（係員の画面からは押せない）。
    /// 係員が触れている最中でも同じ。
    #[test]
    fn age_confirmation_is_pressed_on_the_customer_side() {
        for base in contexts()
            .into_iter()
            .filter(|c| c.mode == Mode::Attended && !c.customer_turn && c.age_pending)
        {
            for c in touch_states(base) {
                assert!(permitted(Side::Customer, Action::ConfirmAge, &c).is_ok());
                assert!(permitted(Side::Staff, Action::ConfirmAge, &c).is_err());
            }
        }
    }

    /// 画面の区別はない：どちらも触れていなければ両方の画面で同じことができ、
    /// 触れている一瞬だけその側に限られる。例外はお客様だけが押すボタン。
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

    /// 有人モードでどちらも触れていなければ、FF はどちらの画面からも登録できる
    #[test]
    fn attended_ff_from_either_screen_when_nobody_touches() {
        for c in contexts().into_iter().filter(|c| {
            c.mode == Mode::Attended && !c.customer_turn && c.phase == Phase::Registering
        }) {
            assert_eq!(active_side(&c), None);
            for side in [Side::Staff, Side::Customer] {
                assert!(permitted(side, Action::RegisterFf, &c).is_ok());
            }
        }
    }

    /// 年確・FF・サービス受付が済むまで、どのモードでも決済できない。
    #[test]
    fn payment_is_gated_until_blockers_clear() {
        for c in contexts().into_iter().filter(|c| c.phase == Phase::Paying) {
            let blocked = c.age_pending || c.bill_pending || c.ff_waiting || c.service_pending;
            for a in [Action::InsertCash, Action::TapEmoney] {
                if blocked {
                    assert!(permitted(Side::Customer, a, &c).is_err());
                }
            }
        }
    }

    /// 代行収納の支払い内容・金額の確認も、年確と同じくお客様側の画面からだけ押せる。
    /// 係員が画面を持っていても、セミセルフでも。
    #[test]
    fn bill_payment_confirmation_is_pressed_on_the_customer_side() {
        let pending: Vec<_> = contexts().into_iter().filter(|c| c.bill_pending).collect();
        assert!(!pending.is_empty());
        for c in pending {
            assert!(
                permitted(Side::Customer, Action::ConfirmBillPayment, &c).is_ok(),
                "{c:?}"
            );
            assert!(
                permitted(Side::Staff, Action::ConfirmBillPayment, &c).is_err(),
                "{c:?}"
            );
            assert_eq!(c.payment_blocker().is_some(), true);
        }
        for c in contexts().into_iter().filter(|c| !c.bill_pending) {
            assert!(permitted(Side::Customer, Action::ConfirmBillPayment, &c).is_err());
        }
    }

    #[test]
    fn locked_staff_screen_rejects_everything() {
        for c in contexts() {
            let locked = Context {
                staff: StaffState::Locked,
                ..c
            };
            for &a in ACTIONS
                .iter()
                .filter(|&&a| !is_device_action(a) && a != Action::EndStaffMode)
            {
                if c.mode == Mode::Attended {
                    assert!(permitted(Side::Staff, a, &locked).is_err(), "{a:?}");
                }
            }
        }
    }
}
