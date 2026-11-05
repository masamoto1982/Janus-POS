use serde::Serialize;

use crate::rng::Rng;

pub const RETRY_MS: u64 = 200;
pub const HEARTBEAT_MS: u64 = 2_000;
pub const LINK_TIMEOUT_MS: u64 = 5_000;
pub const MIN_RING_MS: u64 = 1_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Bell {
    pub seq: u64,
    pub held: usize,
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
    pub received: Bell,
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
    pub offline: bool,
    pub buzz_until: u64,
    pub rings: u64,
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

    pub fn link_ok(&self, now: u64) -> bool {
        now.saturating_sub(self.last_ack_at) <= LINK_TIMEOUT_MS
    }

    pub fn rx_ok(&self, now: u64) -> bool {
        now.saturating_sub(self.last_rx_at) <= LINK_TIMEOUT_MS
    }

    pub fn buzzing(&self, now: u64) -> bool {
        self.bell_on && (self.buzz_until > now || (self.received.held > 0 && self.rx_ok(now)))
    }

    pub fn tick(&mut self, now: u64, bell: Bell, loss_percent: u8, rng: &mut Rng) {
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

    pub fn kick(&mut self) {
        self.last_sent_at = None;
    }
}
