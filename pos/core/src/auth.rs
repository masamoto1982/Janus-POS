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
    password: &'static str,
}

pub const ACCOUNTS: &[StaffAccount] = &[
    StaffAccount {
        id: "000",
        name: "スタッフ000",
        role: Role::Owner,
        password: "9999",
    },
    StaffAccount {
        id: "001",
        name: "スタッフ001",
        role: Role::Crew,
        password: "1111",
    },
    StaffAccount {
        id: "002",
        name: "スタッフ002",
        role: Role::Crew,
        password: "2222",
    },
];

const STORE_PASSWORD: &str = "7777";

pub const MAX_FAILURES: u32 = 3;
pub const LOCKOUT_MS: u64 = 30_000;

pub fn account(id: &str) -> Option<&'static StaffAccount> {
    ACCOUNTS.iter().find(|a| a.id == id)
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Guard {
    pub(crate) failures: u32,
    pub(crate) locked_until: u64,
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
    pub(crate) owner: Guard,
    pub(crate) store: Guard,
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Authenticator {
    pub fn verify_owner(&mut self, staff_id: &str, password: &str, now: u64) -> Result<(), String> {
        let acc = account(staff_id).ok_or("従業員が見つかりません")?;
        if acc.role != Role::Owner {
            return Err("販売規則を変更できるのはオーナーだけです".into());
        }
        let ok = constant_time_eq(password, acc.password);
        self.owner.check(ok, now)
    }

    pub fn verify_store(&mut self, password: &str, now: u64) -> Result<(), String> {
        if password.is_empty() {
            return Err("店舗パスワードを入力してください".into());
        }
        let ok = constant_time_eq(password, STORE_PASSWORD);
        self.store.check(ok, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn crew_cannot_reauthenticate_as_owner() {
        let mut a = Authenticator::default();
        assert!(a.verify_owner("001", "1111", 0).is_err());
        assert!(a.verify_owner("000", "9999", 0).is_ok());
    }

    #[test]
    fn store_password_locks_after_repeated_failures() {
        let mut a = Authenticator::default();
        assert!(a.verify_store("7777", 0).is_ok());
        for _ in 0..MAX_FAILURES {
            assert!(a.verify_store("0000", 0).is_err());
        }
        assert!(a.verify_store("7777", 1).is_err(), "ロック中は正しくても通さない");
        assert!(a.verify_store("7777", LOCKOUT_MS).is_ok());
    }
}
