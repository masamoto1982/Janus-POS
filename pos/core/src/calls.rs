//! 店員呼び出し（問題8への対策）。
//!
//! - 通知先は「発生したレジ」と「全 MAT」だけ。他のレジには出さない。
//!   どのレジが鳴っているかが、そのまま店員への情報になる。
//! - 呼び出しは一件ずつ独立した記録で、単一の順番待ちキューも、後続を
//!   止めるモーダルも持たない。どれか一件の対応が、他の呼び出しの通知を
//!   妨げることはない。
//! - 呼び出しは原因が片付いたとき（年確が済んだ、FF を登録した、
//!   サービス商品の受付が済んだ、店員がそのレジに来た）に自動で閉じる。

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CallKind {
    AgeCheck,
    FastFood,
    Service,
    Tobacco,
    CustomerHelp,
    /// 排出口に釣り銭が残ったまま 2 分（有人モードは 1 分）たった（釣銭忘れ）
    ChangeLeft,
    /// セミセルフで公共料金の会計ボタンが押された（店員が払込票の内容を確認し、収納印を押す）
    Bill,
    /// セミセルフのお買い物の途中に、電子マネーの扇形が押された（スタッフがログインして有人モードで手続きする）
    Emoney,
}

impl CallKind {
    pub fn label(self) -> &'static str {
        match self {
            CallKind::AgeCheck => "年齢確認",
            CallKind::FastFood => "FF の注文",
            CallKind::Service => "サービス受付",
            CallKind::Tobacco => "たばこ",
            CallKind::CustomerHelp => "お呼び出し",
            CallKind::ChangeLeft => "釣銭忘れ",
            CallKind::Bill => "公共料金",
            CallKind::Emoney => "電子マネー",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Call {
    pub id: u32,
    pub register: u8,
    pub kind: CallKind,
    pub label: &'static str,
    pub created_at: u64,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedCall {
    pub id: u32,
    pub register: u8,
    pub label: &'static str,
    pub created_at: u64,
    pub resolved_at: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Calls {
    next_id: u32,
    pub open: Vec<Call>,
    pub history: Vec<ResolvedCall>,
    /// 新しい呼び出しが立った回数（MAT の鳴動合図に使う）
    pub raised: u64,
}

impl Calls {
    /// 同じレジ・同じ種類の呼び出しがすでに開いていれば、新たには立てない
    pub fn raise(
        &mut self,
        register: u8,
        kind: CallKind,
        now: u64,
        detail: impl Into<String>,
    ) -> bool {
        if self
            .open
            .iter()
            .any(|c| c.register == register && c.kind == kind)
        {
            return false;
        }
        self.next_id += 1;
        self.open.push(Call {
            id: self.next_id,
            register,
            kind,
            label: kind.label(),
            created_at: now,
            detail: detail.into(),
        });
        self.raised += 1;
        true
    }

    pub fn resolve(&mut self, register: u8, kind: CallKind, now: u64) {
        while let Some(i) = self
            .open
            .iter()
            .position(|c| c.register == register && c.kind == kind)
        {
            let c = self.open.remove(i);
            self.history.push(ResolvedCall {
                id: c.id,
                register: c.register,
                label: c.label,
                created_at: c.created_at,
                resolved_at: now,
            });
            if self.history.len() > 30 {
                self.history.remove(0);
            }
        }
    }

    pub fn for_register(&self, register: u8) -> Vec<Call> {
        self.open
            .iter()
            .filter(|c| c.register == register)
            .cloned()
            .collect()
    }

    pub fn is_open(&self, register: u8, kind: CallKind) -> bool {
        self.open
            .iter()
            .any(|c| c.register == register && c.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_are_independent_per_register_and_kind() {
        let mut c = Calls::default();
        assert!(c.raise(2, CallKind::FastFood, 0, ""));
        assert!(c.raise(1, CallKind::CustomerHelp, 1, ""));
        assert!(
            !c.raise(2, CallKind::FastFood, 2, ""),
            "duplicate is not raised twice"
        );
        // 後から来たレジ1を先に片付けても、レジ2はそのまま
        c.resolve(1, CallKind::CustomerHelp, 3);
        assert!(c.is_open(2, CallKind::FastFood));
        assert!(!c.is_open(1, CallKind::CustomerHelp));
    }
}
