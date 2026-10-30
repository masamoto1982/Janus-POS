//! 宅配便（荷物を送る）の料金。
//!
//! 現実の受付と同じく、荷物の三辺の合計と重さから取り扱いサイズを決め、
//! 届け先の郵便番号から地域を引いて、サイズと地域で料金が決まる。
//! 料金表はシミュレーター用の見本（発送するお店は関東にあるものとする）。

use serde::Serialize;

/// 取り扱いサイズ（サイズ, 三辺の合計の上限 cm, 重さの上限 g）。小さい順
pub const SIZES: [(u32, u32, u32); 6] = [
    (60, 60, 2_000),
    (80, 80, 5_000),
    (100, 100, 10_000),
    (120, 120, 15_000),
    (140, 140, 20_000),
    (160, 160, 25_000),
];

/// 一段階目：三辺の合計がいちばん大きいサイズに収まるか
pub fn check_length(sum_cm: u32) -> Result<(), String> {
    if sum_cm == 0 {
        return Err("長さを入力してください".into());
    }
    if sum_cm > SIZES[SIZES.len() - 1].1 {
        return Err("長さが 160cm を超える荷物は受け付けられません".into());
    }
    Ok(())
}

/// 三辺の合計と重さの両方が収まる、いちばん小さいサイズ
pub fn size_for(sum_cm: u32, weight_g: u32) -> Result<u32, String> {
    if sum_cm == 0 || weight_g == 0 {
        return Err("重さを入力してください".into());
    }
    SIZES
        .iter()
        .find(|&&(_, cm, g)| sum_cm <= cm && weight_g <= g)
        .map(|&(size, _, _)| size)
        .ok_or_else(|| {
            if weight_g > SIZES[SIZES.len() - 1].2 {
                "25kg を超える荷物は受け付けられません".into()
            } else {
                "長さが 160cm を超える荷物は受け付けられません".into()
            }
        })
}

/// 届け先の地域（関東から遠いほど料金が上がる）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Region {
    Hokkaido,
    NorthTohoku,
    SouthTohoku,
    Kanto,
    Shinetsu,
    Hokuriku,
    Chubu,
    Kansai,
    Chugoku,
    Shikoku,
    Kyushu,
    Okinawa,
}

impl Region {
    pub fn label(self) -> &'static str {
        match self {
            Region::Hokkaido => "北海道",
            Region::NorthTohoku => "北東北",
            Region::SouthTohoku => "南東北",
            Region::Kanto => "関東",
            Region::Shinetsu => "信越",
            Region::Hokuriku => "北陸",
            Region::Chubu => "中部",
            Region::Kansai => "関西",
            Region::Chugoku => "中国",
            Region::Shikoku => "四国",
            Region::Kyushu => "九州",
            Region::Okinawa => "沖縄",
        }
    }

    /// 関東（発送するお店）からの遠さの段階
    fn tier(self) -> u32 {
        match self {
            Region::Kanto | Region::Shinetsu => 0,
            Region::SouthTohoku | Region::Hokuriku | Region::Chubu => 1,
            Region::NorthTohoku | Region::Kansai => 2,
            Region::Hokkaido | Region::Chugoku | Region::Shikoku => 3,
            Region::Kyushu => 4,
            Region::Okinawa => 6,
        }
    }
}

/// 郵便番号（「123-4567」でも「1234567」でもよい）を 7 桁の数字にそろえる
pub fn normalize_postal(s: &str) -> Result<String, String> {
    let d: String = s
        .chars()
        .filter(|c| !matches!(c, '-' | '－' | 'ー' | ' ' | '　'))
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
            _ => c,
        })
        .collect();
    if d.len() == 7 && d.chars().all(|c| c.is_ascii_digit()) {
        Ok(d)
    } else {
        Err("郵便番号は 7 桁の数字で入力してください".into())
    }
}

