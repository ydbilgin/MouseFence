//! Controller behaviour over fake ports (docs/rust2/parity.tsv rows B001-B015, B029 and J02; Track C).
//! C# reference: Ui/TrayApplicationContext.cs (1.1.1, public main 19b769c).

mod support;

use fence_app::{
    BalloonKind, Event, Flow, HOOK_PROBE_INTERVAL_MS, HotkeySlot, MenuCommand, MenuEntry, RECONFIGURE_DEBOUNCE_MS,
    TimerId, TrayIcon, TrayView,
};
use fence_config::{LoadSource, Loaded, Settings};
use fence_i18n::Lang;
use fence_types::{HookHealthVerdict, Hotkey, PlanReport, Rect, RuntimeState};
use support::*;

fn balloon_texts(h: &Harness) -> Vec<(u32, BalloonKind, String)> {
    h.balloons().into_iter().map(|b| (b.millis, b.kind, b.text)).collect()
}

#[test]
fn j26_probe_logs_hook_metrics_every_three_seconds_on_tray_thread() {
    let mut h = Harness::started(Settings::default());
    h.w.borrow_mut().metrics = Some(fence_types::HookMetrics {
        callbacks: 100000,
        sampled_steps: 99999,
        max_step_us: 1501,
        p99_upper_us: 20,
        reinstall_attempts: 2,
        degraded: 3,
        panics: 4,
        trace_dropped: 5,
    });
    h.advance(2999);
    assert!(h.w.borrow().logs.is_empty());
    h.advance(1);
    assert_eq!(
        h.w.borrow().logs,
        vec![
            "hook metrics since start: callbacks=100000 sampled=99999 max_us=1501 p99_upper_us=20 reinstall_attempts=2 degraded=3 panics=4 trace_dropped=5"
        ]
    );
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    h.advance(3000);
    assert_eq!(h.w.borrow().logs.len(), 2);
}

#[test]
fn j25_notification_queue_is_probed_every_three_seconds_even_while_paused() {
    let mut h = Harness::started(Settings::default());
    h.advance(2999);
    assert_eq!(h.w.borrow().notification_probes, 0);
    h.advance(1);
    assert_eq!(h.w.borrow().notification_probes, 1);
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    h.advance(3000);
    assert_eq!(h.w.borrow().notification_probes, 2);
}

#[test]
fn j24_trace_is_opt_in_drains_each_two_seconds_and_accident_key_only_updates_tooltip() {
    let mut off = Harness::started(Settings::default());
    assert!(!off.w.borrow().tracing);
    assert!(!off.w.borrow().registered.contains_key(&HotkeySlot::MarkAccident));
    off.ev(Event::Hotkey(HotkeySlot::MarkAccident));
    assert_eq!(off.w.borrow().trace_marks, 0);
    let mut h = Harness::started(Settings {
        edge_trace: true,
        ..Settings::default()
    });
    assert!(h.w.borrow().tracing);
    assert!(h.w.borrow().registered.contains_key(&HotkeySlot::MarkAccident));
    assert!(h.w.borrow().timers.contains_key(&TimerId::TraceDrain));
    h.clear_log();
    let before = h.balloons().len();
    h.ev(Event::Hotkey(HotkeySlot::MarkAccident));
    assert_eq!(h.w.borrow().trace_marks, 1);
    assert_eq!(h.balloons().len(), before);
    assert!(h.last_view().tooltip.contains("Accidents marked: 1"));
    h.ev(Event::Timer(TimerId::TraceDrain));
    assert_eq!(h.w.borrow().trace_marks, 1);
}

#[test]
fn j21_tray_switch_names_match_settings_and_keep_all_shortcuts_when_paused() {
    let s = Settings::default();
    let mut h = Harness::started(s.clone());
    let v = h.last_view();
    assert_eq!(v.gate.text, en("HotkeyHead"));
    assert_eq!(v.side.text, en("SideHotkeyHead"));
    assert_eq!(v.game.text, en("MenuGame"));
    assert_eq!(v.pause.text, en("MenuPause"));
    assert_eq!(
        (&v.gate.shortcut, &v.side.shortcut, &v.game.shortcut, &v.pause.shortcut),
        (
            &hk(&s.hotkey()),
            &hk(&s.side_hotkey()),
            &hk(&s.confine_hotkey()),
            &hk(&s.pause_hotkey())
        )
    );
    h.ev(Event::Menu(MenuCommand::Pause));
    let v = h.last_view();
    assert_eq!(v.pause.text, en("MenuPause"));
    assert_eq!(v.pause.shortcut, hk(&s.pause_hotkey()));
    assert!(!v.gate.enabled && !v.side.enabled && !v.game.enabled && v.pause.checked);
}

#[test]
fn j16_cleared_optional_confine_shortcut_does_not_warn() {
    let s = Settings {
        confine_hot_key: 0,
        confine_mod_ctrl: false,
        confine_mod_alt: false,
        confine_mod_shift: false,
        confine_mod_win: false,
        ..Settings::default()
    };
    let h = Harness::started(s);
    assert!(
        !h.balloons().iter().any(|b| b.text.contains("(none)")),
        "cleared optional shortcut must not warn"
    );
}

// ---------------------------------------------------------------- B001 startup order

