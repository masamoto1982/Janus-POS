//! 混雑シミュレーション：お客様とスタッフを自動で動かし、レジのハングアップ（立ち往生）が起きないかを確かめる。
//! コマンドは画面の操作と同じものを Store に送るので、Janus-POS の規則そのものを試す

use serde::{Deserialize, Serialize};

use super::{CallTopic, Command, Register, RevertRule, Store};
use crate::authority::{Mode, PaymentMethod, Side};
use crate::rng::Rng;

/// シミュレーションの刻み
pub const RUSH_STEP_MS: u64 = 500;
/// お客様がセルフで一点スキャンする時間
const SELF_SCAN_MS: u64 = 5_000;
/// スタッフが一点スキャンする時間
const STAFF_SCAN_MS: u64 = 2_500;
/// 画面のボタンを一つ押す時間（お客様）
const SELF_STEP_MS: u64 = 5_000;
/// 画面のボタンを一つ押す時間（スタッフ）
const STAFF_STEP_MS: u64 = 2_000;
/// 現金を受け取って投入する時間
const CASH_MS: u64 = 10_000;
/// スタッフがレジからレジへ移る時間
const WALK_MS: u64 = 8_000;
/// 呼び出しに気づいて動き出すまでの時間
const NOTICE_MS: u64 = 3_000;
/// 有人レジの持ち場で、列が無いままこの時間が過ぎたら持ち場を離れる（ログアウトしてセルフ操作モードに戻す）
const POST_IDLE_MS: u64 = 60_000;
/// 到着が終わってから、この時間を過ぎたら打ち切る
const OVERTIME_MS: u64 = 60 * 60_000;
/// 記録する出来事の上限
const LOG_MAX: usize = 400;

/// 商品（年確・数量制限のない品）。価格は catalog と同じ
const GOODS: &[(&str, u32)] = &[
    ("4900000000028", 320),
    ("4900000000011", 160),
    ("4900000000073", 150),
    ("4910000000014", 160),
    ("4910000000038", 480),
    ("4900000000035", 150),
];
const DRINK: &str = "4900000000035";
const BADGES: &[(&str, &str)] = &[
    ("ST001", "スタッフ001"),
    ("ST002", "スタッフ002"),
    ("ST000", "スタッフ000"),
];
const RUSH_MAX_STAFF: u8 = 3;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(default)]
pub struct RushParams {
    /// レジの台数
    pub registers: u8,
    /// スタッフの人数（0〜3）
    pub staff: u8,
    /// 来店するお客様の人数
    pub customers: u32,
    /// お客様が来店する時間帯の長さ（分）
    pub minutes: u32,
    /// 客単価（円）。飲み物ひとつのお客様を除く
    pub spend: u32,
    /// 飲み物ひとつだけのお客様の割合（%）
    pub drink_only_pct: u8,
    /// 現金で払うお客様の割合（%）。セルフ操作モードは現金を扱わないのでスタッフを呼ぶ
    pub cash_pct: u8,
    /// 手のかかるお客様（トラブル）の割合（%）。商品を読んだあとスタッフを呼ぶ
    pub trouble_pct: u8,
    /// 手のかかるお客様の対応にかかる時間（秒）
    pub trouble_secs: u32,
    /// 列がこの人数以上になったら、手の空いたスタッフが空いたレジを有人レジとして開ける（0＝開けない）
    pub open_at_queue: u32,
    /// 有人レジを開けるとき、お客様が来る前にログインして待つ
    pub pre_login: bool,
    /// スタッフのいない有人レジで、お客様が画面に触れる間隔（秒、0＝触れない）
    pub touch_secs: u32,
    /// お客様が待てる時間（秒）。来店からこの時間を過ぎたら買わずに帰る
    pub patience_secs: u32,
    pub seed: u64,
    /// セルフ操作モードに戻す規則
    pub rule: RevertRule,
}

impl Default for RushParams {
    fn default() -> Self {
        RushParams {
            registers: 3,
            staff: 2,
            customers: 60,
            minutes: 30,
            spend: 600,
            drink_only_pct: 30,
            cash_pct: 20,
            trouble_pct: 10,
            trouble_secs: 180,
            open_at_queue: 2,
            pre_login: true,
            touch_secs: 20,
            patience_secs: 600,
            seed: 1,
            rule: RevertRule::Fixed,
        }
    }
}

