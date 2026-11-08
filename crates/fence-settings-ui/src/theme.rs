//! Colours and theme resolution (port of C# `Ui/Theming.cs`: the `Theme` record and `Theming.Resolve`).

use eframe::egui::{self, Color32};

/// Which palette is in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeKind {
    Light,
    Dark,
    HighContrast,
}

/// C# `Theming.Resolve(pref)`: Windows high contrast wins; then "Dark" and "Light" are explicit; anything else
/// ("System", unknown) follows the registry value AppsUseLightTheme (dark when it is 0). The OS facts are passed
/// in so the rule is testable (parity row B034).
pub fn resolve(pref: &str, high_contrast: bool, system_dark: bool) -> ThemeKind {
    if high_contrast {
        return ThemeKind::HighContrast;
    }
    match pref {
        "Dark" => ThemeKind::Dark,
        "Light" => ThemeKind::Light,
        _ if system_dark => ThemeKind::Dark,
        _ => ThemeKind::Light,
    }
}

/// The six WinForms `SystemColors` the high-contrast palette reads (C# `Theme.HighContrast`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemColors {
    pub window: Color32,
    pub window_text: Color32,
    pub gray_text: Color32,
    pub highlight: Color32,
    pub control: Color32,
    pub control_text: Color32,
}

impl Default for SystemColors {
    /// The standard (non high contrast) Windows values; the real ones come from `GetSysColor`.
    fn default() -> Self {
        Self {
            window: Color32::from_rgb(255, 255, 255),
            window_text: Color32::from_rgb(0, 0, 0),
            gray_text: Color32::from_rgb(109, 109, 109),
            highlight: Color32::from_rgb(0, 120, 215),
            control: Color32::from_rgb(240, 240, 240),
            control_text: Color32::from_rgb(0, 0, 0),
        }
    }
}

/// C# `record struct Theme` (19 colours) plus its derived members.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub kind: ThemeKind,
    pub back: Color32,
    pub surface: Color32,
    pub surface2: Color32,
    pub text: Color32,
    pub subtle: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_weak: Color32,
    pub accent_line: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub chip: Color32,
    pub chip_text: Color32,
    pub star: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub success: Color32,
    pub focus_ring: Color32,
    pub disabled_surface: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

impl Theme {
    pub const LIGHT: Theme = Theme {
        kind: ThemeKind::Light,
        back: rgb(246, 247, 249),
        surface: rgb(253, 253, 254),
        surface2: rgb(254, 254, 255),
        text: rgb(32, 37, 45),
        subtle: rgb(95, 105, 119),
        faint: rgb(132, 142, 156),
        accent: rgb(53, 111, 214),
        accent_weak: rgb(231, 238, 251),
        accent_line: rgb(169, 196, 242),
        border: rgb(210, 216, 224),
        border_strong: rgb(183, 192, 203),
        chip: rgb(238, 241, 245),
        chip_text: rgb(65, 73, 85),
        star: rgb(206, 142, 12),
        danger: rgb(180, 35, 24),
        warning: rgb(139, 93, 0),
        success: rgb(38, 115, 77),
        focus_ring: rgb(169, 196, 242),
        disabled_surface: rgb(238, 241, 245),
    };

    pub const DARK: Theme = Theme {
        kind: ThemeKind::Dark,
        back: rgb(23, 27, 33),
        surface: rgb(29, 34, 42),
        surface2: rgb(35, 42, 52),
        text: rgb(232, 235, 240),
        subtle: rgb(173, 181, 194),
        faint: rgb(125, 136, 151),
        accent: rgb(116, 160, 242),
        accent_weak: rgb(37, 51, 74),
        accent_line: rgb(63, 93, 137),
        border: rgb(54, 63, 75),
        border_strong: rgb(81, 93, 108),
        chip: rgb(42, 50, 61),
        chip_text: rgb(202, 209, 219),
        star: rgb(232, 181, 65),
        danger: rgb(255, 139, 126),
        warning: rgb(240, 196, 109),
        success: rgb(118, 201, 160),
        focus_ring: rgb(63, 93, 137),
        disabled_surface: rgb(25, 30, 37),
    };

    /// C# `Theme.HighContrast`, built from the live system colours.
    pub fn high_contrast(sys: &SystemColors) -> Theme {
        Theme {
            kind: ThemeKind::HighContrast,
            back: sys.window,
            surface: sys.window,
            surface2: sys.window,
            text: sys.window_text,
            subtle: sys.window_text,
            faint: sys.gray_text,
            accent: sys.highlight,
            accent_weak: sys.window,
            accent_line: sys.highlight,
            border: sys.window_text,
            border_strong: sys.window_text,
            chip: sys.control,
            chip_text: sys.control_text,
            star: sys.highlight,
            danger: sys.window_text,
            warning: sys.window_text,
            success: sys.window_text,
            focus_ring: sys.highlight,
            disabled_surface: sys.control,
        }
    }

