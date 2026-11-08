//! The settings JSON exactly as 1.1.1 reads and writes it (System.Text.Json defaults in .NET 9).
//!
//! Reading (`JsonSerializer.Deserialize<Settings>`):
//! - strict RFC 8259 text: no comments, no trailing commas, depth at most 64; a U+FEFF before the value is an
//!   error (1.1.1 passes a string whose byte order mark `File.ReadAllText` already removed, see `decode_text`);
//! - members are applied in file order, so a repeated member is applied twice (a bad first value fails even
//!   when a later one is good); property names match case-sensitively; unknown members are skipped unread;
//! - `int`: an integer literal within i32 ("-0" is 0; "4.0", "1e2" and strings fail); `bool`: true/false;
//!   WinForms `Keys`: a number only (no strings, no null); strings and lists accept null;
//! - the three tolerant enums (C# `TolerantEnumConverter`): a name or number `Enum.TryParse` accepts and the
//!   enum defines, else `Unknown`; null, true and false are `Unknown`; an object or array fails;
//! - a string with an unpaired surrogate escape fails only when its value is used (STJ decodes lazily).
//!
//! Writing (`JsonSerializer.Serialize(settings, WriteIndented = true)`): properties in C# declaration order,
//! two-space indentation, CRLF line ends (Environment.NewLine on Windows), `Keys` as numbers, enums as names,
//! and STJ's default escaping: everything outside printable ASCII plus `"&'+<>\`` as `\uXXXX` (uppercase
//! hex), with `\\ \n \r \t \b \f` short forms.

use crate::{RawState, Settings, Stored, StoredRule, UpLink};
use fence_types::{BoundaryDefault, Control, Direction, RuleBehavior};

const MAX_DEPTH: usize = 64;

/// A parsed JSON value. Numbers keep their text; a string with an unpaired surrogate escape is `Err(())`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(Result<String, ()>),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

pub(crate) fn parse(text: &str) -> Result<Json, String> {
    let bytes = text.as_bytes();
    let mut p = Parser {
        s: bytes,
        i: 0,
        depth: 0,
    };
    p.ws();
    if p.i >= p.s.len() {
        return Err("the JSON text is empty".into());
    }
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("unexpected text after the JSON value at byte {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at byte {}", self.i))
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn literal(&mut self, word: &[u8], v: Json) -> Result<Json, String> {
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            self.err("invalid literal")
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal(b"true", Json::Bool(true)),
            Some(b'f') => self.literal(b"false", Json::Bool(false)),
            Some(b'n') => self.literal(b"null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => self.err("expected a JSON value"),
        }
    }

    fn enter(&mut self) -> Result<(), String> {
        if self.depth >= MAX_DEPTH {
            return self.err("JSON nested deeper than 64");
        }
        self.depth += 1;
        self.i += 1;
        Ok(())
    }

    fn object(&mut self) -> Result<Json, String> {
        self.enter()?;
        let mut members = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Json::Obj(members));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return self.err("expected a property name");
            }
            let name = match self.string()? {
                Ok(n) => n,
                Err(()) => return self.err("invalid UTF-16 in a property name"),
            };
            self.ws();
            if self.peek() != Some(b':') {
                return self.err("expected ':'");
            }
            self.i += 1;
            self.ws();
            let v = self.value()?;
            members.push((name, v));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    self.depth -= 1;
                    return Ok(Json::Obj(members));
                }
                _ => return self.err("expected ',' or '}'"),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.enter()?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    self.depth -= 1;
                    return Ok(Json::Arr(items));
                }
                _ => return self.err("expected ',' or ']'"),
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, String> {
        if self.i + 4 > self.s.len() {
            return self.err("short \\u escape");
        }
        let text = std::str::from_utf8(&self.s[self.i..self.i + 4]).map_err(|_| "bad \\u escape".to_string())?;
        if !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return self.err("bad \\u escape");
        }
        self.i += 4;
        u16::from_str_radix(text, 16).map_err(|e| e.to_string())
    }

    /// Reads a string token (the opening quote is at `self.i`). The outer error is a syntax error; the inner
    /// `Err(())` marks a syntactically valid string holding an unpaired surrogate.
    fn string(&mut self) -> Result<Result<String, ()>, String> {
        self.i += 1;
        let mut units: Vec<u16> = Vec::new();
        let mut buf = String::new();
        let mut valid = true;
        loop {
            let Some(b) = self.peek() else {
                return self.err("unterminated string");
            };
            match b {
                b'"' => {
                    self.i += 1;
                    break;
                }
                b'\\' => {
                    self.i += 1;
                    let Some(e) = self.peek() else {
                        return self.err("unterminated escape");
                    };
                    self.i += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{C}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            units.push(self.hex4()?);
                            continue;
                        }
                        _ => return self.err("invalid escape"),
                    };
                    valid &= flush_units(&mut units, &mut buf);
                    buf.push(c);
                }
                0x00..=0x1F => return self.err("control character in a string"),
                _ => {
                    valid &= flush_units(&mut units, &mut buf);
                    // Copy one UTF-8 character; the input is a valid &str, so the lead byte gives its length.
                    let len = match b {
                        0x00..=0x7F => 1,
                        0xC0..=0xDF => 2,
                        0xE0..=0xEF => 3,
                        _ => 4,
                    };
                    let end = (self.i + len).min(self.s.len());
                    let ch = std::str::from_utf8(&self.s[self.i..end]).map_err(|e| e.to_string())?;
                    buf.push_str(ch);
                    self.i = end;
                }
            }
        }
        valid &= flush_units(&mut units, &mut buf);
        Ok(if valid { Ok(buf) } else { Err(()) })
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.i += 1;
                }
            }
            _ => return self.err("invalid number"),
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.err("invalid number");
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.err("invalid number");
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|e| e.to_string())?;
        Ok(Json::Num(text.to_string()))
    }
}

