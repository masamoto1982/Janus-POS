use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxRate {
    Reduced8,
    Standard10,
}

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
    pub code: &'static str,
    pub name: &'static str,
    pub price: u32,
    pub tax: TaxRate,
    pub category: Category,
    pub needs_staff: bool,
    pub utility_bill: bool,
    pub cash_only: bool,
    pub parcel_send: bool,
    pub parcel_pickup: bool,
    pub parcel_collect: bool,
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
        parcel_pickup: false,
        parcel_collect: false,
    }
}

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
    p("4900000000028", "サンドイッチ", 320, Reduced8, Food),
    p("4900000000042", "牛乳", 198, Reduced8, Drink),
    p("4900000000011", "パン", 160, Reduced8, Food),
    p("4900000000035", "ソフトドリンク", 150, Reduced8, Drink),
    p("4900000000103", "アルコール", 248, Standard10, Alcohol),
    p("4900000000073", "袋菓子", 150, Reduced8, Food),
    p("4900000000066", "即席麺", 214, Reduced8, Food),
    p("4910000000014", "新聞", 160, Standard10, Newspaper),
    p("4910000000038", "雑誌", 480, Standard10, Magazine),
    p("4900000000127", "たばこ", 600, Standard10, Tobacco),
    p("4900000000134", "ライター", 110, Standard10, SmokingGoods),
    p("4900000000141", "切手", 110, Standard10, Stamp),
    p("2000000000015", "レジ袋 S", 3, Standard10, Bag),
    p("2000000000022", "レジ袋 L", 5, Standard10, Bag),
    p("2000000000039", "フォーク", 3, Standard10, Bag),
    p("2000000000046", "スプーン", 3, Standard10, Bag),
    p("2000000000053", "お手拭き", 3, Standard10, Bag),
    Product {
        parcel_send: true,
        ..p("2100000000014", "荷物を送る 発払い", 0, Standard10, Service)
    },
    Product {
        parcel_send: true,
        parcel_collect: true,
        ..p("2100000000045", "荷物を送る 着払い", 0, Standard10, Service)
    },
    Product {
        parcel_pickup: true,
        ..p("2100000000038", "荷物を受取る", 0, Standard10, Service)
    },
    bill("2100000000021", "電気料金", 5000),
    bill("2100000000043", "水道料金", 4000),
    bill("2100000000050", "ガス料金", 3000),
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

/// 作り置きの概念が無い FF（注文を受けてから作る・注ぐ）。在庫を持たず、ボタンはいつも積極
pub const FF_MADE_TO_ORDER: &[&str] = &[
    "FF31", "FF32", "FF33", "FF34", "FF35", "FF36", "FF37", "FF39", "FF51", "FF52", "FF53", "FF54",
];

/// 作り置きのある FF の初期在庫（二桁＝豊富、一桁＝希薄、0＝無し）
pub const INITIAL_FF_STOCK: &[(&str, u32)] = &[
    ("FF01", 12),
    ("FF02", 2),
    ("FF03", 15),
    ("FF04", 0),
    ("FF21", 10),
    ("FF22", 6),
    ("FF23", 4),
    ("FF24", 11),
    ("FF05", 10),
    ("FF61", 3),
    ("FF62", 12),
    ("FF63", 0),
    ("FF71", 20),
    ("FF73", 14),
    ("FF74", 5),
    ("FF75", 8),
];

pub fn is_made_to_order(code: &str) -> bool {
    FF_MADE_TO_ORDER.contains(&code)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scanned {
    Product(&'static Product),
    PointCard(String),
    Coupon(&'static Coupon),
    Staff(String),
    Pickup(String),
    ParcelSlip(String),
    Unknown,
}

pub const POINT_CARD_PREFIX: &str = "PC";
pub const COUPON_PREFIX: &str = "CP";
pub const PICKUP_PREFIX: &str = "PK";
pub const SLIP_PREFIX: &str = "PS";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "yen")]
pub enum CouponKind {
    Exchange,
    Discount(u32),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Coupon {
    pub code: &'static str,
    pub name: &'static str,
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
    if let Some(no) = code.strip_prefix(PICKUP_PREFIX) {
        return Scanned::Pickup(no.to_string());
    }
    if let Some(no) = code.strip_prefix(SLIP_PREFIX) {
        return Scanned::ParcelSlip(no.to_string());
    }
    if code.starts_with(COUPON_PREFIX) {
        return COUPONS
            .iter()
            .find(|c| c.code == code)
            .map_or(Scanned::Unknown, Scanned::Coupon);
    }
    match find(code) {
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
                find(code).is_some()
                    && (INITIAL_FF_STOCK.iter().any(|(c, _)| c == code) != is_made_to_order(code)),
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
        for p in CATALOG.iter().filter(|p| p.category == Category::FastFood) {
            assert!(FF_MENU.iter().any(|(c, _)| *c == p.code), "{}", p.code);
            assert!(
                INITIAL_FF_STOCK.iter().any(|(c, _)| *c == p.code) || is_made_to_order(p.code),
                "{}",
                p.code
            );
        }
        assert_eq!(classify("9999"), Scanned::Unknown);
    }
}
