//! Bell（据え付け端末・可搬性端末・ストアコンピューター。旧称 MAT）＝呼び鈴（問題10への対策）。
//!
//! 設計メモ（ユーザー 2026-10-26）：Bell は、鳴るべきときに確実に鳴ることが何より重要である。
//! そのため、到達確認（ACK）や再送の仕組みを持たない通信（投げっぱなしの通知・ブロードキャスト等）を
//! Bell への伝達に使ってはならない。実機でも、届いたことを確かめられない経路は採らない。
//!
//! MAT は「鳴る」ことだけが役目で、呼び出しの中身は表示しない。店員は
//! 鳴動を合図にレジカウンターへ近づき、どのレジが鳴っているかで状況を
//! 判断し、そのレジの画面で対応する。
//!
//! 役目が単純なぶん、確実に鳴らすことに集中する。
//!
//! - 投げっぱなしのイベント通知ではなく、**状態同期 + ACK + 再送 + 死活監視**。
//!   レジ側は「鳴動状態」の版数を持ち、MAT は受け取った版数を ACK で返す。
//!   ACK が最新に追いつくまで一定間隔で再送する（状態の丸ごと送信なので
//!   重複しても壊れない）。
//! - 鳴るのはお客様が呼び出しボタン（FF・サービス・たばこ・係員を呼ぶ）を押している間だけ。
//!   鳴らし直し（スヌーズ）はしない。押した回数も版数とは別に送るので、短く押されて
//!   「押した」状態の電文が失われても、届いた時点で必ず短く鳴る（押したのに鳴らない、をなくす）。
//! - 変化がなくてもハートビートを送り、ACK が途絶えた MAT は「不通」として
//!   MAT とレジの両方に表示する。鳴らない状態を黙って放置しない。

use serde::Serialize;

use crate::rng::Rng;

pub const RETRY_MS: u64 = 200;
pub const HEARTBEAT_MS: u64 = 2_000;
pub const LINK_TIMEOUT_MS: u64 = 5_000;
/// 押された（または年確・代行収納の読み取りで呼ばれた）ことを知ったときの最短の鳴動
pub const MIN_RING_MS: u64 = 1_000;

/// レジ側から MAT へ送る鳴動状態。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Bell {
    /// 版数（変化のたびに増える）
    pub seq: u64,
    /// 呼び出しボタンが押されているレジの数。1 以上の間 MAT は鳴り続ける
    pub held: usize,
    /// 呼び出しの回数。増えたら MAT は少なくとも MIN_RING_MS 鳴る
    pub presses: u64,
}

#[derive(Clone, Debug)]
struct Packet {
    bell: Bell,
    deliver_at: u64,
    lost_down: bool,
    lost_up: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Mat {
    pub id: String,
    /// MAT 側が最後に受け取った鳴動状態
    pub received: Bell,
    /// レジ側が把握している、この MAT の ACK 済み版数
    pub acked_seq: u64,
    pub last_rx_at: u64,
    pub last_ack_at: u64,
    #[serde(skip)]
    last_sent_at: Option<u64>,
    #[serde(skip)]
    last_sent_seq: u64,
    #[serde(skip)]
    in_flight: Vec<Packet>,
    pub sent: u64,
    pub retries: u64,
    /// 障害注入：電源断・圏外
    pub offline: bool,
    pub buzz_until: u64,
    pub rings: u64,
    /// 呼び鈴機能。OFF の端末は鳴らず、不通の警告も出さない（据え付けの呼び鈴は常に ON）
    pub bell_on: bool,
}

impl Mat {
    pub fn new(id: &str) -> Mat {
        Mat {
            id: id.into(),
            received: Bell::default(),
            acked_seq: 0,
            last_rx_at: 0,
            last_ack_at: 0,
            last_sent_at: None,
            last_sent_seq: 0,
            in_flight: vec![],
            sent: 0,
            retries: 0,
            offline: false,
            buzz_until: 0,
            rings: 0,
            bell_on: true,
        }
    }

    /// レジ側から見たリンク状態
    pub fn link_ok(&self, now: u64) -> bool {
        now.saturating_sub(self.last_ack_at) <= LINK_TIMEOUT_MS
    }

    /// MAT 自身から見た受信状態
    pub fn rx_ok(&self, now: u64) -> bool {
        now.saturating_sub(self.last_rx_at) <= LINK_TIMEOUT_MS
    }

    /// 押されている間は鳴る。ただし受信が途絶えたら（「離した」が届かない恐れ）鳴り止む。
    /// 途絶えは不通としてレジ側に表示される
    pub fn buzzing(&self, now: u64) -> bool {
        self.bell_on && (self.buzz_until > now || (self.received.held > 0 && self.rx_ok(now)))
    }

    pub fn tick(&mut self, now: u64, bell: Bell, loss_percent: u8, rng: &mut Rng) {
        // 到着処理
        let (arrived, pending): (Vec<Packet>, Vec<Packet>) = std::mem::take(&mut self.in_flight)
            .into_iter()
            .partition(|p| p.deliver_at <= now);
        self.in_flight = pending;
        for p in arrived {
            if p.lost_down {
                continue;
            }
            if p.bell.seq >= self.received.seq {
                if p.bell.presses > self.received.presses {
                    self.buzz_until = now + MIN_RING_MS;
                    self.rings += 1;
                }
                self.received = p.bell;
            }
            self.last_rx_at = now;
            if !p.lost_up {
                self.acked_seq = self.acked_seq.max(p.bell.seq);
                self.last_ack_at = now;
            }
        }

        // 送信判定：未 ACK の版があれば RETRY_MS ごとに再送、なければハートビート。
        // ボタンが押されている間は「押されている」を RETRY_MS ごとに送り続ける
        // （MAT は受信が途絶えると鳴り止むので、通信ロスがあっても鳴り続けさせる）
        let since = self.last_sent_at.map(|t| now.saturating_sub(t));
        let behind = self.acked_seq < bell.seq;
        let due = match since {
            None => true,
            Some(s) => ((behind || bell.held > 0) && s >= RETRY_MS) || s >= HEARTBEAT_MS,
        };
        if due {
            if behind && self.last_sent_at.is_some() && self.last_sent_seq == bell.seq {
                self.retries += 1;
            }
            let lost_down = self.offline || rng.chance(loss_percent);
            let lost_up = self.offline || rng.chance(loss_percent);
            self.in_flight.push(Packet {
                bell,
                deliver_at: now + rng.range(60, 200),
                lost_down,
                lost_up,
            });
            self.sent += 1;
            self.last_sent_at = Some(now);
            self.last_sent_seq = bell.seq;
        }
    }

    /// 鳴動状態が変わったら次の tick で即送信させる
    pub fn kick(&mut self) {
        self.last_sent_at = None;
    }
}
