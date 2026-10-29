//! 商品マスタとバーコードの振り分け。
//!
//! 会計画面になるまでは、どのバーコードもいつでも受け付ける。読んだ
//! バーコードの種類（商品・ポイントカード・スタッフ・レジ袋・サービス商品）
//! を [`classify`] で一か所で判定し、ふさわしい処理へつなげる。
//! 「先に支払方法を選べ」「先にポイントカードを出せ」といった順番の縛りは持たない。
//!
//! 年齢確認の要否は商品マスタには持たせない。商品は「分類」だけを持ち、
//! 年確の対象かどうかは [`crate::age_rules`] の設定で決まる。

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxRate {
    /// 軽減税率 8%
    Reduced8,
    /// 標準税率 10%
    Standard10,
}

/// 商品の分類。年確ルールは分類単位でも指定できる。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Food,
    Drink,
    Daily,
    Newspaper,
    Magazine,
    Alcohol,
    Tobacco,
    SmokingGoods,
    AdultBook,
    AdultGoods,
    Lottery,
    Fireworks,
    FastFood,
    Bag,
    Service,
    /// 切手（現金でしか売れない）
    Stamp,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Food => "食品",
            Category::Drink => "飲料",
            Category::Daily => "日用品",
            Category::Newspaper => "新聞",
            Category::Magazine => "雑誌",
            Category::Alcohol => "酒類",
            Category::Tobacco => "たばこ",
            Category::SmokingGoods => "喫煙具・ライター",
            Category::AdultBook => "18禁書籍",
            Category::AdultGoods => "アダルトグッズ",
            Category::Lottery => "宝くじ",
            Category::Fireworks => "花火",
            Category::FastFood => "FF",
            Category::Bag => "レジ袋",
            Category::Service => "サービス",
            Category::Stamp => "切手",
        }
    }

    pub const ALL: &'static [Category] = &[
        Category::Food,
        Category::Drink,
        Category::Daily,
        Category::Newspaper,
        Category::Magazine,
        Category::Alcohol,
        Category::Tobacco,
        Category::SmokingGoods,
        Category::AdultBook,
        Category::AdultGoods,
        Category::Lottery,
        Category::Fireworks,
        Category::FastFood,
        Category::Bag,
        Category::Service,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Product {
    /// バーコード（FF はバーコードを持たず、店員画面のメニューから登録する）
    pub code: &'static str,
    pub name: &'static str,
    /// 税込価格（円）。荷物のサービス商品は受付時に金額が決まるため 0（払込票は印字された金額）
    pub price: u32,
    pub tax: TaxRate,
    pub category: Category,
    /// 「要店員」：お客様側で読まれたら店員呼び出しをかけ、店員の受付操作を待つ
    pub needs_staff: bool,
    /// 公共料金の払込票（代行収納）。ほかの商品と一緒に登録できず、会計の後に店員が払込票の内容を確認する
    pub utility_bill: bool,
    /// 現金でしか払えない（切手・代行収納）。ほかの商品と一緒に買えるが、
    /// キャッシュレスで払えるのはこれを除いた分だけ（有人・セミセルフ共通）
    pub cash_only: bool,
    /// 宅配便の発送（荷物を送る）。サイズと届け先の郵便番号で料金が決まる。ほかの商品と一緒に会計できない
    pub parcel_send: bool,
}

const fn p(
    code: &'static str,
    name: &'static str,
    price: u32,
    tax: TaxRate,
    category: Category,
) -> Product {
    Product {
        code,
        name,
        price,
        tax,
        category,
        needs_staff: matches!(category, Category::Service),
        utility_bill: false,
        cash_only: matches!(category, Category::Stamp),
        parcel_send: false,
    }
}

/// 公共料金の払込票（代行収納）。読み取りは普通の商品と同じ。会計ボタンで店員を呼び、店員が内容を確認する。現金のみ
const fn bill(code: &'static str, name: &'static str, price: u32) -> Product {
    Product {
        needs_staff: false,
        utility_bill: true,
        cash_only: true,
        ..p(code, name, price, TaxRate::Standard10, Category::Service)
    }
}

use Category::*;
use TaxRate::*;

