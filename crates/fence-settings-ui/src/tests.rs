//! Headless parity tests of the settings window (rows P002-P028, P167, P224, B038 of docs/rust2/parity.tsv).
//! Each test builds the real window state, drives `egui::Context` frames with synthetic pointer and key events
//! (no window, no GPU, no real cursor), and checks what the frame drew (the recorder), what the editor holds and,
//! for P003/P224, the pixels the CPU rasterizer paints from egui's own meshes.

use std::rc::Rc;

use eframe::egui::{self, Color32, Event, Key, Modifiers, Pos2, Vec2, vec2};
use fence_config::{Settings, Stored, StoredRule};
use fence_i18n::Lang;
use fence_types::{BoundaryDefault, Control, Direction, MonitorInfo, RuleBehavior, RuntimeState};

use crate::backend::Backend;
use crate::editor::{ButtonRole, Dialogs, Editor, Outcome};
use crate::headless::Headless;
use crate::testkit::{FakeBackend, FakeDialogs, TempFile, fake, mon, names, rule};
use crate::text::Tr;
use crate::theme::Theme;
use crate::ui::widgets::{Rec, regular};
use crate::ui::{Action, Services, SettingsUi, SystemTheme};

#[test]
fn j15_side_top_boundary_shows_automatic_blocked_in_both_languages() {
    let monitors = vec![
        mon("SYN-TOP", 1, false, -421, -1440, 3019, 0),
        mon("SYN-LEFT", 2, false, -1920, 0, 0, 1080),
        mon("SYN-MAIN", 3, true, 0, 0, 2560, 1440),
        mon("SYN-RIGHT", 4, false, 2560, 0, 4480, 1080),
    ];
    for lang in [Lang::En, Lang::Tr] {
        let mut h = H::with_backend(
            &Settings::default(),
            monitors.clone(),
            lang,
            None,
            Rc::new(crate::backend::RealBackend),
        );
        let index = h
            .e()
            .boundaries
            .iter()
            .position(|b| {
                [b.first.stable_id.as_str(), b.second.stable_id.as_str()].contains(&"SYN-TOP")
                    && [b.first.stable_id.as_str(), b.second.stable_id.as_str()].contains(&"SYN-LEFT")
            })
            .unwrap();
        h.e_mut().open_boundary(index);
        h.run(3);
        assert_eq!(h.rec("drawer.overall").text, Tr(lang).t("AutomaticBlocked"));
        assert!(h.e().drawer.tiles_selected[2]);
    }
}

/// Test services: injected dialogs, recorded identify calls, a fixed theme.
#[derive(Default)]
pub struct FakeServices {
    pub held_win: bool,
    pub dialogs: FakeDialogs,
    pub identified: Vec<Vec<MonitorInfo>>,
    pub system: SystemTheme,
}

impl Services for FakeServices {
    fn windows_key_down(&self) -> bool {
        self.held_win
    }
    fn dialogs(&mut self) -> &mut dyn Dialogs {
        &mut self.dialogs
    }
    fn identify(&mut self, monitors: &[MonitorInfo], _names: &[(String, String)], _theme: &Theme) {
        self.identified.push(monitors.to_vec());
    }
    fn system_theme(&self) -> SystemTheme {
        self.system
    }
}

/// The window with fakes around it.
pub struct H {
    pub h: Headless,
    pub s: FakeServices,
}

impl H {
    pub fn with_backend(
        settings: &Settings,
        monitors: Vec<MonitorInfo>,
        lang: Lang,
        live: Option<RuntimeState>,
        backend: Rc<dyn Backend>,
    ) -> Self {
        let editor = Editor::new(settings, monitors, backend, Tr(lang), live);
        let ui = SettingsUi::new(editor, SystemTheme::default());
        let mut h = Self {
            h: Headless::new(ui, vec2(1064.0, 712.0), 1.0, false),
            s: FakeServices::default(),
        };
        h.run(3);
        h
    }

    pub fn new(settings: &Settings, monitors: Vec<MonitorInfo>, lang: Lang) -> Self {
        Self::with_backend(settings, monitors, lang, None, fake())
    }

    pub fn sized(mut self, w: f32, h: f32, ppp: f32) -> Self {
        self.h.size = vec2(w, h);
        self.h.ppp = ppp;
        self.run(4);
        self
    }

    pub fn e(&self) -> &Editor {
        &self.h.ui.editor
    }

    pub fn e_mut(&mut self) -> &mut Editor {
        &mut self.h.ui.editor
    }

    pub fn run(&mut self, n: usize) {
        self.h.settle(&mut self.s, n);
    }

    pub fn rec(&self, tag: &str) -> Rec {
        self.h
            .ui
            .rec
            .get(tag)
            .cloned()
            .unwrap_or_else(|| panic!("nothing recorded as {tag}"))
    }

    pub fn has(&self, tag: &str) -> bool {
        self.h.ui.rec.get(tag).is_some()
    }

    pub fn click(&mut self, tag: &str) {
        let p = self.rec(tag).rect.center();
        self.h.click_at(&mut self.s, p);
        self.run(2);
    }

    pub fn click_at(&mut self, p: Pos2) {
        self.h.click_at(&mut self.s, p);
        self.run(2);
    }

    pub fn key(&mut self, key: Key, m: Modifiers) {
        self.h.key(&mut self.s, key, m);
        self.run(1);
    }

    pub fn tab(&mut self, i: usize) {
        self.click(&format!("nav.{i}"));
        assert_eq!(self.e().tab, i, "sidebar click selects page {i}");
    }

    pub fn open_first_boundary(&mut self, hide_list: bool) {
        self.h.ui.open_first_boundary_for_preview(hide_list);
        self.run(4);
    }

    pub fn act(&mut self, a: Action) {
        self.h.ui.apply(a, &mut self.s);
        self.run(2);
    }

    /// Single-line width of a text in a font, as the window measures it.
    pub fn text_width(&self, text: &str, font: egui::FontId) -> f32 {
        self.h
            .ctx
            .fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, Color32::WHITE).size().x)
    }

    pub fn wrapped_height(&self, text: &str, font: egui::FontId, width: f32, italics: bool) -> f32 {
        let mut job = egui::text::LayoutJob::simple(text.to_string(), font, Color32::WHITE, width);
        if italics {
            for s in &mut job.sections {
                s.format.italics = true;
            }
        }
        self.h.ctx.fonts_mut(|f| f.layout_job(job).size().y)
    }
}

fn en() -> Tr {
    Tr(Lang::En)
}

fn tr_of(lang: Lang) -> Tr {
    Tr(lang)
}

fn two_side_by_side() -> Vec<MonitorInfo> {
    vec![
        mon("left", 1, true, 0, 0, 100, 100),
        mon("right", 2, false, 100, 0, 200, 100),
    ]
}

fn five_screens() -> Vec<MonitorInfo> {
    vec![
        mon("top", 1, false, 0, -900, 1600, 0),
        mon("main", 2, true, 0, 0, 1920, 1080),
        mon("right", 3, false, 1920, 0, 3200, 1024),
        mon("left", 4, false, -1280, 0, 0, 1024),
        mon("far-top", 5, false, 1600, -768, 2624, 0),
    ]
}

fn tiles(h: &H) -> Vec<Rec> {
    ["drawer.tile.free", "drawer.tile.push", "drawer.tile.blocked"]
        .iter()
        .map(|t| h.rec(t))
        .collect()
}

fn selected(r: &Rec) -> bool {
    r.state.starts_with("selected")
}

fn enabled(r: &Rec) -> bool {
    !r.state.contains("disabled")
}

fn near(a: [u8; 4], b: Color32, tol: i32) -> bool {
    (a[0] as i32 - b.r() as i32).abs() <= tol
        && (a[1] as i32 - b.g() as i32).abs() <= tol
        && (a[2] as i32 - b.b() as i32).abs() <= tol
}

// ------------------------------------------------------------------------------------------------- P002

// Parity row P002 (tests/Tests.cs:149).
#[test]
fn p002_settings_transaction_draft_is_a_deep_copy_and_cancel_cannot_mutate_live_settings() {
    let live = Settings {
        mode: "Manual".into(),
        manual_monitors: vec!["top".into()],
        monitor_names: names(&[("top", "Studio")]),
        boundary_rules: vec![rule(
            "main",
            "top",
            Direction::Up,
            RuleBehavior::Push,
            Control::Automatic,
        )],
        ..Settings::default()
    };
    let monitors = vec![
        mon("top", 1, false, 0, -100, 100, 0),
        mon("main", 2, true, 0, 0, 100, 100),
    ];
    let snapshot = live.clone();
    let mut h = H::new(&live, monitors, Lang::En);
    {
        let d = &mut h.e_mut().draft;
        d.manual_monitors.clear();
        d.monitor_names = names(&[("top", "Changed")]);
        d.boundary_rules[0].behavior = Stored::Known(RuleBehavior::Blocked);
    }
    // Edits through the window too: rename a screen in its row and choose a tile.
    h.act(Action::SetName("main".into(), "Desk".into()));
    h.open_first_boundary(false);
    h.click("drawer.tile.free");
    h.click("footer.cancel");
    assert_eq!(h.e().outcome, Outcome::Cancelled);
    assert_eq!(live, snapshot, "the caller's settings are untouched");
    assert_eq!(live.manual_monitors, ["top"]);
    assert_eq!(live.boundary_rules[0].behavior, Stored::Known(RuleBehavior::Push));
    assert!(h.e().draft.manual_monitors.is_empty());
}

// ------------------------------------------------------------------------------------------------- P003

