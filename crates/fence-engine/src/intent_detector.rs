//! A conservative local motion heuristic, not a learned claim about a user's intention.
//! 64 five-millisecond buckets retain 320ms at any report rate without allocation.
use fence_types::Direction;

#[derive(Clone, Copy, Debug, Default)]
struct Bin {
    stamp: u64,
    dx: i64,
    dy: i64,
    travel_x: i64,
    travel_y: i64,
}

#[derive(Clone, Copy, Debug)]
struct Contact {
    site: usize,
    dir: Direction,
    start: u64,
    good: bool,
}

#[derive(Clone, Debug)]
pub struct Detector {
    bins: [Bin; 64],
    last: Option<u64>,
    buttons: u16,
    contact: Option<Contact>,
    features: [i32; 12],
    judged: bool,
    steady_push_ms: i32,
}

impl Default for Detector {
    fn default() -> Self {
        Self {
            bins: [Bin::default(); 64],
            last: None,
            buttons: 0,
            contact: None,
            features: [0; 12],
            judged: false,
            steady_push_ms: 0,
        }
    }
}

/// Steady-push hold time per ease level when `SteadyPushMs` is 0 (follow the slider).
pub const STEADY_PUSH_MS: [u64; 5] = [90, 75, 60, 50, 40];
/// The last 15ms of push must keep at least this share of the first 15ms after contact.
/// A decelerating overshoot dies out; a deliberate push keeps its pace (a mouse has no wall to feel).
pub const STEADY_KEEP: [f64; 5] = [0.85, 0.85, 0.82, 0.8, 0.8];
/// Counts per millisecond. Above this a steady push is treated as a game swipe and stays held.
/// The older 12 counts/ms limit also held ordinary brisk crossings on high-DPI mice.
pub const STEADY_MAX_SPEED: f64 = 30.0;

impl Detector {
    /// -1 turns the steady-push path off (2.0.1 behaviour), 0 follows the ease slider,
    /// any other value is the hold time in milliseconds (clamped to 30..=500).
    pub fn with_steady_push_ms(mut self, ms: i32) -> Self {
        self.steady_push_ms = ms;
        self
    }
    fn steady_need_us(&self, ease: usize) -> Option<u64> {
        match self.steady_push_ms {
            ms if ms < 0 => None,
            0 => Some(STEADY_PUSH_MS[ease] * 1000),
            ms => Some(ms.clamp(30, 500) as u64 * 1000),
        }
    }
    /// Walks the 5ms buckets in [start, end) and keeps the newest uninterrupted push run:
    /// (run start, outward buckets, buckets with motion, newest moving bucket went outward).
    /// A single silent bucket is normal at 125-200 Hz; two in a row are a pause that
    /// restarts the run, so pressure from before a pause cannot be reused.
    fn push_run(&self, dir: Direction, start: u64, end: u64) -> (u64, u64, u64, bool) {
        let (mut run, mut outward, mut moving, mut gap, mut last_out) = (start, 0, 0, 0, false);
        let mut t = start;
        while t < end {
            let stamp = t / 5000 + 1;
            let b = self.bins[(stamp % 64) as usize];
            if b.stamp == stamp && b.travel_x + b.travel_y > 0 {
                if gap >= 2 || moving == 0 {
                    (run, outward, moving) = (t, 0, 0);
                }
                moving += 1;
                gap = 0;
                last_out = project(dir, b.dx, b.dy).0 > 0;
                outward += u64::from(last_out);
            } else {
                gap += 1;
            }
            t += 5000;
        }
        if gap >= 2 {
            return (end, 0, 0, false);
        }
        (run, outward, moving, last_out)
    }

    /// Leaving a crossing attempt invalidates its contact. A later visit starts fresh.
    pub fn begin_move(&mut self) {
        if !self.judged {
            self.contact = None;
        }
        self.judged = false;
        self.features = [0; 12];
    }
    pub fn features(&self) -> [i32; 12] {
        self.features
    }
    pub fn raw(&mut self, dx: i32, dy: i32, buttons: u16, now: u64) {
        if self.last.is_some_and(|last| now < last || now - last > 250_000) {
            *self = Self::default().with_steady_push_ms(self.steady_push_ms);
        }
        let stamp = now / 5000 + 1;
        let bin = &mut self.bins[(stamp % 64) as usize];
        if bin.stamp != stamp {
            *bin = Bin {
                stamp,
                ..Bin::default()
            };
        }
        bin.dx += i64::from(dx);
        bin.dy += i64::from(dy);
        bin.travel_x += i64::from(dx).abs();
        bin.travel_y += i64::from(dy).abs();
        self.last = Some(now);
        self.buttons = buttons;
        // A reversal of two raw counts invalidates all pressure, including a return overshoot.
        if self
            .contact
            .is_some_and(|c| project(c.dir, i64::from(dx), i64::from(dy)).0 <= -2)
        {
            self.contact = None;
        }
    }

    // Half-open windows exclude arrival's bucket from post-contact pressure.
    fn window(&self, dir: Direction, start: u64, end: u64) -> (f64, f64, usize) {
        let mut p = 0i64;
        let mut o = 0i64;
        let mut active = 0;
        for b in &self.bins {
            if b.stamp == 0 {
                continue;
            }
            let time = (b.stamp - 1) * 5000;
            if time >= start && time < end {
                p += project(dir, b.dx, b.dy).0;
                o += if dir.is_vertical() { b.travel_x } else { b.travel_y };
                if b.travel_x + b.travel_y > 0 {
                    active += 1;
                }
            }
        }
        (p as f64, o as f64, active)
    }

