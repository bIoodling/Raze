#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Cps,
    Kps,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behavior {
    Toggle,
    Hold,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindKind {
    None,
    Keyboard,
    Mouse,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureTarget {
    None,
    Spam,
    Macro,
    Ui,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    pub kind: BindKind,
    pub vk: u16,
    pub scan: u16,
    pub extended: bool,
    pub mouse_button: u8,
}
impl KeyBinding {
    pub const fn none() -> Self {
        Self {
            kind: BindKind::None,
            vk: 0,
            scan: 0,
            extended: false,
            mouse_button: 0,
        }
    }
    pub const fn keyboard(vk: u16) -> Self {
        Self {
            kind: BindKind::Keyboard,
            vk,
            scan: 0,
            extended: false,
            mouse_button: 0,
        }
    }
    pub const fn mouse(button: u8) -> Self {
        Self {
            kind: BindKind::Mouse,
            vk: 0,
            scan: 0,
            extended: false,
            mouse_button: button,
        }
    }
    pub fn is_none(self) -> bool {
        self.kind == BindKind::None
    }
    pub fn same_key(self, other: Self) -> bool {
        self.kind == other.kind
            && match self.kind {
                BindKind::None => true,
                BindKind::Keyboard => self.vk == other.vk,
                BindKind::Mouse => self.mouse_button == other.mouse_button,
            }
    }
}
pub const MAX_RATE: u32 = 1400;
#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub mode: Mode,
    pub rate: u32,
    pub spam: KeyBinding,
    pub macro_key: KeyBinding,
    pub ui_key: KeyBinding,
    pub behavior: Behavior,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Cps,
            rate: 500,
            spam: KeyBinding::none(),
            macro_key: KeyBinding::none(),
            ui_key: KeyBinding::none(),
            behavior: Behavior::Hold,
        }
    }
}
pub fn safe_binding(b: KeyBinding) -> bool {
    match b.kind {
        BindKind::None => true,
        BindKind::Mouse => (3..=5).contains(&b.mouse_button),
        // Deliberate allowlist: letters, digits, numpad digits/operators, F1–F11.
        // F12 is reserved for debuggers. Navigation, lock, media, power, Windows,
        // modifiers, Escape and shell keys are excluded.
        BindKind::Keyboard => matches!(b.vk, 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0x70..=0x7A),
    }
}
pub fn can_activate(c: Config) -> bool {
    !c.macro_key.is_none()
        && !c.ui_key.is_none()
        && (c.mode == Mode::Cps || !c.spam.is_none())
        && safe_binding(c.spam)
        && safe_binding(c.macro_key)
        && safe_binding(c.ui_key)
        && !c.macro_key.same_key(c.ui_key)
        && (c.spam.is_none() || (!c.spam.same_key(c.macro_key) && !c.spam.same_key(c.ui_key)))
}
pub fn assign(c: &mut Config, target: CaptureTarget, b: KeyBinding) -> Result<(), &'static str> {
    if !safe_binding(b) {
        return Err("Use A–Z, 0–9, F1–F11 or Mouse3–5");
    }
    // Derive scan codes from the validated VK at injection time; never trust persisted scan metadata.
    let b = if b.kind == BindKind::Keyboard { KeyBinding::keyboard(b.vk) } else { b };
    let others = match target {
        CaptureTarget::Spam => [c.macro_key, c.ui_key],
        CaptureTarget::Macro => [c.spam, c.ui_key],
        CaptureTarget::Ui => [c.spam, c.macro_key],
        CaptureTarget::None => return Err("Select a binding first"),
    };
    if !b.is_none() && others.iter().any(|x| !x.is_none() && x.same_key(b)) {
        return Err("That key is already assigned");
    }
    match target {
        CaptureTarget::Spam => c.spam = b,
        CaptureTarget::Macro => c.macro_key = b,
        CaptureTarget::Ui => c.ui_key = b,
        CaptureTarget::None => {}
    }
    Ok(())
}
pub fn parse_rate(s: &str) -> Result<u32, &'static str> {
    let v = s
        .trim()
        .parse::<u32>()
        .map_err(|_| "Enter a whole number from 1 to 1400")?;
    if (1..=MAX_RATE).contains(&v) {
        Ok(v)
    } else {
        Err("Rate must be between 1 and 1400")
    }
}
pub fn binding_string(b: KeyBinding) -> String {
    match b.kind {
        BindKind::None => "NONE".into(),
        BindKind::Mouse => format!("M{}", b.mouse_button),
        BindKind::Keyboard => format!("K:{:04X}:{:04X}:{}", b.vk, b.scan, u8::from(b.extended)),
    }
}
pub fn parse_binding(s: &str) -> Option<KeyBinding> {
    if s == "NONE" {
        return Some(KeyBinding::none());
    }
    let b = if let Some(n) = s.strip_prefix('M') {
        KeyBinding::mouse(n.parse().ok()?)
    } else {
        let p: Vec<_> = s.split(':').collect();
        if p.len() != 4 || p[0] != "K" || !matches!(p[3], "0" | "1") {
            return None;
        }
        KeyBinding {
            kind: BindKind::Keyboard,
            vk: u16::from_str_radix(p[1], 16).ok()?,
            scan: u16::from_str_radix(p[2], 16).ok()?,
            extended: p[3] == "1",
            mouse_button: 0,
        }
    };
    safe_binding(b).then_some(b)
}
pub fn decode_config(s: &str) -> Config {
    let mut c = Config::default();
    for line in s.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k {
            "mode" => c.mode = if v == "KPS" { Mode::Kps } else { Mode::Cps },
            "rate" => {
                if let Ok(v) = parse_rate(v) {
                    c.rate = v;
                }
            }
            "behavior" => {
                c.behavior = if v == "Toggle" {
                    Behavior::Toggle
                } else {
                    Behavior::Hold
                }
            }
            "spam" | "macro" | "ui" => {
                if let Some(b) = parse_binding(v) {
                    let t = match k {
                        "spam" => CaptureTarget::Spam,
                        "macro" => CaptureTarget::Macro,
                        _ => CaptureTarget::Ui,
                    };
                    let _ = assign(&mut c, t, b);
                }
            }
            _ => {}
        }
    }
    c
}
pub fn encode_config(c: Config) -> String {
    format!(
        "mode={}\nrate={}\nspam={}\nmacro={}\nui={}\nbehavior={}\n",
        if c.mode == Mode::Cps { "CPS" } else { "KPS" },
        c.rate,
        binding_string(c.spam),
        binding_string(c.macro_key),
        binding_string(c.ui_key),
        if c.behavior == Behavior::Hold {
            "Hold"
        } else {
            "Toggle"
        }
    )
}
pub fn dev_key_valid(key: &str) -> bool {
    cfg!(feature = "dev-activation") && key.trim() == "RAZE-DEV-2026"
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_install_is_unassigned_and_inactive() {
        let c = Config::default();
        assert!(c.spam.is_none() && c.macro_key.is_none() && c.ui_key.is_none());
        assert!(!can_activate(c));
        assert_eq!(c.rate, 500);
    }
    #[test]
    fn unsafe_inputs_are_rejected() {
        for vk in [
            0, 1, 2, 8, 9, 13, 16, 17, 18, 19, 20, 27, 32, 0x5B, 0x5C, 0x5D, 0x7B, 0x90, 0x91,
            0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xAD, 0xFF,
        ] {
            assert!(!safe_binding(KeyBinding::keyboard(vk)), "{vk}");
        }
        assert!(!safe_binding(KeyBinding::mouse(1)));
        assert!(!safe_binding(KeyBinding::mouse(2)));
        for n in 3..=5 {
            assert!(safe_binding(KeyBinding::mouse(n)));
        }
    }
    #[test]
    fn duplicate_identity_ignores_scan_metadata() {
        let mut c = Config::default();
        assign(&mut c, CaptureTarget::Spam, KeyBinding::keyboard(65)).unwrap();
        let b = KeyBinding {
            scan: 30,
            extended: true,
            ..KeyBinding::keyboard(65)
        };
        assert!(assign(&mut c, CaptureTarget::Macro, b).is_err());
        assign(&mut c, CaptureTarget::Spam, KeyBinding::none()).unwrap();
        assign(&mut c, CaptureTarget::Macro, b).unwrap();
    }
    #[test]
    fn saved_scan_metadata_cannot_inject_a_different_key() {
        let mut c = Config::default();
        let b = parse_binding("K:0041:005B:1").unwrap();
        assign(&mut c, CaptureTarget::Spam, b).unwrap();
        assert_eq!(c.spam, KeyBinding::keyboard(65));
    }
    #[test]
    fn rate_boundaries_and_paste() {
        for s in ["", "0", "1401", "-1", "1.5", "500cps", "42949672960"] {
            assert!(parse_rate(s).is_err());
        }
        for (s, n) in [("1", 1), ("1400", 1400), (" 950 ", 950)] {
            assert_eq!(parse_rate(s).unwrap(), n);
        }
    }
    #[test]
    fn persistence_round_trip() {
        let mut c = Config {
            mode: Mode::Kps,
            rate: 1400,
            ..Config::default()
        };
        assign(&mut c, CaptureTarget::Spam, KeyBinding::keyboard(65)).unwrap();
        assign(&mut c, CaptureTarget::Macro, KeyBinding::mouse(4)).unwrap();
        assign(&mut c, CaptureTarget::Ui, KeyBinding::keyboard(0x70)).unwrap();
        let d = decode_config(&encode_config(c));
        assert_eq!(encode_config(c), encode_config(d));
        assert!(can_activate(d));
    }
    #[test]
    fn corrupted_or_unsafe_saved_bindings_fail_closed() {
        let c = decode_config("spam=M1\nmacro=K:005B:0000:0\nui=M2\nrate=999999\n");
        assert!(!can_activate(c));
        assert!(c.spam.is_none());
        assert_eq!(c.rate, 500);
        assert!(parse_binding("K:0041:001E:2").is_none());
    }
    #[test]
    fn both_modes_require_control_bindings() {
        let mut c = Config { macro_key: KeyBinding::mouse(4), ..Config::default() };
        assert!(!can_activate(c));
        c.ui_key = KeyBinding::mouse(5);
        assert!(can_activate(c));
        c.mode = Mode::Kps;
        assert!(!can_activate(c));
        c.spam = KeyBinding::keyboard(65);
        assert!(can_activate(c));
    }
    #[test]
    fn development_activation_is_explicit() {
        assert_eq!(
            dev_key_valid("RAZE-DEV-2026"),
            cfg!(feature = "dev-activation")
        );
        assert!(!dev_key_valid("anything"));
        assert!(!dev_key_valid(""));
    }
}