// Parity row P003 (tests/Tests.cs:176).
#[test]
fn p003_footer_dark_surface_status_action_cancel_and_save_are_themed() {
    let s = Settings {
        language: "en".into(),
        theme: "Dark".into(),
        ..Settings::default()
    };
    let mut h = H::new(&s, vec![], Lang::En);
    assert_eq!(h.h.ui.theme, Theme::DARK);
    let footer = h.rec("footer").rect;
    let status = h.rec("footer.status");
    let cancel = h.rec("footer.cancel");
    let save = h.rec("footer.save");
    assert!(
        !h.has("footer.action"),
        "the validation action is hidden until a seal error"
    );
    assert!(cancel.state.contains("Secondary") && save.state.contains("Primary"));
    let img = h.h.render().clone_pixels();
    let px = |p: Pos2| img.get(p.x as usize, p.y as usize);
    // Footer and status background: a spot on the status line right of its text.
    let blank = Pos2::new(status.rect.right() - 4.0, footer.top() + 6.0);
    assert!(
        near(px(blank), Theme::DARK.surface, 2),
        "footer surface {:?}",
        px(blank)
    );
    let left = Pos2::new(footer.left() + 6.0, footer.bottom() - 6.0);
    assert!(
        near(px(left), Theme::DARK.surface, 2),
        "status column surface {:?}",
        px(left)
    );
    let fill = |r: egui::Rect| px(Pos2::new(r.left() + 5.0, r.center().y));
    assert!(
        near(fill(cancel.rect), Theme::DARK.surface2, 2),
        "cancel {:?}",
        fill(cancel.rect)
    );
    assert!(
        near(fill(save.rect), Theme::DARK.accent, 2),
        "save {:?}",
        fill(save.rect)
    );
    // A seal error shows the third button, themed the same way.
    let mut sealed = H::new(
        &Settings {
            theme: "Dark".into(),
            boundary_rules: vec![rule(
                "right",
                "left",
                Direction::Left,
                RuleBehavior::Blocked,
                Control::Always,
            )],
            ..Settings::default()
        },
        two_side_by_side(),
        Lang::En,
    );
    sealed.click("footer.save");
    assert_eq!(sealed.e().outcome, Outcome::Open);
    let action = sealed.rec("footer.action");
    assert_eq!(action.text, en().t("OpenBoundary"));
    let img = sealed.h.render().clone_pixels();
    let p = Pos2::new(action.rect.left() + 5.0, action.rect.center().y);
    let c = img.get(p.x as usize, p.y as usize);
    assert!(
        near(c, Theme::DARK.surface2, 2) || near(c, Theme::DARK.surface, 2),
        "quiet action fill {c:?}"
    );
    assert_eq!(sealed.rec("footer.status").state, "error");
}

/// A copy of the painted pixels (the raster keeps its textures for the next render).
trait ClonePixels {
    fn clone_pixels(&self) -> Pixels;
}

pub struct Pixels {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 4]>,
}

impl Pixels {
    pub fn get(&self, x: usize, y: usize) -> [u8; 4] {
        self.px[y.min(self.h - 1) * self.w + x.min(self.w - 1)]
    }
}

impl ClonePixels for crate::raster::Raster {
    fn clone_pixels(&self) -> Pixels {
        Pixels {
            w: self.width,
            h: self.height,
            px: self.pixels.clone(),
        }
    }
}

// ------------------------------------------------------------------------------------------------- P004/P005

fn v2_shell(lang: Lang, ppp: f32) {
    let tr = tr_of(lang);
    let s = Settings {
        language: if lang == Lang::Tr { "tr" } else { "en" }.into(),
        downward_boundary_default: Stored::Known(BoundaryDefault::Blocked),
        ..Settings::default()
    };
    let mut h = H::new(&s, five_screens(), lang).sized(1064.0, 712.0, ppp);
    let nav: Vec<String> = (0..4).map(|i| h.rec(&format!("nav.{i}")).text).collect();
    assert_eq!(
        nav,
        [
            tr.t("NavDisplays"),
            tr.t("NavAdvanced"),
            tr.t("NavShortcuts"),
            tr.t("NavGeneral")
        ],
        "four pages"
    );
    h.tab(1);
    assert!(h.rec("adv.toggle").text.starts_with("›  "), "initial chevron");
    assert!(!h.has("adv.grid"));
    h.click("adv.toggle");
    assert!(
        h.e().advanced_expanded && h.has("adv.grid") && h.rec("adv.toggle").text.starts_with("⌄  "),
        "revealed"
    );
    // Contained: nothing on the Advanced page reaches past the content measure; resets stay inside their rows.
    let body = h.rec("body");
    let page = h.rec("page.1").rect;
    let content_right = page.left() + (body.rect.width() - parse_padding(&body.state).1 as f32);
    for r in h.h.ui.rec.all("adv.") {
        assert!(
            r.rect.right() <= content_right + 1.0,
            "{} right {} > {}",
            r.tag,
            r.rect.right(),
            content_right
        );
    }
    for i in 0..7 {
        let row = h.rec(&format!("adv.numeric.{i}")).rect;
        let reset = h.rec(&format!("adv.numeric.{i}.reset")).rect;
        assert!(reset.right() <= row.right() + 1.0, "reset {i} inside its row");
    }
    // Truthful return line and a full-width drawer with static disclosures. The fake backend answers as 1.1.1
    // ("never crosses"); the real 2.0 engine (MF_UI_REAL_BACKEND=1) ships J05, which demotes this sealed top
    // pair to a firm push, as s_join_j05_return_line_of_the_sealed_top_pair_is_a_firm_push_in_2_0 pins.
    h.open_first_boundary(false);
    assert_eq!(h.e().tab, 0);
    let truthful = if crate::testkit::real_backend_on() {
        "ComingDownPush"
    } else {
        "ComingDownBlocked"
    };
    assert_eq!(h.rec("drawer.return").text, tr.t(truthful), "truthful return");
    let drawer = h.rec("drawer").rect;
    let when = h.rec("drawer.when");
    let separate = h.rec("drawer.separate");
    assert!(
        when.rect.top() >= drawer.top() && when.rect.bottom() <= drawer.bottom(),
        "when link inside the drawer"
    );
    assert!(!enabled(&separate), "a vertical boundary has no separate directions");
    let (left, right) = parse_padding(&h.rec("body").state);
    let body_w = h.rec("body").rect.width();
    assert!(
        drawer.width() >= body_w - (left + right) as f32 - 17.0 - 8.0,
        "drawer {} body {body_w}",
        drawer.width()
    );
    // Collapse again.
    h.tab(1);
    h.click("adv.toggle");
    assert!(
        !h.has("adv.grid") && h.rec("adv.toggle").text.starts_with("›  "),
        "collapsed"
    );
}

fn parse_padding(state: &str) -> (i32, i32) {
    let mut it = state
        .split_whitespace()
        .skip(1)
        .map(|n| n.parse::<i32>().expect("number"));
    (it.next().expect("left"), it.next().expect("right"))
}

// Parity row P004 (tests/Tests.cs:239), at 96 and 144 DPI.
#[test]
fn p004_v2_shell_en_four_pages_physical_pixel_fold_full_width_editor_static_disclosures() {
    v2_shell(Lang::En, 1.0);
    v2_shell(Lang::En, 1.5);
}

// Parity row P005 (tests/Tests.cs:239), at 96 and 144 DPI.
#[test]
fn p005_v2_shell_tr_four_pages_physical_pixel_fold_full_width_editor_static_disclosures() {
    v2_shell(Lang::Tr, 1.0);
    v2_shell(Lang::Tr, 1.5);
}

// ------------------------------------------------------------------------------------------------- P006-P009

fn overall_link_width(lang: Lang, active: bool) {
    let tr = tr_of(lang);
    let mut h = H::new(&Settings::default(), two_side_by_side(), lang);
    h.open_first_boundary(false);
    if active {
        h.click("drawer.tile.free");
    }
    let link = h.rec("drawer.overall");
    let measured = h.text_width(&link.text, regular(9.0)) + 4.0;
    let want = tr.t(if active {
        "UseOverallBehavior"
    } else {
        "UsingOverallBehavior"
    });
    assert_eq!(link.text, want);
    assert!(
        link.rect.width() + 0.01 >= measured,
        "width {} < measured {measured}",
        link.rect.width()
    );
    assert_eq!(enabled(&link), active);
}

// Parity row P006 (tests/Tests.cs:270).
#[test]
fn p006_inherited_overall_link_gets_its_full_measured_width_en() {
    overall_link_width(Lang::En, false);
}

// Parity row P007 (tests/Tests.cs:277).
#[test]
fn p007_active_overall_link_gets_its_full_measured_width_en() {
    overall_link_width(Lang::En, true);
}

// Parity row P008 (tests/Tests.cs:270).
#[test]
fn p008_inherited_overall_link_gets_its_full_measured_width_tr() {
    overall_link_width(Lang::Tr, false);
}

// Parity row P009 (tests/Tests.cs:277).
#[test]
fn p009_active_overall_link_gets_its_full_measured_width_tr() {
    overall_link_width(Lang::Tr, true);
}

// ------------------------------------------------------------------------------------------------- P010-P012

// Parity row P010 (tests/Tests.cs:296).
#[test]
fn p010_confirmed_valid_restore_produces_the_result_without_mutating_live_settings() {
    let file = TempFile::new("valid-restore");
    let mut source = Editor::new(
        &Settings {
            up_cross_min: 20,
            ..Settings::default()
        },
        vec![],
        fake(),
        en(),
        None,
    );
    assert!(source.backup_to_file(&file.0));
    let live = Settings {
        up_cross_min: 3,
        ..Settings::default()
    };
    let mut h = H::new(&live, vec![], Lang::En);
    h.s.dialogs.restore_path = Some(file.0.clone());
    h.s.dialogs.confirm = true;
    h.tab(3);
    h.click("gen.import");
    assert_eq!(*h.s.dialogs.confirmations.borrow(), 1);
    match &h.e().outcome {
        Outcome::Saved(r) => assert_eq!(r.up_cross_min, 20),
        o => panic!("expected a result, got {o:?}"),
    }
    assert_eq!(live.up_cross_min, 3);
}

// Parity rows P011 and P012 (tests/Tests.cs:314, 322).
#[test]
fn p011_repairable_invalid_import_remains_a_draft_and_leaves_live_settings_untouched() {
    let (h, live) = repairable_import();
    assert_eq!(*h.s.dialogs.confirmations.borrow(), 0);
    assert_eq!(h.e().outcome, Outcome::Open);
    assert!(h.e().draft.has_repairable_import_errors);
    assert_eq!(h.e().draft.up_cross_min, 60);
    assert_eq!(live.up_cross_min, 3);
    assert_eq!(h.rec("footer.status").text, en().t("ImportInvalid"));
}

