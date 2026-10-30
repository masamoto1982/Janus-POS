//! 操作権のモデル（問題3・5への対策）。
//!
//! レジには画面が二つ（店員側・お客様側）あるが、**画面ごとの役割の区別はない**。
//! 二つの画面は一つの買い物画面（情報空間）を同じ内容で映し、どちらからでも操作できる。
//! 排他は触れている一瞬だけ：どちらかが操作した直後のわずかな間（[`TOUCH_LOCK_MS`]）、
//! もう一方からの操作は利かない。どちらも触れていなければ、両方の画面が操作できる。
//!
//! 何ができるかは **モード** だけで決まる。名札でログインした係員がいれば有人モード、
//! いなければセミセルフモード（[`rules`]）。どちらの画面から操作するかは関係ない。
//! 例外は店員本人が判断・確認するボタン（年齢確認・公共料金の払込票の確認とお渡し）で、
//! 店員側の画面からしか押せない（問題5、[`is_staff_only`]）。
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
    /// セミセルフ：係員なし。お客様が登録して支払う（FF・サービス・たばこは係員を呼ぶ）。
    /// 完全なセルフレジはコンビニには合わないので持たない（有人とセミセルフだけ）
    SemiSelf,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Attended => "有人",
            Mode::SemiSelf => "セミセルフ",
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
    /// ポイント利用（ポイントカード・ポイントアプリを入金ボックスにかざす）
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

/// 店員側の状態。`Ready` のときが「店員操作モード」。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaffState {
    Absent,
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
    /// 有人モードで、お客様が FF を受け取ったことを確かめる（お客様側の「確認しました」）。
    /// スタッフ側の照合が済んでから押せる（相互チェック）。両方が済むまで会計に進めない
    ReceiveFf,
    CompleteService,
    /// 荷物の料金が決まった後、次の荷物があるか（「ある」「ない」）を答える
    ParcelNext,
    RemoveLine,
    /// 読み取ったクーポンの取消（お会計中でもできる。取り消したクーポンはお客様に返す）
    RemoveCoupon,
    Checkout,
    BackToRegistering,
    SelectPayment,
    /// 会計時のポイントカードの最終確認（「ポイントカードなしで進む」）
    ConfirmPointCard,
    /// 年確商品があるとき、店員が「年齢確認をする／しない」を選ぶ
    ChooseAgeCheck,
    /// 「する」を選んだ後、店員が身分証の生年月日を入力して判定する
    CheckId,
    /// 公共料金の払込票の内容確認（店員が一枚ずつ事業者・金額を確かめてチェックを入れる。会計ボタンの後）
    CheckBill,
    /// 決済の後、払込票に収納印を押してお渡しした（店員の「お渡し済」）
    HandOverBills,
    InsertCash,
    TapEmoney,
    CallStaff,
    VoidTransaction,
    EndStaffMode,
    /// レジ休止中にする（店舗メニューから。スタッフバーコードで再開する）
    Pause,
    CashOut,
    CashIn,
    /// 電子マネーのチャージ・残高確認を始める・やめる（お買い物の途中でもできる）
    Emoney,
    /// 電子マネーの残高が足りないときの選択（やめる・残りを現金で・チャージする）
    ResolveEmoneyShort,
}

