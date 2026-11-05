use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::catalog::{self, TaxRate};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxKind {
    Standard,
    Reduced,
    Zero,
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

    fn always_zero(self) -> bool {
        matches!(self, TaxKind::Zero | TaxKind::Exempt)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rounding {
    Floor,
    HalfUp,
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
    pub rate_bp: u32,
}

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
    pub products: BTreeMap<String, String>,
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

    fn default_class(rate: TaxRate) -> &'static str {
        match rate {
            TaxRate::Standard10 => "standard",
            TaxRate::Reduced8 => "reduced",
        }
    }

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