fn repairable_import() -> (H, Settings) {
    let file = TempFile::new("repairable-restore");
    std::fs::write(
        &file.0,
        r#"{"SettingsVersion":4,"UpCrossMin":60,"UpwardBoundaryDefault":"Surprise"}"#,
    )
    .expect("temp");
    let live = Settings {
        up_cross_min: 3,
        upward_boundary_default: Stored::Known(BoundaryDefault::Automatic),
        ..Settings::default()
    };
    let mut h = H::new(&live, vec![], Lang::En);
    h.s.dialogs.restore_path = Some(file.0.clone());
    h.s.dialogs.confirm = true;
    h.tab(3);
    h.click("gen.import");
    (h, live)
}

#[test]
fn p012_repairing_the_imported_draft_permits_a_valid_result_while_live_stays_isolated() {
    let (mut h, live) = repairable_import();
    // Repair through the window: Advanced > Upward > Automatic.
    h.act(Action::SetDefault(0, BoundaryDefault::Automatic));
    h.click("footer.save");
    match &h.e().outcome {
        Outcome::Saved(r) => {
            assert_eq!(r.up_cross_min, 60);
            assert!(!r.has_repairable_import_errors);
        }
        o => panic!("expected a result, got {o:?}"),
    }
    assert_eq!(live.up_cross_min, 3);
}

// ------------------------------------------------------------------------------------------------- P013/P014

fn split_pair() -> Settings {
    Settings {
        boundary_rules: vec![
            rule(
                "left",
                "right",
                Direction::Right,
                RuleBehavior::Push,
                Control::Automatic,
            ),
            rule("right", "left", Direction::Left, RuleBehavior::Free, Control::Automatic),
        ],
        ..Settings::default()
    }
}

// Parity row P013 (tests/Tests.cs:354).
#[test]
fn p013_differing_pair_shows_different_by_direction_and_no_selected_primary_tile() {
    let mut h = H::new(&split_pair(), two_side_by_side(), Lang::En);
    h.open_first_boundary(false);
    assert_eq!(h.rec("drawer.caption").text, en().t("DifferentByDirection"));
    assert!(tiles(&h).iter().all(|t| !selected(t)));
}

// Parity row P014 (tests/Tests.cs:362).
#[test]
fn p014_selecting_a_primary_tile_reunifies_both_directions() {
    let mut h = H::new(&split_pair(), two_side_by_side(), Lang::En);
    h.open_first_boundary(false);
    h.click("drawer.tile.blocked");
    assert_eq!(h.rec("drawer.caption").text, en().t("BothDirections"));
    assert!(selected(&h.rec("drawer.tile.blocked")));
    let pair: Vec<&StoredRule> = h
        .e()
        .draft
        .boundary_rules
        .iter()
        .filter(|r| {
            (r.from_stable_id == "left" && r.to_stable_id == "right")
                || (r.from_stable_id == "right" && r.to_stable_id == "left")
        })
        .collect();
    assert_eq!(pair.len(), 2);
    assert!(pair.iter().all(|r| r.behavior == Stored::Known(RuleBehavior::Blocked)));
}

// ------------------------------------------------------------------------------------------------- P015/P016

fn live(side: bool) -> RuntimeState {
    RuntimeState {
        gate_open: true,
        side_contain: side,
        confine: false,
        paused: false,
        hook_installed: true,
    }
}

// Parity row P015 (tests/Tests.cs:384).
#[test]
fn p015_status_slot_is_empty_when_inherited_behavior_agrees_with_live_state() {
    let mut h = H::with_backend(
        &Settings::default(),
        two_side_by_side(),
        Lang::En,
        Some(live(true)),
        fake(),
    );
    h.open_first_boundary(false);
    assert_eq!(h.rec("drawer.overall").text, en().t("UsingOverallBehavior"));
    assert_eq!(h.rec("drawer.status").text, "");
}

// Parity row P016 (tests/Tests.cs:389).
#[test]
fn p016_status_slot_names_a_live_result_that_diverges_from_the_selected_tile() {
    let mut h = H::with_backend(
        &Settings::default(),
        two_side_by_side(),
        Lang::En,
        Some(live(true)),
        fake(),
    );
    h.open_first_boundary(false);
    h.e_mut().set_live(Some(live(false)));
    h.run(2);
    assert_eq!(h.rec("drawer.status").text, en().t("StatusSideOff"));
}

// ------------------------------------------------------------------------------------------------- P017-P019

fn excluded_target(lang: Lang) {
    let tr = tr_of(lang);
    let s = Settings {
        language: if lang == Lang::Tr { "tr" } else { "en" }.into(),
        excluded_devices: vec!["right".into()],
        monitor_names: names(&[("right", "Studio")]),
        ..Settings::default()
    };
    let mut h = H::new(&s, two_side_by_side(), lang);
    h.open_first_boundary(false);
    let expected = tr.f("ExcludedTargetExplanation", &["Studio"]);
    assert!(
        tiles(&h).iter().all(|t| !enabled(t) && !selected(t)),
        "no false selection"
    );
    assert_eq!(h.rec("drawer.caption").text, tr.t("BlockedByKeepOut"));
    assert_eq!(h.rec("drawer.status").text, expected);
    let allow = h.rec("drawer.allow");
    assert!(enabled(&allow) && allow.state.contains("Primary"), "{}", allow.state);
    assert_eq!(h.rec("drawer.allow.description").text, expected);
    // Clicking a disabled tile does nothing.
    h.click("drawer.tile.blocked");
    assert!(h.e().draft.boundary_rules.is_empty());
    h.click("drawer.allow");
    assert!(!h.e().is_excluded("right"));
    let t = tiles(&h);
    assert!(t.iter().all(enabled) && t.iter().filter(|x| selected(x)).count() == 1);
    let allow = h.rec("drawer.allow");
    assert!(!enabled(&allow) && allow.state.contains(&format!("{:?}", ButtonRole::Quiet)));
}

// Parity row P017 (tests/Tests.cs:430).
#[test]
fn p017_excluded_target_editor_en_no_false_selection_plain_explanation_primary_allow() {
    excluded_target(Lang::En);
}

// Parity row P018 (tests/Tests.cs:430).
#[test]
fn p018_excluded_target_editor_tr_no_false_selection_plain_explanation_primary_allow() {
    excluded_target(Lang::Tr);
}

// Parity row P019 (tests/Tests.cs:466).
#[test]
fn p019_excluded_lower_endpoint_blocks_a_vertical_boundary_and_can_be_allowed() {
    let s = Settings {
        excluded_devices: vec!["lower".into()],
        monitor_names: names(&[("lower", "Lower")]),
        ..Settings::default()
    };
    let ms = vec![
        mon("lower", 1, true, 0, 0, 100, 100),
        mon("upper", 2, false, 0, -100, 100, 0),
    ];
    let mut h = H::new(&s, ms, Lang::En);
    h.open_first_boundary(false);
    let expected = en().f("ExcludedTargetExplanation", &["Lower"]);
    assert!(tiles(&h).iter().all(|t| !enabled(t) && !selected(t)));
    assert_eq!(h.rec("drawer.status").text, expected);
    let allow = h.rec("drawer.allow");
    assert!(enabled(&allow) && allow.state.contains("Primary"));
    h.click("drawer.allow");
    assert!(!h.e().is_excluded("lower"));
    let t = tiles(&h);
    assert!(t.iter().all(enabled) && t.iter().filter(|x| selected(x)).count() == 1);
}

// ------------------------------------------------------------------------------------------------- P020

struct Composition {
    map: i32,
    body: i32,
    list_band: i32,
    drawer_band: i32,
    padding: (i32, i32),
    map_width: f32,
}

fn compose(w: f32, hgt: f32) -> Composition {
    let s = crate::screenshot::demo_settings(Some("Light"), "en", &FakeBackend);
    let mut h = H::new(&s, crate::screenshot::demo_monitors(), Lang::En).sized(w, hgt, 1.0);
    let list = h.h.ui.layout_metrics().expect("metrics");
    let map_width = h.rec("map").rect.width();
    let padding = parse_padding(&h.rec("body").state);
    h.open_first_boundary(true);
    let drawer = h.h.ui.layout_metrics().expect("metrics");
    assert_eq!((list.mode, drawer.mode), ("list", "drawer"));
    Composition {
        map: list.map,
        body: list.body,
        list_band: list.band,
        drawer_band: drawer.band,
        padding,
        map_width,
    }
}

// Parity row P020 (tests/Tests.cs:506).
#[test]
fn p020_large_window_composition_map_grows_and_footer_bands_stay_within_12_percent() {
    let compact = compose(1600.0, 1040.0);
    let maximized = compose(2560.0, 1352.0);
    for c in [&compact, &maximized] {
        let limit = c.body as f64 * 0.12 + 2.0;
        assert!(
            c.list_band as f64 <= limit && c.drawer_band as f64 <= limit,
            "bands {}/{} body {}",
            c.list_band,
            c.drawer_band,
            c.body
        );
        assert!(c.map_width <= 1241.0);
        assert!((c.padding.0 - c.padding.1).abs() <= 2);
    }
    assert!(maximized.map > compact.map, "map {} > {}", maximized.map, compact.map);
    // Body heights are exact; the map heights depend on the measured status reserve (font metrics), so they are
    // checked against the C# clamp: between round(.39 * body) and round(.59 * body). C# with Segoe UI gave 488/760.
    assert_eq!((compact.body, maximized.body), (976, 1288));
    for c in [&compact, &maximized] {
        let (lo, hi) = (
            (c.body as f64 * 0.39).round() as i32,
            (c.body as f64 * 0.59).round() as i32,
        );
        assert!((lo..=hi).contains(&c.map), "map {} outside {lo}..={hi}", c.map);
    }
}

// ------------------------------------------------------------------------------------------------- P021-P024