/// Decodes pending `\uXXXX` units into `buf`; false when one is an unpaired surrogate.
fn flush_units(units: &mut Vec<u16>, buf: &mut String) -> bool {
    let mut ok = true;
    for r in char::decode_utf16(units.drain(..)) {
        match r {
            Ok(c) => buf.push(c),
            Err(_) => ok = false,
        }
    }
    ok
}

// ------------------------------------------------------------------------------------------------
// Reading the Settings object
// ------------------------------------------------------------------------------------------------

/// `Utf8JsonReader.TryGetInt32`: an integer literal (no fraction, no exponent) within i32.
pub(crate) fn json_i32(v: &Json) -> Option<i32> {
    match v {
        Json::Num(t) if !t.contains(['.', 'e', 'E']) => t.parse::<i32>().ok(),
        _ => None,
    }
}

fn want_i32(name: &str, v: &Json) -> Result<i32, String> {
    json_i32(v).ok_or_else(|| format!("{name}: not an Int32"))
}

fn want_bool(name: &str, v: &Json) -> Result<bool, String> {
    match v {
        Json::Bool(b) => Ok(*b),
        _ => Err(format!("{name}: not a boolean")),
    }
}

fn want_str(name: &str, v: &Json) -> Result<Option<String>, String> {
    match v {
        Json::Null => Ok(None),
        Json::Str(Ok(s)) => Ok(Some(s.clone())),
        Json::Str(Err(())) => Err(format!("{name}: invalid UTF-16 text")),
        _ => Err(format!("{name}: not a string")),
    }
}

/// A list of strings: null is an empty list; null elements become "" and their positions are returned.
fn want_str_list(name: &str, v: &Json) -> Result<(Vec<String>, Vec<usize>), String> {
    match v {
        Json::Null => Ok((Vec::new(), Vec::new())),
        Json::Arr(items) => {
            let mut out = Vec::with_capacity(items.len());
            let mut nulls = Vec::new();
            for (i, item) in items.iter().enumerate() {
                match want_str(name, item)? {
                    Some(s) => out.push(s),
                    None => {
                        nulls.push(i);
                        out.push(String::new());
                    }
                }
            }
            Ok((out, nulls))
        }
        _ => Err(format!("{name}: not an array")),
    }
}

/// C# `Enum.TryParse<T>(text, ignoreCase: true)` for the small boundary enums (values 0..names.len()).
fn enum_try_parse(text: &str, names: &[&str]) -> Option<i64> {
    let value = text.trim_start();
    let first = value.chars().next()?;
    if first.is_ascii_digit() || first == '-' || first == '+' {
        match parse_enum_number(value) {
            NumberParse::Ok(n) => return Some(n),
            NumberParse::Overflow => return None,
            NumberParse::NotANumber => {}
        }
    }
    // TryParseByName: comma-separated names, each trimmed, OR-ed together.
    let mut result: i64 = 0;
    let mut rest = value;
    while !rest.is_empty() {
        let (part, next) = match rest.find(',') {
            None => (rest.trim(), ""),
            Some(idx) if idx != rest.len() - 1 => (rest[..idx].trim(), &rest[idx + 1..]),
            Some(_) => return None,
        };
        let idx = names
            .iter()
            .position(|n| fence_engine::layout::ordinal_ignore_case_eq(n, part))?;
        result |= idx as i64;
        rest = next;
    }
    Some(result)
}

enum NumberParse {
    Ok(i64),
    Overflow,
    NotANumber,
}

/// .NET `Number.TryParseBinaryIntegerStyle(value, AllowLeadingSign | AllowTrailingWhite, invariant)` for Int32.
fn parse_enum_number(value: &str) -> NumberParse {
    let bytes = value.as_bytes();
    let mut i = 0;
    let mut negative = false;
    if let Some(&s) = bytes.first()
        && (s == b'-' || s == b'+')
    {
        negative = s == b'-';
        i = 1;
    }
    let digits_start = i;
    let mut n: i64 = 0;
    let mut overflow = false;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        n = n * 10 + (bytes[i] - b'0') as i64;
        if n > i32::MAX as i64 + 1 {
            overflow = true;
            n = i32::MAX as i64 + 1;
        }
        i += 1;
    }
    if i == digits_start {
        return NumberParse::NotANumber;
    }
    while i < bytes.len() && matches!(bytes[i], 0x09..=0x0D | 0x20) {
        i += 1;
    }
    if i != bytes.len() {
        return NumberParse::NotANumber;
    }
    let v = if negative { -n } else { n };
    if overflow || v > i32::MAX as i64 || v < i32::MIN as i64 {
        NumberParse::Overflow
    } else {
        NumberParse::Ok(v)
    }
}

