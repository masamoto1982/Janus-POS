//! 店舗全体（レジ・MAT・共有設定）の状態と、コマンド処理。
//!
//! レジ 1 台につき情報空間は 1 つだけ。店員画面とお客様画面は、同じ取引を
//! 別の切り口で見せる「ビュー」にすぎない（問題2）。お客様ビューには公開してよい
//! 情報だけを載せる。画面が 1 枚の機種になっても、ビューの出し分けを変えるだけで済む。

use serde::{Deserialize, Serialize};

use crate::age_rules::{AgeRules, RuleTarget};
use crate::auth::{self, Authenticator, Role, ACCOUNTS};
use crate::authority::{
    active_side, fixed_side, permitted, Action, Context, Mode, PaymentMethod, Phase, Side,
    StaffState, ACTIONS, ITEMS_FULL, MAX_ITEMS, TOUCH_LOCK_MS,
};
use crate::calls::{Call, CallKind, Calls, ResolvedCall};
use crate::cash::{is_valid_denomination, Cash, Drawer};
use crate::catalog::{
    self, Category, Coupon, FfTab, Product, Scanned, CATALOG, FF_MENU, INITIAL_FF_STOCK,
};
use crate::mat::{Bell, Mat};
use crate::rng::Rng;
use crate::tax::{self, Rounding, TaxKind, TaxSettings};

pub const MAX_REGISTERS: u8 = 10;
pub const MAX_MATS: u8 = 5;

/// お渡し済みの FF の数量変更・取り消しは店員が行う（増やせばお渡し前に戻り、減らせば回収が要る）
const FF_RECEIVED_SELF: &str = "お渡し済みの FF の数量は係員が変更します。係員をお呼びください";
const CASH_ONLY: &str = "現金のみの商品（切手・お支払い）は現金でお支払いください";
const NO_CASH_MIX: &str =
    "クレジットカード・バーコード決済は現金と併用できません（切手・お支払いは現金のみ）";
const NO_POINTS: &str = "切手・お支払い・たばこにはポイントを使えません";
const EMONEY_NO_CHARGE: &str =
    "お買い物の途中は電子マネーのチャージ・残高確認はできません。お会計のあとにどうぞ";

// ---------------------------------------------------------------------------
// コマンド
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CashOutKind {
    /// 返品返金
    Refund,
    /// 経費支出
    Expense,
    /// 緊急出金
    Emergency,
    /// 途中集金
    Pickup,
    /// 自動釣銭機からの回収
    ChangerCollect,
}

/// 店舗メニュー（稼働モードのバッジから開く）。一度に一つの画面だけを、開いた側の画面に出す。
/// 自動釣銭機へのお金の出し入れはお客様側（出し入れ口が売り場側にある）、それ以外は店員側
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuScreen {
    /// 項目の一覧
    Top,
    Refund,
    Emergency,
    Deposit,
    Expense,
    /// 自動釣銭機（両替）
    Changer,
    /// ジャーナル確認（店員側。仮設）
    Journal,
    /// 廃棄登録（店員側。仮設）
    Waste,
    /// FF 鮮度管理：作成数の登録（店員側。仮設）
    FfFresh,
}

impl MenuScreen {
    /// この画面を出せる側（一覧はどちらにもある）
    fn side(self) -> Option<Side> {
        match self {
            MenuScreen::Top => None,
            MenuScreen::Refund
            | MenuScreen::Emergency
            | MenuScreen::Deposit
            | MenuScreen::Expense
            | MenuScreen::Changer => Some(Side::Customer),
            MenuScreen::Journal | MenuScreen::Waste | MenuScreen::FfFresh => Some(Side::Staff),
        }
    }
}

/// お客様側から店舗メニューを開くためのパスワードの文字（英大文字と数字）と桁数
const MENU_CODE_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const MENU_CODE_LEN: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Menu {
    /// 開いた側の画面（その画面にだけ出す）
    pub side: Side,
    pub screen: MenuScreen,
    /// 両替で受け取る金種
    pub exchange_to: u32,
}

/// 取引外の入金（お金を入れる操作はパスワード不要）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CashInKind {
    /// 取引外入金
    Deposit,
    /// 自動釣銭機への釣銭補充
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

/// 電子マネーの手続き（レジ画面の最下段の右のボタン）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmoneyKind {
    /// 「電子マネー」ボタンを押した直後：チャージか残高確認かを選ぶ
    Menu,
    /// チャージ：紙幣を入金ボックスに入れて、電子マネーをかざす
    Charge,
    /// 残高確認：電子マネーをかざすと残高を出す
    Inquiry,
}

/// 電子マネーの残高が足りないときの選択
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortChoice {
    /// 電子マネーを使うのをやめる（支払方法を選び直す）
    Cancel,
    /// 残高をすべて使い、残りを現金で払う（まず案内を出す）
    Cash,
    /// 「残高を引き出し、残りを現金で」の案内を確認した。ここで初めて電子マネーを受け付ける
    CashConfirmed,
    /// 電子マネーをチャージする（済んだらもう一度かざす）
    Charge,
}

/// 電子マネーの残高の上限（チャージで超えられない）
pub const EMONEY_MAX: u32 = 20_000;
/// シミュレーターの電子マネー（財布に 1 枚）の最初の残高。残高不足を試しやすい額にしておく
pub const EMONEY_INITIAL: u32 = 1_000;

/// 開いている電子マネーの手続き
#[derive(Clone, Debug)]
enum EmoneyTask {
    /// チャージか残高確認かを選ぶ
    Menu,
    /// shown：かざして残高を出した
    Inquiry { shown: bool },
    /// inserted：チャージ用に入れた紙幣
    Charge { inserted: Cash },
}

/// 店員呼び出しボタンのショートカット（FF は FfButton）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallTopic {
    /// 代行収納・宅配便
    Service,
    Tobacco,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Scan {
        register: u8,
        side: Side,
        code: String,
    },
    RegisterFf {
        register: u8,
        /// 省略時は店員側。有人モードではお客様側の画面からも登録できる
        #[serde(default = "staff_side")]
        side: Side,
        code: String,
        qty: u32,
    },
    FfButton {
        register: u8,
    },
    /// FF の明細のお渡し照合。渡した個数を入力するたびに判定し、注文数と合えばお渡し済み、合わなければお渡し前になる
    /// （チェックだけだと数量を意識せずに済ませられ、渡し忘れを防げないため。照合が揃うまで会計に進めない）
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
    RemoveLine {
        register: u8,
        side: Side,
        line_id: u32,
    },
    /// 明細の個数を手で変える（取り消しは RemoveLine）。変えた行は動きがあったので一番上に出る
    SetQty {
        register: u8,
        side: Side,
        line_id: u32,
        qty: u32,
    },
    /// 読み取ったクーポンを取り消す（お会計中でもよい）。取り消したクーポンはお客様に返す
    RemoveCoupon {
        register: u8,
        side: Side,
        coupon_id: u32,
    },
    /// クーポンの返却の案内を閉じる（返却しました／受け取りました）
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
    /// 年確商品があるとき、店員が年齢確認を「する」（身分証の生年月日を入力）か「しない」か選ぶ
    ChooseAgeCheck {
        register: u8,
        #[serde(default = "staff_side")]
        side: Side,
        id_check: bool,
    },
    /// 身分証の生年月日を入力して判定する（YYYY-MM-DD）。today はレジの日付
    CheckId {
        register: u8,
        #[serde(default = "staff_side")]
        side: Side,
        birth: String,
        today: String,
    },
    /// 回収する商品の一覧を閉じる（店員が商品を引き取った）
    DismissConfiscated {
        register: u8,
    },
    /// お渡し済みの FF を減らした・取り消した分の回収の案内を閉じる（店員が「回収しました」）
    DismissFfCollect {
        register: u8,
    },
    /// 代行収納の支払い内容・金額の確認ボタン。年確と同じくお客様側の画面からしか押せない
    ConfirmBillPayment {
        register: u8,
        #[serde(default = "customer_side")]
        side: Side,
    },
    InsertCash {
        register: u8,
        denomination: u32,
        count: u32,
    },
    /// 「投入おわり」（入金ボックスの Enter はその近道）：現金の投入を終え、決済する（投入額が足りなければ受け付けない）
    FinishInsert {
        register: u8,
    },
    /// 財布のカード・スマホを端末にかざす（電子マネー・バーコード決済・クレジットカード）。
    /// 選ばれた支払方法と違うものをかざすと受け付けない（省略時は選ばれた方法）
    TapEmoney {
        register: u8,
        #[serde(default)]
        method: Option<PaymentMethod>,
    },
    /// 電子マネーのチャージ・残高確認を始める（お買い物の途中でもよい）
    StartEmoney {
        register: u8,
        side: Side,
        kind: EmoneyKind,
    },
    /// 電子マネーの手続きをやめる・閉じる（チャージ用に入れた紙幣は出金ボックスに返す）
    CloseEmoney {
        register: u8,
        side: Side,
    },
    /// 電子マネーの残高が足りないときの選択
    ResolveEmoneyShort {
        register: u8,
        side: Side,
        choice: ShortChoice,
    },
    /// 店員呼び出しボタンに触れた。topic はショートカットボタン（サービス・たばこ）。
    /// 省略時は「係員を呼ぶ」。離す（ReleaseCall）まで MAT が鳴る
    CallStaff {
        register: u8,
        #[serde(default)]
        topic: Option<CallTopic>,
    },
    /// 会計時のポイントカードの最終確認で「ポイントカードなし」で進む
    ConfirmPointCard {
        register: u8,
        side: Side,
    },
    /// 呼び出しボタン（FF・サービス・たばこ・係員を呼ぶ）から指を離した
    ReleaseCall {
        register: u8,
    },
    VoidTransaction {
        register: u8,
    },
    EndStaffMode {
        register: u8,
    },
    /// レジ休止中にする：ログアウトし、スタッフバーコードを読むまでどの操作も受け付けない
    Pause {
        register: u8,
    },
    /// 出金（お客様側の店舗メニューに入っていれば、パスワードは要らない）
    CashOut {
        register: u8,
        kind: CashOutKind,
        amount: u32,
    },
    CashIn {
        register: u8,
        kind: CashInKind,
        amount: u32,
    },
    /// 店舗メニューを開く・画面を切り替える（None で閉じる）。店員側はパスワード不要。
    /// お客様側は、開くときだけ店員画面に出た使い捨てのパスワード（`code`）が要る
    OpenMenu {
        register: u8,
        screen: Option<MenuScreen>,
        #[serde(default = "staff_side")]
        side: Side,
        #[serde(default)]
        code: Option<String>,
        #[serde(default)]
        exchange_to: Option<u32>,
    },
    /// お客様側から店舗メニューを開こうとする：使い捨てのパスワードを作り、店員画面にだけ出す
    RequestMenuCode {
        register: u8,
    },
    /// 投入口にお金を入れる（金額で指定）。店舗メニューの入金・両替画面ならそちらに使う
    Insert {
        register: u8,
        amount: u32,
    },
    /// 排出口のお金を取り出す
    TakeOutlet {
        register: u8,
    },
    /// 有人モードで、グレーの画面にタッチして操作を引き継ぐ
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
        min_age: Option<u8>,
    },
    SetSaleLimit {
        owner_id: String,
        password: String,
        code: String,
        limit: Option<u32>,
    },
    /// 商品ルール設定を開くときのオーナー認証（変更は操作のたびにも認証する）
    VerifyOwner {
        owner_id: String,
        password: String,
    },
    /// 税区分の税率を変える（0.01% 単位。1000 = 10%）
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
    SetMatLoss {
        percent: u8,
    },
    SetMatOffline {
        mat: usize,
        offline: bool,
    },
    /// 検品端末・ストアコンピューターの呼び鈴機能の ON/OFF（Bell エリアの歯車から）
    SetBellOn {
        mat: usize,
        on: bool,
    },
}