#[test]
fn b001_startup_runs_in_the_1x_order() {
    let h = Harness::started(Settings::default());
    let log = h.log();
    let order = [
        "store.load",
        "tray.add_icon(Closed,MouseFence)",
        "hotkeys.register(Gate)",
        "hotkeys.register(Confine)",
        "hotkeys.register(Side)",
        "hotkeys.register(Pause)",
        "system.monitors",
        "core.plan",
        "guard.configure",
        "guard.gate(false)",
        "guard.side(true)",
        "guard.start",
        "timers.start(HookProbe,3000)",
        "tray.show",
        "system.intel",
    ];
    let at: Vec<usize> = order.iter().map(|l| pos(&log, l)).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "order {order:?} in {log:#?}");
    assert_eq!(HOOK_PROBE_INTERVAL_MS, 3000);
    assert!(
        !log.iter().any(|l| l.starts_with("timers.start(Reconfigure")),
        "the 750 ms debounce only starts on a display change"
    );
    let w = h.w.borrow();
    assert!(w.enabled && w.installed, "the hook is started");
    assert_eq!(w.icon_added, Some((TrayIcon::Closed, "MouseFence".to_string())));
}

#[test]
fn b001_gate_is_not_start_gate_closed_and_side_is_start_side_contain_on() {
    for (closed, side_on) in [(true, true), (false, false), (true, false), (false, true)] {
        let s = Settings {
            start_gate_closed: closed,
            start_side_contain_on: side_on,
            ..Settings::default()
        };
        let h = Harness::started(s);
        let w = h.w.borrow();
        assert_eq!((w.gate, w.side), (!closed, side_on));
        assert_eq!(h.ctl.runtime_state().gate_open, !closed);
    }
}

#[test]
fn b001_language_is_applied_from_settings_and_the_os() {
    let tr = Settings {
        language: "tr".into(),
        ..Settings::default()
    };
    let h = Harness::started(tr);
    assert_eq!(h.ctl.lang(), Lang::Tr);
    assert_eq!(h.last_view().settings.text, fence_i18n::get(Lang::Tr, "MenuSettings"));

    let mut h = Harness::new(World {
        turkish: true,
        ..World::default()
    });
    h.ctl.start();
    assert_eq!(h.ctl.lang(), Lang::Tr, "auto follows a Turkish OS");

    let h = Harness::started(Settings::default());
    assert_eq!(h.ctl.lang(), Lang::En, "auto on a non-Turkish OS is English");
}

#[test]
fn b001_display_changes_are_followed_only_after_start() {
    let mut h = Harness::new(World::default());
    h.ev(Event::DisplayChanged);
    assert!(!h.log().iter().any(|l| l.starts_with("timers.start(Reconfigure")));
    h.ctl.start();
    h.ev(Event::DisplayChanged);
    assert!(h.log().contains(&"timers.start(Reconfigure,750)".to_string()));
}

#[test]
fn b001_a_load_problem_is_logged_and_the_tray_still_starts() {
    let mut h = Harness::new(World {
        load: Some(Loaded {
            settings: Settings::default(),
            source: LoadSource::DefaultsNotSaved {
                problem: "bad bytes".into(),
            },
        }),
        ..World::default()
    });
    h.ctl.start();
    let w = h.w.borrow();
    assert!(w.logs.iter().any(|l| l.contains("bad bytes")), "{:?}", w.logs);
    assert!(w.enabled);
    // 2.0 (join, JUDGE-C ruling 2): he is told in words, not only in the debug log.
    let b = w.balloons.first().expect("a balloon about the unreadable file");
    assert_eq!(
        (b.millis, b.kind, b.text.as_str()),
        (8000, BalloonKind::Warning, en("TipSettingsUnreadable").as_str())
    );
    let added = w.log.iter().position(|l| l.starts_with("tray.add_icon"));
    let shown = w.log.iter().position(|l| l.starts_with("tray.balloon"));
    assert!(
        added.is_some() && shown.is_some() && added < shown,
        "icon first: {:?}",
        w.log
    );
}

// ---------------------------------------------------------------- J02 startup choices (commit 16c1753)

fn start_choices(mask: u32) -> Settings {
    Settings {
        start_gate_closed: mask & 1 != 0,
        start_side_contain_on: mask & 2 != 0,
        start_confine_on: mask & 4 != 0,
        start_paused: mask & 8 != 0,
        ..Settings::default()
    }
}

#[test]
fn j02_all_16_combinations_apply_gate_side_confine_and_pause() {
    for mask in 0..16 {
        let s = start_choices(mask);
        let h = Harness::started(s.clone());
        let w = h.w.borrow();
        assert_eq!(
            (w.gate, w.side, w.confine),
            (!s.start_gate_closed, s.start_side_contain_on, s.start_confine_on),
            "combination {mask}: gate, side, confine"
        );
        assert_eq!(
            h.count("guard.start"),
            usize::from(!s.start_paused),
            "combination {mask}: starts"
        );
        assert_eq!(w.enabled, !s.start_paused, "combination {mask}: enabled");
        let state = h.ctl.runtime_state();
        assert_eq!(
            (state.gate_open, state.side_contain, state.confine, state.paused),
            (
                !s.start_gate_closed,
                s.start_side_contain_on,
                s.start_confine_on,
                s.start_paused
            ),
            "combination {mask}: the live state the settings window sees"
        );
    }
}

#[test]
fn j02_the_hook_starts_only_after_gate_side_and_confine_are_set() {
    let h = Harness::started(Settings {
        start_confine_on: true,
        ..Settings::default()
    });
    let log = h.log();
    let order = [
        "guard.configure",
        "guard.gate(false)",
        "guard.side(true)",
        "guard.confine(true)",
        "guard.start",
        "timers.start(HookProbe,3000)",
    ];
    let at: Vec<usize> = order.iter().map(|l| pos(&log, l)).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "order {order:?} in {log:#?}");
    assert_eq!(h.count("guard.start"), 1);
    assert!(
        h.last_view().game.checked,
        "the game item shows the confine it started with"
    );
}