/// C# `TolerantEnumConverter<T>.Read`; returns the defined value index (0 = Unknown).
fn tolerant(name: &str, v: &Json, names: &[&str]) -> Result<usize, String> {
    let defined = |n: i64| (0..names.len() as i64).contains(&n).then_some(n as usize);
    match v {
        Json::Str(Ok(s)) => Ok(enum_try_parse(s, names).and_then(defined).unwrap_or(0)),
        Json::Str(Err(())) => Err(format!("{name}: invalid UTF-16 text")),
        Json::Num(_) => Ok(json_i32(v).and_then(|n| defined(n as i64)).unwrap_or(0)),
        Json::Null | Json::Bool(_) => Ok(0),
        Json::Arr(_) | Json::Obj(_) => Err(format!("{name}: the converter cannot read an object or array")),
    }
}

const DIRECTION_NAMES: &[&str] = &["Unknown", "Up", "Down", "Left", "Right"];
const BEHAVIOR_NAMES: &[&str] = &["Unknown", "Free", "Push", "Blocked", "OverallFeel"];
const CONTROL_NAMES: &[&str] = &["Unknown", "Automatic", "Always"];
const DEFAULT_NAMES: &[&str] = &["Unknown", "Automatic", "Free", "Push", "Blocked"];

fn direction_of(i: usize) -> Stored<Direction> {
    match i {
        1 => Stored::Known(Direction::Up),
        2 => Stored::Known(Direction::Down),
        3 => Stored::Known(Direction::Left),
        4 => Stored::Known(Direction::Right),
        _ => Stored::Unknown,
    }
}

fn behavior_of(i: usize) -> Stored<RuleBehavior> {
    match i {
        1 => Stored::Known(RuleBehavior::Free),
        2 => Stored::Known(RuleBehavior::Push),
        3 => Stored::Known(RuleBehavior::Blocked),
        4 => Stored::Known(RuleBehavior::OverallFeel),
        _ => Stored::Unknown,
    }
}

fn control_of(i: usize) -> Stored<Control> {
    match i {
        1 => Stored::Known(Control::Automatic),
        2 => Stored::Known(Control::Always),
        _ => Stored::Unknown,
    }
}

fn default_of(i: usize) -> Stored<BoundaryDefault> {
    match i {
        1 => Stored::Known(BoundaryDefault::Automatic),
        2 => Stored::Known(BoundaryDefault::Free),
        3 => Stored::Known(BoundaryDefault::Push),
        4 => Stored::Known(BoundaryDefault::Blocked),
        _ => Stored::Unknown,
    }
}

pub(crate) fn direction_name(d: &Stored<Direction>) -> &'static str {
    match d {
        Stored::Known(x) => x.name(),
        Stored::Unknown => "Unknown",
    }
}

pub(crate) fn behavior_name(b: &Stored<RuleBehavior>) -> &'static str {
    match b {
        Stored::Known(RuleBehavior::Free) => "Free",
        Stored::Known(RuleBehavior::Push) => "Push",
        Stored::Known(RuleBehavior::Blocked) => "Blocked",
        Stored::Known(RuleBehavior::OverallFeel) => "OverallFeel",
        Stored::Unknown => "Unknown",
    }
}

pub(crate) fn control_name(c: &Stored<Control>) -> &'static str {
    match c {
        Stored::Known(Control::Automatic) => "Automatic",
        Stored::Known(Control::Always) => "Always",
        Stored::Unknown => "Unknown",
    }
}

pub(crate) fn default_name(d: &Stored<BoundaryDefault>) -> &'static str {
    match d {
        Stored::Known(BoundaryDefault::Automatic) => "Automatic",
        Stored::Known(BoundaryDefault::Free) => "Free",
        Stored::Known(BoundaryDefault::Push) => "Push",
        Stored::Known(BoundaryDefault::Blocked) => "Blocked",
        Stored::Unknown => "Unknown",
    }
}