pub const CATALOG: &[Product] = &[
    // デモ用に商品名は抽象化し（将来 UI を英語にしても困らない一般名）、コンビニの 4 大分類ごとに代表的な商品を二つから四つ置く
    // （FF は下のバーコードなしの商品。レジ画面下のタブから登録する）
    // 日配食品
    p("4900000000028", "サンドイッチ", 320, Reduced8, Food),
    p("4900000000042", "牛乳", 198, Reduced8, Drink),
    p("4900000000011", "パン", 160, Reduced8, Food),
    // 加工食品
    p("4900000000035", "ソフトドリンク", 150, Reduced8, Drink),
    p("4900000000103", "アルコール", 248, Standard10, Alcohol),
    p("4900000000073", "袋菓子", 150, Reduced8, Food),
    p("4900000000066", "即席麺", 214, Reduced8, Food),
    // 非食品
    p("4910000000014", "新聞", 160, Standard10, Newspaper),
    p("4910000000038", "雑誌", 480, Standard10, Magazine),
    p("4900000000127", "たばこ", 600, Standard10, Tobacco),
    // 店舗判断の年確のデモ：ライターは店舗ルールの初期値 20 歳
    p("4900000000134", "ライター", 110, Standard10, SmokingGoods),
    // 切手は現金でしか売れない（消費税は非課税）
    p("4900000000141", "切手", 110, Standard10, Stamp),
    // レジまわり（カトラリー・レジ袋）とサービス（お客様が持ち込む荷物・払込票）
    p("2000000000015", "レジ袋 S", 3, Standard10, Bag),
    p("2000000000022", "レジ袋 L", 5, Standard10, Bag),
    p("2000000000039", "フォーク", 3, Standard10, Bag),
    p("2000000000046", "スプーン", 3, Standard10, Bag),
    p("2000000000053", "お手拭き", 3, Standard10, Bag),
    Product {
        parcel_send: true,
        ..p("2100000000014", "荷物を送る", 0, Standard10, Service)
    },
    p("2100000000038", "荷物を受取る", 0, Standard10, Service),
    // 公共料金の払込票（金額は印字済み）。1 枚目を読むと全部の枚数を尋ね、
    // 申告した枚数を読み終えたらお客様が件数・金額を確認する。ほかの商品とは一緒に登録できない
    // 設計メモ（ユーザー 2026-10-28）：実際の POS レジでは払込票ごとに手数料があり得るが、
    // シミュレーターでは手数料の概念を扱わない（店員の確認は種類ごとの枚数、お客様への案内は枚数だけ。web の billVerifyScreen）
    bill("2100000000021", "電気料金", 5000),
    bill("2100000000043", "水道料金", 4000),
    bill("2100000000050", "ガス料金", 3000),
    // FF（バーコードなし。レジ画面下のタブから登録する）
    p("FF01", "フライドチキン", 220, Reduced8, FastFood),
    p("FF02", "スパイシーチキン", 220, Reduced8, FastFood),
    p("FF03", "アメリカンドック", 140, Reduced8, FastFood),
    p("FF04", "コロッケ", 110, Reduced8, FastFood),
    p("FF21", "焼き鳥（モモシオ）", 140, Reduced8, FastFood),
    p("FF22", "焼き鳥（モモタレ）", 140, Reduced8, FastFood),
    p("FF23", "大学芋", 250, Reduced8, FastFood),
    p("FF24", "揚げ餅", 200, Reduced8, FastFood),
    p("FF31", "ホットコーヒー S", 110, Reduced8, FastFood),
    p("FF32", "ホットコーヒー M", 150, Reduced8, FastFood),
    p("FF33", "ホットコーヒー L", 190, Reduced8, FastFood),
    p("FF34", "アイスコーヒー S", 110, Reduced8, FastFood),
    p("FF35", "アイスコーヒー M", 150, Reduced8, FastFood),
    p("FF36", "アイスコーヒー L", 190, Reduced8, FastFood),
    p("FF37", "ホットカフェラテ S", 180, Reduced8, FastFood),
    p("FF39", "アイスカフェラテ S", 180, Reduced8, FastFood),
    p("FF51", "ミルクソフト（コーン）", 300, Reduced8, FastFood),
    p("FF52", "ミルクソフト（カップ）", 300, Reduced8, FastFood),
    p("FF53", "チョコソフト（コーン）", 320, Reduced8, FastFood),
    p("FF54", "チョコソフト（カップ）", 320, Reduced8, FastFood),
    p("FF05", "肉まん", 180, Reduced8, FastFood),
    p("FF61", "ピザまん", 190, Reduced8, FastFood),
    p("FF62", "あんまん", 160, Reduced8, FastFood),
    p("FF63", "カレーまん", 190, Reduced8, FastFood),
    p("FF71", "大根", 100, Reduced8, FastFood),
    p("FF73", "厚揚げ", 120, Reduced8, FastFood),
    p("FF74", "煮卵", 100, Reduced8, FastFood),
    p("FF75", "牛筋", 150, Reduced8, FastFood),
];

