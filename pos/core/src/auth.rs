//! 従業員と認証（問題6への対策）。
//!
//! - ログインはスタッフバーコードを読むだけ（繁用操作なので手数を増やさない）。
//! - お金が出ていく操作（返品返金・経費支出・緊急出金・途中集金）は、
//!   ログイン中であっても **操作のたびに** 出金パスワードを求める。
//!   出金パスワードはログインとは別物で、店舗共通の 1 つ。記録には
//!   ログイン中のスタッフ名が残るので、防犯カメラと合わせて操作者を追える。
//! - 年確ルールの変更はオーナーのみ。オーナー本人のパスワードで再認証する。
//! - 権限は画面の表示・非表示ではなく、コアが操作ごとに判定する。
//! - パスワードの連続失敗は一時ロックする。

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Crew,
    Owner,
}

#[derive(Clone, Copy, Debug)]
pub struct StaffAccount {
    pub id: &'static str,
    pub name: &'static str,
    pub role: Role,
    /// オーナーの再認証用パスワード（シミュレーター用の固定値。実機ではハッシュで保持する）
    password: &'static str,
}

pub const ACCOUNTS: &[StaffAccount] = &[
    StaffAccount {
        id: "1001",
        name: "山田",
        role: Role::Crew,
        password: "1111",
    },
    StaffAccount {
        id: "1002",
        name: "鈴木",
        role: Role::Crew,
        password: "2222",
    },
    StaffAccount {
        id: "9001",
        name: "佐藤（オーナー）",
        role: Role::Owner,
        password: "9999",
    },
];

pub const MAX_FAILURES: u32 = 3;
pub const LOCKOUT_MS: u64 = 30_000;

pub fn account(id: &str) -> Option<&'static StaffAccount> {
    ACCOUNTS.iter().find(|a| a.id == id)
}

#[derive(Clone, Debug, Default)]
struct Guard {
    failures: u32,
    locked_until: u64,
}

impl Guard {
    fn check(&mut self, ok: bool, now: u64) -> Result<(), String> {
        if now < self.locked_until {
            return Err(format!(
                "パスワードの連続失敗によりロック中です（あと {} 秒）",
                (self.locked_until - now).div_ceil(1000)
            ));
        }
        if ok {
            self.failures = 0;
            return Ok(());
        }
        self.failures += 1;
        if self.failures >= MAX_FAILURES {
            self.failures = 0;
            self.locked_until = now + LOCKOUT_MS;
            Err("パスワードの連続失敗により 30 秒間ロックしました".into())
        } else {
            Err(format!(
                "パスワードが違います（あと {} 回で一時ロック）",
                MAX_FAILURES - self.failures
            ))
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Authenticator {
    menu_code: Guard,
    owner: Guard,
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Authenticator {
    /// お客様側から店舗メニューを開くときの使い捨てパスワード。続けて間違えると一時ロックする
    pub fn verify_menu_code(
        &mut self,
        given: &str,
        expected: &str,
        now: u64,
    ) -> Result<(), String> {
        let ok = constant_time_eq(given, expected);
        self.menu_code.check(ok, now)
    }

    pub fn verify_owner(&mut self, staff_id: &str, password: &str, now: u64) -> Result<(), String> {
        let acc = account(staff_id).ok_or("従業員が見つかりません")?;
        if acc.role != Role::Owner {
            return Err("商品ルールを変更できるのはオーナーだけです".into());
        }
        let ok = constant_time_eq(password, acc.password);
        self.owner.check(ok, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_code_locks_after_repeated_failures() {
        let mut a = Authenticator::default();
        assert!(a.verify_menu_code("0000", "U8X3", 0).is_err());
        assert!(a.verify_menu_code("0000", "U8X3", 1).is_err());
        assert!(a.verify_menu_code("0000", "U8X3", 2).is_err());
        assert!(
            a.verify_menu_code("U8X3", "U8X3", 3).is_err(),
            "locked even with the right code"
        );
        assert!(a.verify_menu_code("U8X3", "U8X3", 2 + LOCKOUT_MS).is_ok());
    }

    #[test]
    fn crew_cannot_reauthenticate_as_owner() {
        let mut a = Authenticator::default();
        assert!(a.verify_owner("1001", "1111", 0).is_err());
        assert!(a.verify_owner("9001", "9999", 0).is_ok());
    }
}