impl Command {
    /// 対象のレジ（店舗全体の操作なら None）
    pub fn register(&self) -> Option<u8> {
        use Command as C;
        match self {
            C::Scan { register, .. }
            | C::RegisterFf { register, .. }
            | C::FfButton { register, .. }
            | C::CheckFf { register, .. }
            | C::CompleteService { register, .. }
            | C::RemoveLine { register, .. }
            | C::SetQty { register, .. }
            | C::RemoveCoupon { register, .. }
            | C::DismissReturned { register }
            | C::Checkout { register, .. }
            | C::BackToRegistering { register, .. }
            | C::SelectPayment { register, .. }
            | C::ChooseAgeCheck { register, .. }
            | C::CheckId { register, .. }
            | C::DismissConfiscated { register }
            | C::DismissFfCollect { register }
            | C::ConfirmBillPayment { register, .. }
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
            | C::RequestMenuCode { register }
            | C::Insert { register, .. }
            | C::TakeOutlet { register, .. }
            | C::Claim { register, .. } => Some(*register),
            _ => None,
        }
    }

    /// どちらの画面が触られたか（機器の操作と店舗全体の操作は None）
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
            | C::StartEmoney { side, .. }
            | C::CloseEmoney { side, .. }
            | C::ResolveEmoneyShort { side, .. }
            | C::Claim { side, .. }
            | C::OpenMenu { side, .. } => Some(*side),
            // 自動釣銭機への入出金はお客様画面の店舗メニューで行う
            C::FfButton { .. }
            | C::CallStaff { .. }
            | C::RequestMenuCode { .. }
            | C::CashOut { .. }
            | C::CashIn { .. } => Some(Side::Customer),
            C::CompleteService { .. } | C::VoidTransaction { .. } => Some(Side::Staff),
            _ => None,
        }
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