#[test]
fn j02_defaults_start_the_guard_unconfined() {
    let h = Harness::started(Settings::default());
    let w = h.w.borrow();
    assert!(!w.confine && w.enabled && w.installed);
    assert_eq!(h.count("guard.confine(false)"), 1, "confine is set explicitly, off");
    assert_eq!(h.count("guard.start"), 1);
}

#[test]
fn j02_a_paused_start_shows_the_paused_view_and_the_pause_key_resumes() {
    let s = Settings {
        start_paused: true,
        start_confine_on: true,
        ..Settings::default()
    };
    let mut h = Harness::started(s.clone());
    {
        let w = h.w.borrow();
        assert!(!w.enabled && !w.installed, "no hook at launch");
        assert!(w.balloons.is_empty(), "a paused start is silent");
    }
    let v = h.last_view();
    assert_eq!(v.icon, TrayIcon::Paused);
    assert_eq!(v.tooltip, enf("TrayPaused", &[&hk(&s.hotkey())]));
    assert!(v.pause.checked && !v.gate.enabled);
    assert!(
        h.log().contains(&"timers.start(HookProbe,3000)".to_string()),
        "the probe timer still runs"
    );

    h.clear_log();
    h.advance(9000);
    assert!(
        !h.log().iter().any(|l| l.starts_with("guard.check_health")),
        "a deliberately paused start is never probed or reinstalled"
    );

    h.ev(Event::Hotkey(HotkeySlot::Pause));
    let w = h.w.borrow();
    assert!(w.enabled && w.installed, "resume starts the hook");
    assert!(w.confine, "with the confine it was told at launch");
    drop(w);
    assert_eq!(h.balloons().last().unwrap().text, en("TipResumed"));
    let v = h.last_view();
    assert_eq!(v.icon, TrayIcon::Closed);
    assert!(v.game.checked && !v.pause.checked);
}

// ---------------------------------------------------------------- B002 menu order

#[test]
fn b002_menu_order_is_gate_side_game_pause_separator_settings_exit() {
    let h = Harness::started(Settings::default());
    let view = h.last_view();
    let order: Vec<Option<MenuCommand>> = view
        .menu()
        .iter()
        .map(|e| match e {
            MenuEntry::Item(c, _) => Some(*c),
            MenuEntry::Separator => None,
        })
        .collect();
    assert_eq!(
        order,
        vec![
            Some(MenuCommand::Gate),
            Some(MenuCommand::Side),
            Some(MenuCommand::Game),
            Some(MenuCommand::Pause),
            None,
            Some(MenuCommand::Settings),
            Some(MenuCommand::Exit),
        ]
    );
    assert_eq!(view.settings.text, en("MenuSettings"));
    assert_eq!(view.exit.text, en("MenuExit"));
    assert_eq!(view.game.text, en("MenuGame"));
    for c in MenuCommand::ALL {
        assert_ne!(c.id(), 0, "0 means no choice in TrackPopupMenu");
        assert_eq!(MenuCommand::from_id(c.id()), Some(c));
    }
    assert_eq!(MenuCommand::from_id(0), None);
}

// ---------------------------------------------------------------- B003 double click

#[test]
fn b003_double_click_toggles_the_gate() {
    let mut h = Harness::started(Settings::default());
    assert!(!h.w.borrow().gate);
    h.clear_log();
    assert_eq!(h.ev(Event::TrayDoubleClick), Flow::Continue);
    assert!(h.w.borrow().gate);
    assert_eq!(h.last_view().icon, TrayIcon::Open);
    h.ev(Event::TrayDoubleClick);
    assert!(!h.w.borrow().gate);
    assert_eq!(h.last_view().icon, TrayIcon::Closed);
}

// ---------------------------------------------------------------- B004 hotkey wiring

#[test]
fn b004_each_hotkey_drives_its_toggle() {
    let mut h = Harness::started(Settings::default());
    h.ev(Event::Hotkey(HotkeySlot::Gate));
    assert!(h.w.borrow().gate, "gate hotkey toggles the gate");
    h.ev(Event::Hotkey(HotkeySlot::Confine));
    assert!(h.w.borrow().confine, "confine hotkey toggles game mode");
    h.ev(Event::Hotkey(HotkeySlot::Side));
    assert!(!h.w.borrow().side, "side hotkey toggles side containment");
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    assert!(!h.w.borrow().enabled, "pause hotkey pauses");
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    assert!(h.w.borrow().enabled, "and resumes");
    for s in HotkeySlot::ALL {
        assert_eq!(HotkeySlot::from_id(s.id()), Some(s));
    }
    assert_eq!(HotkeySlot::Gate.id(), 0xB10C, "the 1.x hotkey id");
}

#[test]
fn b004_menu_items_drive_the_same_toggles() {
    let mut h = Harness::started(Settings::default());
    h.ev(Event::Menu(MenuCommand::Gate));
    h.ev(Event::Menu(MenuCommand::Side));
    h.ev(Event::Menu(MenuCommand::Game));
    {
        let w = h.w.borrow();
        assert!(w.gate && !w.side && w.confine);
    }
    h.ev(Event::Menu(MenuCommand::Pause));
    assert!(!h.w.borrow().enabled);
}

// ---------------------------------------------------------------- B005 registration failures