fn read_rule(v: &Json) -> Result<Option<StoredRule>, String> {
    let members = match v {
        Json::Null => return Ok(None),
        Json::Obj(m) => m,
        _ => return Err("BoundaryRules: an element is not an object".into()),
    };
    // C# `new BoundaryRule()`: empty ids, every enum at default(T) = Unknown.
    let mut rule = StoredRule {
        from_stable_id: String::new(),
        to_stable_id: String::new(),
        direction: Stored::Unknown,
        behavior: Stored::Unknown,
        control: Stored::Unknown,
    };
    for (name, value) in members {
        match name.as_str() {
            "FromStableId" => rule.from_stable_id = want_str(name, value)?.unwrap_or_default(),
            "ToStableId" => rule.to_stable_id = want_str(name, value)?.unwrap_or_default(),
            "Direction" => rule.direction = direction_of(tolerant(name, value, DIRECTION_NAMES)?),
            "Behavior" => rule.behavior = behavior_of(tolerant(name, value, BEHAVIOR_NAMES)?),
            "Control" => rule.control = control_of(tolerant(name, value, CONTROL_NAMES)?),
            _ => {}
        }
    }
    Ok(Some(rule))
}

fn read_up_link(v: &Json) -> Result<Option<UpLink>, String> {
    let members = match v {
        Json::Null => return Ok(None),
        Json::Obj(m) => m,
        _ => return Err("UpLinks: an element is not an object".into()),
    };
    let mut link = UpLink::default();
    for (name, value) in members {
        match name.as_str() {
            "FromDevice" => link.from_device = want_str(name, value)?.unwrap_or_default(),
            "ToDevice" => link.to_device = want_str(name, value)?.unwrap_or_default(),
            _ => {}
        }
    }
    Ok(Some(link))
}

/// Apply one top-level member to the settings being read (one C# property setter call).
fn apply_member(s: &mut Settings, name: &str, v: &Json) -> Result<(), String> {
    let keys = |v: &Json| -> Result<u32, String> { Ok(want_i32(name, v)? as u32) };
    match name {
        "SettingsVersion" => {
            s.settings_version = want_i32(name, v)?;
            s.raw.version_specified = true;
        }
        "HotKey" => s.hot_key = keys(v)?,
        "ModCtrl" => s.mod_ctrl = want_bool(name, v)?,
        "ModAlt" => s.mod_alt = want_bool(name, v)?,
        "ModShift" => s.mod_shift = want_bool(name, v)?,
        "ModWin" => s.mod_win = want_bool(name, v)?,
        "ConfineHotKey" => s.confine_hot_key = keys(v)?,
        "ConfineModCtrl" => s.confine_mod_ctrl = want_bool(name, v)?,
        "ConfineModAlt" => s.confine_mod_alt = want_bool(name, v)?,
        "ConfineModShift" => s.confine_mod_shift = want_bool(name, v)?,
        "ConfineModWin" => s.confine_mod_win = want_bool(name, v)?,
        "PauseHotKey" => s.pause_hot_key = keys(v)?,
        "PauseModCtrl" => s.pause_mod_ctrl = want_bool(name, v)?,
        "PauseModAlt" => s.pause_mod_alt = want_bool(name, v)?,
        "PauseModShift" => s.pause_mod_shift = want_bool(name, v)?,
        "PauseModWin" => s.pause_mod_win = want_bool(name, v)?,
        "SideHotKey" => s.side_hot_key = keys(v)?,
        "SideModCtrl" => s.side_mod_ctrl = want_bool(name, v)?,
        "SideModAlt" => s.side_mod_alt = want_bool(name, v)?,
        "SideModShift" => s.side_mod_shift = want_bool(name, v)?,
        "SideModWin" => s.side_mod_win = want_bool(name, v)?,
        "StartGateClosed" => s.start_gate_closed = want_bool(name, v)?,
        "StartConfineOn" => s.start_confine_on = want_bool(name, v)?,
        "StartPaused" => s.start_paused = want_bool(name, v)?,
        "StartSideContainOn" => s.start_side_contain_on = want_bool(name, v)?,
        "AutoStart" => s.auto_start = want_bool(name, v)?,
        "DeliberateCross" => s.deliberate_cross = want_bool(name, v)?,
        "DescentRouting" => s.descent_routing = want_bool(name, v)?,
        "UpwardBoundaryDefault" => s.upward_boundary_default = default_of(tolerant(name, v, DEFAULT_NAMES)?),
        "SideBoundaryDefault" => s.side_boundary_default = default_of(tolerant(name, v, DEFAULT_NAMES)?),
        "DownwardBoundaryDefault" => s.downward_boundary_default = default_of(tolerant(name, v, DEFAULT_NAMES)?),
        "BoundaryRules" => {
            s.boundary_rules = match v {
                Json::Null => Vec::new(),
                // Null elements are dropped: every C# reader of the list skips them (Normalize, Compile).
                Json::Arr(items) => items
                    .iter()
                    .map(read_rule)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect(),
                _ => return Err("BoundaryRules: not an array".into()),
            }
        }
        "UpCrossMin" => s.up_cross_min = want_i32(name, v)?,
        "UpCrossSlack" => s.up_cross_slack = want_i32(name, v)?,
        "PassageInsetPx" => s.passage_inset_px = want_i32(name, v)?,
        "EdgeHoldbackPx" => s.edge_holdback_px = want_i32(name, v)?,
        "SeamTolerancePx" => s.seam_tolerance_px = want_i32(name, v)?,
        "SafetyEscapes" => s.safety_escapes = want_bool(name, v)?,
        "BoundaryTipSeen" => s.boundary_tip_seen = want_bool(name, v)?,
        "SideCrossMin" => s.side_cross_min = want_i32(name, v)?,
        "SideCrossSlack" => s.side_cross_slack = want_i32(name, v)?,
        // A null Mode behaves like any invalid text (Normalize makes it AutoTop).
        "Mode" => s.mode = want_str(name, v)?.unwrap_or_default(),
        "ManualMonitors" => (s.manual_monitors, s.raw.manual_monitor_nulls) = want_str_list(name, v)?,
        "ExcludedDevices" => (s.excluded_devices, s.raw.excluded_device_nulls) = want_str_list(name, v)?,
        "MonitorNames" => {
            s.monitor_names = match v {
                Json::Null => Vec::new(),
                Json::Obj(members) => {
                    // Dictionary indexer: a repeated key replaces the value in place.
                    let mut out: Vec<(String, String)> = Vec::new();
                    for (key, value) in members {
                        let text = want_str(name, value)?.unwrap_or_default();
                        match out.iter_mut().find(|(k, _)| k == key) {
                            Some(slot) => slot.1 = text,
                            None => out.push((key.clone(), text)),
                        }
                    }
                    out
                }
                _ => return Err("MonitorNames: not an object".into()),
            }
        }
        "Language" => {
            let value = want_str(name, v)?;
            s.raw.language_null = value.is_none();
            s.language = value.unwrap_or_default();
        }
        "Theme" => {
            let value = want_str(name, v)?;
            s.raw.theme_null = value.is_none();
            s.theme = value.unwrap_or_default();
        }
        "CrossingPathMode" => {
            let value = want_str(name, v)?;
            s.raw.crossing_path_mode_null = value.is_none();
            s.legacy_crossing_path_mode = value;
        }
        "UpLinks" => {
            let (links, nulls) = match v {
                Json::Null => (Vec::new(), 0),
                Json::Arr(items) => {
                    let read = items.iter().map(read_up_link).collect::<Result<Vec<_>, _>>()?;
                    let nulls = read.iter().filter(|l| l.is_none()).count();
                    (read.into_iter().flatten().collect(), nulls)
                }
                _ => return Err("UpLinks: not an array".into()),
            };
            s.legacy_up_links = links;
            s.raw.legacy_up_link_nulls = nulls;
        }
        _ => {}
    }
    Ok(())
}

