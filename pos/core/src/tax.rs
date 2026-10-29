//! 税率設定（軽減税率・インボイス・ゼロ税率）。
//!
//! - 税率は商品マスタに固定せず、この表の「税区分」で持つ。法改正で税率が変わっても、
//!   新しい税率ができても、オーナーが区分の税率を変える・区分を足すだけで対応できる。
//! - 商品マスタの税率は初期値。オーナーは商品ごとに区分を付け替えられる。
//! - 価格は税込（内税）。インボイス制度に合わせ、1 回のお会計で税率ごとに
//!   対象額を合計してから 1 回だけ端数処理する。
//! - 税率は商品を登録した時点のものを明細に残す（会計の途中で設定を変えても、
//!   登録済みの明細の税率は変わらない）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::catalog::{self, TaxRate};

/// 税区分の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxKind {
    /// 標準税率
    Standard,
    /// 軽減税率
    Reduced,
    /// ゼロ税率（輸出免税など。課税取引だが税率 0%）
    Zero,
    /// 非課税・不課税（代行収納など。消費税の対象外）
    Exempt,
}

impl TaxKind {
    pub fn label(self) -> &'static str {
        match self {
            TaxKind::Standard => "標準税率",
            TaxKind::Reduced => "軽減税率",
            TaxKind::Zero => "ゼロ税率",
            TaxKind::Exempt => "非課税・不課税",
        }
    }

    /// 税率が 0% に決まっている種類
    fn always_zero(self) -> bool {
        matches!(self, TaxKind::Zero | TaxKind::Exempt)
    }
}

/// 端数処理（1 円未満）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rounding {
    /// 切り捨て
    Floor,
    /// 四捨五入
    HalfUp,
    /// 切り上げ
    Ceil,
}