#[test]
fn b005_failed_registrations_warn_in_slot_order() {
    let s = Settings::default();
    let mut h = Harness::new(World {
        refuse: HotkeySlot::ALL.into_iter().collect(),
        load: Some(Loaded {
            settings: s.clone(),
            source: LoadSource::V2,
        }),
        ..World::default()
    });
    h.ctl.start();
    let fails: Vec<_> = balloon_texts(&h)
        .into_iter()
        .filter(|(ms, kind, _)| *ms == 3000 && *kind == BalloonKind::Warning)
        .map(|(_, _, t)| t)
        .collect();
    assert_eq!(
        fails,
        vec![
            enf("TipHotkeyFail", &[&hk(&s.hotkey())]),
            enf("TipHotkeyFail", &[&hk(&s.confine_hotkey())]),
            enf("TipHotkeyFail", &[&hk(&s.side_hotkey())]),
            enf("TipHotkeyFail", &[&hk(&s.pause_hotkey())]),
        ]
    );
}

#[test]
fn b005_gate_warns_and_optional_shortcuts_warn_only_when_set() {
    let s = Settings {
        hot_key: 0,
        confine_hot_key: 0,
        side_hot_key: 0,
        pause_hot_key: 0,
        ..Settings::default()
    };
    let h = Harness::started(s.clone());
    let fails: Vec<_> = balloon_texts(&h).into_iter().filter(|(ms, _, _)| *ms == 3000).collect();
    assert_eq!(fails.len(), 1, "only the required gate warns: {fails:?}");
    assert_eq!(fails[0].2, enf("TipHotkeyFail", &[&hk(&s.hotkey())]));
    let log = h.log();
    assert!(
        !log.contains(&"hotkeys.register(Side)".to_string()),
        "a cleared side key is not registered"
    );
    assert!(
        log.contains(&"hotkeys.register(Pause)".to_string()),
        "pause registration always runs (its result is remembered)"
    );
}

#[test]
fn b005_pause_registration_result_feeds_the_escape_key_label() {
    let isolated = PlanReport {
        isolated: vec!["SYN-B".into()],
        isolated_indices: vec![2],
        ..PlanReport::default()
    };
    // Registered: the label is the pause chord.
    let s = Settings::default();
    let mut h = Harness::new(World {
        reports: [isolated.clone()].into(),
        load: Some(Loaded {
            settings: s.clone(),
            source: LoadSource::V2,
        }),
        ..World::default()
    });
    h.ctl.start();
    let warn = balloon_texts(&h).into_iter().find(|(ms, _, _)| *ms == 5000).unwrap();
    assert_eq!(warn.2, enf("TipIsolated", &["2", &hk(&s.pause_hotkey())]));

    // Set but refused: "(not registered)".
    let mut h = Harness::new(World {
        reports: [isolated.clone()].into(),
        refuse: [HotkeySlot::Pause].into(),
        load: Some(Loaded {
            settings: s.clone(),
            source: LoadSource::V2,
        }),
        ..World::default()
    });
    h.ctl.start();
    let warn = balloon_texts(&h).into_iter().find(|(ms, _, _)| *ms == 5000).unwrap();
    assert_eq!(warn.2, enf("TipIsolated", &["2", &en("NoneRegistered")]));

    // Cleared on purpose: the cleared chord's own text ("(none)").
    let cleared = Settings {
        pause_hot_key: 0,
        ..Settings::default()
    };
    let mut h = Harness::new(World {
        reports: [isolated].into(),
        load: Some(Loaded {
            settings: cleared.clone(),
            source: LoadSource::V2,
        }),
        ..World::default()
    });
    h.ctl.start();
    let warn = balloon_texts(&h).into_iter().find(|(ms, _, _)| *ms == 5000).unwrap();
    assert_eq!(warn.2, enf("TipIsolated", &["2", &hk(&cleared.pause_hotkey())]));
}

// ---------------------------------------------------------------- B006 toggles and balloons

#[test]
fn b006_toggles_show_their_balloons_while_enabled() {
    let mut h = Harness::started(Settings::default());
    h.clear_log();
    h.ev(Event::Menu(MenuCommand::Gate));
    h.ev(Event::Menu(MenuCommand::Gate));
    h.ev(Event::Menu(MenuCommand::Side));
    h.ev(Event::Menu(MenuCommand::Side));
    h.ev(Event::Menu(MenuCommand::Game));
    h.ev(Event::Menu(MenuCommand::Game));
    assert_eq!(
        balloon_texts(&h),
        vec![
            (1000, BalloonKind::Info, en("TipOpen")),
            (1000, BalloonKind::Info, en("TipClosed")),
            (1000, BalloonKind::Info, en("TipSideOff")),
            (1000, BalloonKind::Info, en("TipSideOn")),
            (1200, BalloonKind::Info, en("TipGameOn")),
            (1200, BalloonKind::Info, en("TipGameOff")),
        ]
    );
    assert_eq!(h.count("tray.show"), 6, "every toggle updates the UI");
}

#[test]
fn b006_toggles_while_paused_update_the_ui_without_a_balloon() {
    let mut h = Harness::started(Settings::default());
    h.ev(Event::Menu(MenuCommand::Pause));
    h.clear_log();
    h.ev(Event::Menu(MenuCommand::Gate));
    h.ev(Event::Hotkey(HotkeySlot::Side));
    h.ev(Event::Hotkey(HotkeySlot::Confine));
    assert!(h.balloons().is_empty());
    assert_eq!(h.count("tray.show"), 3);
    let w = h.w.borrow();
    assert!(w.gate && !w.side && w.confine, "the switches still change while paused");
}

// ---------------------------------------------------------------- B007 pause

#[test]
fn b007_pause_and_resume_confirm_with_balloons() {
    let mut h = Harness::started(Settings::default());
    h.clear_log();
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    assert_eq!(
        balloon_texts(&h),
        vec![
            (1200, BalloonKind::Info, en("TipPaused")),
            (1200, BalloonKind::Info, en("TipResumed")),
        ]
    );
    let log = h.log();
    assert!(pos(&log, "guard.stop") < pos(&log, "guard.start"));
}

