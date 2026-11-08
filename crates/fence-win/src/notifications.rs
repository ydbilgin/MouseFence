//! Tray-thread notification policy; tests substitute the state query and never touch the shell.
use fence_engine::fixes::Fixes;
use std::collections::VecDeque;
use windows_sys::Win32::UI::Shell::{QUNS_ACCEPTS_NOTIFICATIONS, SHQueryUserNotificationState};

pub trait NotificationStatePort {
    fn query(&self) -> i32;
}
pub struct SystemNotificationState;
impl NotificationStatePort for SystemNotificationState {
    fn query(&self) -> i32 {
        let mut state = -1;
        // SAFETY: valid out-pointer, read-only shell query; failure remains quiet.
        if unsafe { SHQueryUserNotificationState(&mut state) } >= 0 {
            state
        } else {
            -1
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub warning: bool,
    pub millis: u32,
}

pub struct QuietBalloons {
    pending: VecDeque<Notice>,
    fixes: Fixes,
}
impl Default for QuietBalloons {
    fn default() -> Self {
        Self::with_fixes(Fixes::ALL)
    }
}
impl QuietBalloons {
    pub fn with_fixes(fixes: Fixes) -> Self {
        Self {
            pending: VecDeque::with_capacity(4),
            fixes,
        }
    }
    pub fn submit(&mut self, port: &dyn NotificationStatePort, notice: Notice) -> Option<Notice> {
        if !self.fixes.j25_fullscreen_quiet || port.query() == QUNS_ACCEPTS_NOTIFICATIONS {
            return Some(notice);
        }
        if self.pending.len() == 4 {
            self.pending.pop_front();
        }
        self.pending.push_back(notice);
        None
    }
    pub fn pop_ready(&mut self, port: &dyn NotificationStatePort) -> Option<Notice> {
        if self.pending.is_empty() {
            return None;
        }
        if self.fixes.j25_fullscreen_quiet && port.query() != QUNS_ACCEPTS_NOTIFICATIONS {
            return None;
        }
        self.pending.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use windows_sys::Win32::UI::Shell::{QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
    struct Fake(Cell<i32>);
    impl NotificationStatePort for Fake {
        fn query(&self) -> i32 {
            self.0.get()
        }
    }
    fn notice(n: usize) -> Notice {
        Notice {
            text: n.to_string(),
            warning: n.is_multiple_of(2),
            millis: 3000,
        }
    }
    #[test]
    fn j25_busy_fullscreen_and_presentation_keep_newest_four_without_delivering() {
        for state in [QUNS_BUSY, QUNS_RUNNING_D3D_FULL_SCREEN, QUNS_PRESENTATION_MODE, -1] {
            let port = Fake(Cell::new(state));
            let mut queue = QuietBalloons::default();
            for n in 0..7 {
                assert_eq!(queue.submit(&port, notice(n)), None);
            }
            assert_eq!(queue.pop_ready(&port), None);
            port.0.set(QUNS_ACCEPTS_NOTIFICATIONS);
            for n in 3..7 {
                assert_eq!(queue.pop_ready(&port), Some(notice(n)));
            }
            assert_eq!(queue.pop_ready(&port), None);
        }
    }
    #[test]
    fn j25_every_queued_delivery_rechecks_fullscreen_and_exact_mode_stays_legacy() {
        let port = Fake(Cell::new(QUNS_BUSY));
        let mut queue = QuietBalloons::default();
        queue.submit(&port, notice(1));
        queue.submit(&port, notice(2));
        port.0.set(QUNS_ACCEPTS_NOTIFICATIONS);
        assert_eq!(queue.pop_ready(&port), Some(notice(1)));
        port.0.set(QUNS_RUNNING_D3D_FULL_SCREEN);
        assert_eq!(queue.pop_ready(&port), None);
        port.0.set(QUNS_ACCEPTS_NOTIFICATIONS);
        assert_eq!(queue.pop_ready(&port), Some(notice(2)));
        let mut exact = QuietBalloons::with_fixes(Fixes::EXACT_1_1_1);
        port.0.set(QUNS_BUSY);
        assert_eq!(exact.submit(&port, notice(3)), Some(notice(3)));
    }
}