fn numeric_helpers(lang: Lang) {
    let tr = tr_of(lang);
    let s = Settings {
        language: if lang == Lang::Tr { "tr" } else { "en" }.into(),
        monitor_names: names(&[("left", "Twenty Four Character Name")]),
        ..Settings::default()
    };
    let mut h = H::new(&s, two_side_by_side(), lang);
    let (strength, holdback) = if lang == Lang::En {
        (
            "3 = default · 20 = firm · 60 = very firm.",
            "0 = exactly at the edge (default). Applies wherever the pointer is stopped — Never crosses and a failed firm push alike. These are physical pixels.",
        )
    } else {
        (
            "3 = varsayılan · 20 = sıkı · 60 = çok sıkı.",
            "0 = tam kenarda (varsayılan). İşaretçinin durdurulduğu her yerde uygulanır: hem Asla geçmez hem de başarısız sıkı itiş için. Bunlar fiziksel piksellerdir.",
        )
    };
    assert_eq!(tr.t("UpStrengthHelp"), strength);
    assert_eq!(tr.t("EdgeHoldbackHelpV2"), holdback);
    h.tab(1);
    h.click("adv.toggle");
    let texts: Vec<String> =
        h.h.ui
            .rec
            .all("adv.numeric.")
            .iter()
            .filter(|r| r.tag.ends_with(".help"))
            .map(|r| r.text.clone())
            .collect();
    assert!(texts.iter().any(|t| t == strength), "{texts:?}");
    assert!(texts.iter().any(|t| t == holdback));
}

fn status_reserve(lang: Lang) {
    let s = Settings {
        language: if lang == Lang::Tr { "tr" } else { "en" }.into(),
        monitor_names: names(&[("left", "Twenty Four Character Name")]),
        ..Settings::default()
    };
    let mut h = H::new(&s, two_side_by_side(), lang);
    h.open_first_boundary(false);
    let status = h.rec("drawer.status").rect;
    let width = status.width().max(220.0);
    for text in h.e().drawer_status_candidates() {
        let need = h.wrapped_height(&text, regular(8.5), width, true) + 4.0;
        assert!(
            status.height() + 0.01 >= need,
            "reserved {} < {need} for {text:?}",
            status.height()
        );
    }
}

// Parity row P021 (tests/Tests.cs:533).
#[test]
fn p021_numeric_helpers_en_frozen_strength_and_holdback_sentences_are_in_the_grid() {
    numeric_helpers(Lang::En);
}

// Parity row P022 (tests/Tests.cs:542).
#[test]
fn p022_status_reserve_en_fits_every_candidate() {
    status_reserve(Lang::En);
}

// Parity row P023 (tests/Tests.cs:533).
#[test]
fn p023_numeric_helpers_tr_frozen_strength_and_holdback_sentences_are_in_the_grid() {
    numeric_helpers(Lang::Tr);
}

// Parity row P024 (tests/Tests.cs:542).
#[test]
fn p024_status_reserve_tr_fits_every_candidate() {
    status_reserve(Lang::Tr);
}

// ------------------------------------------------------------------------------------------------- P025

// Parity row P025 (tests/Tests.cs:560).
#[test]
fn p025_duplicate_chord_flags_the_later_row_and_footer_names_both_actions() {
    let s = Settings {
        hot_key: 0x26,
        mod_ctrl: true,
        mod_alt: true,
        pause_hot_key: 0x26,
        pause_mod_ctrl: true,
        pause_mod_alt: true,
        ..Settings::default()
    };
    let mut h = H::new(&s, vec![], Lang::En);
    let expected = en().f(
        "DuplicateShortcutsNamed",
        &[&en().t("HotkeyHead"), &en().t("PauseHotkeyHead")],
    );
    h.tab(2);
    let errors: Vec<bool> = (0..4)
        .map(|i| h.rec(&format!("short.{i}.box")).state == "error")
        .collect();
    assert_eq!(errors, [false, false, true, false]);
    assert_eq!(h.e().inline.text, expected);
    assert_eq!(h.rec("footer.status").text, expected);
    // Capturing through the box: click it, press Ctrl+Alt+K; the conflict clears.
    h.click("short.2.box");
    h.key(Key::K, Modifiers::CTRL | Modifiers::ALT);
    assert_eq!(h.rec("short.2.box").text, "Ctrl + Alt + K");
    assert_eq!(h.rec("short.2.box").state, "");
    // Clear works on optional rows and is disabled on the required one.
    assert!(!enabled(&h.rec("short.0.clear")));
    h.click("short.3.clear");
    assert_eq!(h.rec("short.3.box").text, en().t("HotkeyPressKeys"));
}

// ------------------------------------------------------------------------------------------------- P026

#[test]
fn f03_ambiguous_display_identity_warns_and_disables_persisted_ownership() {
    for lang in [Lang::En, Lang::Tr] {
        let ms = vec![
            mon("shared#ambiguous:one", 1, true, 0, 0, 100, 100),
            mon("shared#ambiguous:two", 2, false, 100, 0, 200, 100),
        ];
        let mut h = H::new(&Settings::default(), ms, lang);
        assert_eq!(h.e().inline.text, tr_of(lang).t("AmbiguousDisplayIdentity"));
        assert!(!enabled(&h.rec("row.0.name")) && !enabled(&h.rec("row.1.name")));
        assert!(!enabled(&h.rec("row.1.keep")));
        h.e_mut().set_monitor_name("shared#ambiguous:one", "Wrong owner");
        h.e_mut().set_keep_out("shared#ambiguous:two", true);
        assert!(h.e().draft.monitor_names.is_empty() && h.e().draft.excluded_devices.is_empty());
        h.e_mut().draft.manual_monitors.push("shared#ambiguous:one".into());
        h.e_mut().save();
        assert_eq!(h.e().outcome, Outcome::Open);
        assert_eq!(h.e().inline.text, tr_of(lang).t("AmbiguousDisplayIdentity"));
    }
}

#[test]
fn f05_optional_incomplete_chords_stay_repairable_on_save_and_import() {
    for index in 1..4 {
        for (vk, ctrl) in [(0x87, false), (0, true)] {
            let mut h = H::new(&Settings::default(), vec![], Lang::En);
            h.e_mut().capture_hotkey(index, vk, ctrl, false, false);
            h.e_mut().save();
            assert_eq!(h.e().outcome, Outcome::Open);
            assert!(h.e().hotkey_errors[index]);
            assert!(!h.e().inline.text.is_empty());
            let file = TempFile::new("f05-import");
            std::fs::write(&file.0, fence_config::to_json(&h.e().draft)).unwrap();
            let mut imported = H::with_backend(
                &Settings::default(),
                vec![],
                Lang::En,
                None,
                Rc::new(crate::backend::RealBackend),
            );
            imported.s.dialogs.restore_path = Some(file.0.clone());
            imported.s.dialogs.confirm = true;
            let mut dialogs = FakeDialogs {
                restore_path: Some(file.0.clone()),
                confirm: true,
                ..Default::default()
            };
            imported.e_mut().import(&mut dialogs);
            assert_eq!(*dialogs.confirmations.borrow(), 0);
            assert_eq!(imported.e().outcome, Outcome::Open);
            assert_eq!(
                fence_config::shortcut_issue(&imported.e().draft),
                Some(fence_config::ShortcutIssue::Incomplete { index })
            );
        }
    }
    let mut h = H::new(&Settings::default(), vec![], Lang::En);
    for i in 1..4 {
        h.e_mut().clear_hotkey(i);
    }
    h.e_mut().save();
    assert!(matches!(h.e().outcome, Outcome::Saved(_)));
}

#[test]
fn f11_model_picker_shows_push_distances_only_for_stop_then_push() {
    for lang in [Lang::En, Lang::Tr] {
        let mut h = H::new(&Settings::default(), vec![], lang).sized(1200.0, 1040.0, 1.0);
        h.tab(1);
        assert!(h.has("combo.crossing-model"));
        assert_eq!(h.rec("combo.crossing-model").state, "1");
        assert!(!h.has("adv.numeric.7") && !h.has("adv.numeric.8"));
        h.h.ui.apply(
            Action::SetCrossingModel(fence_types::CrossingModel::StopThenPush),
            &mut h.s,
        );
        h.run(3);
        assert_eq!(h.rec("combo.crossing-model").state, "2");
        assert!(h.has("adv.numeric.7") && h.has("adv.numeric.8"));
        h.h.ui.apply(Action::SetNumeric(7, 350), &mut h.s);
        h.h.ui.apply(Action::SetNumeric(8, 360), &mut h.s);
        assert_eq!(
            (h.e().draft.up_push_distance_px, h.e().draft.side_push_distance_px),
            (350, 360)
        );
        h.h.ui
            .apply(Action::SetCrossingModel(fence_types::CrossingModel::Classic), &mut h.s);
        h.run(3);
        assert_eq!(h.rec("combo.crossing-model").state, "0");
        assert!(!h.has("adv.numeric.7") && !h.has("adv.numeric.8"));
    }
}

#[test]
fn intent_201_single_slider_is_localized_and_changes_the_saved_value() {
    for lang in [Lang::En, Lang::Tr] {
        let s = Settings {
            crossing_model: fence_types::CrossingModel::Intent,
            ..Settings::default()
        };
        let mut h = H::new(&s, vec![], lang).sized(1200.0, 1040.0, 1.0);
        h.tab(1);
        assert_eq!(h.rec("adv.crossing-ease").state, "3");
        assert_eq!(h.rec("adv.crossing-ease").text, tr_of(lang).t("CrossingEase"));
        h.h.ui.apply(Action::SetCrossingEase(5), &mut h.s);
        h.run(3);
        assert_eq!(h.rec("adv.crossing-ease").state, "5");
        h.e_mut().save();
        assert!(matches!(&h.e().outcome,Outcome::Saved(saved) if saved.crossing_ease==5));
    }
}