impl Rounding {
    pub fn label(self) -> &'static str {
        match self {
            Rounding::Floor => "切り捨て",
            Rounding::HalfUp => "四捨五入",
            Rounding::Ceil => "切り上げ",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TaxClass {
    pub id: String,
    pub name: String,
    pub kind: TaxKind,
    /// 税率（0.01% 単位。1000 = 10%）
    pub rate_bp: u32,
}

/// 税込の対象額に含まれる消費税額（内税）
pub fn included_tax(gross: u32, rate_bp: u32, rounding: Rounding) -> u32 {
    if rate_bp == 0 {
        return 0;
    }
    let num = u64::from(gross) * u64::from(rate_bp);
    let den = 10_000 + u64::from(rate_bp);
    let tax = match rounding {
        Rounding::Floor => num / den,
        Rounding::HalfUp => (2 * num + den) / (2 * den),
        Rounding::Ceil => num.div_ceil(den),
    };
    tax as u32
}

/// 税率の表示（1000 → "10%"、1050 → "10.5%"）
pub fn rate_label(rate_bp: u32) -> String {
    let whole = rate_bp / 100;
    let frac = rate_bp % 100;
    if frac == 0 {
        format!("{whole}%")
    } else if frac.is_multiple_of(10) {
        format!("{whole}.{}%", frac / 10)
    } else {
        format!("{whole}.{frac:02}%")
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TaxSettings {
    pub classes: Vec<TaxClass>,
    /// 商品コード → 税区分（商品マスタの初期値から付け替えたものだけ）
    pub products: BTreeMap<String, String>,
    /// 適格請求書発行事業者の登録番号（T + 13 桁。未登録なら空）
    pub invoice_no: String,
    pub rounding: Rounding,
    #[serde(skip)]
    next_id: u32,
}

impl Default for TaxSettings {
    fn default() -> Self {
        let class = |id: &str, name: &str, kind, rate_bp| TaxClass {
            id: id.into(),
            name: name.into(),
            kind,
            rate_bp,
        };
        let mut products = BTreeMap::new();
        // 代行収納（お支払い）と切手は消費税の対象外
        for p in catalog::CATALOG {
            if p.cash_only {
                products.insert(p.code.to_string(), "exempt".to_string());
            }
        }
        TaxSettings {
            classes: vec![
                class("standard", "標準税率", TaxKind::Standard, 1000),
                class("reduced", "軽減税率", TaxKind::Reduced, 800),
                class("zero", "ゼロ税率", TaxKind::Zero, 0),
                class("exempt", "非課税・不課税", TaxKind::Exempt, 0),
            ],
            products,
            invoice_no: String::new(),
            rounding: Rounding::Floor,
            next_id: 1,
        }
    }
}

impl TaxSettings {
    fn class(&self, id: &str) -> Option<&TaxClass> {
        self.classes.iter().find(|c| c.id == id)
    }

    /// 商品マスタの税率（初期値）にあたる区分
    fn default_class(rate: TaxRate) -> &'static str {
        match rate {
            TaxRate::Standard10 => "standard",
            TaxRate::Reduced8 => "reduced",
        }
    }

    /// この商品の税区分
    pub fn class_for(&self, code: &str) -> &TaxClass {
        let id = self
            .products
            .get(code)
            .map(String::as_str)
            .or_else(|| catalog::find(code).map(|p| Self::default_class(p.tax)));
        id.and_then(|id| self.class(id))
            .or_else(|| self.class("standard"))
            .unwrap_or(&self.classes[0])
    }

    /// 表示に使う税率（0% を除き、低い順。区分が違っても同じ税率は一つにまとめる）
    pub fn display_rates(&self) -> Vec<u32> {
        let mut rates: Vec<u32> = self
            .classes
            .iter()
            .map(|c| c.rate_bp)
            .filter(|r| *r > 0)
            .collect();
        rates.sort_unstable();
        rates.dedup();
        rates
    }

    /// 区分の税率を変える（法改正への対応）。変更前の説明を返す
    pub fn set_rate(&mut self, id: &str, rate_bp: u32) -> Result<String, String> {
        if rate_bp > 10_000 {
            return Err("税率は 0〜100% で指定してください".into());
        }
        let c = self
            .classes
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or("税区分が見つかりません")?;
        if c.kind.always_zero() && rate_bp != 0 {
            return Err(format!("{}は 0% から変えられません", c.kind.label()));
        }
        if c.rate_bp == rate_bp {
            return Err("変更はありません".into());
        }
        let before = c.rate_bp;
        c.rate_bp = rate_bp;
        Ok(format!(
            "{}：{} → {}",
            c.name,
            rate_label(before),
            rate_label(rate_bp)
        ))
    }

    /// 新しい税区分を足す（新しい税率ができたとき）
    pub fn add_class(&mut self, name: &str, kind: TaxKind, rate_bp: u32) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("区分名を入れてください".into());
        }
        if self.classes.iter().any(|c| c.name == name) {
            return Err("同じ名前の税区分があります".into());
        }
        if rate_bp > 10_000 {
            return Err("税率は 0〜100% で指定してください".into());
        }
        let rate_bp = if kind.always_zero() { 0 } else { rate_bp };
        let id = format!("c{}", self.next_id);
        self.next_id += 1;
        self.classes.push(TaxClass {
            id,
            name: name.into(),
            kind,
            rate_bp,
        });
        Ok(format!(
            "税区分を追加：{name}（{}・{}）",
            kind.label(),
            rate_label(rate_bp)
        ))
    }

    /// 商品の税区分を付け替える
    pub fn set_product(&mut self, code: &str, id: &str) -> Result<String, String> {
        let p = catalog::find(code).ok_or("商品が見つかりません")?;
        let to = self.class(id).ok_or("税区分が見つかりません")?.name.clone();
        let before = self.class_for(code).name.clone();
        if before == to {
            return Err("変更はありません".into());
        }
        if Self::default_class(p.tax) == id && !p.utility_bill {
            self.products.remove(code);
        } else {
            self.products.insert(code.into(), id.into());
        }
        Ok(format!("{}の税区分：{before} → {to}", p.name))
    }

    /// インボイスの登録番号と端数処理
    pub fn set_invoice(&mut self, invoice_no: &str, rounding: Rounding) -> Result<String, String> {
        let no = invoice_no.trim().to_uppercase();
        let valid = no.is_empty()
            || (no.len() == 14
                && no.starts_with('T')
                && no[1..].bytes().all(|b| b.is_ascii_digit()));
        if !valid {
            return Err("登録番号は T と 13 桁の数字で入れてください".into());
        }
        if no == self.invoice_no && rounding == self.rounding {
            return Err("変更はありません".into());
        }
        self.invoice_no = no;
        self.rounding = rounding;
        Ok(format!(
            "インボイス：登録番号 {}・端数 {}",
            if self.invoice_no.is_empty() {
                "未登録"
            } else {
                &self.invoice_no
            },
            rounding.label()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn included_tax_rounds_once_per_rate() {
        assert_eq!(included_tax(248, 1000, Rounding::Floor), 22);
        assert_eq!(included_tax(248, 1000, Rounding::HalfUp), 23);
        assert_eq!(included_tax(220, 1000, Rounding::Ceil), 20);
        assert_eq!(included_tax(108, 800, Rounding::Floor), 8);
        assert_eq!(included_tax(500, 0, Rounding::Ceil), 0);
    }

    #[test]
    fn rate_labels() {
        assert_eq!(rate_label(1000), "10%");
        assert_eq!(rate_label(1050), "10.5%");
        assert_eq!(rate_label(825), "8.25%");
    }

    #[test]
    fn zero_and_exempt_stay_at_zero() {
        let mut t = TaxSettings::default();
        assert!(t.set_rate("zero", 500).is_err());
        assert!(t.set_rate("reduced", 1000).is_ok());
        assert_eq!(t.display_rates(), vec![1000]);
    }

    #[test]
    fn invoice_number_format() {
        let mut t = TaxSettings::default();
        assert!(t.set_invoice("T123", Rounding::Floor).is_err());
        assert!(t.set_invoice("t1234567890123", Rounding::HalfUp).is_ok());
        assert_eq!(t.invoice_no, "T1234567890123");
    }
}