/// FF のタブ（レジ画面の下 1/3）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FfTab {
    Fried,
    Drink,
    Cold,
    Bun,
    Oden,
}

impl FfTab {
    pub fn label(self) -> &'static str {
        match self {
            FfTab::Fried => "フライヤー",
            FfTab::Drink => "ドリンク",
            FfTab::Cold => "コールド",
            FfTab::Bun => "中華まん",
            FfTab::Oden => "おでん",
        }
    }
}

/// FF のタブ分けと並び順。各タブの中は売れ筋順で、先頭ページの最上部に売れ筋が来る。
/// 画面に収めるため各タブ 4 品（1 ページ）。フライヤーとドリンクは 8 品（2 ページ）。
/// フライヤーの 2 ページ目は、廃止した常温総菜タブの商品
pub const FF_MENU: &[(&str, FfTab)] = &[
    ("FF01", FfTab::Fried),
    ("FF02", FfTab::Fried),
    ("FF03", FfTab::Fried),
    ("FF04", FfTab::Fried),
    ("FF21", FfTab::Fried),
    ("FF22", FfTab::Fried),
    ("FF23", FfTab::Fried),
    ("FF24", FfTab::Fried),
    ("FF31", FfTab::Drink),
    ("FF32", FfTab::Drink),
    ("FF33", FfTab::Drink),
    ("FF34", FfTab::Drink),
    ("FF35", FfTab::Drink),
    ("FF36", FfTab::Drink),
    ("FF37", FfTab::Drink),
    ("FF39", FfTab::Drink),
    ("FF51", FfTab::Cold),
    ("FF52", FfTab::Cold),
    ("FF53", FfTab::Cold),
    ("FF54", FfTab::Cold),
    ("FF05", FfTab::Bun),
    ("FF61", FfTab::Bun),
    ("FF62", FfTab::Bun),
    ("FF63", FfTab::Bun),
    ("FF71", FfTab::Oden),
    ("FF73", FfTab::Oden),
    ("FF74", FfTab::Oden),
    ("FF75", FfTab::Oden),
];

pub fn find(code: &str) -> Option<&'static Product> {
    CATALOG.iter().find(|p| p.code == code)
}

/// FF の初期在庫。コロッケは最初から品切れ（登録できないことを確認できる）。
/// カウンターのドリンクはカップがある限り作れるので多めにしておく。
pub const INITIAL_FF_STOCK: &[(&str, u32)] = &[
    ("FF01", 3),
    ("FF02", 2),
    ("FF03", 4),
    ("FF04", 0),
    ("FF21", 6),
    ("FF22", 6),
    ("FF23", 3),
    ("FF24", 4),
    ("FF31", 99),
    ("FF32", 99),
    ("FF33", 99),
    ("FF34", 99),
    ("FF35", 99),
    ("FF36", 99),
    ("FF37", 99),
    ("FF39", 99),
    ("FF51", 20),
    ("FF52", 20),
    ("FF53", 20),
    ("FF54", 20),
    ("FF05", 5),
    ("FF61", 3),
    ("FF62", 4),
    ("FF63", 3),
    ("FF71", 10),
    ("FF73", 10),
    ("FF74", 10),
    ("FF75", 10),
];