#[test]
fn b007_a_resume_that_cannot_hook_says_guard_lost() {
    let mut h = Harness::started(Settings::default());
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    h.w.borrow_mut().install_ok = false;
    h.clear_log();
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    assert_eq!(
        balloon_texts(&h),
        vec![(8000, BalloonKind::Warning, en("TipGuardLost"))]
    );
}

#[test]
fn b007_a_pause_cycle_rearms_the_outage_latch() {
    let mut h = Harness::started(Settings::default());
    h.w.borrow_mut().verdicts = [HookHealthVerdict::Dead, HookHealthVerdict::Dead].into();
    h.advance(6000);
    assert_eq!(h.balloons().len(), 1, "one outage, one balloon");
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    h.clear_log();
    h.w.borrow_mut().verdicts = [HookHealthVerdict::Dead].into();
    h.advance(3000);
    assert_eq!(
        balloon_texts(&h),
        vec![(4000, BalloonKind::Info, en("TipGuardRestored"))],
        "announced again after the manual pause/resume"
    );
}

// ---------------------------------------------------------------- B008 paused view

#[test]
fn b008_paused_view() {
    let s = Settings::default();
    let mut h = Harness::started(s.clone());
    h.ev(Event::Menu(MenuCommand::Gate)); // open, so a check mark is set before the pause
    h.ev(Event::Menu(MenuCommand::Pause));
    let v = h.last_view();
    assert_eq!(v.icon, TrayIcon::Paused);
    assert_eq!(v.tooltip, enf("TrayPaused", &[&hk(&s.hotkey())]));
    assert_eq!(
        (v.gate.text.as_str(), v.gate.enabled),
        (en("GatePausedMenu").as_str(), false)
    );
    assert_eq!(
        (v.side.text.as_str(), v.side.enabled),
        (en("SideGatePausedMenu").as_str(), false)
    );
    assert!(!v.game.enabled);
    assert_eq!(v.game.text, en("MenuGame"));
    assert_eq!(
        (v.pause.text.as_str(), v.pause.checked),
        (en("MenuPause").as_str(), true)
    );
    // 1.x leaves the check marks of the last running view untouched while paused.
    assert!(v.gate.checked, "gate check mark kept from before the pause");
    assert!(v.side.checked, "side check mark kept from before the pause");
}

// ---------------------------------------------------------------- B009 running view

#[test]
fn b009_running_view_follows_the_switches() {
    let s = Settings::default();
    let mut h = Harness::started(s.clone());
    let v = h.last_view();
    assert_eq!(v.icon, TrayIcon::Closed);
    assert_eq!(v.tooltip, enf("TrayClosed", &[&hk(&s.hotkey())]));
    assert_eq!((v.gate.text.clone(), v.gate.checked), (en("HotkeyHead"), false));
    assert_eq!((v.side.text.clone(), v.side.checked), (en("SideHotkeyHead"), true));
    assert_eq!((v.pause.text.clone(), v.pause.checked), (en("MenuPause"), false));
    assert!(v.gate.enabled && v.side.enabled && v.game.enabled && !v.game.checked);

    h.ev(Event::Menu(MenuCommand::Gate));
    h.ev(Event::Menu(MenuCommand::Side));
    h.ev(Event::Menu(MenuCommand::Game));
    let v = h.last_view();
    assert_eq!(v.icon, TrayIcon::Open);
    assert_eq!(v.tooltip, enf("TrayOpen", &[&hk(&s.hotkey())]));
    assert_eq!((v.gate.text.clone(), v.gate.checked), (en("HotkeyHead"), true));
    assert_eq!((v.side.text.clone(), v.side.checked), (en("SideHotkeyHead"), false));
    assert!(v.game.checked);
}

#[test]
fn b009_honesty_gate_running_without_a_hook_shows_guard_lost() {
    let s = Settings::default();
    let mut h = Harness::new(World {
        install_ok: false,
        load: Some(Loaded {
            settings: s.clone(),
            source: LoadSource::V2,
        }),
        ..World::default()
    });
    h.ctl.start();
    let v = h.last_view();
    assert_eq!(v.icon, TrayIcon::Paused);
    assert_eq!(v.tooltip, enf("TrayGuardLost", &[&hk(&s.hotkey())]));
    assert_eq!(
        v.pause.text,
        en("MenuPause"),
        "still running: the pause item says Pause"
    );
    assert!(v.gate.enabled);
}

// ---------------------------------------------------------------- B010 hook probe

#[test]
fn b010_probe_runs_every_3000_ms_and_skips_while_paused() {
    let mut h = Harness::started(Settings::default());
    h.clear_log();
    h.advance(2999);
    assert_eq!(h.count("guard.check_health(Inconclusive)"), 0);
    h.advance(1);
    assert_eq!(h.count("guard.check_health(Inconclusive)"), 1);
    assert_eq!(h.count("tray.show"), 0, "Inconclusive does nothing");
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    h.clear_log();
    h.advance(9000);
    assert!(
        !h.log().iter().any(|l| l.starts_with("guard.check_health")),
        "skipped while paused"
    );
}

#[test]
fn b010_dead_announces_once_per_outage_and_alive_rearms() {
    let mut h = Harness::started(Settings::default());
    h.clear_log();
    h.w.borrow_mut().verdicts = [
        HookHealthVerdict::Dead,
        HookHealthVerdict::Dead,
        HookHealthVerdict::Alive,
        HookHealthVerdict::Dead,
    ]
    .into();
    h.advance(4 * 3000);
    assert_eq!(
        balloon_texts(&h),
        vec![
            (4000, BalloonKind::Info, en("TipGuardRestored")),
            (4000, BalloonKind::Info, en("TipGuardRestored")),
        ],
        "Dead, (latched), Alive re-arms, Dead again"
    );
    assert_eq!(h.count("tray.show"), 3, "every Dead verdict updates the UI");
}

