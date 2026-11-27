use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TaxRate {
    Reduced8,
    Standard10,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    Voucher,
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
            Category::Voucher => "金券",
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
        Category::Voucher,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    /// 複数口の伝票。同じお届け先の荷物を続けて受け付ける
    pub parcel_multi: bool,
    /// 税金（公金）の払込票。領収証に収入印紙が要らない（印紙税の非課税）
    pub stamp_exempt: bool,
    /// チケットの申込券。支払いが済むとこの枚数のチケットを発券する（チケットでなければ 0）
    pub tickets: u32,
    /// 日配食品。販売期限切れの品が紛れないよう、レジ画面で数量を増やせない（一点ずつスキャンする）
    pub fresh: bool,
    /// 販売期限と賞味期限（日配食品・加工食品だけ）
    pub dates: Option<Dates>,
}

/// 販売期限と賞味期限。シミュレーターをいつ開いても同じ状態になるよう、いまからの分で持つ
/// （負の値は期限切れ）。販売期限は賞味期限より前に店が決める売り場から下げる時刻
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Dates {
    pub sell_by_min: i32,
    pub best_before_min: i32,
}

impl Product {
    /// 販売期限が切れている
    pub fn expired(&self) -> bool {
        self.dates.is_some_and(|d| d.sell_by_min < 0)
    }
}

const H: i32 = 60;
const D: i32 = 24 * H;

/// 日配食品（販売期限・賞味期限つき、数量を増やせない）
const fn fresh(
    code: &'static str,
    name: &'static str,
    price: u32,
    category: Category,
    sell_by_min: i32,
    best_before_min: i32,
) -> Product {
    Product {
        fresh: true,
        dates: Some(Dates {
            sell_by_min,
            best_before_min,
        }),
        ..p(code, name, price, Reduced8, category)
    }
}

