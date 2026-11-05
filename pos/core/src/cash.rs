use serde::Serialize;

pub const BILLS: &[u32] = &[10_000, 5_000, 2_000, 1_000];
pub const COINS: &[u32] = &[500, 100, 50, 10, 5, 1];

pub fn is_bill(d: u32) -> bool {
    BILLS.contains(&d)
}

pub fn is_valid_denomination(d: u32) -> bool {
    is_bill(d) || COINS.contains(&d)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Cash(pub Vec<(u32, u32)>);

impl Cash {
    pub fn total(&self) -> u32 {
        self.0.iter().map(|(d, n)| d * n).sum()
    }

    pub fn add(&mut self, d: u32, n: u32) {
        if n == 0 {
            return;
        }
        if let Some(e) = self.0.iter_mut().find(|(x, _)| *x == d) {
            e.1 += n;
        } else {
            self.0.push((d, n));
            self.0.sort_by_key(|e| std::cmp::Reverse(e.0));
        }
    }

    pub fn from_amount(amount: u32) -> Cash {
        let mut c = Cash::default();
        let mut rest = amount;
        for &d in BILLS.iter().filter(|&&d| d != 2_000).chain(COINS) {
            c.add(d, rest / d);
            rest %= d;
        }
        c
    }

    pub fn merge(&mut self, other: &Cash) {
        for (d, n) in &other.0 {
            self.add(*d, *n);
        }
    }

    pub fn of(&self, d: u32) -> u32 {
        self.0.iter().find(|(x, _)| *x == d).map_or(0, |(_, n)| *n)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Drawer {
    pub cash: Cash,
}

impl Default for Drawer {
    fn default() -> Self {
        let mut cash = Cash::default();
        for (d, n) in [
            (10_000, 5),
            (5_000, 10),
            (1_000, 30),
            (500, 20),
            (100, 50),
            (50, 30),
            (10, 50),
            (5, 30),
            (1, 50),
        ] {
            cash.add(d, n);
        }
        Drawer { cash }
    }
}

impl Drawer {
    pub fn put(&mut self, amount: u32) {
        self.cash.merge(&Cash::from_amount(amount));
    }

    pub fn take_exact(&mut self, d: u32, n: u32) -> Result<Cash, String> {
        let e = self
            .cash
            .0
            .iter_mut()
            .find(|(x, _)| *x == d)
            .filter(|(_, have)| *have >= n)
            .ok_or_else(|| format!("{d}円が足りません"))?;
        e.1 -= n;
        let mut out = Cash::default();
        out.add(d, n);
        Ok(out)
    }

    pub fn take(&mut self, amount: u32, for_change: bool) -> Result<Cash, String> {
        let mut rest = amount;
        let mut out = Cash::default();
        for &d in BILLS.iter().chain(COINS) {
            if for_change && (d == 10_000 || d == 2_000) {
                continue;
            }
            let n = (rest / d).min(self.cash.of(d));
            if n > 0 {
                out.add(d, n);
                rest -= d * n;
            }
        }
        if rest > 0 {
            return Err("釣銭機の在高が不足しています".into());
        }
        for (d, n) in &out.0 {
            if let Some(e) = self.cash.0.iter_mut().find(|(x, _)| x == d) {
                e.1 -= n;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_is_split_without_10k_or_2k() {
        let mut d = Drawer::default();
        let c = d.take(8_766, true).unwrap();
        assert_eq!(c.of(5_000), 1);
        assert_eq!(c.of(1_000), 3);
        assert_eq!(c.total(), 8_766);
    }

    #[test]
    fn shortage_leaves_drawer_untouched() {
        let mut d = Drawer {
            cash: Cash(vec![(100, 1)]),
        };
        assert!(d.take(150, true).is_err());
        assert_eq!(d.cash.total(), 100);
    }
}
