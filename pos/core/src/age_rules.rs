//! 年齢確認ルール（問題7への対策と、店舗判断での年確）。
//!
//! - 年確の対象は商品マスタに固定せず、このルール表で決める。
//! - 法令ルール（酒・たばこ 20 歳）は全店共通で、店舗からは外せない。
//! - 店舗ルールはオーナーだけが追加・削除できる（ライター、18禁書籍など）。
//!   分類単位でも、個別の商品単位でも指定できる。
//! - 判定するのは会計ボタンを押した時点だけ。対象品が複数あれば、
//!   最も厳しい年齢で 1 回だけ確認する。
//! - あわせて、商品ごとの販売個数の上限（1 回のお会計で何点まで）もオーナーが決める。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::catalog::{self, Category};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleTarget {
    Category { category: Category },
    Product { code: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct AgeRule {
    pub id: u32,
    pub target: RuleTarget,
    pub label: String,
    pub min_age: u8,
    /// 法令ルールは削除できない
    pub legal: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleChange {
    pub at: u64,
    pub by: String,
    pub change: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AgeRules {
    pub rules: Vec<AgeRule>,
    pub history: Vec<RuleChange>,
    /// 商品コード → 1 回のお会計で売れる上限の個数
    pub limits: BTreeMap<String, u32>,
    #[serde(skip)]
    next_id: u32,
}

fn label_of(target: &RuleTarget) -> String {
    match target {
        RuleTarget::Category { category } => format!("分類：{}", category.label()),
        RuleTarget::Product { code } => match catalog::find(code) {
            Some(p) => format!("商品：{}", p.name),
            None => format!("商品：{code}"),
        },
    }
}

impl Default for AgeRules {
    fn default() -> Self {
        let mut r = AgeRules {
            rules: vec![],
            history: vec![],
            limits: BTreeMap::new(),
            next_id: 1,
        };
        r.push(
            RuleTarget::Category {
                category: Category::Alcohol,
            },
            20,
            true,
        );
        r.push(
            RuleTarget::Category {
                category: Category::Tobacco,
            },
            20,
            true,
        );
        // 店舗ルールの初期値（オーナーが商品ごとに変更できる）
        for p in catalog::CATALOG {
            let age = match p.category {
                Category::SmokingGoods => 20,
                Category::AdultBook | Category::AdultGoods => 18,
                _ => continue,
            };
            r.push(
                RuleTarget::Product {
                    code: p.code.into(),
                },
                age,
                false,
            );
        }
        r
    }
}

impl AgeRules {
    fn push(&mut self, target: RuleTarget, min_age: u8, legal: bool) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.rules.push(AgeRule {
            id,
            label: label_of(&target),
            target,
            min_age,
            legal,
        });
        id
    }

    /// この商品に適用される最も厳しい年齢（対象外なら None）
    pub fn age_for(&self, code: &str, category: Category) -> Option<u8> {
        self.rules
            .iter()
            .filter(|r| match &r.target {
                RuleTarget::Category { category: c } => *c == category,
                RuleTarget::Product { code: c } => c == code,
            })
            .map(|r| r.min_age)
            .max()
    }

    /// 法令ルールで決まる年齢（店舗からは変えられない部分）
    pub fn legal_age_for(&self, code: &str, category: Category) -> Option<u8> {
        self.rules
            .iter()
            .filter(|r| r.legal)
            .filter(|r| match &r.target {
                RuleTarget::Category { category: c } => *c == category,
                RuleTarget::Product { code: c } => c == code,
            })
            .map(|r| r.min_age)
            .max()
    }

    /// この商品だけに店舗が付けた年齢
    pub fn store_age_for(&self, code: &str) -> Option<u8> {
        self.rules
            .iter()
            .find(|r| !r.legal && r.target == RuleTarget::Product { code: code.into() })
            .map(|r| r.min_age)
    }

    /// 商品ごとの店舗ルールを設定する（None で外す）。法令の対象品（酒・たばこ）は変更できない。
    pub fn set_product(
        &mut self,
        code: &str,
        min_age: Option<u8>,
        by: &str,
        now: u64,
    ) -> Result<(), String> {
        let p = catalog::find(code).ok_or("商品が見つかりません")?;
        if let Some(l) = self.legal_age_for(code, p.category) {
            return Err(format!(
                "{}は法令どおり{l}歳以上で固定です（変更できません）",
                p.name
            ));
        }
        if let Some(a) = min_age {
            if !(1..=99).contains(&a) {
                return Err("年齢は 1〜99 で指定してください".into());
            }
        }
        let before = self.store_age_for(code);
        if before == min_age {
            return Ok(());
        }
        let target = RuleTarget::Product { code: code.into() };
        self.rules.retain(|r| r.legal || r.target != target);
        if let Some(a) = min_age {
            self.push(target, a, false);
        }
        let fmt = |a: Option<u8>| a.map_or("なし".to_string(), |a| format!("{a}歳以上"));
        self.history.push(RuleChange {
            at: now,
            by: by.into(),
            change: format!("{}：{} → {}", p.name, fmt(before), fmt(min_age)),
        });
        Ok(())
    }

    /// この商品を 1 回のお会計で売れる上限（なければ None）
    pub fn limit_for(&self, code: &str) -> Option<u32> {
        self.limits.get(code).copied()
    }

    /// 商品ごとの販売個数の上限を設定する（None で外す）
    pub fn set_limit(
        &mut self,
        code: &str,
        limit: Option<u32>,
        by: &str,
        now: u64,
    ) -> Result<(), String> {
        let p = catalog::find(code).ok_or("商品が見つかりません")?;
        if let Some(n) = limit {
            if !(1..=99).contains(&n) {
                return Err("販売個数の上限は 1〜99 で指定してください".into());
            }
        }
        let before = self.limit_for(code);
        if before == limit {
            return Ok(());
        }
        match limit {
            Some(n) => self.limits.insert(code.into(), n),
            None => self.limits.remove(code),
        };
        let fmt = |n: Option<u32>| n.map_or("制限なし".to_string(), |n| format!("{n}点まで"));
        self.history.push(RuleChange {
            at: now,
            by: by.into(),
            change: format!("{}の販売個数：{} → {}", p.name, fmt(before), fmt(limit)),
        });
        Ok(())
    }

    pub fn add(
        &mut self,
        target: RuleTarget,
        min_age: u8,
        by: &str,
        now: u64,
    ) -> Result<u32, String> {
        if !(1..=99).contains(&min_age) {
            return Err("年齢は 1〜99 で指定してください".into());
        }
        if let RuleTarget::Product { code } = &target {
            let p = catalog::find(code).ok_or("商品が見つかりません")?;
            if self.legal_age_for(code, p.category).is_some() {
                return Err(format!(
                    "{}は法令どおりで固定です（変更できません）",
                    p.name
                ));
            }
        }
        if self.rules.iter().any(|r| r.target == target) {
            return Err("同じ対象のルールがすでにあります".into());
        }
        let label = label_of(&target);
        let id = self.push(target, min_age, false);
        self.history.push(RuleChange {
            at: now,
            by: by.into(),
            change: format!("追加：{label}（{min_age}歳以上）"),
        });
        Ok(id)
    }

    pub fn remove(&mut self, id: u32, by: &str, now: u64) -> Result<(), String> {
        let i = self
            .rules
            .iter()
            .position(|r| r.id == id)
            .ok_or("ルールが見つかりません")?;
        if self.rules[i].legal {
            return Err("法令ルールは外せません".into());
        }
        let r = self.rules.remove(i);
        self.history.push(RuleChange {
            at: now,
            by: by.into(),
            change: format!("削除：{}（{}歳以上）", r.label, r.min_age),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_rules_cannot_be_removed() {
        let mut r = AgeRules::default();
        assert!(r.remove(1, "オーナー", 0).is_err());
        assert_eq!(r.age_for("4900000000103", Category::Alcohol), Some(20));
    }

    #[test]
    fn store_rule_by_product_takes_the_strictest_age() {
        let mut r = AgeRules::default();
        assert_eq!(r.age_for("4900000000028", Category::Food), None);
        r.add(
            RuleTarget::Product {
                code: "4900000000028".into(),
            },
            20,
            "オーナー",
            0,
        )
        .unwrap();
        assert_eq!(r.age_for("4900000000028", Category::Food), Some(20));
        r.add(
            RuleTarget::Category {
                category: Category::Food,
            },
            18,
            "オーナー",
            0,
        )
        .unwrap();
        assert_eq!(r.age_for("4900000000028", Category::Food), Some(20));
        assert_eq!(r.history.len(), 2);
    }

    #[test]
    fn every_product_can_have_its_own_store_age() {
        let mut r = AgeRules::default();
        let lighter = "4900000000134";
        assert_eq!(r.store_age_for(lighter), Some(20));
        r.set_product(lighter, Some(18), "オーナー", 0).unwrap();
        assert_eq!(r.age_for(lighter, Category::SmokingGoods), Some(18));
        r.set_product(lighter, None, "オーナー", 0).unwrap();
        assert_eq!(r.age_for(lighter, Category::SmokingGoods), None);
        // 法令の対象（酒・たばこ）は法令どおりで、緩くも厳しくもできない
        let beer = "4900000000103";
        for age in [None, Some(18), Some(25)] {
            assert!(r.set_product(beer, age, "オーナー", 0).is_err());
        }
        assert_eq!(r.age_for(beer, Category::Alcohol), Some(20));
        assert_eq!(r.history.len(), 2);
    }
}