    pub fn for_kind(kind: ThemeKind, sys: &SystemColors) -> Theme {
        match kind {
            ThemeKind::Light => Theme::LIGHT,
            ThemeKind::Dark => Theme::DARK,
            ThemeKind::HighContrast => Theme::high_contrast(sys),
        }
    }

    /// C# `Theme.IsDark => Back.R < 128`.
    pub fn is_dark(&self) -> bool {
        self.back.r() < 128
    }

    /// C# `Theme.AccentText`.
    pub fn accent_text(&self) -> Color32 {
        if self.is_dark() {
            rgb(23, 27, 33)
        } else {
            rgb(253, 253, 254)
        }
    }

    /// C# `Theme.Sidebar`.
    pub fn sidebar(&self) -> Color32 {
        if self.is_dark() {
            rgb(25, 30, 37)
        } else {
            rgb(238, 241, 245)
        }
    }

    /// C# `ControlPaint.Light(accent, .08f)` / `Dark(accent, .08f)` for the primary button's hover and press.
    pub fn accent_hover(&self) -> Color32 {
        lerp(self.accent, Color32::WHITE, 0.08)
    }
    pub fn accent_pressed(&self) -> Color32 {
        lerp(self.accent, Color32::BLACK, 0.08)
    }

    /// egui visuals for the stock widgets this window uses (text boxes, combo boxes, number fields, scroll bars).
    pub fn visuals(&self) -> egui::Visuals {
        let mut v = if self.is_dark() {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.panel_fill = self.back;
        v.window_fill = self.surface;
        v.window_stroke = egui::Stroke::new(1.0, self.border);
        v.extreme_bg_color = self.surface2;
        v.text_edit_bg_color = Some(self.surface2);
        v.faint_bg_color = self.surface;
        v.code_bg_color = self.surface2;
        v.hyperlink_color = self.accent;
        v.selection.bg_fill = self.accent_weak;
        v.selection.stroke = egui::Stroke::new(1.0, self.accent);
        v.override_text_color = Some(self.text);
        let w = &mut v.widgets;
        for (state, fill, stroke) in [
            (&mut w.noninteractive, self.back, self.border),
            (&mut w.inactive, self.surface2, self.border_strong),
            (&mut w.hovered, self.accent_weak, self.subtle),
            (&mut w.active, self.accent_weak, self.accent),
            (&mut w.open, self.surface2, self.accent),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = egui::Stroke::new(1.0, stroke);
            state.fg_stroke = egui::Stroke::new(1.0, self.text);
            state.corner_radius = egui::CornerRadius::same(5);
        }
        w.noninteractive.fg_stroke = egui::Stroke::new(1.0, self.text);
        v.popup_shadow = egui::epaint::Shadow::NONE;
        v.window_shadow = egui::epaint::Shadow::NONE;
        v
    }
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Parity row B034 (C# Ui/Theming.cs:57-70).
    #[test]
    fn b034_theme_high_contrast_wins_system_follows_apps_use_light_theme_light_and_dark_are_explicit() {
        for pref in ["System", "Light", "Dark", "", "nonsense"] {
            for dark in [false, true] {
                assert_eq!(resolve(pref, true, dark), ThemeKind::HighContrast, "{pref} {dark}");
            }
        }
        assert_eq!(resolve("Dark", false, false), ThemeKind::Dark);
        assert_eq!(resolve("Light", false, true), ThemeKind::Light);
        assert_eq!(resolve("System", false, true), ThemeKind::Dark);
        assert_eq!(resolve("System", false, false), ThemeKind::Light);
        assert_eq!(
            resolve("unknown", false, true),
            ThemeKind::Dark,
            "anything but Light/Dark follows the system"
        );
        let sys = SystemColors {
            window: rgb(0, 0, 0),
            window_text: rgb(255, 255, 255),
            gray_text: rgb(63, 242, 63),
            highlight: rgb(26, 235, 255),
            control: rgb(0, 0, 0),
            control_text: rgb(255, 255, 255),
        };
        let hc = Theme::for_kind(ThemeKind::HighContrast, &sys);
        assert_eq!(
            (hc.back, hc.text, hc.accent, hc.faint),
            (sys.window, sys.window_text, sys.highlight, sys.gray_text)
        );
        assert!(
            hc.is_dark(),
            "a black high-contrast window reads as dark (C# Back.R < 128)"
        );
        assert_eq!(Theme::for_kind(ThemeKind::Dark, &sys), Theme::DARK);
        assert_eq!(Theme::for_kind(ThemeKind::Light, &sys), Theme::LIGHT);
        assert!(Theme::DARK.is_dark() && !Theme::LIGHT.is_dark());
        assert_eq!(Theme::DARK.sidebar(), rgb(25, 30, 37));
        assert_eq!(Theme::LIGHT.accent_text(), rgb(253, 253, 254));
    }
}