// JUDGE-C S1 (join 2026-10-13): the J07/J08 counters are read on the probe and logged once per change.
#[test]
fn s_app_hook_fault_counters_reach_the_log_once_per_change() {
    let mut h = Harness::started(Settings::default());
    let faults = |h: &Harness| -> Vec<String> {
        let w = h.w.borrow();
        w.logs.iter().filter(|l| l.starts_with("hook: ")).cloned().collect()
    };
    h.advance(3000);
    assert!(faults(&h).is_empty(), "nothing to say while both counters are 0");
    h.w.borrow_mut().faults = (2, 1);
    h.advance(3000);
    h.advance(3000);
    assert_eq!(
        faults(&h),
        vec!["hook: 2 contained panic(s), 1 failed cursor placement(s) let through, since the hook started"],
        "logged once, not every tick"
    );
    h.w.borrow_mut().faults = (2, 5);
    h.advance(3000);
    assert_eq!(faults(&h).len(), 2, "a new change is logged again");
    assert!(
        balloon_texts(&h).is_empty(),
        "no balloon: the moves already passed safely"
    );
}

#[test]
fn b010_dead_with_a_failed_reinstall_warns_and_drops_the_badge() {
    let s = Settings::default();
    let mut h = Harness::started(s.clone());
    {
        let mut w = h.w.borrow_mut();
        w.reinstall_ok = false;
        w.verdicts = [HookHealthVerdict::Dead].into();
    }
    h.clear_log();
    h.advance(3000);
    assert_eq!(
        balloon_texts(&h),
        vec![(8000, BalloonKind::Warning, en("TipGuardLost"))]
    );
    let v = h.last_view();
    assert_eq!(v.icon, TrayIcon::Paused);
    assert_eq!(v.tooltip, enf("TrayGuardLost", &[&hk(&s.hotkey())]));
}

// ---------------------------------------------------------------- B011 display debounce

#[test]
fn b011_a_burst_of_display_changes_configures_once_after_750_ms() {
    let mut h = Harness::started(Settings::default());
    h.clear_log();
    assert_eq!(RECONFIGURE_DEBOUNCE_MS, 750);
    h.ev(Event::DisplayChanged);
    h.advance(200);
    h.ev(Event::DisplayChanged);
    h.advance(500);
    h.ev(Event::DisplayChanged);
    h.advance(749);
    assert_eq!(h.count("core.plan"), 0, "still inside the burst window");
    h.advance(1);
    assert_eq!(h.count("core.plan"), 1, "one Configure per burst");
    let log = h.log();
    let plan = pos(&log, "core.plan");
    assert!(
        log[plan..].contains(&"tray.show".to_string()),
        "UpdateUi after Configure"
    );
    assert!(!h.w.borrow().timers.contains_key(&TimerId::Reconfigure), "one-shot");
    h.advance(10_000);
    assert_eq!(h.count("core.plan"), 1);
}

// ---------------------------------------------------------------- B012 Configure balloons

fn started_with_reports(settings: Settings, reports: Vec<PlanReport>) -> Harness {
    let mut h = Harness::new(World {
        reports: reports.into(),
        load: Some(Loaded {
            settings,
            source: LoadSource::V2,
        }),
        ..World::default()
    });
    h.ctl.start();
    h
}

fn reconfigure(h: &mut Harness) {
    h.ev(Event::DisplayChanged);
    h.advance(750);
}

#[test]
fn b012_demotion_and_pruned_exclusion_balloons() {
    let h = started_with_reports(
        Settings::default(),
        vec![PlanReport {
            has_safety_demotions: true,
            pruned: true,
            ..PlanReport::default()
        }],
    );
    let five: Vec<_> = balloon_texts(&h).into_iter().filter(|(ms, _, _)| *ms == 5000).collect();
    assert_eq!(
        five,
        vec![
            (5000, BalloonKind::Info, en("GeometrySealDemoted")),
            (5000, BalloonKind::Warning, en("ExcludePruned")),
        ]
    );
}

#[test]
fn b012_isolation_warns_once_per_distinct_set_and_rearms_when_healed() {
    let s = Settings {
        monitor_names: vec![("SYN-B".into(), "Desk".into())],
        ..Settings::default()
    };
    let iso = |ids: &[&str], idx: &[u32]| PlanReport {
        isolated: ids.iter().map(|s| s.to_string()).collect(),
        isolated_indices: idx.to_vec(),
        ..PlanReport::default()
    };
    let mut h = started_with_reports(
        s.clone(),
        vec![
            iso(&["SYN-B", "SYN-C"], &[2, 3]),
            iso(&["SYN-C", "SYN-B"], &[3, 2]), // same set, other order: no balloon
            iso(&[], &[]),                     // healed: re-arm
            iso(&["SYN-B", "SYN-C"], &[2, 3]), // back: warn again
        ],
    );
    let warns = |h: &Harness| -> Vec<String> {
        balloon_texts(h)
            .into_iter()
            .filter(|(ms, k, _)| *ms == 5000 && *k == BalloonKind::Warning)
            .map(|(_, _, t)| t)
            .collect()
    };
    let expected = enf("TipIsolated", &["2 — Desk, 3", &hk(&s.pause_hotkey())]);
    assert_eq!(warns(&h), vec![expected.clone()]);
    reconfigure(&mut h);
    assert_eq!(warns(&h).len(), 1, "same set: latched");
    reconfigure(&mut h);
    assert_eq!(warns(&h).len(), 1, "healed: no balloon");
    reconfigure(&mut h);
    assert_eq!(warns(&h), vec![expected.clone(), expected], "re-armed after healing");
}