/// 読み取ったバーコードの種類。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scanned {
    Product(&'static Product),
    PointCard(String),
    Coupon(&'static Coupon),
    Staff(String),
    Unknown,
}

pub const POINT_CARD_PREFIX: &str = "PC";
pub const COUPON_PREFIX: &str = "CP";

/// クーポンの効き方
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "yen")]
pub enum CouponKind {
    /// 引換券：対象商品 1 個が無料
    Exchange,
    /// 割引券：対象商品 1 個から値引き
    Discount(u32),
}

/// 引換券・割引券。商品より先に読んでも、対象商品が登録されたときに効く
#[derive(Debug, PartialEq, Eq)]
pub struct Coupon {
    pub code: &'static str,
    pub name: &'static str,
    /// 対象商品のバーコード
    pub target: &'static str,
    pub kind: CouponKind,
}

pub const COUPONS: &[Coupon] = &[
    Coupon {
        code: "CP0001",
        name: "引換券：ソフトドリンク",
        target: "4900000000035",
        kind: CouponKind::Exchange,
    },
    Coupon {
        code: "CP0002",
        name: "割引券：サンドイッチ 50円引き",
        target: "4900000000028",
        kind: CouponKind::Discount(50),
    },
];

impl Coupon {
    /// 対象商品 1 個に対する値引き額
    pub fn discount(&self, price: u32) -> u32 {
        match self.kind {
            CouponKind::Exchange => price,
            CouponKind::Discount(yen) => yen.min(price),
        }
    }
}
pub const STAFF_PREFIX: &str = "ST";

pub fn classify(code: &str) -> Scanned {
    let code = code.trim();
    if let Some(no) = code.strip_prefix(POINT_CARD_PREFIX) {
        return Scanned::PointCard(no.to_string());
    }
    if let Some(id) = code.strip_prefix(STAFF_PREFIX) {
        return Scanned::Staff(id.to_string());
    }
    if code.starts_with(COUPON_PREFIX) {
        return COUPONS
            .iter()
            .find(|c| c.code == code)
            .map_or(Scanned::Unknown, Scanned::Coupon);
    }
    match find(code) {
        // FF はバーコードでは登録しない（店員操作モードのメニューからのみ）
        Some(p) if p.category == FastFood => Scanned::Unknown,
        Some(p) => Scanned::Product(p),
        None => Scanned::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ff_tabs_fit_one_page_except_fryer_and_drinks() {
        for tab in [
            FfTab::Fried,
            FfTab::Drink,
            FfTab::Cold,
            FfTab::Bun,
            FfTab::Oden,
        ] {
            let n = FF_MENU.iter().filter(|(_, t)| *t == tab).count();
            assert_eq!(
                n,
                if matches!(tab, FfTab::Drink | FfTab::Fried) {
                    8
                } else {
                    4
                },
                "{tab:?}"
            );
        }
        for (code, _) in FF_MENU {
            assert!(
                find(code).is_some() && INITIAL_FF_STOCK.iter().any(|(c, _)| c == code),
                "{code}"
            );
        }
    }

    #[test]
    fn classifies_every_kind_of_barcode() {
        assert!(matches!(classify("4900000000011"), Scanned::Product(p) if p.category == Food));
        assert_eq!(
            classify("PC12345678"),
            Scanned::PointCard("12345678".into())
        );
        assert_eq!(classify("ST001"), Scanned::Staff("001".into()));
        assert!(matches!(classify("2000000000015"), Scanned::Product(p) if p.category == Bag));
        assert!(matches!(classify("2100000000014"), Scanned::Product(p) if p.needs_staff));
        assert_eq!(classify("FF01"), Scanned::Unknown);
        // FF はどれもタブに入り、在庫を持つ
        for p in CATALOG.iter().filter(|p| p.category == Category::FastFood) {
            assert!(FF_MENU.iter().any(|(c, _)| *c == p.code), "{}", p.code);
            assert!(
                INITIAL_FF_STOCK.iter().any(|(c, _)| *c == p.code),
                "{}",
                p.code
            );
        }
        assert_eq!(classify("9999"), Scanned::Unknown);
    }
}