#[test]
fn f13_trace_checkbox_and_fifth_shortcut_edit_the_saved_draft() {
    for lang in [Lang::En, Lang::Tr] {
        let mut h = H::new(&Settings::default(), vec![], lang).sized(1200.0, 1040.0, 1.0);
        h.tab(1);
        assert!(!h.e().draft.edge_trace);
        assert_eq!(h.rec("adv.edge-trace").text, tr_of(lang).t("EdgeTrace"));
        h.click("adv.edge-trace");
        assert!(h.e().draft.edge_trace);
        h.tab(2);
        assert_eq!(h.rec("short.4.box").text, "Ctrl + Alt + M");
        assert_eq!(h.rec("short.4.box").rect.height(), 36.0);
        h.click("short.4.box");
        h.key(Key::F24, Modifiers::CTRL | Modifiers::SHIFT);
        assert_eq!(h.e().draft.mark_accident_hotkey().vk, 0x87);
        assert!(h.e().draft.mark_accident_mod_ctrl && h.e().draft.mark_accident_mod_shift);
        h.e_mut().save();
        match &h.e().outcome {
            Outcome::Saved(s) => assert!(s.edge_trace && s.mark_accident_hot_key == 0x87),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn f03_ambiguous_manual_and_boundary_controls_cannot_create_saved_choices() {
    for lang in [Lang::En, Lang::Tr] {
        let ms = vec![
            mon("ordinary", 1, true, 0, 0, 100, 100),
            mon("shared#ambiguous:two", 2, false, 100, 0, 200, 100),
        ];
        let mut h = H::new(&Settings::default(), ms, lang);
        h.e_mut().set_manual_mode(true);
        assert!(!h.e().manual_rows()[0].2);
        assert_eq!(h.e().manual_choose_count(), 0);
        h.e_mut().set_manual_monitor("shared#ambiguous:two", true);
        assert!(h.e().draft.manual_monitors.is_empty());
        h.tab(1);
        h.click("adv.toggle");
        assert!(!enabled(&h.rec("adv.manual.0")));
        h.tab(0);
        h.open_first_boundary(false);
        for tag in [
            "drawer.tile.free",
            "drawer.tile.push",
            "drawer.tile.blocked",
            "drawer.when",
            "drawer.separate",
        ] {
            assert!(!enabled(&h.rec(tag)), "{tag}");
        }
        assert_eq!(h.e().drawer.status, tr_of(lang).t("AmbiguousDisplayIdentity"));
        h.e_mut().choose_primary_behavior(fence_types::Behavior::Blocked);
        h.e_mut().choose_separate_behavior(0, fence_types::Behavior::Free);
        h.e_mut().set_control(Control::Always);
        h.e_mut().toggle_separate();
        h.e_mut().toggle_when();
        assert!(h.e().draft.boundary_rules.is_empty());
        assert!(!h.e().separate_expanded && !h.e().when_expanded);
    }
}

#[test]
fn f05_capture_keeps_an_already_held_windows_key() {
    let mut h = H::new(&Settings::default(), vec![], Lang::En);
    h.tab(2);
    h.click("short.0.box");
    h.s.held_win = true;
    h.key(Key::F24, Modifiers::SHIFT);
    assert_eq!(h.e().draft.hot_key, 0x87);
    assert!(h.e().draft.mod_shift && h.e().draft.mod_win);
    assert_eq!(h.rec("short.0.box").text, "Shift + Win + F24");
}

#[test]
fn f05_missing_required_and_duplicate_imports_remain_drafts_before_confirmation() {
    for missing in [true, false] {
        let mut s = Settings::default();
        if missing {
            s.hot_key = 0;
        } else {
            s.pause_hot_key = s.hot_key;
            s.pause_mod_ctrl = s.mod_ctrl;
            s.pause_mod_alt = s.mod_alt;
        }
        let file = TempFile::new("f05-repairable");
        std::fs::write(&file.0, fence_config::to_json(&s)).unwrap();
        let mut h = H::with_backend(
            &Settings::default(),
            vec![],
            Lang::En,
            None,
            Rc::new(crate::backend::RealBackend),
        );
        let mut dialogs = FakeDialogs {
            restore_path: Some(file.0.clone()),
            confirm: true,
            ..Default::default()
        };
        h.e_mut().import(&mut dialogs);
        assert_eq!(h.e().outcome, Outcome::Open);
        assert_eq!(*dialogs.confirmations.borrow(), 0);
        assert_eq!(h.e().draft.hot_key, s.hot_key);
        assert_eq!(h.e().draft.pause_hot_key, s.pause_hot_key);
        assert!(!h.e().inline.text.is_empty());
    }
    let mut h = H::new(&Settings::default(), vec![], Lang::En);
    for (i, (ctrl, alt, shift, win)) in [
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, false, true),
    ]
    .into_iter()
    .enumerate()
    {
        h.e_mut().capture_hotkey_with_win(i, 0x87, ctrl, alt, shift, win);
    }
    h.e_mut().save();
    assert!(matches!(h.e().outcome, Outcome::Saved(_)));
}

#[test]
fn f03_interrupted_backup_keeps_previous_bytes_reports_failure_and_removes_sibling() {
    use std::io::Write;
    let file = TempFile::new("f03-backup");
    std::fs::write(&file.0, b"previous backup bytes").unwrap();
    let mut h = H::new(&Settings::default(), vec![], Lang::En);
    assert!(!h.e_mut().backup_with_writer(&file.0, |target, bytes| {
        target.write_all(&bytes[..20])?;
        Err(std::io::Error::other("injected interrupted backup"))
    }));
    assert_eq!(std::fs::read(&file.0).unwrap(), b"previous backup bytes");
    assert_eq!(h.e().inline.text, en().t("ExportFailed"));
    let siblings = || {
        std::fs::read_dir(file.0.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|p| {
                p.file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{}.", file.0.file_name().unwrap().to_string_lossy()))
            })
            .count()
    };
    assert_eq!(siblings(), 0);
    assert!(h.e_mut().backup_to_file(&file.0));
    assert!(fence_config::from_json(&std::fs::read_to_string(&file.0).unwrap()).is_ok());
    assert_eq!(h.e().inline.text, en().t("BackupDone"));
    assert_eq!(siblings(), 0);
}

#[test]
fn f09_shortcut_fields_are_36_points_high_centered_and_unclipped() {
    for lang in [Lang::En, Lang::Tr] {
        let mut h = H::new(&Settings::default(), vec![], lang);
        h.tab(2);
        for i in 0..h.e().shortcut_count() {
            let row = h.rec(&format!("short.{i}.head")).rect;
            let frame = h.rec(&format!("short.{i}.box")).rect;
            let clear = h.rec(&format!("short.{i}.clear")).rect;
            assert_eq!(frame.height(), 36.0);
            assert_eq!(clear.height(), 36.0);
            assert_eq!(frame.center().y, row.center().y);
            assert!(row.contains_rect(frame) && row.contains_rect(clear));
            assert!(
                h.rec("body").rect.contains_rect(frame),
                "row {i} must fit the compact viewport"
            );
        }
    }
}

#[test]
fn f09_screen_name_moves_above_selected_gate_when_readable_space_remains() {
    use crate::ui::displays::name_above_pill;
    let label = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), vec2(100.0, 80.0));
    let pill = egui::Rect::from_min_size(egui::pos2(0.0, 60.0), vec2(120.0, 30.0));
    let above = name_above_pill(label, pill).unwrap();
    assert_eq!(above.top(), label.top());
    assert_eq!(above.bottom(), pill.top() - 4.0);
    assert!(name_above_pill(label, pill.translate(vec2(0.0, -21.0))).is_none());
    let args = ["--screenshot-demo", "x.png", "Light", "-1", "en", "1600x1040"].map(str::to_string);
    let a = crate::screenshot::parse_args(&args).unwrap().unwrap();
    let ui = crate::screenshot::build_ui(&a, fake(), SystemTheme::default(), None);
    let mut h = Headless::new(ui, vec2(1600.0, 1040.0), 1.0, false);
    h.settle(&mut FakeServices::default(), 8);
    let map = h.ui.rec.get("map").unwrap().rect;
    assert!(h.last.as_ref().unwrap().shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Top" && map.contains(text.pos))), "selected gate must leave Top's name painted in the map");
}

#[test]
fn f09_light_and_dark_documentation_views_render_complete_headless() {
    for theme in ["Light", "Dark"] {
        for view in ["-1", "2"] {
            let args = ["--screenshot-demo", "x.png", theme, view, "en"].map(str::to_string);
            let a = crate::screenshot::parse_args(&args).unwrap().unwrap();
            let ui = crate::screenshot::build_ui(&a, fake(), SystemTheme::default(), None);
            let mut h = Headless::new(ui, vec2(1064.0, 820.0), 1.0, false);
            h.settle(&mut FakeServices::default(), 8);
            assert_eq!(
                h.ui.layout_metrics().unwrap().mode,
                if view == "-1" { "drawer" } else { "shortcuts" }
            );
            assert!(h.ui.rec.get("footer.save").is_some());
            if view == "2" {
                assert!(h.ui.rec.get("short.3.box").is_some());
            }
            let image = h.render().clone_pixels();
            assert_ne!(image.get(0, 0), image.get(300, 100), "{theme}/{view}");
        }
    }
}

// Parity row P026 (tests/Tests.cs:579).
#[test]
fn p026_backup_writes_the_draft_and_posts_localized_success() {
    let ms = vec![
        mon("top", 1, false, 0, -100, 100, 0),
        mon("main", 2, true, 0, 0, 100, 100),
    ];
    let file = TempFile::new("m2p2");
    let mut h = H::new(&Settings::default(), ms, Lang::En);
    assert!(h.e_mut().backup_to_file(&file.0));
    let restored = FakeBackend.parse_json(&std::fs::read_to_string(&file.0).expect("written"));
    assert!(restored.is_ok());
    assert_eq!(h.e().inline.text, en().t("BackupDone"));
    // The same through the General page and the injected save dialog.
    let other = TempFile::new("m2p2-ui");
    h.s.dialogs.backup_path = Some(other.0.clone());
    h.tab(3);
    h.click("gen.export");
    assert_eq!(*h.s.dialogs.backup_picks.borrow(), 1);
    assert!(other.0.exists());
    assert_eq!(h.rec("footer.status").text, en().t("BackupDone"));
}

// ------------------------------------------------------------------------------------------------- P028

// Parity row P028 (tests/Tests.cs:610).
#[test]
fn p028_map_is_one_tab_stop_with_screens_and_boundary_as_accessible_children_and_24px_target() {
    let ms = vec![
        mon("top", 1, false, 0, -100, 100, 0),
        mon("main", 2, true, 0, 0, 100, 100),
    ];
    let mut h = H::new(&Settings::default(), ms.clone(), Lang::En);
    assert_eq!(h.e().boundaries.len(), 1);
    let target = h.rec("map.boundary.0").rect;
    assert!(target.width() >= 24.0 && target.height() >= 24.0, "{target:?}");
    let update =
        h.h.last
            .as_ref()
            .and_then(|o| o.platform_output.accesskit_update.clone())
            .expect("accesskit update");
    let map_id = egui::Id::new("monitor-map").accesskit_id();
    let map = update
        .nodes
        .iter()
        .find(|(id, _)| *id == map_id)
        .map(|(_, n)| n.clone())
        .expect("map node");
    let children = map.children().to_vec();
    assert_eq!(children.len(), ms.len() + 1, "two screens and one boundary");
    let label = |id: egui::accesskit::NodeId| {
        update
            .nodes
            .iter()
            .find(|(n, _)| *n == id)
            .and_then(|(_, n)| n.label().map(str::to_string))
            .unwrap_or_default()
    };
    assert!(
        label(children[ms.len()]).contains(&en().t("DrawerTitle")),
        "{}",
        label(children[ms.len()])
    );
    assert!(
        map.supports_action(egui::accesskit::Action::Focus),
        "the map is focusable"
    );
    // One tab stop: Tab reaches the map; arrows pick the boundary; Enter opens it.
    let id = egui::Id::new("monitor-map");
    let mut reached = false;
    for _ in 0..40 {
        h.key(Key::Tab, Modifiers::NONE);
        if h.h.ctx.memory(|m| m.has_focus(id)) {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab reaches the map");
    h.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(h.e().map_selected_boundary, Some(0));
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.e().drawer_visible, "Enter opens the selected boundary");
    assert_eq!(h.e().outcome, Outcome::Open, "Enter on the map is not Save");
    // An AccessKit click on the boundary child opens it too.
    h.key(Key::Escape, Modifiers::NONE);
    assert!(!h.e().drawer_visible, "Escape closes the drawer first");
    h.h.frame(
        &mut h.s,
        vec![Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: children[ms.len()],
            data: None,
        })],
    );
    h.run(2);
    assert!(h.e().drawer_visible);
    // A mouse click on the boundary's hit target opens it.
    h.key(Key::Escape, Modifiers::NONE);
    h.click_at(target.center());
    assert!(h.e().drawer_visible);
}