/// 加工食品（販売期限・賞味期限つき）
const fn packaged(
    code: &'static str,
    name: &'static str,
    price: u32,
    tax: TaxRate,
    category: Category,
    sell_by_min: i32,
    best_before_min: i32,
) -> Product {
    Product {
        dates: Some(Dates {
            sell_by_min,
            best_before_min,
        }),
        ..p(code, name, price, tax, category)
    }
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
        parcel_multi: false,
        stamp_exempt: false,
        tickets: 0,
        fresh: false,
        dates: None,
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

/// トレカ。1回のお会計で買えるのは既定で3パックまで（設定 → 個数制限で変えられる）
pub const TRADING_CARD: &str = "4910000000090";
/// トレカの 1 回のお会計の販売個数の上限（既定値。設定で変えられる）
pub const TRADING_CARD_LIMIT: u32 = 3;

pub const CATALOG: &[Product] = &[
    // 日配食品：販売期限・賞味期限は時刻まで。数量はレジ画面で増やせない
    fresh("4900000000028", "サンドイッチ", 320, Food, 6 * H, 10 * H),
    fresh("4900000000011", "パン", 160, Food, D, 2 * D),
    // 販売期限切れを登録したときのデモ。賞味期限はまだ先でも、販売期限を過ぎたら売らない
    fresh("4900000000080", "おにぎり", 150, Food, -2 * H, 4 * H),
    // 加工食品：販売期限・賞味期限は日付まで
    packaged(
        "4900000000035",
        "ソフトドリンク",
        150,
        Reduced8,
        Drink,
        150 * D,
        180 * D,
    ),
    packaged(
        "4900000000103",
        "アルコール",
        248,
        Standard10,
        Alcohol,
        240 * D,
        270 * D,
    ),
    packaged(
        "4900000000073",
        "袋菓子",
        150,
        Reduced8,
        Food,
        90 * D,
        120 * D,
    ),
    p("4910000000014", "新聞", 160, Standard10, Newspaper),
    p("4910000000038", "雑誌", 480, Standard10, Magazine),
    p("4900000000127", "たばこ", 600, Standard10, Tobacco),
    // コンビニでは比較的高額な品。たくさん買うと合計が税抜 5 万円を超え、レシートに収入印紙が要る
    p("4900000000158", "たばこカートン", 6000, Standard10, Tobacco),
    // 年確18。20歳の確認を済ませたあとに加わると、確認をやり直すデモ
    p("4910000000076", "花火", 980, Standard10, Fireworks),
    p("4910000000069", "化粧品", 2200, Standard10, Daily),
    // トレーディングカード（1パック）。金券の扱いで、スタッフ操作モードでだけ売る。
    // 品薄になりやすく、販売規則の個数制限で1回のお会計3パックまで
    p(TRADING_CARD, "トレカ", 180, Standard10, Voucher),
    p("4900000000134", "ライター", 110, Standard10, SmokingGoods),
    p("4900000000141", "切手 110円", 110, Standard10, Stamp),
    // バーコードのない金券（スタッフ操作モードのレジカウンター側の「金券」のタブから打つ、user 2026-11-26）。
    // 金券類は管理が難しいので需要の少ないものは扱わない（印紙は 200 円だけ、コンビニの一般的な運用）。
    // 切手と同じく現金のみ・スタッフがログインしているレジでのみ扱う
    p("4900000000189", "切手 85円", 85, Standard10, Stamp),
    p("4900000000165", "はがき 通常", 85, Standard10, Stamp),
    p(
        "4900000000219",
        "はがき インクジェット",
        85,
        Standard10,
        Stamp,
    ),
    p("4900000000172", "収入印紙 200円", 200, Standard10, Stamp),
    p(
        "4900000000196",
        "ゴミ処理券 300円 東京都",
        300,
        Standard10,
        Stamp,
    ),
    p("2000000000015", "レジ袋 S", 3, Standard10, Bag),
    p("2000000000039", "フォーク", 3, Standard10, Bag),
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
        parcel_send: true,
        parcel_multi: true,
        ..p("2100000000081", "荷物を送る 複数口", 0, Standard10, Service)
    },
    Product {
        parcel_pickup: true,
        ..p("2100000000038", "荷物を受取る", 0, Standard10, Service)
    },
    bill("2100000000021", "電気料金", 50000),
    bill("2100000000043", "水道料金", 4000),
    // 申込券1枚でチケット2枚（7,800円×2）。支払いが済むと発券し、スタッフが枚数を照合してお渡しする
    Product {
        needs_staff: false,
        cash_only: true,
        tickets: 2,
        ..p("2100000000074", "チケット", 15600, Standard10, Service)
    },
    // 同じ5万円でも税金の領収証には収入印紙が要らない
    Product {
        stamp_exempt: true,
        ..bill("2100000000067", "自動車税", 50000)
    },
    p("FF01", "チキン", 220, Reduced8, FastFood),
    p("FF02", "辛口チキン", 220, Reduced8, FastFood),
    p("FF03", "フランク", 140, Reduced8, FastFood),
    p("FF04", "ポテト", 110, Reduced8, FastFood),
    p("FF21", "えびフライ", 140, Reduced8, FastFood),
    p("FF22", "白身フライ", 140, Reduced8, FastFood),
    p("FF23", "大学芋", 250, Reduced8, FastFood),
    p("FF24", "ドーナツ", 200, Reduced8, FastFood),
    p("FF31", "ホット珈琲 S", 110, Reduced8, FastFood),
    p("FF32", "ホット珈琲 M", 150, Reduced8, FastFood),
    p("FF33", "ホット珈琲 L", 190, Reduced8, FastFood),
    p("FF34", "アイス珈琲 S", 110, Reduced8, FastFood),
    p("FF35", "アイス珈琲 M", 150, Reduced8, FastFood),
    p("FF36", "アイス珈琲 L", 190, Reduced8, FastFood),
    p("FF37", "抹茶ラテ", 180, Reduced8, FastFood),
    p("FF39", "カフェラテ", 180, Reduced8, FastFood),
    p("FF51", "バニラソフト", 300, Reduced8, FastFood),
    p("FF52", "チョコパフェ", 380, Reduced8, FastFood),
    p("FF53", "かき氷", 250, Reduced8, FastFood),
    p("FF54", "フラッペ", 320, Reduced8, FastFood),
    p("FF05", "肉まん", 180, Reduced8, FastFood),
    p("FF61", "ピザまん", 190, Reduced8, FastFood),
    p("FF62", "あんまん", 160, Reduced8, FastFood),
    p("FF63", "カレーまん", 190, Reduced8, FastFood),
    p("FF71", "じゃがいも", 100, Reduced8, FastFood),
    p("FF72", "たこ", 150, Reduced8, FastFood),
    p("FF73", "こんにゃく", 90, Reduced8, FastFood),
    p("FF74", "煮卵", 100, Reduced8, FastFood),
    p("FF76", "えび巻き", 130, Reduced8, FastFood),
    p("FF77", "キャベツ巻", 150, Reduced8, FastFood),
    p("FF78", "いか巻き", 130, Reduced8, FastFood),
    p("FF79", "しいたけ", 90, Reduced8, FastFood),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FfTab {
    Fried,
    Drink,
    Cold,
    Bun,
    Oden,
}

impl FfTab {
    pub const ALL: [FfTab; 5] = [
        FfTab::Fried,
        FfTab::Drink,
        FfTab::Cold,
        FfTab::Bun,
        FfTab::Oden,
    ];

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
    ("FF74", FfTab::Oden),
    ("FF73", FfTab::Oden),
    ("FF72", FfTab::Oden),
    ("FF76", FfTab::Oden),
    ("FF77", FfTab::Oden),
    ("FF78", FfTab::Oden),
    ("FF79", FfTab::Oden),
];

/// カートンのたばこ（10 個入り）。仮登録の単位（個・カートン）と読み取ったたばこの単位を合わせるのに使う（user 2026-11-26）
pub const TOBACCO_CARTONS: &[&str] = &["4900000000158"];

pub fn is_carton(code: &str) -> bool {
    TOBACCO_CARTONS.contains(&code)
}

pub fn find(code: &str) -> Option<&'static Product> {
    CATALOG.iter().find(|p| p.code == code)
}