// ---------------------------------------------------------------------------
// 取引
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FfState {
    /// 登録済み・お渡し確認のチェック前
    Waiting,
    /// お渡し確認のチェック済み
    Received,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LineKind {
    Item,
    Ff {
        state: FfState,
    },
    /// `confirm_pending`：受付した内容・金額をお客様がまだ確認していない（代行収納）
    Service {
        done: bool,
        confirm_pending: bool,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct Line {
    pub id: u32,
    /// 最後に動きのあった順番（明細番号と同じ通し番号）。画面は動きの新しい行ほど上に出す
    pub touched: u32,
    pub code: &'static str,
    pub name: &'static str,
    pub unit_price: u32,
    pub qty: u32,
    /// 登録した時点の税区分と税率（あとで設定を変えても、登録済みの明細は変わらない）
    pub tax: String,
    pub rate_bp: u32,
    pub category: Category,
    /// 現金でしか払えない（切手・代行収納）
    pub cash_only: bool,
    #[serde(flatten)]
    pub kind: LineKind,
}

impl Line {
    pub fn amount(&self) -> u32 {
        self.unit_price * self.qty
    }
}

#[derive(Clone, Debug, Default)]
struct Txn {
    /// たばこボタンで呼ばれた店員がログインした（たばこを登録するまで Item のたばこを点滅させる）
    tobacco_asked: bool,
    lines: Vec<Line>,
    next_line: u32,
    paying: bool,
    point_card: Option<String>,
    /// 会計ボタンの時点で求められた年齢（年確不要なら None）
    required_age: Option<u8>,
    /// 読み取った引換券・割引券（取消用の番号つき。対象商品の登録を待つものも含む）。
    /// 同じクーポンを何枚使ってもよい
    coupons: Vec<(u32, &'static Coupon)>,
    /// 取り消したクーポン（お客様に返却する。true は適用先がなかったもの）
    returned: Vec<(&'static str, bool)>,
    /// 店員が選んだ年齢確認のやり方（None はまだ選んでいない）
    age_method: Option<AgeMethod>,
    /// 年確が済んだ年齢（「しない」ならお客様の申告、「する」なら身分証の年齢）
    verified_age: Option<u8>,
    /// 年確で販売できず、買い物から取り除いた商品（店員が回収する）
    confiscated: Vec<(&'static str, u32)>,
    /// お渡し済みの FF を減らした・取り消した分（店員がお客様から回収する。品名と個数）
    ff_collect: Vec<(&'static str, u32)>,
    method: Option<PaymentMethod>,
    /// 会計ボタンの後のポイントカードの最終確認が済んだ
    point_checked: bool,
    inserted: Cash,
    /// セミセルフで店員が会計に進め、お客様に支払いを任せている（店員が名札を読むと解除）
    customer_turn: bool,
    /// キャッシュレスで先に払った額（電子マネーの残高不足で「不足分を現金で払う」を選んだとき、
    /// または現金のみの商品（切手・代行収納）を除いた分をキャッシュレスで払ったとき）。残りは現金で払う
    prepaid: u32,
    /// 先に払ったキャッシュレスの種類
    prepaid_method: Option<PaymentMethod>,
    /// 電子マネーで払おうとしたが残高が足りず、どうするか選んでいない
    emoney_short: bool,
    /// 「不足分を現金で払う」を選んだ（残高を引き出し、残りを現金で払う）
    emoney_partial: bool,
    /// その案内を確認し、電子マネーをかざすのを待っている（確認前にかざしても引き落とさない）
    emoney_partial_ok: bool,
}

impl Txn {
    /// 読み取ったクーポンと、その値引き額（対象商品がまだなければ None）。
    /// 対象商品 1 個につきクーポン 1 枚。読んだ順番（商品より先か後か）は関係ない
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
                // 値引きは対象商品の税率の対象額から引く
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

    /// これから払う額（キャッシュレスで先に払った分を除く）
    fn due(&self) -> u32 {
        self.total().saturating_sub(self.prepaid)
    }

    /// 現金でしか払えない商品（切手・代行収納）の合計
    fn cash_only(&self) -> u32 {
        self.lines
            .iter()
            .filter(|l| l.cash_only)
            .map(Line::amount)
            .sum()
    }

    /// これから払う額のうち、キャッシュレスで払える分（現金のみの商品を除く）
    fn cashless_due(&self) -> u32 {
        self.due().saturating_sub(self.cash_only())
    }

    /// その支払方法が使えない理由（使えれば None）。決済の組み合わせの原則（有人・セミセルフ共通）：
    /// 現金と併用できるのは電子マネーだけ。クレジットカード・バーコード決済は現金と併用できないので、
    /// 現金のみの商品（切手・代行収納）がある取引では使えない。ポイントは現金のみの商品とたばこには使えない
    fn method_refusal(&self, m: PaymentMethod) -> Option<&'static str> {
        match m {
            PaymentMethod::Cash => None,
            _ if self.cashless_due() == 0 => Some(CASH_ONLY),
            PaymentMethod::Qr | PaymentMethod::Credit if self.cash_only() > 0 => Some(NO_CASH_MIX),
            PaymentMethod::Points
                if self.cash_only() > 0
                    || self.lines.iter().any(|l| l.category == Category::Tobacco) =>
            {
                Some(NO_POINTS)
            }
            _ => None,
        }
    }

    /// 税率ごとの対象額と内税額（インボイス：税率ごとに合計してから 1 回だけ端数処理する）。
    /// `rates` は明細がなくても出す税率（表示が増えたり減ったりしないように）
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

    /// お客様の確認待ちの代行収納の金額の合計（なければ None）
    fn bill_pending(&self) -> Option<u32> {
        let mut pending = self
            .lines
            .iter()
            .filter(|l| {
                matches!(
                    l.kind,
                    LineKind::Service {
                        confirm_pending: true,
                        ..
                    }
                )
            })
            .peekable();
        pending.peek()?;
        Some(pending.map(Line::amount).sum())
    }

    /// お客様の確認待ちの代行収納の件数
    fn bill_count(&self) -> u32 {
        self.lines
            .iter()
            .filter(|l| {
                matches!(
                    l.kind,
                    LineKind::Service {
                        confirm_pending: true,
                        ..
                    }
                )
            })
            .count() as u32
    }

    /// 承認済みの年齢で、いまの年確商品がすべて足りている
    fn age_cleared(&self) -> bool {
        match (self.required_age, self.verified_age) {
            (None, _) => true,
            (Some(need), Some(ok)) => ok >= need,
            _ => false,
        }
    }

    /// 店員の「する／しない」待ち
    fn age_choice_pending(&self) -> bool {
        self.paying && !self.age_cleared() && self.age_method.is_none()
    }

    /// 「する」を選び、店員の生年月日の入力待ち
    fn id_pending(&self) -> bool {
        self.paying && !self.age_cleared() && self.age_method == Some(AgeMethod::Id)
    }

    /// 買い物点数（数量の合計）
    /// 適用先（対象商品）のないクーポンがある
    fn coupon_unmatched(&self) -> bool {
        self.coupon_uses().iter().any(|(_, _, u)| u.is_none())
    }

    fn item_count(&self) -> u32 {
        self.lines.iter().map(|l| l.qty).sum()
    }

    /// この商品をいくつ登録しているか（行が分かれていても合計する）
    fn qty_of(&self, code: &str) -> u32 {
        self.lines
            .iter()
            .filter(|l| l.code == code)
            .map(|l| l.qty)
            .sum()
    }

    fn push(&mut self, p: &'static Product, qty: u32, kind: LineKind, tax: (String, u32)) {
        // 同じ商品を続けて読んだときだけ、一番上（直前に登録した）の行に足す。
        // 間に別の商品をはさんだら、前に登録した行にはまとめず新しい行にする
        // （個数を変える人が、ずっと前に登録した分まで覚えていなくて済むように）
        self.next_line += 1;
        let touched = self.next_line;
        if let Some(l) = self.lines.iter_mut().max_by_key(|l| l.touched).filter(|l| {
            l.code == p.code && l.kind == kind && !matches!(kind, LineKind::Service { .. })
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
            tax: tax.0,
            rate_bp: tax.1,
            category: p.category,
            cash_only: p.cash_only,
            kind,
        });
    }
}

#[derive(Clone, Debug, Serialize)]
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
    /// 最後に操作された時刻（無操作でセミセルフに戻すため）
    last_op: u64,
    /// 排出口（売り場側）にたまっているお金。取り出すまで残る
    outlet: Cash,
    /// 排出口にお金が残り始めた時刻。残っている間は、無操作でセミセルフに戻るカウントダウンを止める
    outlet_since: Option<u64>,
    /// お客様画面に開いている店舗メニュー
    menu: Option<Menu>,
    /// お客様側から店舗メニューを開くための使い捨てパスワード（店員画面にだけ出す）
    menu_code: Option<String>,
    /// レジ休止中（スタッフバーコードを読むまで、どの操作も受け付けない）
    paused: bool,
    /// いま触れている側と、その一瞬が終わる時刻（その間だけもう一方は操作できない）
    touch: Option<(Side, u64)>,
    /// お客様画面の呼び出しボタンが押されている（触れている間 MAT が鳴る）
    pressing: bool,
    /// 開いている電子マネーの手続き（チャージ・残高確認）
    emoney: Option<EmoneyTask>,
}

impl Register {
    /// 運用モードは名札で決まる：名札でログイン中なら有人、いなければセミセルフ
    fn sync_mode(&mut self) {
        if self.staff_state() != StaffState::Ready {
            self.menu = None;
            self.menu_code = None;
        }
        self.mode = if self.staff.is_some() {
            Mode::Attended
        } else {
            Mode::SemiSelf
        };
    }

    /// 取引も呼び出しもなく、待機に戻してよい状態か
    /// 釣り銭の取り忘れ：お客様画面に案内するまでと、店員を呼ぶまで（ミリ秒）
    fn change_timers(&self) -> (u64, u64) {
        if self.mode == Mode::Attended {
            (ATTENDED_CHANGE_REMIND_MS, ATTENDED_CHANGE_CALL_MS)
        } else {
            (CHANGE_REMIND_MS, CHANGE_CALL_MS)
        }
    }

    /// 取引がない（明細・クーポン・会計・電子マネーの手続きのどれもない）
    fn idle(&self) -> bool {
        self.txn.lines.is_empty()
            && self.txn.coupons.is_empty()
            && !self.txn.paying
            && self.emoney.is_none()
    }

    /// 排出口のお金が取り出されたら、止めていた無操作のカウントダウンを続きから再開する
    fn resume_idle_countdown(&mut self, now: u64) {
        if let Some(since) = self.outlet_since.take() {
            self.last_op = (self.last_op + now.saturating_sub(since)).min(now);
        }
    }

    /// 無操作でセミセルフに戻るまでの残り（排出口にお金が残っている間は止まって見える）
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

    fn context(&self) -> Context {
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
            // 確認の優先順：代行収納の金額と件数（受付で即座に）＞年確＞ポイントカード＞決済手段。
            // 一つの画面で複数の確認をしないよう、先の確認が済むまで後の確認は始めない
            age_choice_pending: t.age_choice_pending() && t.bill_pending().is_none(),
            id_pending: t.id_pending() && t.bill_pending().is_none(),
            ff_waiting: t.any_ff(FfState::Waiting),
            has_ff: t
                .lines
                .iter()
                .any(|l| matches!(l.kind, LineKind::Ff { .. })),
            service_pending: t.service_pending(),
            bill_pending: t.bill_pending().is_some(),
            // 年確が先。年確で販売が成立しないこともあるので、ポイントカードの確認はその後
            point_pending: t.paying
                && !t.point_checked
                && t.bill_pending().is_none()
                && !(t.age_choice_pending() || t.id_pending()),
            has_coupons: !t.coupons.is_empty(),
            coupon_unmatched: t.paying && t.coupon_unmatched(),
            method: t.method,
            customer_turn: t.customer_turn,
            // パスワードの入力を待つ間は、お客様側で入力する
            menu_side: self
                .menu
                .map(|m| m.side)
                .or(self.menu_code.as_ref().map(|_| Side::Customer)),
            touching: self.touch.map(|(side, _)| side),
            items_full: t.item_count() >= MAX_ITEMS,
            emoney_short: t.emoney_short || t.emoney_partial,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AuditEntry {
    pub at: u64,
    pub register: u8,
    pub staff: &'static str,
    pub operation: &'static str,
    pub amount: u32,
    pub result: String,
}

// ---------------------------------------------------------------------------
// 店舗
// ---------------------------------------------------------------------------

pub struct Store {
    now: u64,
    rng: Rng,
    registers: Vec<Register>,
    mats: Vec<Mat>,
    calls: Calls,
    age_rules: AgeRules,
    tax: TaxSettings,
    auth: Authenticator,
    ff_stock: Vec<(&'static str, u32)>,
    audit: Vec<AuditEntry>,
    mat_loss: u8,
    bell: Bell,
    seen_raised: u64,
    /// MAT を鳴らした回数（ボタンを押した・呼び出しが起きた）
    presses: u64,
    receipts: u32,
    /// 財布の電子マネーの残高
    emoney_balance: u32,
}

fn customer_side() -> Side {
    Side::Customer
}

/// 年確のやり方（店員が選ぶ）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AgeMethod {
    /// 「しない」：店員の判断で年確を済ませる
    Skip,
    /// 「する」：身分証の生年月日で判定する
    Id,
}

/// 読み取ったクーポン。discount が None なら対象商品の登録待ち
#[derive(Clone, Debug, Serialize)]
pub struct CouponView {
    /// 取消に使う番号
    pub id: u32,
    pub code: &'static str,
    pub name: &'static str,
    pub discount: Option<u32>,
}

/// 取り消して、お客様に返すクーポン
#[derive(Clone, Debug, Serialize)]
pub struct ReturnedCoupon {
    pub name: &'static str,
    /// 適用先（対象商品）がなかった
    pub unmatched: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Confiscated {
    pub name: &'static str,
    pub qty: u32,
}

/// 日付を読む。西暦 8 桁（19821002）か YYYY-MM-DD。存在しない日付は読まない
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

/// 生年月日 birth の人の today 時点の年齢。
///
/// 生年月日は身分証の西暦を 8 桁で打ち込む（19821002）。カレンダーから選ぶ方式は、
/// 今年から 18〜20 年も遡るのがかえって手間なので使わない。
/// 設計メモ：和暦（S57.10.2 のような元号入力）にも対応できるが、身分証は西暦表記が
/// 主流になりつつあり導入の重要性は薄いため、いまは西暦のみ受け付ける。
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

/// 無操作がこの時間続くと、レジはセミセルフモードに戻って待機する
/// 決済後の「ありがとうございました」を出す時間。過ぎると新しいお客様の待ち受けに戻る
pub const THANKS_MS: u64 = 3000;

pub const IDLE_REVERT_MS: u64 = 5 * 60 * 1000;
/// 排出口に釣り銭が残ったまま、お客様画面に「釣り銭が残っています」を出すまで（セミセルフ）
pub const CHANGE_REMIND_MS: u64 = 60 * 1000;
/// 排出口に釣り銭が残ったまま、店員を呼ぶ（釣銭忘れ）まで（セミセルフ）
pub const CHANGE_CALL_MS: u64 = 2 * 60 * 1000;
/// 有人モードは店員がその場にいるので、早めに知らせる
pub const ATTENDED_CHANGE_REMIND_MS: u64 = 30 * 1000;
pub const ATTENDED_CHANGE_CALL_MS: u64 = 60 * 1000;
/// 放置された呼び出しほど大きく鳴らす。最初は RING_MIN、RING_FULL_MS 放置で最大
pub const RING_MIN: f32 = 0.25;
pub const RING_FULL_MS: u64 = 60 * 1000;

/// 呼び出しの放置時間から鳴動の音量（0〜1）を決める。複数のレジが鳴っても、
/// 先に呼んだ（長く待たせている）レジほど大きく聞こえる
pub fn ring_volume(waiting_ms: u64) -> f32 {
    let t = waiting_ms.min(RING_FULL_MS) as f32 / RING_FULL_MS as f32;
    RING_MIN + (1.0 - RING_MIN) * t
}

impl Store {
    /// 基本構成はレジ 2 台・MAT 1 台。レジは 10 台、MAT は 5 台まで増やせる。
    pub fn new(seed: u64, registers: u8, mats: u8) -> Store {
        let registers = registers.clamp(1, MAX_REGISTERS);
        let mats = mats.clamp(1, MAX_MATS);
        let regs = (1..=registers)
            .map(|id| {
                // 既定はセミセルフ（名札なし）
                Register {
                    id,
                    mode: Mode::SemiSelf,
                    staff: None,
                    txn: Txn::default(),
                    drawer: Drawer::default(),
                    last_receipt: None,
                    last_op: 0,
                    outlet: Cash::default(),
                    outlet_since: None,
                    menu: None,
                    menu_code: None,
                    paused: false,
                    touch: None,
                    pressing: false,
                    emoney: None,
                }
            })
            .collect();
        Store {
            now: 0,
            rng: Rng::new(seed),
            registers: regs,
            mats: (0..mats).map(|i| Mat::new(&bell_name(i))).collect(),
            calls: Calls::default(),
            age_rules: AgeRules::default(),
            tax: TaxSettings::default(),
            auth: Authenticator::default(),
            ff_stock: INITIAL_FF_STOCK.to_vec(),
            audit: vec![],
            mat_loss: 0,
            bell: Bell::default(),
            seen_raised: 0,
            presses: 0,
            receipts: 0,
            emoney_balance: EMONEY_INITIAL,
        }
    }

    pub fn now(&self) -> u64 {
        self.now
    }

    // ---- 時間の経過：MAT の鳴動状態の同期 --------------------------------

    pub fn tick(&mut self, now: u64) {
        self.now = self.now.max(now);
        let now = self.now;
        // 触れている一瞬が過ぎたら、両方の画面に戻す
        for r in &mut self.registers {
            if r.touch.is_some_and(|(_, until)| until <= now) {
                r.touch = None;
            }
            // お礼の画面は、排出口の釣り銭が取り出されるまで初期画面に戻さない
            if r.outlet.total() == 0
                && r.last_receipt
                    .as_ref()
                    .is_some_and(|rc| rc.at + THANKS_MS <= now)
            {
                r.last_receipt = None;
            }
        }
        // MAT はデジタルな呼び鈴：ボタンが押されている間だけ鳴る。鳴らし直しはしない。
        // 年確・代行収納の読み取りのようにボタンを介さない呼び出しは、起きた瞬間に短く鳴る
        if self.calls.raised > self.seen_raised {
            self.seen_raised = self.calls.raised;
            self.presses += 1;
        }
        let mut bell = self.bell;
        bell.held = self.registers.iter().filter(|r| r.pressing).count();
        bell.presses = self.presses;
        if bell != self.bell {
            bell.seq += 1;
            self.bell = bell;
            for m in &mut self.mats {
                m.kick();
            }
        }
        // 呼び鈴機能を OFF にした端末には鳴動状態を送らない
        for m in self.mats.iter_mut().filter(|m| m.bell_on) {
            m.tick(now, self.bell, self.mat_loss, &mut self.rng);
        }
        // 排出口の釣り銭：残ったまま 2 分（有人モードは 1 分）で店員を呼ぶ（釣銭忘れ）。取り出せばカウントダウンを再開する
        for r in &mut self.registers {
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
        // 無操作が 5 分続いたら、登録中の取引も中止し、名札のログインを外してセミセルフに戻る。
        // 5 分あれば、財布を車に取りに行くくらいは待てる（取引の保留機能はあえて持たない）。
        // 排出口にお金が残っている間と、係員を呼んでいる間は数えない
        let due: Vec<u8> = self
            .registers
            .iter()
            .filter(|r| {
                (r.mode != Mode::SemiSelf || !r.idle())
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
                if r.mode != Mode::SemiSelf {
                    r.staff = None;
                    r.mode = Mode::SemiSelf;
                }
                r.menu = None;
                r.menu_code = None;
                r.last_op = now;
            }
        }
    }

    /// 取引を中止する。入れたお金とチャージ用の紙幣は出金ボックスへ、電子マネーで払った分は残高へ戻す。
    /// 戻したお金の額を返す
    fn void_txn(&mut self, register: u8, now: u64) -> u32 {
        let Some(r) = self.registers.iter_mut().find(|r| r.id == register) else {
            return 0;
        };
        r.menu = None;
        let txn = std::mem::take(&mut r.txn);
        // 入れたお金と、チャージ用に入れた紙幣は出金ボックスに戻す
        r.outlet.merge(&txn.inserted);
        let mut back = txn.inserted.total();
        if let Some(EmoneyTask::Charge { inserted }) = r.emoney.take() {
            back += inserted.total();
            r.outlet.merge(&inserted);
        }
        r.emoney = None;
        // 電子マネーで先に払った分は、電子マネーに戻す（ほかのキャッシュレスは取り消す）
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
        ] {
            self.calls.resolve(register, k, now);
        }
        back
    }

    // ---- コマンド --------------------------------------------------------

    fn reg(&self, id: u8) -> Option<&Register> {
        self.registers.iter().find(|r| r.id == id)
    }

    fn reg_mut(&mut self, id: u8) -> Result<&mut Register, String> {
        self.registers
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("レジ{id}はありません"))
    }

    /// 画面を指定しないコマンド（店舗メニュー・呼び出しボタンなど）は、操作側の画面からの操作
    fn check_active(&self, register: u8, action: Action) -> Result<(), String> {
        let r = self
            .reg(register)
            .ok_or_else(|| format!("レジ{register}はありません"))?;
        let c = r.context();
        permitted(active_side(&c).unwrap_or(Side::Customer), action, &c).map_err(String::from)
    }

    fn check(&self, register: u8, side: Side, action: Action) -> Result<(), String> {
        let r = self
            .reg(register)
            .ok_or_else(|| format!("レジ{register}はありません"))?;
        permitted(side, action, &r.context()).map_err(String::from)
    }

    pub fn dispatch(&mut self, cmd: Command) -> Result<Outcome, String> {
        // レジ休止中は、スタッフバーコード（再開）と出金ボックスのお金の取り出しだけを受け付ける
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
        // 成否にかかわらず、触られたレジは「操作あり」
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
            } => self.register_ff(register, side, &code, qty),
            C::FfButton { register } => {
                self.check_active(register, Action::FfButton)?;
                self.call(register, CallKind::FastFood, "FF のご注文");
                self.press(register)?;
                done("係員をお呼びしました。FF のご注文を伺います")
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
                // 入力のたびに判定する。合えばお渡し済み（OK）、合わなければお渡し前（NG）
                let matched = handed == l.qty;
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
                l.unit_price = amount;
                // 代行収納は、受付した内容・金額をお客様が会計時に確認する
                l.kind = LineKind::Service {
                    done: true,
                    confirm_pending: catalog::find(l.code).is_some_and(|p| p.customer_confirms),
                };
                let name = l.name;
                if !self.reg_mut(register)?.txn.service_pending() {
                    self.calls.resolve(register, CallKind::Service, self.now);
                }
                done(format!("{name}を受け付けました（{amount}円）"))
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
                let received = r.txn.lines[i].kind
                    == LineKind::Ff {
                        state: FfState::Received,
                    };
                // お渡し済みの FF は店員がお客様から回収する。係員のいないセミセルフでは取り消せない
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
                if !r.txn.service_pending() {
                    self.calls.resolve(register, CallKind::Service, self.now);
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
                if r.txn.prepaid > 0 {
                    return Err("一部お支払い済みです。残りを現金でお支払いください".into());
                }
                r.txn.paying = false;
                r.txn.method = None;
                r.txn.customer_turn = false;
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
                    return Err("現金が投入済みです。現金でお支払いください".into());
                }
                if let Some(why) = r.txn.method_refusal(method) {
                    return Err(why.into());
                }
                r.txn.method = Some(method);
                // ポイント利用なのにポイントカードが読まれていなければ、ポイントカードの確認に差し戻す
                if method == PaymentMethod::Points && r.txn.point_card.is_none() {
                    r.txn.point_checked = false;
                    return done(
                        "ポイントカードが読まれていません。ポイントカードを読み取ってください",
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
                let r = self.reg_mut(register)?;
                let age = r.txn.required_age.unwrap_or(0);
                if id_check {
                    r.txn.age_method = Some(AgeMethod::Id);
                    done("身分証の生年月日を入力してください")
                } else {
                    // 店員が見た目で判断して確認しないなら、年確はそこで済む（お客様のボタンは挟まない）
                    r.txn.age_method = Some(AgeMethod::Skip);
                    r.txn.verified_age = r.txn.required_age;
                    done(format!("{age}歳以上の年齢確認を省略しました"))
                }
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
                    "{}を取り消しました。お客様に返却してください",
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
                r.txn.confiscated.clear();
                done("販売できない商品を回収しました")
            }
            C::DismissFfCollect { register } => {
                let r = self.reg_mut(register)?;
                r.txn.ff_collect.clear();
                done("FF を回収しました")
            }
            C::ConfirmBillPayment { register, side } => {
                self.check(register, side, Action::ConfirmBillPayment)?;
                let r = self.reg_mut(register)?;
                let amount = r.txn.bill_pending().unwrap_or(0);
                for l in &mut r.txn.lines {
                    if let LineKind::Service {
                        confirm_pending, ..
                    } = &mut l.kind
                    {
                        *confirm_pending = false;
                    }
                }
                done(format!(
                    "お客様が代行収納の支払い内容・金額（{amount}円）を確認しました"
                ))
            }
            C::InsertCash {
                register,
                denomination,
                count,
            } => self.insert_cash(register, denomination, count),
            C::FinishInsert { register } => self.finish_insert(register),
            C::TapEmoney { register, method } => {
                // チャージ・残高確認の手続き中なら、そちらに使う
                if self.reg(register).is_some_and(|r| r.emoney.is_some()) {
                    return self.tap_emoney_task(register, method);
                }
                // 「不足分を現金で払う」：案内（残高を引き出し、残りを現金で）を確認してから、かざした電子マネーの残高を引き出す
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
                        "電子マネーで {drawn}円 お支払いしました。残り {due}円を現金でお支払いください"
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
                // 現金のみの商品（切手・代行収納）があれば、キャッシュレスで払うのはそれを除いた分だけ。残りは現金で
                let part = r.txn.cashless_due();
                if part == 0 {
                    return Err(CASH_ONLY.into());
                }
                if part < due {
                    // 現金と併用できるのは電子マネーだけ
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
                        "{}で {part}円 お支払いしました。残り {rest}円（現金のみの商品）を現金でお支払いください",
                        method.label()
                    ));
                }
                if method == PaymentMethod::Emoney {
                    if balance < due {
                        // 残高不足：やめる・残りを現金で払う・チャージするから選んでもらう
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
                // セミセルフでは、お買い物の途中にチャージ・残高確認はできない
                let shopping = !r.txn.lines.is_empty() || !r.txn.coupons.is_empty();
                if r.mode == Mode::SemiSelf && shopping {
                    return Err(EMONEY_NO_CHARGE.into());
                }
                // 選ぶ画面からは、チャージ・残高確認に進める
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
                            "チャージをやめました。{back}円を出金ボックスから取り出してください"
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
                if r.mode == Mode::SemiSelf && choice == ShortChoice::Charge {
                    return Err(EMONEY_NO_CHARGE.into());
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
                        done("電子マネーを使うのをやめました。お支払い方法を選んでください")
                    }
                    ShortChoice::Cash => {
                        // すぐには引き落とさない：かざしたままの電子マネーで、案内を見ないうちに決済が走らないように
                        r.txn.emoney_partial = true;
                        let drawn = balance.min(r.txn.cashless_due());
                        let rest = r.txn.due().saturating_sub(drawn);
                        done(format!(
                            "電子マネーの残高 {drawn}円を引き出し、残り {rest}円を現金でお支払いいただきます"
                        ))
                    }
                    ShortChoice::CashConfirmed => unreachable!(),
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
                let (kind, detail) = match topic {
                    Some(CallTopic::Service) => (CallKind::Service, "お支払い・荷物"),
                    Some(CallTopic::Tobacco) => (CallKind::Tobacco, ""),
                    None => (CallKind::CustomerHelp, "お客様からの呼び出し"),
                };
                self.call(register, kind, detail);
                self.press(register)?;
                done(match topic {
                    Some(CallTopic::Service) => "係員をお呼びしました（サービス）",
                    Some(CallTopic::Tobacco) => "係員をお呼びしました（たばこ）",
                    None => "係員をお呼びしました",
                })
            }
            C::ConfirmPointCard { register, side } => {
                self.check(register, side, Action::ConfirmPointCard)?;
                let t = &mut self.reg_mut(register)?.txn;
                t.point_checked = true;
                // ポイントカードなしなら、差し戻し前に選ばれていたポイント利用は取り消す
                if t.method == Some(PaymentMethod::Points) {
                    t.method = None;
                }
                done("ポイントカードなしで進みます。お支払い方法を選んでください")
            }
            C::ReleaseCall { register } => {
                self.reg_mut(register)?.pressing = false;
                done("")
            }
            C::VoidTransaction { register } => {
                self.check_active(register, Action::VoidTransaction)?;
                let now = self.now;
                let r = self.reg_mut(register)?;
                // 開いていた店舗メニューは閉じる
                r.menu = None;
                let back = self.void_txn(register, now);
                if back > 0 {
                    done(format!(
                        "取引を中止しました。{back}円を出金ボックスから取り出してください"
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
                r.menu_code = None;
                r.touch = None;
                r.paused = true;
                done("レジ休止中にしました（スタッフバーコードで再開）")
            }
            C::EndStaffMode { register } => {
                self.check_active(register, Action::EndStaffMode)?;
                let r = self.reg_mut(register)?;
                r.staff = None;
                r.menu = None;
                r.menu_code = None;
                done("ログアウトしました（セミセルフモード）")
            }
            C::CashOut {
                register,
                kind,
                amount,
            } => self.cash_out(register, kind, amount),
            C::CashIn {
                register,
                kind,
                amount,
            } => self.cash_in(register, kind, amount),
            C::OpenMenu {
                register,
                screen,
                side,
                code,
                exchange_to,
            } => self.open_menu(register, side, screen, code, exchange_to),
            C::RequestMenuCode { register } => {
                let r = self.reg(register).ok_or("レジがありません")?;
                if r.staff_state() != StaffState::Ready {
                    return Err("店舗メニューは名札でログインした店員だけが使えます".into());
                }
                if r.menu.is_some() {
                    return Err("店舗メニューを開いています".into());
                }
                let code = self.new_menu_code();
                self.reg_mut(register)?.menu_code = Some(code);
                done("店員画面に表示されたパスワードを入力してください")
            }
            C::Insert { register, amount } => self.insert(register, amount),
            C::TakeOutlet { register } => {
                let now = self.now;
                let r = self.reg_mut(register)?;
                let total = r.outlet.total();
                if total == 0 {
                    return Err("出金ボックスにお金はありません".into());
                }
                r.outlet = Cash::default();
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
                let by = self.owner(&owner_id, &password)?;
                done(format!("{by}として認証しました"))
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
            C::SetMatLoss { percent } => {
                self.mat_loss = percent.min(100);
                done(format!("MAT の通信ロスを {}% にしました", self.mat_loss))
            }
            C::SetBellOn { mat, on } => {
                let bell = self.bell;
                let m = self.mats.get_mut(mat).ok_or("MAT が見つかりません")?;
                if bell_kind(mat as u8) == BellKind::Fixed {
                    return Err("据え付け端末は OFF にできません".into());
                }
                if on && !m.bell_on {
                    // OFF の間の呼び出しを、ON にした途端に鳴らさない
                    m.received.presses = bell.presses;
                    m.buzz_until = 0;
                }
                m.bell_on = on;
                done(format!(
                    "{}の呼び鈴を{}にしました",
                    m.id,
                    if on { "ON" } else { "OFF" }
                ))
            }
            C::SetMatOffline { mat, offline } => {
                let m = self.mats.get_mut(mat).ok_or("MAT が見つかりません")?;
                m.offline = offline;
                done(format!(
                    "{}を{}にしました",
                    m.id,
                    if offline { "不通" } else { "復旧" }
                ))
            }
        }
    }

    /// この商品の今の税区分と税率
    fn tax_for(&self, code: &str) -> (String, u32) {
        let c = self.tax.class_for(code);
        (c.id.clone(), c.rate_bp)
    }

    fn owner(&mut self, id: &str, password: &str) -> Result<&'static str, String> {
        self.auth.verify_owner(id, password, self.now)?;
        Ok(auth::account(id).map_or("?", |a| a.name))
    }

    /// 商品ごとの販売個数の上限を超えないか（add 個増やすとき）
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

    fn call(&mut self, register: u8, kind: CallKind, detail: &str) {
        // 店員がすでにそのレジで操作できる状態なら、呼び出しは不要
        if self
            .reg(register)
            .is_some_and(|r| r.staff_state() == StaffState::Ready)
        {
            return;
        }
        self.calls.raise(register, kind, self.now, detail);
    }

    /// 呼び出しボタンに触れた：離すまで MAT を鳴らす。触れるたびに必ず鳴る
    fn press(&mut self, register: u8) -> Result<(), String> {
        self.reg_mut(register)?.pressing = true;
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
        // 取消と同じく、登録画面での明細の手直し
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
            // お渡し済みの FF の数量を変えるのは店員（増やせばお渡し前に戻し、減らせば回収してもらう）
            _ if received && r.staff_state() != StaffState::Ready => {
                return Err(FF_RECEIVED_SELF.into())
            }
            _ => {}
        }
        if qty > old && r.txn.item_count() - old + qty > MAX_ITEMS {
            return Err(ITEMS_FULL.into());
        }
        if qty > old {
            self.check_limit(register, code, qty - old)?;
        }
        // FF は増やした分を在庫から取る。減らした分は、お渡し前なら在庫に戻す
        // （お渡し済みはお客様の手に渡ったので戻さず、回収してもらう）
        if matches!(kind, LineKind::Ff { .. }) {
            if qty > old {
                let stock = self
                    .ff_stock
                    .iter_mut()
                    .find(|(c, _)| *c == code)
                    .ok_or("在庫がありません")?;
                if stock.1 < qty - old {
                    return Err(format!("{name}は在庫が足りません（残り {}）", stock.1));
                }
                stock.1 -= qty - old;
            } else if !received {
                self.restock(code, old - qty);
            }
        }
        let t = &mut self.reg_mut(register)?.txn;
        t.next_line += 1;
        let touched = t.next_line;
        let l = t.lines.iter_mut().find(|l| l.id == line_id).unwrap();
        l.qty = qty;
        l.touched = touched;
        if received && qty > old {
            // 増やした分はまだ渡していないので、お渡し前に戻す（照合し直す）
            l.kind = LineKind::Ff {
                state: FfState::Waiting,
            };
        }
        if received && qty < old {
            t.ff_collect.push((name, old - qty));
            return done(format!("{name}を{}点回収してください", old - qty));
        }
        done(format!("{name}の個数を {qty} にしました"))
    }

    fn restock(&mut self, code: &str, qty: u32) {
        if let Some(e) = self.ff_stock.iter_mut().find(|(c, _)| *c == code) {
            e.1 += qty;
        }
    }

    fn scan(&mut self, register: u8, side: Side, code: &str) -> Result<Outcome, String> {
        let scanned = catalog::classify(code);
        // スタッフバーコードは、未ログインでも・休止中でも読む（ログイン・再開）
        if let Scanned::Staff(id) = &scanned {
            let acc = auth::account(id).ok_or("登録されていないスタッフバーコードです")?;
            let now = self.now;
            let r = self.reg_mut(register)?;
            let was = r.staff_state();
            let resumed = std::mem::take(&mut r.paused);
            let took_over = r.txn.customer_turn;
            r.staff = Some(StaffSession { account: acc });
            // ログイン後は両方の画面が操作できる（触れている一瞬だけ排他）
            r.touch = None;
            // ログインで店員側が操作できるようになる（お客様の支払い中でも）
            r.txn.customer_turn = false;
            // 店員が来たので、呼び出しボタンの鳴動も止める
            r.pressing = false;
            // 店員が来たら、ボタンでの呼び出しは片付く（サービス商品の受付待ちがあれば残す）
            let service_pending = r.txn.service_pending();
            self.calls.resolve(register, CallKind::CustomerHelp, now);
            if self.calls.is_open(register, CallKind::Tobacco) {
                self.reg_mut(register)?.txn.tobacco_asked = true;
            }
            self.calls.resolve(register, CallKind::Tobacco, now);
            if !service_pending {
                self.calls.resolve(register, CallKind::Service, now);
            }
            return done(match was {
                _ if resumed => format!("{}さんがレジを再開しました（有人モード）", acc.name),
                _ if took_over => format!("{}さんが操作を引き継ぎました", acc.name),
                _ => format!("{}さんがログインしました（有人モード）", acc.name),
            });
        }
        self.check(register, side, Action::Scan)?;
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
        // ポイントカードの最終確認中に読まれたポイントカードは、そのまま確認済みにして支払いへ
        if let Scanned::PointCard(no) = &scanned {
            if r.txn.paying && !r.txn.point_checked {
                r.txn.point_card = Some(no.clone());
                r.txn.point_checked = true;
                return done("ポイントカードを読みました。お支払い方法を選んでください");
            }
        }
        // 買い物点数の上限を超える商品は登録しない（会計画面にもとどまる）
        if matches!(scanned, Scanned::Product(_)) && r.txn.item_count() >= MAX_ITEMS {
            return Err(ITEMS_FULL.into());
        }
        // 会計画面で読まれたら、登録画面に引き返して追加する（投入済みの現金はそのまま）
        let back = if r.txn.paying {
            r.txn.paying = false;
            r.txn.method = None;
            r.txn.customer_turn = false;
            "（登録画面に戻りました）"
        } else {
            ""
        };
        match scanned {
            Scanned::Staff(_) => unreachable!(),
            Scanned::Unknown => Err("登録されていないバーコードです".into()),
            Scanned::PointCard(no) => {
                r.txn.point_card = Some(no);
                done(format!("ポイントカードを読みました{back}"))
            }
            Scanned::Coupon(c) => {
                // 明細と同じ通し番号（先に進めてから使う。明細の番号と重ならない）
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
                        "{}を読みました。対象商品を登録すると値引きします{back}",
                        c.name
                    )
                } else {
                    format!("{}を読みました{back}", c.name)
                })
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
                let ready = r.staff_state() == StaffState::Ready;
                if !ready {
                    self.calls.raise(register, CallKind::Service, now, p.name);
                }
                done(if ready {
                    format!("{}：受付内容を入力してください{back}", p.name)
                } else {
                    format!("{}：係員をお呼びしました{back}", p.name)
                })
            }
            Scanned::Product(p) => {
                if p.category == Category::Tobacco {
                    r.txn.tobacco_asked = false;
                }
                r.txn.push(p, 1, LineKind::Item, tax);
                done(format!("{} {}円{back}", p.name, p.price))
            }
        }
    }

    fn register_ff(
        &mut self,
        register: u8,
        side: Side,
        code: &str,
        qty: u32,
    ) -> Result<Outcome, String> {
        self.check(register, side, Action::RegisterFf)?;
        let p = catalog::find(code)
            .filter(|p| p.category == Category::FastFood)
            .ok_or("FF の商品が見つかりません")?;
        let qty = qty.max(1);
        let count = self.reg(register).map_or(0, |r| r.txn.item_count());
        if count + qty > MAX_ITEMS {
            return Err(ITEMS_FULL.into());
        }
        self.check_limit(register, code, qty)?;
        let stock = self
            .ff_stock
            .iter_mut()
            .find(|(c, _)| *c == code)
            .ok_or("在庫がありません")?;
        if stock.1 < qty {
            return Err(format!("{}は在庫が足りません（残り {}）", p.name, stock.1));
        }
        stock.1 -= qty;
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
            "{} ×{qty} を登録しました。お渡ししたらチェックを入れてください",
            p.name
        ))
    }

    /// 身分証の年齢で判定する。年齢に届かない年確商品は買い物から取り除き、回収の一覧に載せる
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
        r.txn.required_age = r
            .txn
            .lines
            .iter()
            .filter_map(|l| rules.age_for(l.code, l.category))
            .max();
        if r.txn.lines.is_empty() {
            // すべて取り除いたら登録画面に戻る
            r.txn.paying = false;
            r.txn.method = None;
            r.txn.customer_turn = false;
        }
        if removed.is_empty() {
            return done(format!("{age}歳です。年齢確認が済みました"));
        }
        let list = removed
            .iter()
            .map(|(n, q)| format!("{n}×{q}"))
            .collect::<Vec<_>>()
            .join("・");
        r.txn.confiscated.extend(removed);
        done(format!("{age}歳です。販売できない{list}を取り除きました"))
    }

    fn checkout(&mut self, register: u8, side: Side) -> Result<Outcome, String> {
        self.check(register, side, Action::Checkout)?;
        let rules = &self.age_rules;
        let r = self
            .registers
            .iter_mut()
            .find(|r| r.id == register)
            .ok_or("レジがありません")?;
        // 年確の判定はこの時点だけ。対象品が複数あっても最も厳しい年齢で 1 回
        let age = r
            .txn
            .lines
            .iter()
            .filter_map(|l| rules.age_for(l.code, l.category))
            .max();
        r.txn.paying = true;
        r.txn.required_age = age;
        // 済んだ年齢で足りなければ（より厳しい商品を足した等）、年確のやり方の選択からやり直す
        if !r.txn.age_cleared() {
            r.txn.age_method = None;
        }
        // 会計ボタンの後は、まずポイントカードの最終確認。読み取り済みなら確認済み
        r.txn.point_checked = r.txn.point_card.is_some();
        // セミセルフでは、会計に進めたら支払いはお客様に任せる
        r.txn.customer_turn = r.mode == Mode::SemiSelf;
        let ready = r.staff_state() == StaffState::Ready;
        let pending = r.txn.age_choice_pending();
        let mode = r.mode;
        if pending && mode != Mode::Attended && !ready {
            self.calls.raise(
                register,
                CallKind::AgeCheck,
                self.now,
                format!("{}歳以上", age.unwrap_or(0)),
            );
            return done("年齢確認のため係員をお呼びしました");
        }
        if !pending {
            self.calls.resolve(register, CallKind::AgeCheck, self.now);
        }
        if pending {
            done(format!(
                "{}歳以上の年齢確認があります。店員画面で年齢確認をするか選んでください",
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
        if self.reg(register).is_some_and(|r| r.emoney.is_some()) {
            return self.charge_cash(register, c);
        }
        self.check(register, Side::Customer, Action::InsertCash)?;
        self.pay_cash(register, c)
    }

    /// 電子マネーの手続き中に入金ボックスへ入れたお金（チャージは紙幣だけ）
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
            "チャージ {}円。電子マネーをかざしてください",
            inserted.total()
        ))
    }

    /// 電子マネーの手続き中にかざした：残高確認なら残高を出し、チャージなら入れた紙幣の分を足す
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
                    format!("{amount}円チャージしました（残高 {now}円）。もう一度電子マネーをかざしてください")
                } else {
                    format!("{amount}円チャージしました（残高 {now}円）")
                })
            }
            Some(EmoneyTask::Menu) => Err("チャージか残高確認を選んでください".into()),
            None => Err("電子マネーの手続きはしていません".into()),
        }
    }

    /// 投入口：店舗メニューの入金・両替画面が開いていればそちら、なければお会計の支払い
    /// 画面に触れる。もう一方が触れている一瞬や、お客様側に限られる場面では利かない
    fn claim(&mut self, register: u8, side: Side) -> Result<Outcome, String> {
        let r = self.reg(register).ok_or("レジがありません")?;
        let c = r.context();
        match active_side(&c) {
            Some(s) if s != side => Err(if c.menu_side.is_some() {
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

    /// 両替：入れた金額と同じ額を、指定の金種で排出口に出す
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
            "{amount}円を {to}円×{} に両替しました。出金ボックスから取り出してください",
            amount / to
        ))
    }

    /// 現金を入れる。会計の確認が済んでいれば、入れただけで支払方法は現金になる
    /// （キャッシュレスは電子マネーとバーコード決済を取り違える人がいるので、必ず選んでもらう）。
    /// 投入額は積み上がり、「投入おわり」（[`Command::FinishInsert`]）を押すと決済する。入金ボックスの Enter はその近道
    fn pay_cash(&mut self, register: u8, c: Cash) -> Result<Outcome, String> {
        let r = self.reg_mut(register)?;
        r.txn.method = Some(PaymentMethod::Cash);
        r.txn.inserted.merge(&c);
        let paid = r.txn.inserted.total();
        let total = r.txn.due();
        if paid < total {
            return done(format!("{paid}円 投入（あと {}円）", total - paid));
        }
        done(format!(
            "{paid}円 投入。「投入おわり」を押すとお支払いを確定します"
        ))
    }

    fn finish_insert(&mut self, register: u8) -> Result<Outcome, String> {
        self.check(register, Side::Customer, Action::InsertCash)?;
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
        let r = self.reg_mut(register)?;
        let total = r.txn.total();
        let due = r.txn.due();
        // 釣銭は紙幣・硬貨をそろえて一度に渡したものとする（排出タイミングは扱わない。cash.rs 参照）
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
                return Err(format!("{e}。係員をお呼びください"));
            }
        };
        // おつりは排出口に出る（お客様が取り出すまで残る）
        r.outlet.merge(&change);
        let txn = std::mem::take(&mut r.txn);
        let receipt = Receipt {
            no,
            at: now,
            total,
            method: match txn.prepaid_method.filter(|_| txn.prepaid > 0) {
                Some(PaymentMethod::Emoney) => "電子マネー・現金",
                Some(PaymentMethod::Qr) => "バーコード決済・現金",
                Some(PaymentMethod::Credit) => "クレジットカード・現金",
                Some(PaymentMethod::Points) => "ポイント利用・現金",
                _ => method.label(),
            },
            paid,
            change_total: change.total(),
            change,
            items: txn.lines.iter().map(|l| l.qty).sum(),
            points: if txn.point_card.is_some() {
                total / 100
            } else {
                0
            },
            point_card: txn.point_card,
        };
        let msg = if receipt.change_total > 0 {
            format!("お支払いが完了しました。おつり {}円", receipt.change_total)
        } else {
            "お支払いが完了しました".to_string()
        };
        r.last_receipt = Some(receipt);
        for k in [
            CallKind::AgeCheck,
            CallKind::FastFood,
            CallKind::Service,
            CallKind::Tobacco,
        ] {
            self.calls.resolve(register, k, now);
        }
        done(msg)
    }

    /// お客様側から店舗メニューを開くための使い捨てパスワード（英大文字と数字の 4 桁）
    fn new_menu_code(&mut self) -> String {
        (0..MENU_CODE_LEN)
            .map(|_| {
                let i = self.rng.range(0, MENU_CODE_CHARS.len() as u64 - 1) as usize;
                MENU_CODE_CHARS[i] as char
            })
            .collect()
    }

    fn open_menu(
        &mut self,
        register: u8,
        side: Side,
        screen: Option<MenuScreen>,
        code: Option<String>,
        exchange_to: Option<u32>,
    ) -> Result<Outcome, String> {
        let now = self.now;
        let r = self.reg(register).ok_or("レジがありません")?;
        if r.staff_state() != StaffState::Ready {
            return Err("店舗メニューは名札でログインした店員だけが使えます".into());
        }
        let Some(screen) = screen else {
            // 閉じる（お客様側のパスワード入力をやめるときも）
            let r = self.reg_mut(register)?;
            if r.menu.is_some_and(|m| m.side != side) {
                return Err("もう一方の画面で店舗メニューを開いています".into());
            }
            r.menu = None;
            if side == Side::Customer {
                r.menu_code = None;
            }
            return Ok(Outcome::default());
        };
        if screen.side().is_some_and(|s| s != side) {
            return Err(match side {
                Side::Staff => "お金の出し入れは、お客様側の画面の店舗メニューで行います",
                Side::Customer => "この項目は店員側の画面の店舗メニューにあります",
            }
            .into());
        }
        match r.menu {
            Some(m) if m.side != side => {
                return Err("もう一方の画面で店舗メニューを開いています".into())
            }
            Some(_) => {}
            // 店員側はパスワード不要。お客様側は、店員画面に出た使い捨てパスワードを見た人だけが開ける
            None if side == Side::Customer => {
                let Some(expected) = r.menu_code.clone() else {
                    return Err("店員画面にパスワードを出してから入力してください".into());
                };
                let given = code.unwrap_or_default().trim().to_ascii_uppercase();
                if let Err(e) = self.auth.verify_menu_code(&given, &expected, now) {
                    // 続けて間違えたら、パスワードを作り直す（総当たりさせない）
                    if e.contains("ロックしました") {
                        let fresh = self.new_menu_code();
                        self.reg_mut(register)?.menu_code = Some(fresh);
                    }
                    return Err(e);
                }
            }
            None if r.menu_code.is_some() => {
                return Err("お客様側の画面でパスワードを入力中です".into())
            }
            None => {}
        }
        let r = self.reg_mut(register)?;
        // 取引中はお金の出し入れの画面を開けない（一覧・店員側の項目は開ける）
        if screen.side() == Some(Side::Customer) && !r.txn.lines.is_empty() {
            return Err("取引中は店舗メニューを開けません".into());
        }
        let to = exchange_to
            .or(r.menu.map(|m| m.exchange_to))
            .unwrap_or(1_000);
        if !is_valid_denomination(to) {
            return Err("使えない金種です".into());
        }
        // パスワードは一度きり
        r.menu_code = None;
        r.menu = Some(Menu {
            side,
            screen,
            exchange_to: to,
        });
        Ok(Outcome::default())
    }

    /// 自動釣銭機へのお金の出し入れは、お客様側の店舗メニューに入っているときだけ
    fn require_cash_menu(&self, register: u8) -> Result<(), String> {
        let r = self.reg(register).ok_or("レジがありません")?;
        if r.menu.is_some_and(|m| m.side == Side::Customer) {
            Ok(())
        } else {
            Err("お金の出し入れは、お客様側の画面の店舗メニューで行います".into())
        }
    }

    fn cash_out(
        &mut self,
        register: u8,
        kind: CashOutKind,
        amount: u32,
    ) -> Result<Outcome, String> {
        self.check_active(register, Action::CashOut)?;
        self.require_cash_menu(register)?;
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
        if amount == 0 {
            return Err("金額を入力してください".into());
        }
        let r = self.reg_mut(register)?;
        match r.drawer.take(amount, false) {
            Ok(c) => {
                r.outlet.merge(&c);
                log(self, "実行".into());
                done(format!(
                    "{} {amount}円を出金しました。出金ボックスから取り出してください",
                    kind.label()
                ))
            }
            Err(e) => {
                log(self, format!("失敗：{e}"));
                Err(e)
            }
        }
    }

    /// お金を入れる操作。抜く操作と違ってパスワードは求めない（記録は残す）
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

    // ---- スナップショット ------------------------------------------------

    pub fn snapshot(&self) -> Snapshot {
        let now = self.now;
        Snapshot {
            now,
            registers: self
                .registers
                .iter()
                .map(|r| self.register_view(r))
                .collect(),
            mats: self
                .mats
                .iter()
                .enumerate()
                .map(|(i, m)| MatView {
                    id: m.id.clone(),
                    kind: bell_kind(i as u8),
                    buzzing: m.buzzing(now),
                    link_ok: m.rx_ok(now),
                    rings: m.rings,
                    offline: m.offline,
                    bell_on: m.bell_on,
                })
                .collect(),
            mat_links: self
                .mats
                .iter()
                .map(|m| MatLink {
                    id: m.id.clone(),
                    link_ok: m.link_ok(now),
                    acked_seq: m.acked_seq,
                    sent: m.sent,
                    retries: m.retries,
                })
                .collect(),
            bell: self.bell,
            calls: self.calls.open.clone(),
            call_history: self.calls.history.clone(),
            age_rules: AgeRulesView {
                rules: self.age_rules.rules.clone(),
                history: self.age_rules.history.clone(),
            },
            audit: self.audit.clone(),
            ff_stock: self.ff_menu(),
            mat_loss: self.mat_loss,
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

    /// FF のメニュー：タブごとに売れ筋順
    fn ff_menu(&self) -> Vec<FfMenuItem> {
        FF_MENU
            .iter()
            .filter_map(|&(code, tab)| {
                let p = catalog::find(code)?;
                let stock = self.ff_stock.iter().find(|(c, _)| *c == code)?.1;
                Some(FfMenuItem {
                    code: p.code,
                    name: p.name,
                    price: p.price,
                    stock,
                    tab,
                    tab_label: tab.label(),
                })
            })
            .collect()
    }

    fn register_view(&self, r: &Register) -> RegisterView {
        let c = r.context();
        let t = &r.txn;
        let taxes = t.taxes(&self.tax.display_rates(), self.tax.rounding);
        // どちらの画面にも同じボタンが並ぶ。触れている一瞬は、触れている側の画面だけが押せる
        let c_side = active_side(&c).unwrap_or(Side::Customer);
        let actions = |side: Side, list: &[Action]| {
            list.iter()
                .map(|&a| {
                    // 年齢確認・身分証確認などは、どちらが操作側でも決まった側の画面で押す
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
        let mat_down = self
            .mats
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
                limit: self.age_rules.limit_for(l.code),
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
                amount: l.amount(),
                age: self.age_rules.age_for(l.code, l.category),
                limit: self.age_rules.limit_for(l.code),
                cash_only: l.cash_only,
                kind: l.kind,
            })
            .collect();

        RegisterView {
            id: r.id,
            active_side: active_side(&c),
            max_items: MAX_ITEMS,
            lamp: waiting.is_some(),
            call_waiting_ms: waiting,
            ring_volume: waiting.map_or(0.0, ring_volume),
            tobacco_asked: r.mode == Mode::Attended && t.tobacco_asked,
            mode: r.mode,
            mode_label: r.mode.label(),
            outlet: r.outlet.total(),
            outlet_detail: r.outlet.0.clone(),
            menu: r.menu,
            menu_code: r.menu_code.clone(),
            paused: r.paused,
            idle_voids: !r.idle(),
            idle_revert_in: (r.mode != Mode::SemiSelf || !r.idle())
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
                required_age: t.required_age.filter(|_| t.paying),
                age_choice_pending: c.age_choice_pending,
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
                bill_check: t.bill_pending().filter(|_| c.bill_pending),
                bill_count: t.bill_count(),
                point_pending: c.point_pending,
                method: t.method,
                inserted,
                due: t.due().saturating_sub(inserted),
                payment_blocker: c.payment_blocker(),
                calls,
                actions: actions(c_side, ACTIONS),
                drawer_total: r.drawer.cash.total(),
                drawer: r.drawer.cash.0.clone(),
                mat_down,
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
                due: t.due().saturating_sub(inserted),
                point_card: t.point_card.is_some(),
                point_pending: c.point_pending,
                bill_check: t.bill_pending().filter(|_| c.bill_pending),
                bill_count: t.bill_count(),
                staff_called,
                age_wait: t
                    .required_age
                    .filter(|_| c.age_choice_pending && r.mode != Mode::Attended),
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
        }
    }
}

// ---------------------------------------------------------------------------
// ビュー
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct ActionState {
    pub action: Action,
    pub allowed: bool,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StaffLine {
    #[serde(flatten)]
    pub line: Line,
    pub amount: u32,
    /// 年確ルールに該当する年齢（参考表示。判定は会計時）
    pub age: Option<u8>,
    /// 1 回のお会計で買える個数の上限（店舗ルール）
    pub limit: Option<u32>,
}

/// お客様画面に出してよい明細（分類・商品コードは出さない）。
/// 年確と販売個数の上限は注意のしるしとして出す（有人モードの店員側と同じ見た目にする）
#[derive(Clone, Debug, Serialize)]
pub struct CustomerLine {
    pub id: u32,
    pub touched: u32,
    pub name: &'static str,
    pub qty: u32,
    pub amount: u32,
    pub age: Option<u8>,
    pub limit: Option<u32>,
    /// 現金でしか払えない（切手・代行収納）
    pub cash_only: bool,
    #[serde(flatten)]
    pub kind: LineKind,
}

#[derive(Clone, Debug, Serialize)]
pub struct StaffView {
    pub state: StaffState,
    pub name: Option<&'static str>,
    pub role: Option<Role>,
    pub phase: Phase,
    pub lines: Vec<StaffLine>,
    pub total: u32,
    pub taxes: Vec<TaxTotal>,
    /// 読み取った引換券・割引券
    pub coupons: Vec<CouponView>,
    pub point_card: Option<String>,
    pub required_age: Option<u8>,
    /// 店員の「年齢確認をする／しない」待ち
    pub age_choice_pending: bool,
    /// 年確で販売できず取り除いた商品（店員が回収する）
    pub confiscated: Vec<Confiscated>,
    /// お渡し済みの FF を減らした・取り消した分（店員がお客様から回収する）
    pub ff_collect: Vec<Confiscated>,
    /// 会計に進んだが、適用先のないクーポンが残っている（決済できない）
    pub coupon_unmatched: bool,
    /// 取り消して、お客様に返却するクーポン
    pub returned: Vec<ReturnedCoupon>,
    /// 「する」を選び、店員の生年月日の入力待ち
    pub id_pending: bool,
    /// 会計時、お客様の確認待ちの代行収納の金額
    pub bill_check: Option<u32>,
    /// 確認待ちの代行収納の件数
    pub bill_count: u32,
    /// ポイントカードの最終確認待ち（会計ボタンの直後）
    pub point_pending: bool,
    pub method: Option<PaymentMethod>,
    pub inserted: u32,
    pub due: u32,
    pub payment_blocker: Option<&'static str>,
    pub calls: Vec<Call>,
    pub actions: Vec<ActionState>,
    pub drawer_total: u32,
    /// 自動釣銭機の在高（金種, 枚数）
    pub drawer: Vec<(u32, u32)>,
    pub mat_down: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CustomerView {
    pub phase: Phase,
    pub lines: Vec<CustomerLine>,
    pub coupons: Vec<CouponView>,
    /// 会計に進んだが、適用先のないクーポンが残っている（決済できない）
    pub coupon_unmatched: bool,
    /// 取り消して、お返しするクーポン
    pub returned: Vec<ReturnedCoupon>,
    pub total: u32,
    /// 税率ごとの内税額（有人モードと同じ表示をセミセルフにも出す）
    pub taxes: Vec<TaxTotal>,
    pub inserted: u32,
    pub due: u32,
    pub point_card: bool,
    pub point_pending: bool,
    /// お客様の確認待ちの代行収納の金額
    pub bill_check: Option<u32>,
    /// 確認待ちの代行収納の件数
    pub bill_count: u32,
    pub staff_called: bool,
    /// セミセルフで年確の係員を待っている間の年齢（身分証の準備を案内する）
    pub age_wait: Option<u8>,
    pub staff_present: bool,
    pub method: Option<PaymentMethod>,
    pub payment_blocker: Option<&'static str>,
    pub actions: Vec<ActionState>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RegisterView {
    pub id: u8,
    /// 操作できる側が片方に限られているなら、その側（None なら両方の画面が操作できる）
    pub active_side: Option<Side>,
    /// 買い物点数の上限
    pub max_items: u32,
    /// レジ本体の呼び出しランプ。店員が来るまで点きっぱなし（音は MAT だけ。
    /// 呼び出しは順番待ちにせず、どれも発生した瞬間から知らせる）
    pub lamp: bool,
    /// 店員が来ていない呼び出しの放置時間（最も古いもの）
    pub call_waiting_ms: Option<u64>,
    /// 鳴動の音量（放置が長いほど大きい。呼び出しがなければ 0）
    pub ring_volume: f32,
    /// たばこボタンの呼び出しで有人モードになり、まだたばこを登録していない
    pub tobacco_asked: bool,
    pub mode: Mode,
    pub mode_label: &'static str,
    /// 排出口にたまっているお金
    pub outlet: u32,
    pub outlet_detail: Vec<(u32, u32)>,
    /// お客様画面に開いている店舗メニュー
    pub menu: Option<Menu>,
    /// お客様側から店舗メニューを開くための使い捨てパスワード（店員画面にだけ出す）
    pub menu_code: Option<String>,
    /// レジ休止中（スタッフバーコードで再開）
    pub paused: bool,
    /// セミセルフ以外で待機中なら、セミセルフに戻るまでの残り時間（ミリ秒）
    pub idle_revert_in: Option<u64>,
    /// 無操作が続くと、登録中の取引を中止する（取引があるとき）
    pub idle_voids: bool,
    /// 排出口に釣り銭が残ったまま 1 分（有人モードは 30 秒）たった（お客様画面に「釣り銭が残っています」）
    pub change_left: bool,
    pub staff: StaffView,
    pub customer: CustomerView,
    /// 開いている電子マネーの手続き（チャージ・残高確認）
    pub emoney: Option<EmoneyView>,
    /// 電子マネーの残高が足りず、どうするかを選んでもらう（値は残高）
    pub emoney_short: Option<u32>,
    /// 「不足分を現金で払う」：電子マネーをかざして残高を引き出すのを待つ（値は引き出す残高）
    pub emoney_partial: Option<u32>,
    /// その案内を確認済みで、電子マネーをかざすのを待っている
    pub emoney_partial_ok: bool,
    /// このお会計でキャッシュレスで先に払った額（残りを現金で払うとき）
    pub prepaid: u32,
    /// 先に払ったキャッシュレスの名前
    pub prepaid_method: Option<&'static str>,
    /// 現金でしか払えない商品（切手・代行収納）の合計
    pub cash_only: u32,
    /// これから払う額のうち、キャッシュレスで払える分
    pub cashless_due: u32,
    /// いまの取引で使えない支払方法とその理由
    pub unavailable: Vec<Unavailable>,
    pub last_receipt: Option<Receipt>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Unavailable {
    pub method: PaymentMethod,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmoneyView {
    pub kind: EmoneyKind,
    /// チャージ用に入れた紙幣の合計
    pub inserted: u32,
    /// 残高確認でかざしたあとの残高
    pub balance: Option<u32>,
}

/// 呼び鈴（Bell）の種類。据え付け型の呼び鈴が主で、携帯型の検品端末と事務所のストアコンピューターの
/// 呼び鈴は「あれば便利」（設定で OFF にできる）。
/// スタッフが重い端末を常に持ち歩く前提は現実的でない（制服にポケットもない）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BellKind {
    /// 据え付け型の呼び鈴（1 台目）
    Fixed,
    /// 携帯型の検品端末（2 台目、4 台目以降）
    Handheld,
    /// 事務所のストアコンピューター（3 台目）
    StoreComputer,
}

fn bell_kind(i: u8) -> BellKind {
    match i {
        0 => BellKind::Fixed,
        2 => BellKind::StoreComputer,
        _ => BellKind::Handheld,
    }
}

/// 1 台目は「据え付け端末」、2 台目は「可搬性端末」、3 台目は「ストアコンピューター」、
/// 4 台目以降は番号付きの可搬性端末
fn bell_name(i: u8) -> String {
    match i {
        0 => "据え付け端末".into(),
        1 => "可搬性端末".into(),
        2 => "ストアコンピューター".into(),
        n => format!("可搬性端末{}", n - 1),
    }
}

/// MAT 自身の画面：鳴動と不通表示だけ
#[derive(Clone, Debug, Serialize)]
pub struct MatView {
    pub id: String,
    pub kind: BellKind,
    pub buzzing: bool,
    pub link_ok: bool,
    pub rings: u64,
    pub offline: bool,
    pub bell_on: bool,
}

/// レジ側から見た MAT の通信状態（シミュレーター表示用）
#[derive(Clone, Debug, Serialize)]
pub struct MatLink {
    pub id: String,
    pub link_ok: bool,
    pub acked_seq: u64,
    pub sent: u64,
    pub retries: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AgeRulesView {
    pub rules: Vec<crate::age_rules::AgeRule>,
    pub history: Vec<crate::age_rules::RuleChange>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FfMenuItem {
    pub code: &'static str,
    pub name: &'static str,
    pub price: u32,
    pub stock: u32,
    pub tab: FfTab,
    pub tab_label: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct AccountView {
    pub id: &'static str,
    pub barcode: String,
    pub name: &'static str,
    pub role: Role,
}

#[derive(Clone, Debug, Serialize)]
pub struct CatalogItem {
    pub code: &'static str,
    pub name: &'static str,
    pub price: u32,
    pub category: Category,
    pub category_label: &'static str,
    pub age: Option<u8>,
    pub legal_age: Option<u8>,
    pub store_age: Option<u8>,
    /// 1 回のお会計で売れる上限（なければ None）
    pub limit: Option<u32>,
    /// 今の税区分
    pub tax_class: String,
}

/// 税率ごとの対象額（税込）と内税額
#[derive(Clone, Debug, Serialize)]
pub struct TaxTotal {
    pub rate_bp: u32,
    pub rate: String,
    pub base: u32,
    pub tax: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaxClassView {
    pub id: String,
    pub name: String,
    pub kind: TaxKind,
    pub kind_label: &'static str,
    pub rate_bp: u32,
    pub rate: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaxView {
    pub classes: Vec<TaxClassView>,
    pub invoice_no: String,
    pub rounding: Rounding,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub now: u64,
    pub registers: Vec<RegisterView>,
    pub mats: Vec<MatView>,
    pub mat_links: Vec<MatLink>,
    pub bell: Bell,
    pub calls: Vec<Call>,
    pub call_history: Vec<ResolvedCall>,
    pub age_rules: AgeRulesView,
    pub audit: Vec<AuditEntry>,
    pub ff_stock: Vec<FfMenuItem>,
    pub mat_loss: u8,
    pub accounts: Vec<AccountView>,
    pub catalog: Vec<CatalogItem>,
    pub tax: TaxView,
    pub categories: Vec<(Category, &'static str)>,
    /// 財布の電子マネーの残高
    pub emoney_balance: u32,
}