    /// Reasons: 1 raw unavailable, 2 approach held, 3 short push, 4 grazing,
    /// 5 deceleration/flick, 6 deliberate crossing, 7 renewed push, 9 steady push.
    pub fn judge(&mut self, site: usize, dir: Direction, now: u64, ease: u8) -> bool {
        self.judged = true;
        let ease = usize::from(ease.clamp(1, 5) - 1);
        let Some(last) = self.last.filter(|last| *last <= now && now - *last <= 32_000) else {
            self.features = [0; 12];
            self.features[7] = 1;
            self.contact = None;
            return false;
        };
        let (p, o, active) = self.window(dir, now.saturating_sub(200_000), now / 5000 * 5000);
        // Anchor speed to the latest device sample: background WM_INPUT may trail the
        // hook. An undelivered 8ms tail is missing data, not physical deceleration.
        let end = last / 5000 * 5000;
        let (recent, _, _) = self.window(dir, end.saturating_sub(20_000), end);
        let (prior, _, _) = self.window(dir, end.saturating_sub(60_000), end.saturating_sub(20_000));
        let speed = recent.max(0.0) / 20.0;
        let trend = if prior > 0.0 { speed / (prior / 40.0) } else { 1.0 };
        let ratio = [0.48, 0.58, 0.70, 0.82, 0.95][ease]; // max tangent/normal
        let distance = [40.0, 32.0, 24.0, 18.0, 12.0][ease];
        let aligned = p > 0.0 && o <= p * ratio;
        let drag = if self.buttons != 0 { 0.75 } else { 1.0 };
        let good = aligned && p >= distance * drag && active >= 8 && (0.12..=12.0).contains(&speed) && trend >= 0.55;
        let c = match self.contact {
            Some(c) if c.site == site && c.dir == dir && now >= c.start => c,
            _ => Contact {
                site,
                dir,
                start: now,
                good,
            },
        };
        self.contact = Some(c);
        let held = now.saturating_sub(c.start) / 1000;
        let wait = [24, 20, 16, 12, 8][ease];
        let pressure_start = (c.start / 5000 + 2) * 5000;
        let (pressure, tangent, _) = self.window(dir, pressure_start, now / 5000 * 5000);
        let (push, side, push_active) =
            self.window(dir, now.saturating_sub(30_000).max(pressure_start), now / 5000 * 5000);
        let short = [24.0, 20.0, 16.0, 12.0, 8.0][ease] * drag;
        // Bad arrivals cannot turn their overshoot into an immediate pass. A fresh, steady
        // 30ms push after the absorb interval is the escape; just waiting never crosses.
        let renewed = !c.good
            && held >= 65
            && push_active >= 4
            && pressure >= short * 1.5
            && side <= push * ratio
            && (0.12..=12.0).contains(&speed)
            && trend >= 0.75;
        let quick = aligned
            && speed <= 12.0
            && trend >= 0.55
            && pressure >= short
            && tangent <= pressure * ratio
            && ((c.good && held >= wait) || renewed);
        // Steady push (2.0.2): whatever the arrival looked like, keep pushing outward for a
        // short while at an undiminished pace and the edge lets go. Overshoots while aiming at
        // something near the edge die out within a few buckets, reverse, or stop; they stay held.
        let steady = !quick
            && self.steady_need_us(ease).is_some_and(|need| {
                let (run, outward, moving, last_out) = self.push_run(dir, pressure_start, end);
                // Per moving bucket, so 125 Hz reports landing in alternate buckets compare fairly.
                let (first, _, first_n) = self.window(dir, run, run + 15_000);
                let (last15, _, last_n) = self.window(dir, end.saturating_sub(15_000), end);
                let first = first / first_n.max(1) as f64;
                let last15 = last15 / last_n.max(1) as f64;
                let (pushed, side, _) = self.window(dir, run, end);
                // The run starts on a bucket boundary; count the hold from the contact or the
                // run, whichever is later.
                now.saturating_sub(run.max(c.start)) >= need
                    && pushed >= short
                    && side <= pushed * ratio
                    && last_out
                    && outward * 5 >= moving * 4
                    && first > 0.0
                    && last15 >= first * STEADY_KEEP[ease]
                    && speed <= STEADY_MAX_SPEED
            });
        let pass = quick || steady;
        let reason = if steady {
            9
        } else if pass {
            if renewed { 7 } else { 6 }
        } else if !aligned {
            4
        } else if speed > 12.0 || trend < 0.55 {
            5
        } else if !c.good {
            2
        } else {
            3
        };
        self.features = [
            (o.atan2(p.max(0.0)).to_degrees() * 1000.0) as i32,
            (speed * 1000.0) as i32,
            (trend * 1000.0).min(i32::MAX as f64) as i32,
            p.clamp(i32::MIN as f64, i32::MAX as f64) as i32,
            pressure.clamp(i32::MIN as f64, i32::MAX as f64) as i32,
            held.min(i32::MAX as u64) as i32,
            i32::from(self.buttons),
            reason,
            dir.csharp_ordinal(),
            ((now - last) / 1000) as i32,
            0,
            0,
        ];
        pass
    }
}

fn project(dir: Direction, dx: i64, dy: i64) -> (i64, i64) {
    match dir {
        Direction::Up => (-dy, dx.abs()),
        Direction::Down => (dy, dx.abs()),
        Direction::Left => (-dx, dy.abs()),
        Direction::Right => (dx, dy.abs()),
    }
}