// ------------------------------------------------------------------------------------------------- P167

/// Every `Settings` field, classified. The exhaustive pattern stops compiling when a field is added, so the new
/// field must be classified here (C# reflected over the persisted properties).
fn classify(s: &Settings) -> usize {
    let Settings {
        crossing_ease: _,
        crossing_model: _,
        up_push_distance_px: _,
        side_push_distance_px: _,
        push_absorb_ms: _,
        push_pause_reset_ms: _,
        steady_push_ms: _, // JSON-only tuning (SteadyPushMs); the ease slider sets its default
        edge_trace: _,
        mark_accident_hot_key: _,
        mark_accident_mod_ctrl: _,
        mark_accident_mod_alt: _,
        mark_accident_mod_shift: _,
        mark_accident_mod_win: _,
        settings_version: _, // stamped by save, not edited (C# excluded SettingsVersion)
        hot_key: _,
        mod_ctrl: _,
        mod_alt: _,
        mod_shift: _,
        mod_win: _,
        confine_hot_key: _,
        confine_mod_ctrl: _,
        confine_mod_alt: _,
        confine_mod_shift: _,
        confine_mod_win: _,
        pause_hot_key: _,
        pause_mod_ctrl: _,
        pause_mod_alt: _,
        pause_mod_shift: _,
        pause_mod_win: _,
        side_hot_key: _,
        side_mod_ctrl: _,
        side_mod_alt: _,
        side_mod_shift: _,
        side_mod_win: _,
        start_gate_closed: _,
        start_confine_on: _,
        start_paused: _,
        start_side_contain_on: _,
        auto_start: _,
        deliberate_cross: _,
        descent_routing: _,
        upward_boundary_default: _,
        side_boundary_default: _,
        downward_boundary_default: _,
        boundary_rules: _,
        up_cross_min: _,
        up_cross_slack: _,
        passage_inset_px: _,
        edge_holdback_px: _,
        seam_tolerance_px: _,
        safety_escapes: _,
        boundary_tip_seen: _,
        side_cross_min: _,
        side_cross_slack: _,
        mode: _,
        manual_monitors: _,
        excluded_devices: _,
        monitor_names: _,
        language: _,
        theme: _,
        legacy_crossing_path_mode: _, // legacy v0-v3, read for migration only (C# JsonIgnore'd)
        legacy_up_links: _,           // legacy v0-v3, read for migration only (C# JsonIgnore'd)
        has_repairable_import_errors: _, // transient (C# JsonIgnore)
        intel_defaults_applied: _,    // transient (C# JsonIgnore)
        raw: _, // JSON presence/null bookkeeping of the file read (Track B; no C# property, never edited)
    } = s;
    EDITABLE.len() + NOT_EDITED.len()
}

const EDITABLE: [&str; 58] = [
    "crossing_ease",
    "crossing_model",
    "up_push_distance_px",
    "side_push_distance_px",
    "push_absorb_ms",
    "push_pause_reset_ms",
    "edge_trace",
    "mark_accident_hot_key",
    "mark_accident_mod_ctrl",
    "mark_accident_mod_alt",
    "mark_accident_mod_shift",
    "mark_accident_mod_win",
    "hot_key",
    "mod_ctrl",
    "mod_alt",
    "mod_shift",
    "mod_win",
    "confine_hot_key",
    "confine_mod_ctrl",
    "confine_mod_alt",
    "confine_mod_shift",
    "confine_mod_win",
    "pause_hot_key",
    "pause_mod_ctrl",
    "pause_mod_alt",
    "pause_mod_shift",
    "pause_mod_win",
    "side_hot_key",
    "side_mod_ctrl",
    "side_mod_alt",
    "side_mod_shift",
    "side_mod_win",
    "start_gate_closed",
    "start_confine_on",
    "start_paused",
    "start_side_contain_on",
    "auto_start",
    "deliberate_cross",
    "descent_routing",
    "upward_boundary_default",
    "side_boundary_default",
    "downward_boundary_default",
    "boundary_rules",
    "up_cross_min",
    "up_cross_slack",
    "passage_inset_px",
    "edge_holdback_px",
    "seam_tolerance_px",
    "safety_escapes",
    "boundary_tip_seen",
    "side_cross_min",
    "side_cross_slack",
    "mode",
    "manual_monitors",
    "excluded_devices",
    "monitor_names",
    "language",
    "theme",
];

const NOT_EDITED: [&str; 7] = [
    "steady_push_ms",
    "settings_version",
    "legacy_crossing_path_mode",
    "legacy_up_links",
    "has_repairable_import_errors",
    "intel_defaults_applied",
    "raw",
];

type Check = fn(&Settings) -> bool;