/// 仮登録のたばこの明細が指す商品（棚番号だけで、まだどの商品か決まっていない）。値段は 0 円、
/// 1 個ずつの行で、スタッフが商品のたばこを読み取るたびに一行ずつ本登録になり、その商品に置き換わる。カタログ（Item）には並べない
pub static TOBACCO_SHELF: Product = p(
    "tobacco-shelf",
    "たばこ",
    0,
    TaxRate::Standard10,
    Category::Tobacco,
);

/// 作り置きの概念が無い FF（注文を受けてから作る・注ぐ）。在庫を持たず、ボタンはいつも積極
pub const FF_MADE_TO_ORDER: &[&str] = &[
    "FF31", "FF32", "FF33", "FF34", "FF35", "FF36", "FF37", "FF39", "FF51", "FF52", "FF53", "FF54",
];

/// 作り置きのある FF の初期在庫（二桁＝豊富、一桁＝希薄、0＝無し）。
/// 売れ筋は浅い階層に置くのが実務の常なので、1ページ目を豊富・2ページ目を希薄にしている
/// （Janus-POS の仕組みが決めることではない。並べ替えはストアコンピューターで行う）
pub const INITIAL_FF_STOCK: &[(&str, u32)] = &[
    // フライヤー 1ページ目：豊富
    ("FF01", 12),
    ("FF02", 10),
    ("FF03", 15),
    ("FF04", 11),
    // フライヤー 2ページ目：希薄・無し
    ("FF21", 6),
    ("FF22", 4),
    ("FF23", 0),
    ("FF24", 0),
    // 中華まん
    ("FF05", 12),
    ("FF61", 10),
    ("FF62", 3),
    ("FF63", 0),
    // おでん 1ページ目：豊富
    ("FF71", 20),
    ("FF74", 14),
    ("FF73", 12),
    ("FF72", 10),
    // おでん 2ページ目：希薄・無し
    ("FF76", 5),
    ("FF77", 3),
    ("FF78", 4),
    ("FF79", 0),
];