/// C# `JsonSerializer.Deserialize<Settings>(text)`: `Ok(None)` for a JSON `null` document.
pub(crate) fn read_settings(text: &str) -> Result<Option<Settings>, String> {
    match parse(text)? {
        Json::Null => Ok(None),
        Json::Obj(members) => {
            let mut s = Settings::unspecified();
            for (name, value) in &members {
                apply_member(&mut s, name, value)?;
            }
            Ok(Some(s))
        }
        _ => Err("the settings JSON is not an object".into()),
    }
}

pub(crate) fn read_settings_v2(text: &str) -> Result<Option<Settings>, String> {
    let mut settings = read_settings(text)?;
    if let (Some(s), Json::Obj(members)) = (&mut settings, parse(text)?) {
        let Out::Obj(known) = settings_tree(&Settings::unspecified()) else {
            unreachable!()
        };
        for (name, value) in members {
            match name.as_str() {
                "CrossingEase" => s.crossing_ease = want_i32(&name, &value)?,
                "CrossingModel" => {
                    s.crossing_model =
                        fence_types::CrossingModel::parse(want_str(&name, &value)?.as_deref().unwrap_or_default())
                }
                "UpPushDistancePx" => s.up_push_distance_px = want_i32(&name, &value)?,
                "SidePushDistancePx" => s.side_push_distance_px = want_i32(&name, &value)?,
                "PushAbsorbMs" => s.push_absorb_ms = want_i32(&name, &value)?,
                "PushPauseResetMs" => s.push_pause_reset_ms = want_i32(&name, &value)?,
                "SteadyPushMs" => s.steady_push_ms = want_i32(&name, &value)?,
                "EdgeTrace" => s.edge_trace = want_bool(&name, &value)?,
                "MarkAccidentHotKey" => s.mark_accident_hot_key = want_i32(&name, &value)? as u32,
                "MarkAccidentModCtrl" => s.mark_accident_mod_ctrl = want_bool(&name, &value)?,
                "MarkAccidentModAlt" => s.mark_accident_mod_alt = want_bool(&name, &value)?,
                "MarkAccidentModShift" => s.mark_accident_mod_shift = want_bool(&name, &value)?,
                "MarkAccidentModWin" => s.mark_accident_mod_win = want_bool(&name, &value)?,
                _ => {
                    if !known.iter().any(|(key, _)| key == &name) {
                        s.raw.unknown_members.push((name, preserved_value(value)?));
                    }
                }
            }
        }
    }
    Ok(settings)
}