// Parity row P167 (tests/Tests.cs:2271).
#[test]
fn p167_every_persisted_setting_is_edited_by_the_window_and_rule_fields_are_covered() {
    assert_eq!(classify(&Settings::default()), 65);
    let StoredRule {
        from_stable_id: _,
        to_stable_id: _,
        direction: _,
        behavior: _,
        control: _,
    } = rule("a", "b", Direction::Up, RuleBehavior::Push, Control::Always);
    // Each scenario changes its fields through window actions on a fresh window; Save (or the draft when a
    // validation stops Save) must show every listed field changed from where it started.
    let k = 0x4B;
    let hotkey_start = |s: &mut Settings| {
        s.mod_ctrl = false;
        s.mod_win = true;
        s.confine_mod_ctrl = false;
        s.confine_mod_win = true;
        s.pause_mod_ctrl = false;
        s.pause_mod_win = true;
        s.side_mod_ctrl = false;
        s.side_mod_win = true;
        s.mark_accident_mod_ctrl = false;
        s.mark_accident_mod_win = true;
    };
    type Scenario = (&'static [&'static str], fn(&mut Settings), Vec<Action>, Check);
    let scenarios: Vec<Scenario> = vec![
        (
            &["crossing_ease"],
            |_| {},
            vec![Action::SetCrossingEase(5)],
            |s| s.crossing_ease == 5,
        ),
        (
            &[
                "crossing_model",
                "up_push_distance_px",
                "side_push_distance_px",
                "push_absorb_ms",
                "push_pause_reset_ms",
            ],
            |_| {},
            vec![
                Action::SetCrossingModel(fence_types::CrossingModel::StopThenPush),
                Action::SetNumeric(7, 260),
                Action::SetNumeric(8, 270),
                Action::SetNumeric(9, 80),
                Action::SetNumeric(10, 160),
            ],
            |s| {
                s.crossing_model == fence_types::CrossingModel::StopThenPush
                    && s.up_push_distance_px == 260
                    && s.side_push_distance_px == 270
                    && s.push_absorb_ms == 80
                    && s.push_pause_reset_ms == 160
            },
        ),
        (
            &["edge_trace"],
            |_| {},
            vec![Action::SetEdgeTrace(true)],
            |s| s.edge_trace,
        ),
        (
            &[
                "mark_accident_hot_key",
                "mark_accident_mod_ctrl",
                "mark_accident_mod_alt",
                "mark_accident_mod_shift",
                "mark_accident_mod_win",
            ],
            hotkey_start,
            vec![Action::CaptureHotkey(4, 0x87, true, false, true)],
            |s| {
                s.mark_accident_hot_key == 0x87
                    && s.mark_accident_mod_ctrl
                    && !s.mark_accident_mod_alt
                    && s.mark_accident_mod_shift
                    && !s.mark_accident_mod_win
            },
        ),
        (
            &["hot_key", "mod_ctrl", "mod_alt", "mod_shift", "mod_win"],
            hotkey_start,
            vec![Action::CaptureHotkey(0, k, true, false, true)],
            |s| s.hot_key == 0x4B && s.mod_ctrl && !s.mod_alt && s.mod_shift && !s.mod_win,
        ),
        (
            &[
                "confine_hot_key",
                "confine_mod_ctrl",
                "confine_mod_alt",
                "confine_mod_shift",
                "confine_mod_win",
            ],
            hotkey_start,
            vec![Action::CaptureHotkey(1, k, true, false, true)],
            |s| {
                s.confine_hot_key == 0x4B
                    && s.confine_mod_ctrl
                    && !s.confine_mod_alt
                    && s.confine_mod_shift
                    && !s.confine_mod_win
            },
        ),
        (
            &[
                "pause_hot_key",
                "pause_mod_ctrl",
                "pause_mod_alt",
                "pause_mod_shift",
                "pause_mod_win",
            ],
            hotkey_start,
            vec![Action::CaptureHotkey(2, 0x4C, true, false, true)],
            |s| {
                s.pause_hot_key == 0x4C && s.pause_mod_ctrl && !s.pause_mod_alt && s.pause_mod_shift && !s.pause_mod_win
            },
        ),
        (
            &[
                "side_hot_key",
                "side_mod_ctrl",
                "side_mod_alt",
                "side_mod_shift",
                "side_mod_win",
            ],
            hotkey_start,
            vec![Action::CaptureHotkey(3, 0x4D, true, false, true)],
            |s| s.side_hot_key == 0x4D && s.side_mod_ctrl && !s.side_mod_alt && s.side_mod_shift && !s.side_mod_win,
        ),
        (
            &["start_gate_closed"],
            |_| {},
            vec![Action::SetStartGate(false)],
            |s| !s.start_gate_closed,
        ),
        (
            &["start_side_contain_on"],
            |_| {},
            vec![Action::SetStartSide(false)],
            |s| !s.start_side_contain_on,
        ),
        (
            &["auto_start"],
            |_| {},
            vec![Action::SetAutoStart(true)],
            |s| s.auto_start,
        ),
        (
            &["start_confine_on"],
            |_| {},
            vec![Action::SetStartConfine(true)],
            |s| s.start_confine_on,
        ),
        (
            &["start_paused"],
            |_| {},
            vec![Action::SetStartPaused(true)],
            |s| s.start_paused,
        ),
        (
            &["deliberate_cross"],
            |_| {},
            vec![Action::SetDeliberate(false)],
            |s| !s.deliberate_cross,
        ),
        (
            &["descent_routing"],
            |_| {},
            vec![Action::SetDescent(true)],
            |s| s.descent_routing,
        ),
        (
            &[
                "upward_boundary_default",
                "side_boundary_default",
                "downward_boundary_default",
            ],
            |_| {},
            vec![
                Action::SetDefault(0, BoundaryDefault::Blocked),
                Action::SetDefault(1, BoundaryDefault::Free),
                Action::SetDefault(2, BoundaryDefault::Push),
            ],
            |s| {
                s.upward_boundary_default == Stored::Known(BoundaryDefault::Blocked)
                    && s.side_boundary_default == Stored::Known(BoundaryDefault::Free)
                    && s.downward_boundary_default == Stored::Known(BoundaryDefault::Push)
            },
        ),
        (
            &["boundary_rules", "boundary_tip_seen"],
            |_| {},
            vec![
                Action::OpenBoundary(0),
                Action::ChoosePrimary(fence_types::Behavior::Blocked),
            ],
            |s| !s.boundary_rules.is_empty() && s.boundary_tip_seen,
        ),
        (
            &[
                "up_cross_min",
                "up_cross_slack",
                "side_cross_min",
                "side_cross_slack",
                "passage_inset_px",
                "edge_holdback_px",
                "seam_tolerance_px",
            ],
            |_| {},
            (0..7).map(|i| Action::SetNumeric(i, 7 + i as i32)).collect(),
            |s| {
                (s.up_cross_min, s.up_cross_slack, s.side_cross_min, s.side_cross_slack) == (7, 8, 9, 10)
                    && (s.passage_inset_px, s.edge_holdback_px, s.seam_tolerance_px) == (11, 12, 13)
            },
        ),
        (
            &["safety_escapes"],
            |_| {},
            vec![Action::SetSafety(false)],
            |s| !s.safety_escapes,
        ),
        (
            &["mode", "manual_monitors"],
            |_| {},
            vec![
                Action::SetManualMode(true),
                Action::SetManualMonitor("top".into(), true),
            ],
            |s| s.mode == "Manual" && s.manual_monitors == ["top"],
        ),
        (
            &["excluded_devices"],
            |_| {},
            vec![Action::SetKeepOut("right".into(), true)],
            |s| s.excluded_devices == ["right"],
        ),
        (
            &["monitor_names"],
            |_| {},
            vec![Action::SetName("right".into(), "Desk".into())],
            |s| s.monitor_names.iter().any(|(k, v)| k == "right" && v == "Desk"),
        ),
        (
            &["language"],
            |_| {},
            vec![Action::SetLanguage(2)],
            |s| s.language == "tr",
        ),
        (&["theme"], |_| {}, vec![Action::SetTheme(2)], |s| s.theme == "Dark"),
    ];
    let mut covered: Vec<&str> = Vec::new();
    let monitors = vec![
        mon("main", 1, true, 0, 0, 100, 100),
        mon("right", 2, false, 100, 0, 200, 100),
        mon("top", 3, false, 0, -100, 100, 0),
    ];
    for (fields, start, actions, check) in scenarios {
        let mut s = Settings::default();
        start(&mut s);
        let mut h = H::new(&s, monitors.clone(), Lang::En);
        for a in actions {
            h.act(a);
        }
        h.click("footer.save");
        let result = match &h.e().outcome {
            Outcome::Saved(r) => (**r).clone(),
            _ => h.e().draft.clone(),
        };
        assert!(check(&result), "{fields:?} not edited: {result:?}");
        covered.extend_from_slice(fields);
    }
    covered.sort_unstable();
    let mut want: Vec<&str> = EDITABLE.to_vec();
    want.sort_unstable();
    assert_eq!(covered, want, "every editable field has a scenario");
    assert!(NOT_EDITED.iter().all(|f| !EDITABLE.contains(f)));
}

// ------------------------------------------------------------------------------------------------- P224