#[test]
fn b012_isolated_label_falls_back_to_the_live_monitor_index() {
    let mut w = World::default();
    w.monitors
        .push(synthetic_monitor(2, "SYN-B", Rect::new(1920, -1080, 3840, 0), false));
    w.reports = [PlanReport {
        isolated: vec!["SYN-B".into()],
        isolated_indices: Vec::new(),
        ..PlanReport::default()
    }]
    .into();
    let mut h = Harness::new(w);
    h.ctl.start();
    let s = Settings::default();
    assert!(
        balloon_texts(&h)
            .iter()
            .any(|(_, _, t)| *t == enf("TipIsolated", &["2", &hk(&s.pause_hotkey())]))
    );
}

// ---------------------------------------------------------------- B013 settings saved

fn saved_settings() -> Settings {
    Settings {
        language: "tr".into(),
        auto_start: true,
        hot_key: 0x87, // F24
        ..Settings::default()
    }
}

#[test]
fn b013_saved_settings_are_applied_in_the_1x_order() {
    let mut h = Harness::started(Settings::default());
    h.w.borrow_mut().reload = [Ok(saved_settings())].into();
    h.ev(Event::Menu(MenuCommand::Settings));
    h.clear_log();
    h.ev(Event::SettingsExited(0));
    let log = h.log();
    let order = [
        "store.reload",
        "hotkeys.unregister(Gate)",
        "hotkeys.unregister(Confine)",
        "hotkeys.unregister(Pause)",
        "hotkeys.unregister(Side)",
        "autostart.apply(true)",
        "hotkeys.register(Gate)",
        "system.monitors",
        "core.plan",
        "guard.configure",
        "tray.show",
        "system.intel",
    ];
    let at: Vec<usize> = order.iter().map(|l| pos(&log, l)).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "order {order:?} in {log:#?}");
    assert_eq!(h.ctl.settings().hot_key, 0x87, "the new settings are adopted");
    assert_eq!(h.ctl.lang(), Lang::Tr, "language applied");
    let v = h.last_view();
    assert_eq!(
        v.settings.text,
        fence_i18n::get(Lang::Tr, "MenuSettings"),
        "menu text rebuilt"
    );
    assert_eq!(
        h.w.borrow().registered.get(&HotkeySlot::Gate).map(|k| k.vk),
        Some(0x87),
        "re-registered with the new chord"
    );
    let store_calls: Vec<&String> = log.iter().filter(|l| l.starts_with("store.")).collect();
    assert_eq!(
        store_calls,
        vec!["store.reload"],
        "the settings process saved the file; the tray only reloads it"
    );
}

#[test]
fn b013_cancel_changes_nothing() {
    let mut h = Harness::started(Settings::default());
    h.ev(Event::Menu(MenuCommand::Settings));
    h.clear_log();
    h.ev(Event::SettingsExited(1));
    assert!(h.log().is_empty(), "{:?}", h.log());
    assert_eq!(h.ctl.settings(), &Settings::default());
}

// ---------------------------------------------------------------- B014 Intel warning

#[test]
fn b014_no_intel_adapter_no_warning() {
    let mut h = Harness::new(World {
        rotation: vec![Settings::default().hotkey()],
        ..World::default()
    });
    h.ctl.start();
    assert!(h.balloons().iter().all(|b| b.millis != 8000));
}

#[test]
fn b014_intel_defaults_applied_shows_the_info_with_gate_and_side_keys() {
    let s = Settings {
        intel_defaults_applied: true,
        ..Settings::default()
    };
    let mut h = Harness::new(World {
        intel: true,
        load: Some(Loaded {
            settings: s.clone(),
            source: LoadSource::Fresh,
        }),
        ..World::default()
    });
    h.ctl.start();
    assert_eq!(
        balloon_texts(&h).last().cloned(),
        Some((
            8000,
            BalloonKind::Info,
            enf("TipIntelDefaults", &[&hk(&s.hotkey()), &hk(&s.side_hotkey())])
        ))
    );
}

#[test]
fn b014_rotation_clashes_are_named_gate_side_confine_pause() {
    let s = Settings::default();
    let all = vec![s.hotkey(), s.side_hotkey(), s.confine_hotkey(), s.pause_hotkey()];
    let mut h = Harness::new(World {
        intel: true,
        rotation: all,
        load: Some(Loaded {
            settings: s.clone(),
            source: LoadSource::V2,
        }),
        ..World::default()
    });
    h.ctl.start();
    let names = [
        hk(&s.hotkey()),
        hk(&s.side_hotkey()),
        hk(&s.confine_hotkey()),
        hk(&s.pause_hotkey()),
    ]
    .join(", ");
    assert_eq!(
        balloon_texts(&h).last().cloned(),
        Some((8000, BalloonKind::Warning, enf("TipIntelRotation", &[&names])))
    );

    // Only the side chord clashes: only it is named.
    let mut h = Harness::new(World {
        intel: true,
        rotation: vec![s.side_hotkey()],
        ..World::default()
    });
    h.ctl.start();
    assert_eq!(
        balloon_texts(&h).last().cloned(),
        Some((
            8000,
            BalloonKind::Warning,
            enf("TipIntelRotation", &[&hk(&s.side_hotkey())])
        ))
    );

    // Intel, no clash: nothing.
    let mut h = Harness::new(World {
        intel: true,
        ..World::default()
    });
    h.ctl.start();
    assert!(h.balloons().is_empty());
}