/// 画面のボタンになる操作。どちらの画面にも同じボタンが並ぶ（店員画面・お客様画面の区別はない）
pub const ACTIONS: &[Action] = &[
    Action::Scan,
    Action::RegisterFf,
    Action::FfButton,
    Action::CheckFf,
    Action::ReceiveFf,
    Action::CompleteService,
    Action::ParcelNext,
    Action::RemoveLine,
    Action::RemoveCoupon,
    Action::Checkout,
    Action::BackToRegistering,
    Action::SelectPayment,
    Action::ConfirmPointCard,
    Action::ChooseAgeCheck,
    Action::CheckId,
    Action::CheckBill,
    Action::HandOverBills,
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

/// 判定に必要な取引の状態。
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub mode: Mode,
    pub phase: Phase,
    pub staff: StaffState,
    pub has_lines: bool,
    /// 会計時の年確で、店員がまだ「年齢確認をする／しない」を選んでいない
    pub age_choice_pending: bool,
    /// 「する」を選び、店員の生年月日の入力待ち
    pub id_pending: bool,
    /// お渡し確認のチェックが入っていない FF がある
    pub ff_waiting: bool,
    /// お客様がまだお受け取りを確かめていない FF がある（有人モードの「確認しました」）
    pub ff_unconfirmed: bool,
    /// FF の明細がある
    pub has_ff: bool,
    /// 店員の受付が済んでいないサービス商品がある
    pub service_pending: bool,
    /// 荷物の料金が決まり、「次の荷物はありますか」の答えを待っている
    pub parcel_ask: bool,
    /// 公共料金の払込票がある
    pub has_bills: bool,
    /// 店員がまだ内容を確認していない払込票がある
    pub bill_unchecked: bool,
    /// 公共料金の決済の後、払込票のお渡し待ち
    pub bill_handover: bool,
    /// 会計に進んだが、ポイントカードの最終確認がまだ
    pub point_pending: bool,
    /// 読み取ったクーポンがある
    pub has_coupons: bool,
    /// 会計に進んだが、適用先（対象商品）のないクーポンが残っている
    pub coupon_unmatched: bool,
    pub method: Option<PaymentMethod>,
    /// セミセルフで、店員が会計に進めてお客様に支払いを任せている
    pub customer_turn: bool,
    /// 店舗メニューを開いている側（お客様側のパスワード入力中もお客様側）
    pub menu_side: Option<Side>,
    /// いま触れている側（操作した直後の一瞬だけ。その間はもう一方からの操作が利かない）
    pub touching: Option<Side>,
    /// 買い物点数が上限（[`MAX_ITEMS`]）に達している
    pub items_full: bool,
    /// 電子マネーで払おうとしたが残高が足りず、どうするか選んでいない
    pub emoney_short: bool,
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
        if self.emoney_short {
            Some(EMONEY_SHORT)
        } else if self.bill_unchecked {
            Some(BILL_UNCHECKED)
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
const FF_UNRECEIVED: Denial = "お客様の FF のお受け取りの確認が済んでいません";
const PARCEL_ASK: Denial = "次の荷物があるか選んでください";
const POINT_UNCONFIRMED: Denial = "ポイントカードの確認が済んでいません";
/// 似て非なる商品をクーポンのつもりで定価で買ってしまう事故を防ぐため、決済を止める
const COUPON_UNMATCHED: Denial =
    "適用先のないクーポンがあります。対象商品を確かめるか、クーポンを取り消してください";
const BILL_UNCHECKED: Denial = "スタッフが公共料金の内容を確認していません";
const EMONEY_SHORT: Denial = "電子マネーの残高が足りません。どうするか選んでください";

/// 電子マネーのチャージ・残高確認（残高不足の対応中は、その選択肢から）
fn emoney(c: &Context) -> Result<(), Denial> {
    require(!c.emoney_short, EMONEY_SHORT)
}

/// 取引中止：登録した商品かクーポンがあるときだけ
fn void_transaction(c: &Context) -> Result<(), Denial> {
    require(c.has_lines || c.has_coupons, "中止する取引はありません")
}

fn resolve_emoney_short(c: &Context) -> Result<(), Denial> {
    require(c.emoney_short, "電子マネーの残高は足りています")
}

/// 公共料金の払込票の内容確認。会計ボタンの後、店員が名札でログインしてから
fn check_bill(c: &Context) -> Result<(), Denial> {
    require(c.has_bills, "公共料金の払込票はありません")?;
    require(c.phase == Phase::Paying, "会計ボタンを押してから確認します")
}

/// 払込票のお渡し（決済の後）
fn hand_over_bills(c: &Context) -> Result<(), Denial> {
    require(c.bill_handover, "お渡しする払込票はありません")
}

/// お客様本人の意思表示・申告にあたる操作。お客様側の画面からしか押せず、
/// どちらの画面が操作側でも押せる（画面の排他の例外）。
///
/// 有人モードで FF を受け取ったことの確認（「確認しました」）。同じ性質の操作として、
/// クレジット・デビットの暗証番号入力（現実には専用端末で行うため扱わない）や
/// 宅配便の危険物・禁制品がないことの申告（ラベル・ステッカーで行うため扱わない）がある。
pub fn is_customer_only(action: Action) -> bool {
    matches!(action, Action::ReceiveFf)
}

/// 店員本人が判断・確認する操作。店員側の画面からしか押せず、どちらの画面が操作側でも押せる。
/// 年確の「する／しない」の選択と身分証の生年月日の入力、公共料金の払込票の確認とお渡し。
pub fn is_staff_only(action: Action) -> bool {
    matches!(
        action,
        Action::ChooseAgeCheck | Action::CheckId | Action::CheckBill | Action::HandOverBills
    )
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
    require(c.age_choice_pending || c.id_pending, "年齢確認は不要です")
}

/// 身分証の生年月日の入力（「する」を選んだ後）
fn check_id(c: &Context) -> Result<(), Denial> {
    require(c.phase == Phase::Paying, "お会計前です")?;
    require(!c.age_choice_pending, "先に年齢確認をするか選んでください")?;
    require(c.id_pending, "生年月日の入力は不要です")
}

/// 会計ボタンの後のポイントカードの最終確認。年確が先（年確で販売が成立しなければ無駄な手間になる）
fn confirm_point(c: &Context) -> Result<(), Denial> {
    require(c.phase == Phase::Paying, "お会計前です")?;
    require(
        !(c.age_choice_pending || c.id_pending),
        "先に年齢確認をしてください",
    )?;
    require(c.point_pending, "ポイントカードの確認は済んでいます")
}

/// 今、操作できる側が片方に限られているなら、その側（`None` なら両方の画面が操作できる）。
pub fn active_side(c: &Context) -> Option<Side> {
    if let Some(side) = c.menu_side {
        // 店舗メニューは開いた側の画面に出し、その側で操作する
        return Some(side);
    }
    if c.staff != StaffState::Ready {
        // 店員がいない（未ログイン）ならお客様側
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
/// 例外は店員本人が判断・確認するボタン（年確・払込票の確認とお渡し。`is_staff_only`）で、
/// これは店員側の画面からしか押せない。
pub fn permitted(side: Side, action: Action, c: &Context) -> Result<(), Denial> {
    rules(action, c)?;
    if let Some(fixed) = fixed_side(action) {
        // 例外：お客様本人・店員本人が押すもの。決まった側の画面からだけ、
        // どちらの画面が操作側でも押せる
        return if side == fixed {
            Ok(())
        } else if matches!(action, Action::CheckBill | Action::HandOverBills) {
            Err("払込票の確認・お渡しはスタッフ側の画面で行います")
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

/// 有人モード（名札でログインした係員がいる）
fn attended(action: Action, c: &Context) -> Result<(), Denial> {
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
        Action::ReceiveFf => {
            // スタッフの照合が済んでから、お客様が確かめる（相互チェック）
            check_ff(c)?;
            require(!c.ff_waiting, "スタッフのお渡しの確認が済んでいません")?;
            require(c.ff_unconfirmed, "お受け取りの確認は済んでいます")
        }
        Action::CompleteService => require(c.service_pending, "受付待ちのサービス商品はありません"),
        Action::ParcelNext => require(c.parcel_ask, "次の荷物をお尋ねする場面ではありません"),
        Action::RemoveLine => {
            require(!paying, "お会計中です")?;
            require(c.has_lines, "明細がありません")
        }
        Action::RemoveCoupon => require(c.has_coupons, "クーポンはありません"),
        Action::VoidTransaction => void_transaction(c),
        Action::Checkout => {
            require(!paying, "すでに会計画面です")?;
            require(c.has_lines, "明細がありません")?;
            require(!c.ff_waiting, FF_UNCHECKED)?;
            require(!c.ff_unconfirmed, FF_UNRECEIVED)?;
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
        Action::CheckId => check_id(c),
        Action::CheckBill => check_bill(c),
        Action::HandOverBills => hand_over_bills(c),
        Action::InsertCash | Action::TapEmoney => pay_action(action, c),
        Action::FfButton => Err("FF はタブから登録します"),
        // 一人で手に負えないとき、店員が仲間に SOS を出す（Bell を鳴らすだけで、呼び出しの記録・画面表示はなし）
        Action::CallStaff => Ok(()),
        Action::Emoney => emoney(c),
        Action::ResolveEmoneyShort => resolve_emoney_short(c),
    }
}

/// セミセルフモード（名札なし。お客様が登録して支払う。FF は係員を呼ぶ）
fn semi_self(action: Action, c: &Context) -> Result<(), Denial> {
    let paying = c.phase == Phase::Paying;
    match action {
        // 会計画面でも受け付け、登録画面に引き返して追加する
        Action::Scan => Ok(()),
        // 呼び出しボタンの一つ。サービス・たばこと同じく会計中（年確の呼び出し中も）押せる。
        // 係員は「商品登録に戻る」で登録画面に引き返して FF を追加する
        Action::FfButton => Ok(()),
        Action::RegisterFf => Err("FF はスタッフにお申し付けください"),
        Action::CheckFf => check_ff(c),
        // セミセルフでは FF を登録できない（スタッフがログインして有人モードで登録・お渡しする）
        Action::ReceiveFf => Err("FF のお受け取りの確認は有人モードで行います"),
        Action::RemoveLine => {
            require(!paying, "お会計中です")?;
            require(c.has_lines, "明細がありません")
        }
        Action::RemoveCoupon => require(c.has_coupons, "クーポンはありません"),
        Action::Checkout => {
            require(!paying, "すでにお会計中です")?;
            require(c.has_lines, "商品を登録してください")?;
            require(!c.ff_waiting, FF_UNCHECKED)?;
            // 荷物などのサービス商品は、係員が来て受付を済ませてから
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
        // 年確は係員が来て（名札でログインして有人モードになって）から
        Action::ChooseAgeCheck | Action::CheckId => {
            choose_age_check(c)?;
            Err("年齢確認はスタッフがログインしてから行います")
        }
        // 払込票の確認は係員が来て（名札でログインして有人モードになって）から
        Action::CheckBill => {
            check_bill(c)?;
            Err("払込票の確認はスタッフがログインしてから行います")
        }
        // 決済の後に係員がログアウトしていても、お渡しは済ませられる
        Action::HandOverBills => hand_over_bills(c),
        Action::InsertCash | Action::TapEmoney => pay_action(action, c),
        Action::CallStaff => Ok(()),
        Action::Emoney => emoney(c),
        Action::ResolveEmoneyShort => resolve_emoney_short(c),
        // 取引中止はお客様もできる（誤操作を防ぐため、画面で確認と長押しを挟む）
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
        for mode in [Mode::Attended, Mode::SemiSelf] {
            for phase in [Phase::Registering, Phase::Paying] {
                for has_lines in b {
                    for age_choice_pending in b {
                        for (ff_waiting, has_ff, ff_unconfirmed) in [
                            (false, false, false),
                            (false, true, false),
                            (false, true, true),
                            (true, true, true),
                        ] {
                            for (service_pending, bill_unchecked) in
                                [(false, false), (true, false), (false, true)]
                            {
                                for method in
                                    [None, Some(PaymentMethod::Cash), Some(PaymentMethod::Emoney)]
                                {
                                    for customer_turn in b {
                                        v.push(Context {
                                            menu_side: None,
                                            point_pending: false,
                                            has_coupons: false,
                                            coupon_unmatched: false,
                                            touching: None,
                                            items_full: false,
                                            emoney_short: false,
                                            customer_turn,
                                            mode,
                                            phase,
                                            staff: StaffState::Ready,
                                            has_lines,
                                            id_pending: false,
                                            age_choice_pending: age_choice_pending
                                                && phase == Phase::Paying,
                                            ff_waiting,
                                            ff_unconfirmed,
                                            has_ff,
                                            service_pending,
                                            parcel_ask: false,
                                            has_bills: bill_unchecked,
                                            bill_unchecked,
                                            bill_handover: false,
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
            let blocked =
                c.age_choice_pending || c.bill_unchecked || c.ff_waiting || c.service_pending;
            for a in [Action::InsertCash, Action::TapEmoney] {
                if blocked {
                    assert!(permitted(Side::Customer, a, &c).is_err());
                }
            }
        }
    }

    /// 公共料金の払込票の確認は、年確と同じく店員側の画面からだけ、係員がログインしてから。
    /// 確認が済むまで支払いに進めない
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
                c.mode == Mode::Attended,
                "{c:?}"
            );
            assert!(c.payment_blocker().is_some());
        }
        for c in contexts().into_iter().filter(|c| !c.has_bills) {
            assert!(permitted(Side::Staff, Action::CheckBill, &c).is_err());
        }
    }
}