/// 四段目・五段目のナビゲーション（user 2026-11-25・26）：四段目のボタン（FF・金券・たばこ・その他）と、その下の五段目のタブ。
/// レジカウンター側（スタッフ操作モード）はすべて使え、売り場側（セルフ操作モード）はボタン・タブごとに公開を設定する
/// （設定「売り場側の公開」）。公開していないボタン・タブを売り場側で押すと、そのボタンの用件の呼び出しになる
pub struct NavButton {
    pub key: &'static str,
    pub label: &'static str,
    pub tabs: &'static [NavTab],
}

pub struct NavTab {
    pub key: &'static str,
    pub label: &'static str,
    /// タブの商品（FF のタブは FF_MENU から引くので空）
    pub codes: &'static [&'static str],
}

const fn tab(key: &'static str, label: &'static str, codes: &'static [&'static str]) -> NavTab {
    NavTab { key, label, codes }
}

pub const NAV: &[NavButton] = &[
    NavButton {
        key: "ff",
        label: "FF",
        tabs: &[
            tab("fried", "フライヤー", &[]),
            tab("drink", "ドリンク", &[]),
            tab("cold", "コールド", &[]),
            tab("bun", "中華まん", &[]),
            tab("oden", "おでん", &[]),
        ],
    },
    // バーコードのない金券。管理が難しいので需要の少ないものは扱わない（印紙は 200 円だけ、user 2026-11-26）
    NavButton {
        key: "voucher",
        label: "金券",
        tabs: &[
            tab("g_stamp", "切手", &["4900000000189", "4900000000141"]),
            tab("g_postcard", "はがき", &["4900000000165", "4900000000219"]),
            tab("g_revenue", "印紙", &["4900000000172"]),
            tab("g_garbage", "ゴミ処理券", &["4900000000196"]),
        ],
    },
    // たばこはスタッフがバーコードで登録するので、レジカウンター側の「たばこ」は今のところ使い道がない（user 2026-11-26、そのまま置く）。
    // その他のタブは拡張用で商品を置かない
    NavButton {
        key: "tobacco",
        label: "たばこ",
        // 銘柄のタブは置かない（user 2026-11-26）。お客様が五段目で棚番号とお求めの個数を直接入力して仮登録し、
        // ほかにもお求めの銘柄があれば「次の銘柄」で続ける
        tabs: &[],
    },
    NavButton {
        key: "other",
        label: "その他",
        tabs: &[
            tab("other_a", "拡張用", &[]),
            tab("other_b", "拡張用", &[]),
            tab("other_c", "拡張用", &[]),
        ],
    },
];

/// ナビゲーションのキー（ボタンまたはタブ）の表示名。「FF」「FF／ドリンク」の形
pub fn nav_label(key: &str) -> Option<String> {
    NAV.iter().find_map(|b| {
        if b.key == key {
            return Some(b.label.to_string());
        }
        b.tabs
            .iter()
            .find(|t| t.key == key)
            .map(|t| format!("{}／{}", b.label, t.label))
    })
}

/// FF のタブのナビゲーションのキー（FfTab の名前と同じ）
pub fn ff_tab_key(t: FfTab) -> &'static str {
    match t {
        FfTab::Fried => "fried",
        FfTab::Drink => "drink",
        FfTab::Cold => "cold",
        FfTab::Bun => "bun",
        FfTab::Oden => "oden",
    }
}

pub fn ff_tab(code: &str) -> Option<FfTab> {
    FF_MENU.iter().find(|(c, _)| *c == code).map(|&(_, t)| t)
}

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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    fn ff_tabs_fit_one_page_except_fryer_drinks_and_oden() {
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
                if matches!(tab, FfTab::Drink | FfTab::Fried | FfTab::Oden) {
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