// Parity row P224 (tests/Tests.cs:2856): the docs capture state (Dark, selected boundary) painted by the CPU
// rasterizer from egui's meshes; the glow window capture is checked the same way in TRACK-D.txt.
#[test]
fn p224_synthetic_dark_capture_is_aligned_and_owner_drawn_controls_stay_themed() {
    let args: Vec<String> = ["--screenshot-demo", "x.png", "Dark", "-1", "en"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let a = crate::screenshot::parse_args(&args).expect("some").expect("ok");
    let ui = crate::screenshot::build_ui(&a, fake(), SystemTheme::default(), None);
    let mut h = Headless::new(ui, Vec2::new(1064.0, 820.0), 1.0, false);
    let mut s = FakeServices::default();
    h.settle(&mut s, 8);
    assert_eq!(h.ui.theme, Theme::DARK);
    assert_eq!(h.ui.layout_metrics().expect("metrics").mode, "drawer");
    let img = h.render().clone_pixels();
    let (w, hgt) = (img.w, img.h);
    let (mut bright, mut sampled) = (0, 0);
    let mut y = hgt.saturating_sub(58);
    while y < hgt - 10 {
        let mut x = 18;
        while x < w - 18 {
            let p = img.get(x, y);
            if p[0] >= 245 && p[1] >= 245 && p[2] >= 245 {
                bright += 1;
            }
            sampled += 1;
            x += 2;
        }
        y += 2;
    }
    let mut brand_ink = 0;
    for y in 12..56.min(hgt) {
        for x in 40..160.min(w) {
            let p = img.get(x, y);
            if p[0] >= 120 && p[1] >= 120 && p[2] >= 120 {
                brand_ink += 1;
            }
        }
    }
    assert!(bright < sampled / 100, "bright={bright}/{sampled}");
    assert!(brand_ink >= 40, "brandInk={brand_ink}");
    // The footer buttons are painted in the theme (C# CaptureLooksComplete).
    let save = h.ui.rec.get("footer.save").expect("save").rect;
    let p = img.get((save.left() + 5.0) as usize, save.center().y as usize);
    assert!(near(p, Theme::DARK.accent, 2), "save {p:?}");
}

// ------------------------------------------------------------------------------------------------- B038

// Parity row B038 (Ui/SettingsForm.General.cs:41-90).
#[test]
fn b038_backup_validates_the_draft_first_and_restore_goes_through_the_same_validation() {
    let sealing = Settings {
        boundary_rules: vec![rule(
            "right",
            "left",
            Direction::Left,
            RuleBehavior::Blocked,
            Control::Always,
        )],
        ..Settings::default()
    };
    // Export of a sealing draft: no dialog, no file, the seal error with its boundary action.
    let mut h = H::new(&sealing, two_side_by_side(), Lang::En);
    let out = TempFile::new("b038-out");
    h.s.dialogs.backup_path = Some(out.0.clone());
    h.tab(3);
    h.click("gen.export");
    assert_eq!(
        *h.s.dialogs.backup_picks.borrow(),
        0,
        "validation runs before the dialog"
    );
    assert!(!out.0.exists());
    assert_eq!(h.e().inline.text, en().f("InlineSealError", &["Screen 2"]));
    h.click("footer.action");
    assert_eq!(h.e().tab, 0);
    assert!(h.e().drawer_visible, "the action opens the sealing boundary");
    // Restore of a sealing file: it becomes the draft with the same error and is never applied.
    let file = TempFile::new("b038-sealed");
    let mut writer = Editor::new(&Settings::default(), vec![], fake(), en(), None);
    writer.draft = sealing.clone();
    assert!(
        writer.backup_to_file(&file.0),
        "no monitors, nothing sealed while writing"
    );
    let mut h = H::new(&Settings::default(), two_side_by_side(), Lang::En);
    h.s.dialogs.restore_path = Some(file.0.clone());
    h.s.dialogs.confirm = true;
    h.tab(3);
    h.click("gen.import");
    assert_eq!(*h.s.dialogs.confirmations.borrow(), 0);
    assert_eq!(h.e().outcome, Outcome::Open);
    assert_eq!(h.e().draft.boundary_rules.len(), 1);
    assert_eq!(h.e().inline.text, en().f("InlineSealError", &["Screen 2"]));
    // Unreadable JSON and a missing file: ImportInvalid, nothing applied.
    let bad = TempFile::new("b038-bad");
    std::fs::write(&bad.0, "{ not json").expect("temp");
    let mut h = H::new(&Settings::default(), two_side_by_side(), Lang::En);
    h.s.dialogs.restore_path = Some(bad.0.clone());
    h.tab(3);
    h.click("gen.import");
    assert_eq!(h.e().inline.text, en().t("ImportInvalid"));
    h.s.dialogs.restore_path = Some(std::env::temp_dir().join("MouseFence-ui-missing-file.json"));
    h.click("gen.import");
    assert_eq!(h.e().inline.text, en().t("ImportInvalid"));
    assert_eq!(h.e().outcome, Outcome::Open);
    // A declined confirmation keeps the current draft.
    let ok = TempFile::new("b038-ok");
    let mut writer = Editor::new(
        &Settings {
            up_cross_min: 44,
            ..Settings::default()
        },
        vec![],
        fake(),
        en(),
        None,
    );
    assert!(writer.backup_to_file(&ok.0));
    let mut h = H::new(&Settings::default(), two_side_by_side(), Lang::En);
    h.s.dialogs.restore_path = Some(ok.0.clone());
    h.s.dialogs.confirm = false;
    h.tab(3);
    h.click("gen.import");
    assert_eq!(*h.s.dialogs.confirmations.borrow(), 1);
    assert_eq!(h.e().outcome, Outcome::Open);
    assert_eq!(h.e().draft.up_cross_min, 3);
    // No path picked: nothing happens.
    h.s.dialogs.restore_path = None;
    h.click("gen.import");
    assert_eq!(*h.s.dialogs.confirmations.borrow(), 1);
}

// ------------------------------------------------------------------------------------------------- window behaviour

#[test]
fn s_ui_window_keyboard_escape_enter_identify_and_sidebar_arrows() {
    let mut h = H::new(&Settings::default(), two_side_by_side(), Lang::En);
    // Identify all screens: the usable screens only.
    h.act(Action::SetKeepOut("right".into(), true));
    h.click("identify");
    assert_eq!(h.s.identified.len(), 1);
    assert_eq!(
        h.s.identified[0]
            .iter()
            .map(|m| m.stable_id.as_str())
            .collect::<Vec<_>>(),
        ["left"]
    );
    // Sidebar arrows move between pages.
    h.click("nav.0");
    assert_eq!(
        h.h.ctx.memory(|m| m.focused()),
        Some(egui::Id::new(("nav", "nav.0"))),
        "a clicked page button takes focus"
    );
    h.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(h.e().tab, 1);
    h.key(Key::ArrowUp, Modifiers::NONE);
    h.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(h.e().tab, 3, "wraps around");
    // Enter with nothing focused saves; Escape cancels when no drawer is open.
    let mut h = H::new(&Settings::default(), two_side_by_side(), Lang::En);
    h.key(Key::Enter, Modifiers::NONE);
    assert!(matches!(h.e().outcome, Outcome::Saved(_)));
    let mut h = H::new(&Settings::default(), two_side_by_side(), Lang::En);
    h.open_first_boundary(false);
    h.key(Key::Escape, Modifiers::NONE);
    assert!(
        !h.e().drawer_visible && h.e().outcome == Outcome::Open,
        "Escape closes the drawer first"
    );
    h.key(Key::Escape, Modifiers::NONE);
    assert_eq!(h.e().outcome, Outcome::Cancelled);
}

#[test]
fn s_ui_screen_rows_name_box_keep_out_and_list_hover() {
    let mut h = H::new(&Settings::default(), two_side_by_side(), Lang::En);
    let row = h.rec("row.1");
    assert_eq!(row.text, "Screen 2");
    assert_eq!(
        h.rec("row.0.keep").text,
        en().t("MainAlwaysV2"),
        "the main screen cannot be kept out"
    );
    h.click("row.1.keep");
    assert!(h.e().is_excluded("right"));
    assert_eq!(h.rec("row.1.keep").state, "checked");
    // Type a name into the row's box.
    h.click("row.1.name");
    h.h.frame(&mut h.s, vec![Event::Text("Desk".into())]);
    h.run(2);
    assert!(
        h.e()
            .draft
            .monitor_names
            .iter()
            .any(|(k, v)| k == "right" && v == "Desk")
    );
    assert!(h.rec("map.boundary.0").text.contains("Desk"));
    // Hovering a row highlights its screen on the map.
    let p = h.rec("row.0").rect.center();
    h.h.hover(&mut h.s, p);
    h.run(1);
    assert_eq!(h.h.ui.hovered_from_list(), Some("left"));
}

#[test]
fn s_ui_advanced_and_general_controls_edit_the_draft() {
    let ms = vec![
        mon("main", 1, true, 0, 0, 100, 100),
        mon("top", 2, false, 0, -100, 100, 0),
    ];
    let mut h = H::new(&Settings::default(), ms, Lang::En).sized(1064.0, 3000.0, 1.0);
    h.tab(1);
    h.click("adv.manual");
    assert!(h.e().is_manual_mode());
    h.click("adv.manual.0");
    assert_eq!(h.e().draft.manual_monitors, ["top"]);
    h.click("adv.toggle");
    h.click("adv.numeric.0.up");
    assert_eq!(h.e().draft.up_cross_min, 4);
    h.click("adv.numeric.0.reset");
    assert_eq!(h.e().draft.up_cross_min, 3);
    h.click("adv.safety");
    assert!(!h.e().draft.safety_escapes);
    h.click("adv.reset");
    assert!(h.e().draft.safety_escapes && !h.e().is_manual_mode());
    assert_eq!(h.e().inline.text, en().t("ResetDone"));
    h.tab(3);
    h.click("gen.more");
    assert!(h.has("gen.confine"));
    h.click("gen.confine");
    h.click("gen.auto");
    assert_eq!(h.rec("gen.warning").text, en().t("StartupConfineWarning"));
    h.click("footer.save");
    match &h.e().outcome {
        Outcome::Saved(r) => assert!(r.start_confine_on && r.auto_start),
        o => panic!("{o:?}"),
    }
}

// ------------------------------------------------------------------------------------------------- join

// The same window scenarios against the real engine and settings model (run since the join, 2026-10-13).
#[test]
fn s_join_split_direction_excluded_target_and_restore_with_the_real_backend() {
    let real: Rc<dyn Backend> = Rc::new(crate::backend::RealBackend);
    let mut h = H::with_backend(&split_pair(), two_side_by_side(), Lang::En, None, real.clone());
    h.open_first_boundary(false);
    assert_eq!(h.rec("drawer.caption").text, en().t("DifferentByDirection"));
    h.click("drawer.tile.blocked");
    assert_eq!(h.rec("drawer.caption").text, en().t("BothDirections"));
    let s = Settings {
        excluded_devices: vec!["right".into()],
        monitor_names: names(&[("right", "Studio")]),
        ..Settings::default()
    };
    let mut h = H::with_backend(&s, two_side_by_side(), Lang::En, None, real.clone());
    h.open_first_boundary(false);
    assert_eq!(
        h.rec("drawer.status").text,
        en().f("ExcludedTargetExplanation", &["Studio"])
    );
    let file = TempFile::new("join-restore");
    let mut source = Editor::new(
        &Settings {
            up_cross_min: 20,
            ..Settings::default()
        },
        vec![],
        real.clone(),
        en(),
        None,
    );
    assert!(source.backup_to_file(&file.0));
    let mut h = H::with_backend(&Settings::default(), vec![], Lang::En, None, real);
    h.s.dialogs.restore_path = Some(file.0.clone());
    h.s.dialogs.confirm = true;
    h.tab(3);
    h.click("gen.import");
    assert!(matches!(&h.e().outcome, Outcome::Saved(r) if r.up_cross_min == 20));
}

#[test]
fn s_join_large_window_composition_with_the_real_backend() {
    let real: Rc<dyn Backend> = Rc::new(crate::backend::RealBackend);
    let s = crate::screenshot::demo_settings(Some("Light"), "en", &*real);
    let mut h =
        H::with_backend(&s, crate::screenshot::demo_monitors(), Lang::En, None, real).sized(1600.0, 1040.0, 1.0);
    let m = h.h.ui.layout_metrics().expect("metrics");
    assert_eq!(m.body, 976);
    assert!((381..=576).contains(&m.map), "map {}", m.map);
    h.open_first_boundary(true);
    assert_eq!(h.h.ui.layout_metrics().expect("metrics").mode, "drawer");
    assert_eq!(h.e().boundaries.len(), 2, "upper gate and the kept-out laptop's side");
}

// Found at the join by running every window test on the real crates (MF_UI_REAL_BACKEND=1): P004/P005's layout
// (two top screens side by side, Coming down = Blocked) is a sealed group under J05. 1.1.1 says "never crosses"
// there, so the cursor that went up could not come back down; 2.0 demotes one exit to a firm push, and the
// window says so. The parity rows keep the 1.1.1 text through the fake backend; this pins the 2.0 answer.
#[test]
fn s_join_j05_return_line_of_the_sealed_top_pair_is_a_firm_push_in_2_0() {
    use fence_engine::fixes::{Fixes, HIT_J05};
    let s = Settings {
        downward_boundary_default: Stored::Known(BoundaryDefault::Blocked),
        ..Settings::default()
    };
    let policy = fence_config::to_policy(&s);
    let screens = crate::backend::screens_for(&five_screens(), &[]);
    let (exact, exact_hits) = fence_engine::compile::compile_with(&policy, &screens, true, Fixes::EXACT_1_1_1);
    let (shipped, hits) = fence_engine::compile::compile_with(&policy, &screens, true, Fixes::ALL);
    let demoted = |c: &fence_types::CompileResult| c.transitions.iter().filter(|t| t.safety_demoted).count();
    assert_eq!(exact_hits & HIT_J05, 0, "1.1.1 sees no sealed group");
    assert_ne!(hits & HIT_J05, 0, "J05 finds the sealed top pair");
    assert!(demoted(&shipped) > demoted(&exact), "J05 demotes one more exit");
    let real: Rc<dyn Backend> = Rc::new(crate::backend::RealBackend);
    let mut h = H::with_backend(&s, five_screens(), Lang::En, None, real).sized(1064.0, 712.0, 1.0);
    h.open_first_boundary(false);
    assert_eq!(h.rec("drawer.return").text, en().t("ComingDownPush"));
}