impl RushParams {
    pub fn clamp(mut self) -> Self {
        self.registers = self.registers.clamp(1, 5);
        self.staff = self.staff.min(RUSH_MAX_STAFF);
        self.customers = self.customers.clamp(1, 1000);
        self.minutes = self.minutes.clamp(1, 240);
        self.spend = self.spend.clamp(100, 10_000);
        self.drink_only_pct = self.drink_only_pct.min(100);
        self.cash_pct = self.cash_pct.min(100);
        self.trouble_pct = self.trouble_pct.min(100);
        self.trouble_secs = self.trouble_secs.min(3600);
        self.patience_secs = self.patience_secs.clamp(30, 7200);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CustomerState {
    Coming,
    Queue,
    AtRegister,
    Done,
    GaveUp,
}

#[derive(Clone, Debug)]
struct Customer {
    no: u32,
    arrive: u64,
    items: Vec<&'static str>,
    cash: bool,
    trouble: bool,
    state: CustomerState,
    reg: Option<u8>,
    since: u64,
    next_at: u64,
    scanned: usize,
    called: bool,
    trouble_done: bool,
    /// 手のかかる用件の対応が終わる時刻
    trouble_until: u64,
    touched_at: u64,
    incident: Option<usize>,
    left_at: u64,
    paid: bool,
}

impl Customer {
    fn kind(&self) -> String {
        let mut s = if self.items.len() == 1 && self.items[0] == DRINK {
            "飲み物ひとつ".to_string()
        } else {
            format!("{}点", self.items.len())
        };
        if self.cash {
            s.push_str("・現金");
        }
        if self.trouble {
            s.push_str("・手のかかるお客様");
        }
        s
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Job {
    /// 手が空いている（持ち場があればそこにいる）
    Free,
    /// レジへ移動中
    Walk { to: u8, arrive: u64, answer: bool },
    /// そのレジのお客様の対応中
    Serve { reg: u8, answer: bool },
}

#[derive(Clone, Debug)]
struct Staff {
    badge: &'static str,
    name: &'static str,
    at: Option<u8>,
    post: Option<u8>,
    job: Job,
    busy_until: u64,
    post_quiet_since: u64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RushIncident {
    pub customer: u32,
    pub kind: String,
    pub register: u8,
    /// 始まり（シミュレーション開始からの経過）
    pub start_ms: u64,
    pub duration_ms: u64,
    /// 立ち往生のあいだ、お客様の操作（スキャン・画面に触れる）で無操作の時計が戻った回数
    pub resets: u32,
    pub cause: String,
    pub ended_by: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RushLog {
    pub at_ms: u64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RushStaffView {
    pub name: &'static str,
    pub at: Option<u8>,
    pub post: Option<u8>,
    pub doing: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RushRegisterView {
    pub register: u8,
    pub customer: Option<u32>,
    pub kind: String,
    pub waited_ms: u64,
    /// スタッフも呼び出しもなく、セルフにもなっていない（立ち往生）
    pub stranded: bool,
    pub staff_here: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RushView {
    pub params: RushParams,
    pub elapsed_ms: u64,
    pub finished: bool,
    pub arrived: u32,
    pub served: u32,
    pub gave_up: u32,
    pub queue: u32,
    pub max_queue: u32,
    pub avg_wait_ms: u64,
    pub max_wait_ms: u64,
    pub avg_spend: u32,
    pub stranded_customers: u32,
    pub stranded_total_ms: u64,
    pub max_stranded_ms: u64,
    pub incidents: Vec<RushIncident>,
    pub staff: Vec<RushStaffView>,
    pub registers: Vec<RushRegisterView>,
    pub log: Vec<RushLog>,
}

#[derive(Clone, Debug)]
pub struct Rush {
    params: RushParams,
    start: u64,
    now: u64,
    customers: Vec<Customer>,
    staff: Vec<Staff>,
    queue: Vec<usize>,
    max_queue: u32,
    incidents: Vec<RushIncident>,
    log: Vec<RushLog>,
    finished: bool,
    receipts: Vec<(u8, u32)>,
}

impl Rush {
    /// `store` は params.registers 台の、取引のない店。開始時刻は store の今。店の復帰規則を params.rule にする
    pub fn new(params: RushParams, store: &mut Store) -> Rush {
        let params = params.clamp();
        store.set_revert_rule(params.rule);
        let start = store.now();
        let mut rng = Rng::new(params.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5bd1_e995);
        let window = u64::from(params.minutes) * 60_000;
        let mean_gap = window / u64::from(params.customers).max(1);
        let mut t = start;
        let mut customers = Vec::new();
        for no in 1..=params.customers {
            // 到着の間隔は指数分布（ポアソン到着）
            let u = (rng.next_u64() % 1_000_000) as f64 / 1_000_000.0;
            let gap = (-(1.0 - u).ln() * mean_gap as f64) as u64;
            t = (t + gap).min(start + window);
            let items = if rng.chance(params.drink_only_pct) {
                vec![DRINK]
            } else {
                let mut items = vec![];
                let target = params.spend * rng.range(60, 140) as u32 / 100;
                let mut sum = 0;
                while sum < target || items.is_empty() {
                    let (code, price) = GOODS[rng.range(0, GOODS.len() as u64 - 1) as usize];
                    items.push(code);
                    sum += price;
                    if items.len() >= 30 {
                        break;
                    }
                }
                items
            };
            customers.push(Customer {
                no,
                arrive: t,
                items,
                cash: rng.chance(params.cash_pct),
                trouble: rng.chance(params.trouble_pct),
                state: CustomerState::Coming,
                reg: None,
                since: t,
                next_at: t,
                scanned: 0,
                called: false,
                trouble_done: false,
                trouble_until: 0,
                touched_at: t,
                incident: None,
                left_at: 0,
                paid: false,
            });
        }
        let staff = BADGES
            .iter()
            .take(usize::from(params.staff))
            .map(|&(badge, name)| Staff {
                badge,
                name,
                at: None,
                post: None,
                job: Job::Free,
                busy_until: start,
                post_quiet_since: start,
            })
            .collect();
        Rush {
            params,
            start,
            now: start,
            customers,
            staff,
            queue: vec![],
            max_queue: 0,
            incidents: vec![],
            log: vec![],
            finished: false,
            receipts: vec![],
        }
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    /// 店の時計を `ms` 進め、そのあいだお客様とスタッフを動かす
    pub fn advance(&mut self, store: &mut Store, ms: u64) {
        // 画面の操作や実時間の tick で店の時計が先に進んでいたら、そこから
        self.now = self.now.max(store.now());
        let end = self.now + ms;
        while self.now < end && !self.finished {
            self.now = (self.now + RUSH_STEP_MS).min(end);
            store.tick(self.now);
            self.step(store);
        }
    }

    /// 最後まで流す
    pub fn run(params: RushParams) -> RushView {
        let params = params.clamp();
        let mut store = Store::new(params.seed, params.registers, 1);
        let mut rush = Rush::new(params, &mut store);
        while !rush.finished {
            rush.advance(&mut store, 60_000);
        }
        rush.view(&store)
    }

    fn note(&mut self, text: String) {
        if self.log.len() >= LOG_MAX {
            self.log.remove(0);
        }
        self.log.push(RushLog {
            at_ms: self.now - self.start,
            text,
        });
    }

    fn send(store: &mut Store, cmd: Command) -> bool {
        store.dispatch(cmd).is_ok()
    }

    fn staff_here(&self, reg: u8) -> bool {
        self.staff.iter().any(|s| {
            s.at == Some(reg) && !matches!(s.job, Job::Walk { .. })
        })
    }

    fn customer_at(&self, reg: u8) -> Option<usize> {
        self.customers
            .iter()
            .position(|c| c.state == CustomerState::AtRegister && c.reg == Some(reg))
    }

    fn step(&mut self, store: &mut Store) {
        let now = self.now;
        // 来店
        for i in 0..self.customers.len() {
            if self.customers[i].state == CustomerState::Coming && self.customers[i].arrive <= now {
                self.customers[i].state = CustomerState::Queue;
                self.queue.push(i);
            }
        }
        self.max_queue = self.max_queue.max(self.queue.len() as u32);
        // 諦めて帰る
        for i in 0..self.customers.len() {
            let c = &self.customers[i];
            let waiting = matches!(c.state, CustomerState::Queue | CustomerState::AtRegister);
            if waiting && now.saturating_sub(c.arrive) >= u64::from(self.params.patience_secs) * 1000 {
                self.give_up(store, i);
            }
        }
        // 決済が済んだレジのお客様（「ありがとうございました」は数秒で消えるので、刻みごとに見る）
        for r in &store.registers {
            let Some(no) = r.last_receipt.as_ref().map(|rc| rc.no) else { continue };
            if self.receipts.iter().any(|&(id, n)| id == r.id && n == no) {
                continue;
            }
            self.receipts.retain(|&(id, _)| id != r.id);
            self.receipts.push((r.id, no));
            if let Some(i) = self.customer_at(r.id) {
                self.customers[i].paid = true;
            }
        }
        self.assign_registers(store);
        for i in 0..self.customers.len() {
            if self.customers[i].state == CustomerState::AtRegister && self.customers[i].next_at <= now {
                self.customer_act(store, i);
            }
        }
        for k in 0..self.staff.len() {
            if self.staff[k].busy_until <= now {
                self.staff_act(store, k);
            }
        }
        self.watch_stranded(store);
        let window_end = self.start + u64::from(self.params.minutes) * 60_000;
        let all_left = self
            .customers
            .iter()
            .all(|c| matches!(c.state, CustomerState::Done | CustomerState::GaveUp));
        if all_left || now >= window_end + OVERTIME_MS {
            self.finished = true;
            for i in 0..self.customers.len() {
                if matches!(self.customers[i].state, CustomerState::Queue | CustomerState::AtRegister) {
                    self.give_up(store, i);
                }
            }
            self.watch_stranded(store);
            self.note("シミュレーション終了".into());
        }
    }

    fn give_up(&mut self, store: &Store, i: usize) {
        let now = self.now;
        let c = &mut self.customers[i];
        let reg = c.reg;
        c.state = CustomerState::GaveUp;
        c.left_at = now;
        let no = c.no;
        let waited = now - c.arrive;
        self.queue.retain(|&q| q != i);
        if let Some(k) = self.customers[i].incident {
            self.close_incident(k, "諦めて帰った");
            self.customers[i].incident = None;
        }
        let left = reg
            .and_then(|r| store.reg(r))
            .is_some_and(|r| !r.txn.lines.is_empty());
        self.note(match reg {
            Some(r) if left => format!("客#{no} が {} 待って諦め、レジ{r}に商品を残して帰る", mmss(waited)),
            Some(r) => format!("客#{no} が {} 待って諦め、レジ{r}から帰る", mmss(waited)),
            None => format!("客#{no} が {} 列に並んで諦めて帰る", mmss(waited)),
        });
    }

    /// 列の先頭から空いたレジへ。お客様は画面の「有人レジ」「セルフレジ」とスタッフの姿を見て選ぶ
    fn assign_registers(&mut self, store: &Store) {
        while let Some(&i) = self.queue.first() {
            let free: Vec<u8> = store
                .registers
                .iter()
                .filter(|r| !r.paused && !r.closing && self.customer_at(r.id).is_none())
                .map(|r| r.id)
                .collect();
            if free.is_empty() {
                return;
            }
            let c = &self.customers[i];
            let needs_staff = c.cash || c.trouble;
            let staffed_here = |id: u8| self.staff_here(id);
            let is_self = |id: u8| store.reg(id).is_some_and(|r| r.mode == Mode::SelfOperated);
            let pick = if needs_staff {
                free.iter().find(|&&id| staffed_here(id)).or_else(|| free.iter().find(|&&id| is_self(id)))
            } else {
                free.iter().find(|&&id| is_self(id)).or_else(|| free.iter().find(|&&id| staffed_here(id)))
            }
            .or(free.first())
            .copied()
            .unwrap();
            self.queue.remove(0);
            let now = self.now;
            let mode = store.reg(pick).map(|r| r.mode);
            let c = &mut self.customers[i];
            c.state = CustomerState::AtRegister;
            c.reg = Some(pick);
            c.since = now;
            c.next_at = now + 3_000;
            c.touched_at = now;
            let (no, kind) = (c.no, c.kind());
            let shown = if mode == Some(Mode::StaffOperated) { "有人レジ" } else { "セルフレジ" };
            self.note(format!("客#{no}（{kind}）がレジ{pick}へ（{shown}）"));
        }
    }

    fn customer_act(&mut self, store: &mut Store, i: usize) {
        let now = self.now;
        let reg = self.customers[i].reg.unwrap();
        let Some(r) = store.reg(reg) else { return };
        let mode = r.mode;
        let lines = r.txn.lines.len();
        let paying = r.txn.paying;
        let method = r.txn.method;
        let outlet = r.outlet.total();
        let done = self.customers[i].paid && lines == 0 && !paying;
        let c = &mut self.customers[i];
        if done {
            if outlet > 0 {
                Self::send(store, Command::TakeOutlet { register: reg });
            }
            c.state = CustomerState::Done;
            c.left_at = now;
            let (no, waited) = (c.no, now - c.arrive);
            self.note(format!("客#{no} がレジ{reg}でお買い物を終える（来店から {}）", mmss(waited)));
            return;
        }
        // 取引中止などで明細が消えたら、読み直す
        if c.scanned > 0 && lines == 0 && !paying {
            c.scanned = 0;
        }
        let staff_here = self.staff_here(reg);
        let c = &mut self.customers[i];
        if mode == Mode::StaffOperated {
            if staff_here {
                c.next_at = now + 1_000;
                return;
            }
            // スタッフのいない有人レジ：お客様は手元の商品をスキャナに通してみて、画面に触れて待つ
            if c.scanned < c.items.len() {
                let code = c.items[c.scanned];
                if Self::send(store, Command::Scan { register: reg, side: Side::Floor, code: code.into() }) {
                    self.customers[i].scanned += 1;
                    self.count_reset(i);
                }
                self.customers[i].next_at = now + SELF_SCAN_MS;
            } else if self.params.touch_secs > 0
                && now >= c.touched_at + u64::from(self.params.touch_secs) * 1000
            {
                c.touched_at = now;
                Self::send(store, Command::Touch { register: reg, side: Some(Side::Floor) });
                self.count_reset(i);
                self.customers[i].next_at = now + 1_000;
            } else {
                c.next_at = now + 1_000;
            }
            return;
        }
        // セルフ操作モード
        if c.scanned == 0 && (lines > 0 || paying) {
            // 前のお客様が残していった取引を中止する
            Self::send(store, Command::VoidTransaction { register: reg });
            let no = c.no;
            self.note(format!("客#{no} がレジ{reg}に残っていた取引を中止する"));
            self.customers[i].next_at = now + SELF_STEP_MS;
            return;
        }
        if !paying {
            if c.scanned < c.items.len() {
                let code = c.items[c.scanned];
                if Self::send(store, Command::Scan { register: reg, side: Side::Floor, code: code.into() }) {
                    c.scanned += 1;
                }
                c.next_at = now + SELF_SCAN_MS;
            } else if c.trouble && !c.trouble_done {
                if !c.called {
                    c.called = true;
                    Self::send(store, Command::CallStaff { register: reg, topic: Some(CallTopic::Other) });
                    let no = c.no;
                    self.note(format!("客#{no} がレジ{reg}でスタッフを呼ぶ（手のかかる用件）"));
                }
                self.customers[i].next_at = now + 1_000;
            } else {
                Self::send(store, Command::Checkout { register: reg, side: Side::Floor });
                c.next_at = now + SELF_STEP_MS;
            }
            return;
        }
        Self::send(store, Command::ConfirmPointCard { register: reg, side: Side::Floor });
        match method {
            None => {
                let m = if c.cash { PaymentMethod::Cash } else { PaymentMethod::Qr };
                Self::send(store, Command::SelectPayment { register: reg, side: Side::Floor, method: m });
                if c.cash {
                    let no = c.no;
                    self.note(format!("客#{no} がレジ{reg}で現金を選ぶ（スタッフを呼ぶ）"));
                }
                self.customers[i].next_at = now + SELF_STEP_MS;
            }
            Some(PaymentMethod::Cash) => c.next_at = now + 1_000,
            Some(_) => {
                Self::send(store, Command::TapEmoney { register: reg, method: Some(PaymentMethod::Qr) });
                c.next_at = now + SELF_STEP_MS;
            }
        }
    }

    fn count_reset(&mut self, i: usize) {
        if let Some(k) = self.customers[i].incident {
            self.incidents[k].resets += 1;
        }
    }

    fn staff_act(&mut self, store: &mut Store, k: usize) {
        let now = self.now;
        match self.staff[k].job {
            Job::Walk { to, arrive, answer } => {
                if arrive <= now {
                    self.staff[k].at = Some(to);
                    self.staff[k].job = if answer || self.customer_at(to).is_some() {
                        Job::Serve { reg: to, answer }
                    } else {
                        Job::Free
                    };
                    self.staff[k].post_quiet_since = now;
                }
            }
            Job::Serve { reg, answer } => self.serve(store, k, reg, answer),
            Job::Free => self.decide(store, k),
        }
    }

    fn walk(&mut self, k: usize, to: u8, answer: bool) {
        let now = self.now;
        let s = &mut self.staff[k];
        if s.at == Some(to) {
            s.job = Job::Serve { reg: to, answer };
            return;
        }
        let from = s.at;
        s.at = None;
        s.job = Job::Walk { to, arrive: now + WALK_MS, answer };
        let name = s.name;
        let still = from.filter(|&f| {
            f != to
        });
        let why = if answer { "呼び出しに応えて" } else { "持ち場へ" };
        self.note(match still {
            Some(f) => format!("{name} がレジ{f}を離れ、{why}レジ{to}へ"),
            None => format!("{name} が{why}レジ{to}へ"),
        });
    }

    /// 手が空いたスタッフの判断：呼び出し → 持ち場 → 列が伸びたら有人レジを開ける
    fn decide(&mut self, store: &mut Store, k: usize) {
        let now = self.now;
        let taken: Vec<u8> = self
            .staff
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != k)
            .filter_map(|(_, s)| match s.job {
                Job::Walk { to, answer: true, .. } => Some(to),
                Job::Serve { reg, .. } => Some(reg),
                _ => None,
            })
            .collect();
        let call = store
            .calls
            .open
            .iter()
            .filter(|c| !taken.contains(&c.register))
            .filter(|c| now.saturating_sub(c.created_at) >= NOTICE_MS)
            .min_by_key(|c| c.created_at)
            .map(|c| c.register);
        if let Some(reg) = call {
            self.walk(k, reg, true);
            return;
        }
        let s = &self.staff[k];
        if let Some(p) = s.post {
            if s.at != Some(p) {
                self.walk(k, p, false);
                return;
            }
            if self.customer_at(p).is_some() {
                self.staff[k].job = Job::Serve { reg: p, answer: false };
                return;
            }
            let r = store.reg(p).unwrap();
            let mine = r.staff.as_ref().is_some_and(|st| st.account.name == s.name);
            if self.params.pre_login && !mine && !r.busy() && !r.handover_pending() {
                let (badge, name) = (s.badge, s.name);
                if Self::send(store, Command::Scan { register: p, side: Side::Counter, code: badge.into() }) {
                    self.note(format!("{name} がレジ{p}にログインして有人レジを開けておく"));
                }
                self.staff[k].busy_until = now + STAFF_STEP_MS;
                return;
            }
            if self.queue.is_empty() {
                if now.saturating_sub(s.post_quiet_since) >= POST_IDLE_MS {
                    let name = s.name;
                    if mine {
                        Self::send(store, Command::EndStaffMode { register: p });
                    }
                    self.staff[k].post = None;
                    self.note(format!("{name} が列がなくなったのでレジ{p}を閉じる（セルフ操作モードに戻す）"));
                }
            } else {
                self.staff[k].post_quiet_since = now;
            }
            return;
        }
        if self.params.open_at_queue > 0 && self.queue.len() as u32 >= self.params.open_at_queue {
            let posted: Vec<u8> = self.staff.iter().filter_map(|s| s.post).collect();
            let pick = store
                .registers
                .iter()
                .map(|r| r.id)
                .find(|&id| !posted.contains(&id) && self.customer_at(id).is_none())
                .or_else(|| {
                    store
                        .registers
                        .iter()
                        .map(|r| r.id)
                        .find(|&id| !posted.contains(&id))
                });
            if let Some(p) = pick {
                let name = self.staff[k].name;
                self.staff[k].post = Some(p);
                self.staff[k].post_quiet_since = now;
                self.note(format!("{name} が列を見てレジ{p}を有人レジにすると決める"));
                self.walk(k, p, false);
            }
        }
    }

    fn serve(&mut self, store: &mut Store, k: usize, reg: u8, answer: bool) {
        let now = self.now;
        let (badge, name) = (self.staff[k].badge, self.staff[k].name);
        let Some(i) = self.customer_at(reg) else {
            // お客様がもういない（帰った・終わった）
            self.staff[k].job = Job::Free;
            return;
        };
        let Some(r) = store.reg(reg) else { return };
        let mine = r.staff.as_ref().is_some_and(|st| st.account.name == name);
        if !mine {
            if Self::send(store, Command::Scan { register: reg, side: Side::Counter, code: badge.into() }) {
                let no = self.customers[i].no;
                self.note(format!("{name} がレジ{reg}にログインして客#{no}の対応に入る"));
            } else {
                // 担当のいる取引には入れない（担当の切り替え不可）
                self.staff[k].job = Job::Free;
            }
            self.staff[k].busy_until = now + STAFF_STEP_MS;
            return;
        }
        let r = store.reg(reg).unwrap();
        let (lines, paying, method, due) = (r.txn.lines.len(), r.txn.paying, r.txn.method, r.txn.due());
        let c = &mut self.customers[i];
        if c.trouble && !c.trouble_done {
            c.trouble_done = true;
            c.trouble_until = now + u64::from(self.params.trouble_secs) * 1000;
            let no = c.no;
            self.note(format!("{name} がレジ{reg}で客#{no}の手のかかる用件に対応中（{}）", mmss(u64::from(self.params.trouble_secs) * 1000)));
        }
        let c = &mut self.customers[i];
        if c.trouble_until > now {
            // 対応のあいだもスタッフはレジカウンター側の画面を操作している
            let until = c.trouble_until;
            Self::send(store, Command::Touch { register: reg, side: Some(Side::Counter) });
            self.staff[k].busy_until = until.min(now + 30_000);
            return;
        }
        if c.scanned > 0 && lines == 0 && !paying {
            c.scanned = 0;
        }
        if !paying {
            if c.scanned < c.items.len() {
                let code = c.items[c.scanned];
                if Self::send(store, Command::Scan { register: reg, side: Side::Counter, code: code.into() }) {
                    self.customers[i].scanned += 1;
                }
                self.staff[k].busy_until = now + STAFF_SCAN_MS;
            } else {
                Self::send(store, Command::Checkout { register: reg, side: Side::Counter });
                self.staff[k].busy_until = now + STAFF_STEP_MS;
            }
            return;
        }
        Self::send(store, Command::ConfirmPointCard { register: reg, side: Side::Counter });
        match method {
            None => {
                let m = if c.cash { PaymentMethod::Cash } else { PaymentMethod::Qr };
                Self::send(store, Command::SelectPayment { register: reg, side: Side::Counter, method: m });
                self.staff[k].busy_until = now + STAFF_STEP_MS;
            }
            Some(PaymentMethod::Cash) => {
                let mut rest = due;
                for d in [10_000, 5_000, 1_000, 500, 100, 50, 10, 5, 1] {
                    let n = rest / d;
                    if n > 0 {
                        Self::send(store, Command::InsertCash { register: reg, denomination: d, count: n });
                        rest -= n * d;
                    }
                }
                Self::send(store, Command::FinishInsert { register: reg });
                self.staff[k].busy_until = now + CASH_MS;
                self.finish_serving(k, reg, answer);
            }
            Some(_) => {
                Self::send(store, Command::TapEmoney { register: reg, method: Some(PaymentMethod::Qr) });
                self.staff[k].busy_until = now + STAFF_STEP_MS;
                self.finish_serving(k, reg, answer);
            }
        }
    }

    fn finish_serving(&mut self, k: usize, reg: u8, _answer: bool) {
        let s = &mut self.staff[k];
        s.job = Job::Free;
        if s.post == Some(reg) {
            s.post_quiet_since = self.now;
        }
    }

    /// 立ち往生：スタッフ操作モードのレジで、スタッフが居らず、呼び出しも出ていない（誰も気づけない）
    fn watch_stranded(&mut self, store: &Store) {
        let now = self.now;
        for i in 0..self.customers.len() {
            let c = &self.customers[i];
            let stranded = c.state == CustomerState::AtRegister
                && !c.paid
                && c.reg.is_some_and(|reg| {
                    store.reg(reg).is_some_and(|r| r.mode == Mode::StaffOperated)
                        && !self.staff_here(reg)
                        && !store.calls.open.iter().any(|call| call.register == reg)
                });
            match (stranded, c.incident) {
                (true, None) => {
                    let reg = c.reg.unwrap();
                    let cause = self.cause(store, reg);
                    let inc = RushIncident {
                        customer: c.no,
                        kind: c.kind(),
                        register: reg,
                        start_ms: now - self.start,
                        duration_ms: 0,
                        resets: 0,
                        cause: cause.clone(),
                        ended_by: None,
                    };
                    let no = c.no;
                    self.incidents.push(inc);
                    self.customers[i].incident = Some(self.incidents.len() - 1);
                    self.note(format!("⚠ 客#{no} がレジ{reg}で立ち往生（{cause}）"));
                }
                (true, Some(k)) => {
                    self.incidents[k].duration_ms = now - self.start - self.incidents[k].start_ms;
                }
                (false, Some(k)) => {
                    let why = match c.reg.and_then(|reg| store.reg(reg)) {
                        _ if c.state == CustomerState::Done => "お買い物を終えた",
                        Some(r) if r.mode == Mode::SelfOperated => "無操作でセルフ操作モードに戻った",
                        Some(r) if self.staff_here(r.id) => "スタッフが来た",
                        _ => "呼び出しが出た",
                    };
                    self.close_incident(k, why);
                    self.customers[i].incident = None;
                }
                (false, None) => {}
            }
        }
    }

    fn close_incident(&mut self, k: usize, why: &str) {
        let inc = &mut self.incidents[k];
        if inc.ended_by.is_some() {
            return;
        }
        inc.duration_ms = self.now - self.start - inc.start_ms;
        inc.ended_by = Some(why.to_string());
        let text = format!(
            "客#{} のレジ{}での立ち往生が終わる：{}（{}、無操作の時計が戻った回数 {}）",
            inc.customer,
            inc.register,
            why,
            mmss(inc.duration_ms),
            inc.resets
        );
        self.note(text);
    }

    fn cause(&self, store: &Store, reg: u8) -> String {
        let Some(st) = store.reg(reg).and_then(|r| r.staff.as_ref()) else {
            return "スタッフ操作モードのまま".into();
        };
        let name = st.account.name;
        let s = self.staff.iter().find(|s| s.name == name);
        let where_ = match s.map(|s| (s.at, s.job)) {
            Some((_, Job::Walk { to, .. })) => format!("レジ{to}へ移動中"),
            Some((Some(at), _)) if at != reg => format!("レジ{at}で対応中"),
            _ => "売り場".into(),
        };
        format!("{name} がログインしたまま{where_}")
    }

    pub fn view(&self, store: &Store) -> RushView {
        let now = self.now;
        let done: Vec<&Customer> = self.customers.iter().filter(|c| c.state == CustomerState::Done).collect();
        let finished_waits: Vec<u64> = self
            .customers
            .iter()
            .filter(|c| matches!(c.state, CustomerState::Done | CustomerState::GaveUp))
            .map(|c| c.left_at - c.arrive)
            .collect();
        let spend: u32 = done
            .iter()
            .map(|c| c.items.iter().map(|code| GOODS.iter().find(|g| g.0 == *code).map_or(0, |g| g.1)).sum::<u32>())
            .sum();
        let mut stranded_customers: Vec<u32> = self.incidents.iter().map(|i| i.customer).collect();
        stranded_customers.dedup();
        RushView {
            params: self.params.clone(),
            elapsed_ms: now - self.start,
            finished: self.finished,
            arrived: self.customers.iter().filter(|c| c.state != CustomerState::Coming).count() as u32,
            served: done.len() as u32,
            gave_up: self.customers.iter().filter(|c| c.state == CustomerState::GaveUp).count() as u32,
            queue: self.queue.len() as u32,
            max_queue: self.max_queue,
            avg_wait_ms: if finished_waits.is_empty() { 0 } else { finished_waits.iter().sum::<u64>() / finished_waits.len() as u64 },
            max_wait_ms: finished_waits.iter().copied().max().unwrap_or(0),
            avg_spend: if done.is_empty() { 0 } else { spend / done.len() as u32 },
            stranded_customers: stranded_customers.len() as u32,
            stranded_total_ms: self.incidents.iter().map(|i| i.duration_ms).sum(),
            max_stranded_ms: self.incidents.iter().map(|i| i.duration_ms).max().unwrap_or(0),
            incidents: self.incidents.clone(),
            staff: self
                .staff
                .iter()
                .map(|s| RushStaffView {
                    name: s.name,
                    at: s.at,
                    post: s.post,
                    doing: match s.job {
                        Job::Walk { to, .. } => format!("レジ{to}へ移動中"),
                        Job::Serve { reg, answer: true } => format!("レジ{reg}で呼び出しの対応"),
                        Job::Serve { reg, .. } => format!("レジ{reg}で接客"),
                        Job::Free => match s.at {
                            Some(r) if s.post == Some(r) => format!("レジ{r}（持ち場）で待機"),
                            Some(r) => format!("レジ{r}のそば"),
                            None => "売り場の作業".into(),
                        },
                    },
                })
                .collect(),
            registers: store
                .registers
                .iter()
                .map(|r: &Register| {
                    let c = self.customer_at(r.id).map(|i| &self.customers[i]);
                    RushRegisterView {
                        register: r.id,
                        customer: c.map(|c| c.no),
                        kind: c.map(|c| c.kind()).unwrap_or_default(),
                        waited_ms: c.map_or(0, |c| now - c.since),
                        stranded: c.is_some_and(|c| c.incident.is_some()),
                        staff_here: self
                            .staff
                            .iter()
                            .filter(|s| s.at == Some(r.id) && !matches!(s.job, Job::Walk { .. }))
                            .map(|s| s.name)
                            .collect(),
                    }
                })
                .collect(),
            log: self.log.clone(),
        }
    }
}

fn mmss(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}