fn preserved_value(value: Json) -> Result<serde_json::Value, String> {
    use serde_json::Value;
    Ok(match value {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(b),
        Json::Num(n) => serde_json::from_str(&n).map_err(|e| e.to_string())?,
        Json::Str(s) => Value::String(s.map_err(|()| "invalid UTF-16 in unknown member")?),
        Json::Arr(items) => Value::Array(items.into_iter().map(preserved_value).collect::<Result<_, _>>()?),
        Json::Obj(members) => Value::Object(
            members
                .into_iter()
                .map(|(k, v)| Ok((k, preserved_value(v)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}

/// C# `Settings.ReadVersion(json)`: the last `SettingsVersion` member when it is an Int32, else 0.
pub(crate) fn read_version(text: &str) -> i32 {
    match parse(text) {
        Ok(Json::Obj(members)) => members
            .iter()
            .rev()
            .find(|(n, _)| n == "SettingsVersion")
            .and_then(|(_, v)| json_i32(v))
            .unwrap_or(0),
        _ => 0,
    }
}

// ------------------------------------------------------------------------------------------------
// Writing
// ------------------------------------------------------------------------------------------------

enum Out {
    Raw(String),
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<Out>),
    Obj(Vec<(String, Out)>),
}

const NEWLINE: &str = "\r\n";

fn escape_into(out: &mut String, text: &str) {
    for unit in text.encode_utf16() {
        match unit {
            0x5C => out.push_str("\\\\"),
            0x0A => out.push_str("\\n"),
            0x0D => out.push_str("\\r"),
            0x09 => out.push_str("\\t"),
            0x08 => out.push_str("\\b"),
            0x0C => out.push_str("\\f"),
            0x22 | 0x26 | 0x27 | 0x2B | 0x3C | 0x3E | 0x60 => out.push_str(&format!("\\u{unit:04X}")),
            0x20..=0x7E => out.push(unit as u8 as char),
            _ => out.push_str(&format!("\\u{unit:04X}")),
        }
    }
}

fn write_value(out: &mut String, v: &Out, indented: bool, depth: usize) {
    let newline_indent = |out: &mut String, d: usize| {
        if indented {
            out.push_str(NEWLINE);
            for _ in 0..d {
                out.push_str("  ");
            }
        }
    };
    match v {
        Out::Raw(text) => out.push_str(text),
        Out::Null => out.push_str("null"),
        Out::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Out::Int(n) => out.push_str(&n.to_string()),
        Out::Str(s) => {
            out.push('"');
            escape_into(out, s);
            out.push('"');
        }
        Out::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline_indent(out, depth + 1);
                write_value(out, item, indented, depth + 1);
            }
            if !items.is_empty() {
                newline_indent(out, depth);
            }
            out.push(']');
        }
        Out::Obj(members) => {
            out.push('{');
            for (i, (name, item)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline_indent(out, depth + 1);
                out.push('"');
                escape_into(out, name);
                out.push_str(if indented { "\": " } else { "\":" });
                write_value(out, item, indented, depth + 1);
            }
            if !members.is_empty() {
                newline_indent(out, depth);
            }
            out.push('}');
        }
    }
}

fn nullable_list(items: &[String], nulls: &[usize]) -> Out {
    Out::Arr(
        items
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if v.is_empty() && nulls.contains(&i) {
                    Out::Null
                } else {
                    Out::Str(v.clone())
                }
            })
            .collect(),
    )
}

fn nullable_str(value: &str, was_null: bool) -> Out {
    if was_null && value.is_empty() {
        Out::Null
    } else {
        Out::Str(value.to_string())
    }
}

/// The C# property list in declaration order with the values 1.1.1 would write.
fn settings_tree(s: &Settings) -> Out {
    let raw: &RawState = &s.raw;
    let int = |v: i32| Out::Int(v as i64);
    let keys = |v: u32| Out::Int(v as i32 as i64);
    let mut m: Vec<(String, Out)> = Vec::with_capacity(56);
    let mut put = |name: &str, v: Out| m.push((name.to_string(), v));
    put("SettingsVersion", int(s.settings_version));
    put("HotKey", keys(s.hot_key));
    put("ModCtrl", Out::Bool(s.mod_ctrl));
    put("ModAlt", Out::Bool(s.mod_alt));
    put("ModShift", Out::Bool(s.mod_shift));
    put("ModWin", Out::Bool(s.mod_win));
    put("ConfineHotKey", keys(s.confine_hot_key));
    put("ConfineModCtrl", Out::Bool(s.confine_mod_ctrl));
    put("ConfineModAlt", Out::Bool(s.confine_mod_alt));
    put("ConfineModShift", Out::Bool(s.confine_mod_shift));
    put("ConfineModWin", Out::Bool(s.confine_mod_win));
    put("PauseHotKey", keys(s.pause_hot_key));
    put("PauseModCtrl", Out::Bool(s.pause_mod_ctrl));
    put("PauseModAlt", Out::Bool(s.pause_mod_alt));
    put("PauseModShift", Out::Bool(s.pause_mod_shift));
    put("PauseModWin", Out::Bool(s.pause_mod_win));
    put("SideHotKey", keys(s.side_hot_key));
    put("SideModCtrl", Out::Bool(s.side_mod_ctrl));
    put("SideModAlt", Out::Bool(s.side_mod_alt));
    put("SideModShift", Out::Bool(s.side_mod_shift));
    put("SideModWin", Out::Bool(s.side_mod_win));
    put("StartGateClosed", Out::Bool(s.start_gate_closed));
    put("StartConfineOn", Out::Bool(s.start_confine_on));
    put("StartPaused", Out::Bool(s.start_paused));
    put("StartSideContainOn", Out::Bool(s.start_side_contain_on));
    put("AutoStart", Out::Bool(s.auto_start));
    put("DeliberateCross", Out::Bool(s.deliberate_cross));
    put("DescentRouting", Out::Bool(s.descent_routing));
    put(
        "UpwardBoundaryDefault",
        Out::Str(default_name(&s.upward_boundary_default).into()),
    );
    put(
        "SideBoundaryDefault",
        Out::Str(default_name(&s.side_boundary_default).into()),
    );
    put(
        "DownwardBoundaryDefault",
        Out::Str(default_name(&s.downward_boundary_default).into()),
    );
    put(
        "BoundaryRules",
        Out::Arr(
            s.boundary_rules
                .iter()
                .map(|r| {
                    Out::Obj(vec![
                        ("FromStableId".into(), Out::Str(r.from_stable_id.clone())),
                        ("ToStableId".into(), Out::Str(r.to_stable_id.clone())),
                        ("Direction".into(), Out::Str(direction_name(&r.direction).into())),
                        ("Behavior".into(), Out::Str(behavior_name(&r.behavior).into())),
                        ("Control".into(), Out::Str(control_name(&r.control).into())),
                    ])
                })
                .collect(),
        ),
    );
    put("UpCrossMin", int(s.up_cross_min));
    put("UpCrossSlack", int(s.up_cross_slack));
    put("PassageInsetPx", int(s.passage_inset_px));
    put("EdgeHoldbackPx", int(s.edge_holdback_px));
    put("SeamTolerancePx", int(s.seam_tolerance_px));
    put("SafetyEscapes", Out::Bool(s.safety_escapes));
    put("BoundaryTipSeen", Out::Bool(s.boundary_tip_seen));
    put("SideCrossMin", int(s.side_cross_min));
    put("SideCrossSlack", int(s.side_cross_slack));
    put("Mode", Out::Str(s.mode.clone()));
    put(
        "ManualMonitors",
        nullable_list(&s.manual_monitors, &raw.manual_monitor_nulls),
    );
    put(
        "ExcludedDevices",
        nullable_list(&s.excluded_devices, &raw.excluded_device_nulls),
    );
    put(
        "MonitorNames",
        Out::Obj(
            s.monitor_names
                .iter()
                .map(|(k, v)| (k.clone(), Out::Str(v.clone())))
                .collect(),
        ),
    );
    put("Language", nullable_str(&s.language, raw.language_null));
    put("Theme", nullable_str(&s.theme, raw.theme_null));
    // Legacy members: C# getters return null (omitted) once SettingsVersion >= 4, which Normalize always
    // reaches, so 1.1.1 never writes them in practice. Kept for completeness.
    if s.settings_version < crate::CURRENT_SETTINGS_VERSION {
        match &s.legacy_crossing_path_mode {
            Some(mode) => put("CrossingPathMode", Out::Str(mode.clone())),
            None if !raw.crossing_path_mode_null => put("CrossingPathMode", Out::Str("Default".into())),
            None => {}
        }
        let mut links: Vec<Out> = s
            .legacy_up_links
            .iter()
            .map(|l| {
                Out::Obj(vec![
                    ("FromDevice".into(), Out::Str(l.from_device.clone())),
                    ("ToDevice".into(), Out::Str(l.to_device.clone())),
                ])
            })
            .collect();
        links.extend((0..raw.legacy_up_link_nulls).map(|_| Out::Null));
        put("UpLinks", Out::Arr(links));
    }
    Out::Obj(m)
}

/// C# `JsonSerializer.Serialize(settings, new JsonSerializerOptions { WriteIndented = indented })`.
pub(crate) fn write_settings(s: &Settings, indented: bool) -> String {
    let mut out = String::with_capacity(2048);
    write_value(&mut out, &settings_tree(s), indented, 0);
    out
}

pub(crate) fn write_settings_v2(s: &Settings) -> String {
    let Out::Obj(mut members) = settings_tree(s) else {
        unreachable!()
    };
    members.extend([
        ("CrossingEase".into(), Out::Int(s.crossing_ease as i64)),
        ("CrossingModel".into(), Out::Str(s.crossing_model.name().into())),
        ("UpPushDistancePx".into(), Out::Int(s.up_push_distance_px as i64)),
        ("SidePushDistancePx".into(), Out::Int(s.side_push_distance_px as i64)),
        ("PushAbsorbMs".into(), Out::Int(s.push_absorb_ms as i64)),
        ("PushPauseResetMs".into(), Out::Int(s.push_pause_reset_ms as i64)),
        ("SteadyPushMs".into(), Out::Int(s.steady_push_ms as i64)),
        ("EdgeTrace".into(), Out::Bool(s.edge_trace)),
        (
            "MarkAccidentHotKey".into(),
            Out::Int(s.mark_accident_hot_key as i32 as i64),
        ),
        ("MarkAccidentModCtrl".into(), Out::Bool(s.mark_accident_mod_ctrl)),
        ("MarkAccidentModAlt".into(), Out::Bool(s.mark_accident_mod_alt)),
        ("MarkAccidentModShift".into(), Out::Bool(s.mark_accident_mod_shift)),
        ("MarkAccidentModWin".into(), Out::Bool(s.mark_accident_mod_win)),
    ]);
    let known: Vec<_> = members.iter().map(|(name, _)| name.clone()).collect();
    for (name, value) in &s.raw.unknown_members {
        if !known.iter().any(|key| key == name) {
            members.push((name.clone(), Out::Raw(value.to_string())));
        }
    }
    let mut out = String::with_capacity(2048);
    write_value(&mut out, &Out::Obj(members), true, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_config_json_reader_is_strict_like_system_text_json() {
        for bad in [
            "",
            " ",
            "{",
            "{\"a\":1,}",
            "[1,]",
            "{/*c*/}",
            "01",
            "1.",
            ".5",
            "+1",
            "{\"a\" 1}",
            "nul",
            "\"\u{1}\"",
            "{} {}",
            "'a'",
            "\"\\x\"",
            "\u{FEFF}{}",
        ] {
            assert!(parse(bad).is_err(), "{bad:?} must fail");
        }
        for good in ["{}", "[]", "null", "-0", "1e400", "\"\\ud800\"", " {\"a\":[1,2,{}]} "] {
            assert!(parse(good).is_ok(), "{good:?} must parse");
        }
        assert_eq!(parse("\"\\ud800\"").unwrap(), Json::Str(Err(())));
        assert_eq!(parse("\"\\ud83d\\ude00\"").unwrap(), Json::Str(Ok("\u{1F600}".into())));
        let deep_ok = format!("{}{}", "[".repeat(64), "]".repeat(64));
        let deep_bad = format!("{}{}", "[".repeat(65), "]".repeat(65));
        assert!(parse(&deep_ok).is_ok());
        assert!(parse(&deep_bad).is_err());
    }

    #[test]
    fn s_config_int32_rule_follows_utf8_json_reader() {
        let n = |t: &str| json_i32(&Json::Num(t.into()));
        assert_eq!(n("-0"), Some(0));
        assert_eq!(n("2147483647"), Some(i32::MAX));
        assert_eq!(n("2147483648"), None);
        assert_eq!(n("4.0"), None);
        assert_eq!(n("1e2"), None);
    }

    #[test]
    fn s_config_tolerant_enum_follows_enum_try_parse() {
        let d = |t: &str| tolerant("x", &Json::Str(Ok(t.into())), DIRECTION_NAMES).unwrap();
        assert_eq!(d("Up"), 1);
        assert_eq!(d("up"), 1);
        assert_eq!(d(" Right "), 4);
        assert_eq!(d("Up, Down"), 3, "flags OR: 1 | 2 = Left");
        assert_eq!(d("Right, Up"), 0, "4 | 1 = 5 is not defined");
        assert_eq!(d("2"), 2);
        assert_eq!(d("+2 "), 2);
        assert_eq!(d("9"), 0);
        assert_eq!(d("Up,"), 0);
        assert_eq!(d("R\u{131}ght"), 0, "dotless i is not i");
        assert_eq!(d(""), 0);
        assert_eq!(d("99999999999"), 0);
        let num = |t: &str| tolerant("x", &Json::Num(t.into()), DIRECTION_NAMES).unwrap();
        assert_eq!(num("3"), 3);
        assert_eq!(num("3.0"), 0);
        assert_eq!(num("7"), 0);
        assert_eq!(tolerant("x", &Json::Null, DIRECTION_NAMES).unwrap(), 0);
        assert!(tolerant("x", &Json::Obj(vec![]), DIRECTION_NAMES).is_err());
    }

    #[test]
    fn s_config_escaping_matches_system_text_json() {
        let mut out = String::new();
        escape_into(&mut out, "<&>'+`\"\\/\u{7F}\u{1}\u{E9}\u{1F600}\n\r\t\u{8}\u{C}a");
        assert_eq!(
            out,
            "\\u003C\\u0026\\u003E\\u0027\\u002B\\u0060\\u0022\\\\/\\u007F\\u0001\\u00E9\\uD83D\\uDE00\\n\\r\\t\\b\\fa"
        );
    }
}