#[test]
fn b014_the_production_core_rule_is_wired() {
    // fence-engine's IsArrowRotationHotkey is Track B's; the controller only calls it through CorePort.
    use fence_app::{CorePort, EngineCore};
    let h = Hotkey {
        vk: 0x87,
        shift: true,
        ..Hotkey::default()
    };
    assert!(!EngineCore.is_arrow_rotation_hotkey(&h), "F24 is never an arrow");
}

// ---------------------------------------------------------------- B015 exit

#[test]
fn b015_exit_tears_down_in_the_1x_order_and_ignores_later_events() {
    let mut h = Harness::started(Settings::default());
    h.ev(Event::DisplayChanged);
    h.clear_log();
    assert_eq!(h.ev(Event::Menu(MenuCommand::Exit)), Flow::Exit);
    assert_eq!(
        h.log(),
        vec![
            "timers.stop(Reconfigure)",
            "timers.stop(HookProbe)",
            "tray.remove_icon",
            "guard.stop",
            "hotkeys.unregister(Gate)",
            "hotkeys.unregister(Confine)",
            "hotkeys.unregister(Pause)",
            "hotkeys.unregister(Side)",
        ]
    );
    {
        let w = h.w.borrow();
        assert!(w.timers.is_empty() && w.registered.is_empty() && w.icon_added.is_none());
        assert!(!w.enabled && !w.installed, "unhooked");
    }
    h.clear_log();
    assert_eq!(h.ev(Event::DisplayChanged), Flow::Exit, "display events unsubscribed");
    assert_eq!(h.ev(Event::Hotkey(HotkeySlot::Gate)), Flow::Exit);
    assert!(h.log().is_empty());
}

// ---------------------------------------------------------------- B029 settings process

#[test]
fn b029_settings_open_one_process_with_the_live_state() {
    let mut h = Harness::started(Settings::default());
    h.ev(Event::Menu(MenuCommand::Gate));
    h.clear_log();
    h.ev(Event::Menu(MenuCommand::Settings));
    let expected = RuntimeState {
        gate_open: true,
        side_contain: true,
        confine: false,
        paused: false,
        hook_installed: true,
    };
    assert_eq!(h.log(), vec!["store.write_state", "settings.launch"]);
    assert_eq!(h.w.borrow().launches, vec![expected]);
    assert_eq!(h.w.borrow().states, vec![expected]);
    assert!(h.ctl.settings_process_open());
    assert_eq!(
        fence_types::launch::settings_args(&expected),
        vec!["--settings", "--state", "gate=1,side=1,confine=0,paused=0,hook=1"]
    );

    // At most one at a time.
    h.ev(Event::Menu(MenuCommand::Settings));
    assert_eq!(h.w.borrow().launches.len(), 1);

    // While it is open, state.txt follows every change.
    h.ev(Event::Hotkey(HotkeySlot::Pause));
    let last = *h.w.borrow().states.last().unwrap();
    assert!(last.paused && !last.hook_installed);
}

#[test]
fn b029_exit_codes_cancel_fail_and_reload() {
    let mut h = Harness::started(Settings::default());
    // 1 = cancelled: nothing changes, the window may open again.
    h.ev(Event::Menu(MenuCommand::Settings));
    h.ev(Event::SettingsExited(1));
    assert!(!h.ctl.settings_process_open());
    // 2 = failed: logged, nothing changes.
    h.ev(Event::Menu(MenuCommand::Settings));
    h.clear_log();
    h.ev(Event::SettingsExited(2));
    assert!(h.log().is_empty());
    assert!(h.w.borrow().logs.iter().any(|l| l.contains("exit code 2")));
    // A crash code is a failure too.
    h.ev(Event::Menu(MenuCommand::Settings));
    h.ev(Event::SettingsExited(-1073741819));
    assert!(h.w.borrow().logs.iter().any(|l| l.contains("-1073741819")));
    // 0 with an unreadable file: logged, the live settings stay.
    h.ev(Event::Menu(MenuCommand::Settings));
    h.w.borrow_mut().reload = [Err("truncated".to_string())].into();
    h.clear_log();
    h.ev(Event::SettingsExited(0));
    assert_eq!(h.log(), vec!["store.reload"]);
    assert!(h.w.borrow().logs.iter().any(|l| l.contains("truncated")));
    assert_eq!(h.ctl.settings(), &Settings::default());
    // 0 with a good file: applied as B013.
    h.ev(Event::Menu(MenuCommand::Settings));
    h.w.borrow_mut().reload = [Ok(saved_settings())].into();
    h.ev(Event::SettingsExited(0));
    assert_eq!(h.ctl.settings().hot_key, 0x87);
    // Closed: toggles stop rewriting state.txt.
    let n = h.w.borrow().states.len();
    h.ev(Event::Menu(MenuCommand::Gate));
    assert_eq!(h.w.borrow().states.len(), n);
}

#[test]
fn b029_a_launch_failure_is_logged_and_retryable() {
    let mut h = Harness::started(Settings::default());
    h.w.borrow_mut().launch_ok = false;
    h.ev(Event::Menu(MenuCommand::Settings));
    assert!(!h.ctl.settings_process_open());
    assert!(h.w.borrow().logs.iter().any(|l| l.contains("could not start")));
    h.w.borrow_mut().launch_ok = true;
    h.ev(Event::Menu(MenuCommand::Settings));
    assert!(h.ctl.settings_process_open());
}

#[test]
fn s_app_initial_view_matches_a_fresh_notify_icon() {
    let v = TrayView::initial();
    assert_eq!(v.icon, TrayIcon::Closed);
    assert_eq!(v.tooltip, "MouseFence");
    assert!(v.gate.enabled && !v.gate.checked && v.gate.text.is_empty());
}
