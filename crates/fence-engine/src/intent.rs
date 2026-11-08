//! Pure crossing models. All storage is fixed; timestamps are supplied by the caller.
use fence_types::{CrossingModel, Direction, IntentConfig, MoveEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    pub id: usize,
    pub minimum: i32,
    pub slack: i32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Motion {
    dx: i32,
    dy: i32,
    time_ms: u32,
}

#[derive(Clone, Copy, Debug)]
struct Contact {
    site: usize,
    dir: Direction,
    first_us: u64,
    last_out_us: u64,
    distance: i64,
    inward: i64,
    slack: i32,
    last_serial: u64,
    last_answer: bool,
}

#[derive(Clone, Debug)]
pub struct Intent {
    detector: crate::intent_detector::Detector,
    config: IntentConfig,
    ring: [Motion; 64],
    next: usize,
    len: usize,
    last_ms: Option<u32>,
    last_qpc: Option<u64>,
    window_start: u32,
    window_count: u32,
    rates: [f64; 8],
    rate_next: usize,
    rate_len: usize,
    rate: f64,
    serial: u64,
    contact: Option<Contact>,
}

impl Intent {
    pub fn new(config: IntentConfig) -> Self {
        Self {
            detector: crate::intent_detector::Detector::default().with_steady_push_ms(config.steady_push_ms),
            config,
            ring: [Motion::default(); 64],
            next: 0,
            len: 0,
            last_ms: None,
            last_qpc: None,
            window_start: 0,
            window_count: 0,
            rates: [0.0; 8],
            rate_next: 0,
            rate_len: 0,
            rate: 1.0,
            serial: 0,
            contact: None,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.config);
    }
    pub fn configure(&mut self, config: IntentConfig) {
        *self = Self::new(config);
    }
    pub fn estimated_rate(&self) -> f64 {
        self.rate
    }
    pub fn raw(&mut self, dx: i32, dy: i32, buttons: u16, now: u64) {
        self.detector.raw(dx, dy, buttons, now);
    }
    pub fn features(&self) -> [i32; 12] {
        self.detector.features()
    }
    pub fn state(&self) -> u8 {
        match self.contact {
            None => 0,
            Some(c) if c.distance == 0 => 1,
            Some(_) => 2,
        }
    }

    /// Called once for every physical report, including reports that do not reach a boundary.
    pub fn record(&mut self, dx: i32, dy: i32, ev: &MoveEvent) {
        if self.config.model == CrossingModel::Intent {
            self.detector.begin_move();
        }
        self.serial = self.serial.wrapping_add(1);
        let gap = self.last_ms.map(|last| ev.time_ms.wrapping_sub(last));
        if self.last_ms.is_none() || gap.is_some_and(|n| n > 50) {
            self.next = 0;
            self.len = 0;
            self.window_start = ev.time_ms;
            self.window_count = 0;
        }
        if self.last_qpc.is_some_and(|last| ev.qpc_us < last) {
            self.contact = None;
        }
        let elapsed = ev.time_ms.wrapping_sub(self.window_start);
        if elapsed >= 250 {
            if self.window_count >= 100 {
                let rate = (self.window_count as f64 / elapsed as f64).clamp(0.125, 8.0);
                self.rates[self.rate_next] = rate;
                self.rate_next = (self.rate_next + 1) % 8;
                self.rate_len = (self.rate_len + 1).min(8);
                let mut sorted = self.rates;
                sorted[..self.rate_len].sort_by(f64::total_cmp);
                let mid = self.rate_len / 2;
                self.rate = if self.rate_len.is_multiple_of(2) {
                    (sorted[mid - 1] + sorted[mid]) / 2.0
                } else {
                    sorted[mid]
                };
            }
            self.window_start = ev.time_ms;
            self.window_count = 0;
        }
        self.window_count = self.window_count.saturating_add(1);
        self.last_ms = Some(ev.time_ms);
        self.last_qpc = Some(ev.qpc_us);
        self.ring[self.next] = Motion {
            dx,
            dy,
            time_ms: ev.time_ms,
        };
        self.next = (self.next + 1) % 64;
        self.len = (self.len + 1).min(64);
        if let Some(mut c) = self.contact {
            let (primary, orth) = parts(c.dir, dx, dy);
            if primary < 0 {
                c.inward = c.inward.saturating_add(-primary);
            }
            if primary > 0 {
                c.inward = 0;
            }
            let expired = ev.qpc_us.saturating_sub(c.last_out_us) >= u64::from(self.config.push_pause_reset_ms) * 1000;
            if c.inward >= 2 || expired || orth > primary.max(0) + i64::from(c.slack) {
                self.contact = None;
            } else {
                self.contact = Some(c);
            }
        }
    }

    /// The sole intent entry point for compiled, legacy side and legacy upper gates.
    pub fn judge(
        &mut self,
        site: Site,
        dir: Direction,
        primary: i32,
        orthogonal: i32,
        ev: &MoveEvent,
        at_edge: bool,
    ) -> bool {
        let classic = || primary >= site.minimum && orthogonal <= primary.wrapping_add(site.slack);
        match self.config.model {
            CrossingModel::Intent => self.detector.judge(site.id, dir, ev.qpc_us, self.config.crossing_ease),
            CrossingModel::Classic => classic(),
            CrossingModel::Classic1k => {
                let k = (self.rate.round() as usize).clamp(1, 8);
                if k == 1 {
                    return classic();
                }
                let (mut p, mut o) = (0i64, 0i64);
                for i in 0..k.min(self.len) {
                    let motion = self.ring[(self.next + 63 - i) % 64];
                    if ev.time_ms.wrapping_sub(motion.time_ms) > 1 {
                        break;
                    }
                    let (dp, orth) = parts(dir, motion.dx, motion.dy);
                    p += dp;
                    o += orth;
                }
                p >= i64::from(site.minimum) && o <= p + i64::from(site.slack)
            }
            CrossingModel::StopThenPush => {
                let contact = self.contact.filter(|c| c.site == site.id && c.dir == dir);
                let Some(mut c) = contact else {
                    self.contact = Some(Contact {
                        site: site.id,
                        dir,
                        first_us: ev.qpc_us,
                        last_out_us: ev.qpc_us,
                        distance: 0,
                        inward: 0,
                        slack: site.slack,
                        last_serial: self.serial,
                        last_answer: false,
                    });
                    return false;
                };
                if c.last_serial == self.serial {
                    return c.last_answer;
                }
                c.last_serial = self.serial;
                c.last_answer = false;
                if primary > 0 {
                    c.last_out_us = ev.qpc_us;
                }
                // A one-pixel inward detour keeps contact; its return overshoot is held.
                if !at_edge {
                    self.contact = Some(c);
                    return false;
                }
                if primary <= 0 || orthogonal > primary.saturating_add(site.slack) {
                    self.contact = Some(c);
                    return false;
                }
                if ev.qpc_us.saturating_sub(c.first_us) < u64::from(self.config.push_absorb_ms) * 1000 {
                    self.contact = Some(c);
                    return false;
                }
                c.distance = c.distance.saturating_add(i64::from(primary));
                let distance = if dir.is_vertical() {
                    self.config.up_push_distance_px
                } else {
                    self.config.side_push_distance_px
                };
                c.last_answer = c.distance >= i64::from(distance);
                self.contact = Some(c);
                c.last_answer
            }
        }
    }
}

fn parts(dir: Direction, dx: i32, dy: i32) -> (i64, i64) {
    let (dx, dy) = (i64::from(dx), i64::from(dy));
    match dir {
        Direction::Up => (-dy, dx.abs()),
        Direction::Down => (dy, dx.abs()),
        Direction::Left => (-dx, dy.abs()),
        Direction::Right => (dx, dy.abs()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SITE: Site = Site {
        id: 1,
        minimum: 3,
        slack: 5,
    };
    #[test]
    fn j13_rate_window_wrap_gap_and_reset_use_fixed_storage() {
        let mut intent = Intent::new(IntentConfig::default());
        for i in 0..=2400 {
            let ev = MoveEvent {
                x: 0,
                y: 0,
                time_ms: i / 8,
                qpc_us: i as u64 * 125,
            };
            intent.record(1, 0, &ev);
        }
        assert_eq!(intent.estimated_rate(), 8.0);
        let ev = MoveEvent {
            x: 0,
            y: 0,
            time_ms: 400,
            qpc_us: 400000,
        };
        intent.record(1, 0, &ev);
        assert!(
            !intent.judge(SITE, Direction::Right, 1, 0, &ev, true),
            "a >50ms gap clears motion history"
        );
        intent.reset();
        assert_eq!(intent.estimated_rate(), 1.0);
        intent.record(
            1,
            0,
            &MoveEvent {
                time_ms: u32::MAX,
                ..MoveEvent::default()
            },
        );
        intent.record(
            1,
            0,
            &MoveEvent {
                time_ms: 0,
                ..MoveEvent::default()
            },
        );
        assert_eq!(intent.len, 2);
    }
    #[test]
    fn j14_arrival_wait_pause_inward_and_duplicate_site_judges_do_not_build_pressure() {
        let mut intent = Intent::new(IntentConfig {
            model: CrossingModel::StopThenPush,
            ..IntentConfig::default()
        });
        let event = |ms| MoveEvent {
            time_ms: ms,
            qpc_us: ms as u64 * 1000,
            ..MoveEvent::default()
        };
        let ev = event(1);
        intent.record(100, 0, &ev);
        assert!(!intent.judge(SITE, Direction::Right, 100, 0, &ev, false));
        let ev = event(80);
        intent.record(0, 0, &ev);
        assert!(!intent.judge(SITE, Direction::Right, 0, 0, &ev, true));
        for ms in 81..110 {
            let ev = event(ms);
            intent.record(4, 0, &ev);
            let first = intent.judge(SITE, Direction::Right, 4, 0, &ev, true);
            assert_eq!(first, intent.judge(SITE, Direction::Right, 4, 0, &ev, true));
            assert!(!first);
        }
        let ev = event(110);
        intent.record(4, 0, &ev);
        assert!(intent.judge(SITE, Direction::Right, 4, 0, &ev, true));
        let ev = event(111);
        intent.record(-2, 0, &ev);
        assert_eq!(intent.state(), 0);
        let ev = event(112);
        intent.record(1000, 0, &ev);
        assert!(!intent.judge(SITE, Direction::Right, 1000, 0, &ev, true));
        let ev = event(500);
        intent.record(1000, 0, &ev);
        assert!(!intent.judge(SITE, Direction::Right, 1000, 0, &ev, true));
    }
}
