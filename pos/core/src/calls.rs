use serde::Serialize;

/// 最下段の「サービス」ボタンで呼んだときの詳細
pub const SERVICE_BUTTON: &str = "お支払い・荷物";
/// セルフでお支払い方法に「現金」を選んで呼んだときの詳細
pub const CASH_CHOSEN: &str = "現金でのお支払い";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CallKind {
    AgeCheck,
    FastFood,
    Service,
    Tobacco,
    Voucher,
    CustomerHelp,
    ChangeLeft,
    Bill,
    Emoney,
    CashOnly,
    ReceiptStamp,
    Expired,
}

impl CallKind {
    /// 呼び出し画面とログイン後の「用件：〇〇」に出す短い用件。お客様が押したボタンの呼び出しはボタンの文言そのまま、
    /// 商品登録やお会計から発動した呼び出しは「お支払い」「宅配便」など具体的に（user 2026-11-14）
    pub fn topic(self, detail: &str) -> String {
        let s = match self {
            CallKind::AgeCheck => "年確",
            CallKind::FastFood => "FF",
            // 最下段の「サービス」ボタンは detail が「お支払い・荷物」。商品登録から発動するのは荷物の伝票だけ
            CallKind::Service if detail == SERVICE_BUTTON => "サービス",
            CallKind::Service => "宅配便",
            CallKind::Tobacco => "たばこ",
            CallKind::Voucher => "金券",
            CallKind::CustomerHelp => "呼び出し",
            CallKind::ChangeLeft => "釣銭忘れ",
            CallKind::Bill => "お支払い",
            CallKind::Emoney => "電子マネー",
            // 現金のみの商品は商品名（チケット・切手）、複数なら最初の一つ。お支払い方法で現金を選んだときは「現金」
            CallKind::CashOnly => match detail.split('・').next() {
                Some(n) if !n.is_empty() && detail != CASH_CHOSEN => n,
                _ => "現金",
            },
            CallKind::ReceiptStamp => "収入印紙",
            CallKind::Expired => "販売期限切れ",
        };
        s.to_string()
    }

    pub fn label(self) -> &'static str {
        match self {
            CallKind::AgeCheck => "年齢確認",
            CallKind::FastFood => "FF の注文",
            CallKind::Service => "サービス受付",
            CallKind::Tobacco => "たばこ",
            CallKind::Voucher => "金券",
            CallKind::CustomerHelp => "お呼び出し",
            CallKind::ChangeLeft => "釣銭忘れ",
            CallKind::Bill => "公共料金",
            CallKind::Emoney => "電子マネー",
            CallKind::CashOnly => "現金",
            CallKind::ReceiptStamp => "収入印紙",
            CallKind::Expired => "販売期限切れ",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Call {
    pub id: u32,
    pub register: u8,
    pub kind: CallKind,
    pub label: &'static str,
    /// 用件（[`CallKind::topic`]）
    pub topic: String,
    pub created_at: u64,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ResolvedCall {
    pub id: u32,
    pub register: u8,
    pub label: &'static str,
    pub created_at: u64,
    pub resolved_at: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Calls {
    pub(crate) next_id: u32,
    pub open: Vec<Call>,
    pub history: Vec<ResolvedCall>,
    pub raised: u64,
}

impl Calls {
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
        let detail = detail.into();
        self.open.push(Call {
            id: self.next_id,
            register,
            kind,
            label: kind.label(),
            topic: kind.topic(&detail),
            created_at: now,
            detail,
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
        c.resolve(1, CallKind::CustomerHelp, 3);
        assert!(c.is_open(2, CallKind::FastFood));
        assert!(!c.is_open(1, CallKind::CustomerHelp));
    }
}

#[cfg(test)]
mod tests_topic {
    use super::*;

    #[test]
    fn topic_is_concrete() {
        assert_eq!(CallKind::Service.topic(SERVICE_BUTTON), "サービス");
        assert_eq!(CallKind::Service.topic("荷物を送る 発払い"), "宅配便");
        assert_eq!(CallKind::Bill.topic("3枚"), "お支払い");
        assert_eq!(CallKind::CashOnly.topic("チケット・切手"), "チケット");
        assert_eq!(CallKind::CashOnly.topic(CASH_CHOSEN), "現金");
    }
}