/// 郵便番号の上 2 桁から地域を引く（都道府県ごとの番号の割り当てに沿う）
pub fn region(postal: &str) -> Result<Region, String> {
    let d = normalize_postal(postal)?;
    let n: u32 = d[..2].parse().unwrap_or(0);
    Ok(match n {
        0 | 4..=9 => Region::Hokkaido,
        1..=3 => Region::NorthTohoku,
        10..=37 | 40 => Region::Kanto,
        38 | 39 | 94 | 95 => Region::Shinetsu,
        41..=51 => Region::Chubu,
        52..=67 => Region::Kansai,
        68..=75 => Region::Chugoku,
        76..=79 => Region::Shikoku,
        80..=89 => Region::Kyushu,
        90 => Region::Okinawa,
        91..=93 => Region::Hokuriku,
        96..=99 => Region::SouthTohoku,
        _ => return Err("この郵便番号の地域が見つかりません".into()),
    })
}

/// サイズと届け先の地域で決まる料金（税込・円）
pub fn fee(size: u32, region: Region) -> u32 {
    let (base, step) = match size {
        60 => (940, 110),
        80 => (1_230, 120),
        100 => (1_530, 130),
        120 => (1_850, 140),
        140 => (2_190, 150),
        _ => (2_510, 160),
    };
    base + step * region.tier()
}

/// 受付中・受付済みの荷物の内容（明細ごと）
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Parcel {
    /// 三辺の合計（cm）と重さ（g）。サイズを決めたときに入る
    pub sum_cm: u32,
    pub weight_g: u32,
    /// 取り扱いサイズ（未決定なら None）
    pub size: Option<u32>,
    /// 届け先の郵便番号（7 桁）と地域。郵便番号を決めると料金が確定する
    pub postal: Option<String>,
    pub region: Option<&'static str>,
}

/// 荷物を受取る：お預かりしている荷物の番号（お客様の受取用バーコードから引く）と、
/// 保管場所から持ってきた荷物のバーコードを読んで番号が合ったか
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Pickup {
    pub no: String,
    /// 代金引換の金額（円）。荷物が「有り」と決まったら明細の金額になり、会計で受け取る
    pub amount: u32,
    pub matched: bool,
    /// 直前に読んだ、番号の違う荷物（店員側の画面に「お渡しできません」と出す）
    pub wrong: Option<String>,
    /// 保管場所に荷物が見つからなかった（シミュレーターでは店員が「無し」を選ぶ）
    pub missing: bool,
}

impl Pickup {
    /// 荷物のバーコードの番号がお預かりの番号と合うか（区切りの「-」は見ない）
    pub fn matches(&self, scanned: &str) -> bool {
        let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
        digits(&self.no) == digits(scanned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_is_the_smallest_that_fits_both_length_and_weight() {
        assert_eq!(size_for(55, 1_500), Ok(60));
        assert_eq!(size_for(60, 2_000), Ok(60));
        assert_eq!(size_for(61, 1_000), Ok(80));
        // 小さくても重ければ重さで決まる
        assert_eq!(size_for(50, 9_000), Ok(100));
        assert!(size_for(161, 1_000).is_err());
        assert!(size_for(100, 26_000).is_err());
        assert!(size_for(0, 1_000).is_err());
    }

    #[test]
    fn postal_code_decides_the_region_and_fee() {
        assert_eq!(region("100-0001"), Ok(Region::Kanto));
        assert_eq!(region("５３０－０００１"), Ok(Region::Kansai));
        assert_eq!(region("0600001"), Ok(Region::Hokkaido));
        assert_eq!(region("900-0001"), Ok(Region::Okinawa));
        assert!(region("12345").is_err());
        assert_eq!(fee(60, Region::Kanto), 940);
        assert!(fee(60, Region::Kyushu) > fee(60, Region::Kansai));
        assert!(fee(80, Region::Kanto) > fee(60, Region::Kanto));
    }

    #[test]
    fn pickup_number_ignores_separators() {
        let p = Pickup {
            no: "1234-5678-9012".into(),
            amount: 1_000,
            matched: false,
            wrong: None,
            missing: false,
        };
        assert!(p.matches("123456789012"));
        assert!(!p.matches("1234-5678-9013"));
    }
}
