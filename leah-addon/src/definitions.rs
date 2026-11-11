//! Decoder definitions (design/model_railroading.md, "Phase A, item 4"): a decoder's settings by name,
//! each mapped to its CVs and bits, with its kind, range, choices, group, and when it applies. Leah's
//! own format, written from the NMRA's standards and the makers' published manuals; the NMRA's
//! standard set (S-9.2.2) is built in, and every maker's definition starts from it.

use std::collections::BTreeMap;

use leah_addon_kit::{Call, Error, Key, Object, Value};

/// What a setting's value is.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A number from its bits (several CVs: whole bytes, the first the most significant).
    Number,
    /// One of a few values, each with words: the words are its value.
    Choice(Vec<(i64, String)>),
    /// One bit: true or false.
    Flag,
    /// Bit 7 the sign, bits 0 to 6 the amount (S-9.2.2's CV 23 and 24): -127 to 127.
    Signed,
    /// CV 17 and 18's long address (S-9.2.2): 1 to 10239.
    LongAddress,
}

impl Kind {
    fn word(&self) -> &'static str {
        match self {
            Kind::Number => "number",
            Kind::Choice(_) => "choice",
            Kind::Flag => "flag",
            Kind::Signed => "signed",
            Kind::LongAddress => "long_address",
        }
    }
}

/// When a setting applies: another setting's value compared.
#[derive(Clone, Debug, PartialEq)]
pub struct Condition {
    pub setting: String,
    pub op: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub name: String,
    pub cvs: Vec<u16>,
    /// The first bit and how many, within its one CV (all 8 for several CVs).
    pub first: u8,
    pub count: u8,
    pub kind: Kind,
    pub min: i64,
    pub max: i64,
    pub default: Option<Value>,
    pub group: String,
    pub help: String,
    pub read_only: bool,
    /// Left out of "write everything" (the decoder lock's CVs, and any the definition says).
    pub write_all: bool,
    pub shown_when: Option<Condition>,
    /// CV 31 and 32 for an indexed setting.
    pub index: Option<(u8, u8)>,
}

impl Setting {
    fn new(name: &str, cv: u16, group: &str) -> Setting {
        Setting { name: name.into(), cvs: vec![cv], first: 0, count: 8, kind: Kind::Number, min: 0, max: 255, default: None, group: group.into(), help: String::new(), read_only: false, write_all: true, shown_when: None, index: None }
    }

    fn bits(mut self, first: u8, count: u8) -> Setting {
        self.first = first;
        self.count = count;
        self.max = (1 << count) - 1;
        self
    }

    fn flag(self, bit: u8) -> Setting {
        let mut s = self.bits(bit, 1);
        s.kind = Kind::Flag;
        s
    }

    fn choice(mut self, choices: &[(i64, &str)]) -> Setting {
        self.kind = Kind::Choice(choices.iter().map(|(v, w)| (*v, (*w).to_string())).collect());
        self
    }

    fn range(mut self, min: i64, max: i64) -> Setting {
        self.min = min;
        self.max = max;
        self
    }

    fn help(mut self, help: &str) -> Setting {
        self.help = help.into();
        self
    }

    fn default(mut self, v: impl Into<Value>) -> Setting {
        self.default = Some(v.into());
        self
    }

    fn read_only(mut self) -> Setting {
        self.read_only = true;
        self.write_all = false;
        self
    }

    fn when(mut self, setting: &str, value: impl Into<Value>) -> Setting {
        self.shown_when = Some(Condition { setting: setting.into(), op: "==".into(), value: value.into() });
        self
    }

    /// The bits it takes in its CV, as a mask.
    fn mask(&self) -> u8 {
        (((1u16 << self.count) - 1) << self.first) as u8
    }

    /// Its value from the CVs' bytes (all its CVs known), in its own form.
    pub fn value_from(&self, cvs: &BTreeMap<u16, u8>) -> Option<Value> {
        let bytes: Vec<u8> = self.cvs.iter().map(|cv| cvs.get(cv).copied()).collect::<Option<_>>()?;
        let raw = if bytes.len() > 1 { bytes.iter().fold(0i64, |n, b| n << 8 | i64::from(*b)) } else { i64::from((bytes[0] & self.mask()) >> self.first) };
        Some(match &self.kind {
            Kind::Number => Value::from(raw),
            Kind::Flag => Value::from(raw != 0),
            Kind::Signed => Value::from(if raw & 0x80 != 0 { -(raw & 0x7F) } else { raw & 0x7F }),
            Kind::LongAddress => Value::from(((i64::from(bytes[0]) - 192) << 8 | i64::from(bytes[1])).max(0)),
            Kind::Choice(choices) => choices.iter().find(|(v, _)| *v == raw).map_or(Value::from(raw), |(_, w)| Value::from(w.as_str())),
        })
    }

    /// The raw number a value given by a script stands for, checked: `Err` says why not.
    pub fn raw_of(&self, value: &Value) -> Result<i64, String> {
        let wrong = |wanted: &str| format!("the setting \"{}\" takes {wanted}, not {}", self.name, crate::base::describe(value));
        match &self.kind {
            Kind::Flag => match value {
                Value::Bool(b) => Ok(i64::from(*b)),
                v => match v.as_int() {
                    Some(n @ 0..=1) => Ok(n),
                    _ => Err(wrong("true or false")),
                },
            },
            Kind::Choice(choices) => {
                if let Some(t) = value.as_text() {
                    return choices.iter().find(|(_, w)| w.eq_ignore_ascii_case(t)).map(|(v, _)| *v).ok_or_else(|| wrong(&choice_words(choices)));
                }
                match value.as_int() {
                    Some(n) if choices.iter().any(|(v, _)| *v == n) => Ok(n),
                    _ => Err(wrong(&choice_words(choices))),
                }
            }
            Kind::Signed => match value.as_int() {
                Some(n @ -127..=127) => Ok(if n < 0 { 0x80 | -n } else { n }),
                _ => Err(wrong("a whole number from -127 to 127")),
            },
            Kind::Number | Kind::LongAddress => match value.as_int() {
                Some(n) if (self.min..=self.max).contains(&n) => Ok(n),
                _ => Err(wrong(&format!("a whole number from {} to {}", self.min, self.max))),
            },
        }
    }

    /// The CVs' new bytes for a value: `cvs` gives the bytes now, for the bits it leaves as they are.
    pub fn bytes_for(&self, value: &Value, cvs: &BTreeMap<u16, u8>) -> Result<Vec<(u16, u8)>, String> {
        let raw = self.raw_of(value)?;
        if self.kind == Kind::LongAddress {
            return Ok(vec![(self.cvs[0], (192 + (raw >> 8)) as u8), (self.cvs[1], (raw & 0xFF) as u8)]);
        }
        if self.cvs.len() > 1 {
            let n = self.cvs.len();
            return Ok(self.cvs.iter().enumerate().map(|(i, cv)| (*cv, (raw >> (8 * (n - 1 - i))) as u8)).collect());
        }
        let cv = self.cvs[0];
        let old = if self.count == 8 {
            0
        } else {
            *cvs.get(&cv).ok_or_else(|| format!("CV {cv}'s other bits aren't known: read it first (the setting \"{}\" is only part of it)", self.name))?
        };
        Ok(vec![(cv, (old & !self.mask()) | ((raw as u8) << self.first & self.mask()))])
    }

    /// The setting as a script sees it.
    pub fn to_value(&self) -> Value {
        let mut v = vec![("name", Value::from(self.name.as_str()))];
        v.push(("cv", if self.cvs.len() == 1 { Value::from(i64::from(self.cvs[0])) } else { Value::list(self.cvs.iter().map(|c| Value::from(i64::from(*c)))) }));
        if self.count == 1 {
            v.push(("bit", Value::from(i64::from(self.first))));
        } else if self.count != 8 {
            v.push(("bits", Value::list([Value::from(i64::from(self.first)), Value::from(i64::from(self.first + self.count - 1))])));
        }
        v.push(("kind", Value::from(self.kind.word())));
        match &self.kind {
            Kind::Number | Kind::LongAddress => {
                v.push(("min", Value::from(self.min)));
                v.push(("max", Value::from(self.max)));
            }
            Kind::Choice(choices) => v.push(("choices", Value::map(choices.iter().map(|(n, w)| (Key::Int(*n), Value::from(w.as_str())))))),
            _ => {}
        }
        v.push(("group", Value::from(self.group.as_str())));
        if !self.help.is_empty() {
            v.push(("help", Value::from(self.help.as_str())));
        }
        if let Some(d) = &self.default {
            v.push(("default", d.clone()));
        }
        if self.read_only {
            v.push(("read_only", Value::from(true)));
        }
        if !self.write_all && !self.read_only {
            v.push(("write_all", Value::from(false)));
        }
        if let Some(c) = &self.shown_when {
            v.push(("shown_when", Value::list([Value::from(c.setting.as_str()), Value::from(c.op.as_str()), c.value.clone()])));
        }
        if let Some((hi, lo)) = self.index {
            v.push(("index", Value::list([Value::from(i64::from(hi)), Value::from(i64::from(lo))])));
        }
        Value::map(v)
    }
}

fn choice_words(choices: &[(i64, String)]) -> String {
    let words: Vec<String> = choices.iter().map(|(_, w)| format!("\"{w}\"")).collect();
    match words.len() {
        0 => "nothing".into(),
        1 => words[0].clone(),
        n => format!("{} or {}", words[..n - 1].join(", "), words[n - 1]),
    }
}

/// The NMRA's standard settings (S-9.2.2), every decoder's: the base of every definition.
pub fn standard() -> Vec<Setting> {
    let mut s = vec![
        Setting::new("Short address", 1, "Address").range(1, 127).default(3).help("The address the locomotive answers to while \"Address used\" is short."),
        Setting::new("Long address", 17, "Address").range(1, 10239).help("The address the locomotive answers to while \"Address used\" is long (CV 17 and 18)."),
        Setting::new("Start voltage", 2, "Motor").help("Vstart: how much of the track's voltage the motor gets at speed step 1."),
        Setting::new("Acceleration", 3, "Motor").help("How slowly the locomotive speeds up: 0 at once, higher slower."),
        Setting::new("Deceleration", 4, "Motor").help("How slowly the locomotive slows down: 0 at once, higher slower."),
        Setting::new("Top voltage", 5, "Motor").help("Vhigh: how much of the track's voltage the motor gets at the top speed step (0 or 1: all of it)."),
        Setting::new("Middle voltage", 6, "Motor").help("Vmid: the voltage at the middle speed step, which bends the speed curve."),
        Setting::new("Kick start", 65, "Motor").help("A short push at start-up for a motor that sticks."),
        Setting::new("Forward trim", 66, "Motor").help("Scales the speed going forward: 128 is as it is, less slower, more faster."),
        Setting::new("Reverse trim", 95, "Motor").help("Scales the speed in reverse: 128 is as it is, less slower, more faster."),
        Setting::new("Version", 7, "Identity").read_only().help("The maker's version number for the decoder."),
        Setting::new("Maker", 8, "Identity").read_only().help("The maker, by the NMRA's list of manufacturer IDs."),
        Setting::new("User ID 1", 105, "Identity").help("Free for the owner's own use, such as a road number."),
        Setting::new("User ID 2", 106, "Identity").help("Free for the owner's own use."),
        Setting::new("Direction reversed", 29, "Configuration").flag(0).help("Forward and reverse swapped."),
        Setting::new("Speed steps", 29, "Configuration").bits(1, 1).choice(&[(0, "14"), (1, "28/128")]).help("How many speed steps the decoder takes from the throttle."),
        Setting::new("Analog running", 29, "Configuration").flag(2).help("Runs on a plain DC layout too."),
        Setting::new("RailCom", 29, "Configuration").flag(3).help("Answers by RailCom, where the command station listens for it."),
        Setting::new("Speed curve", 29, "Configuration").bits(4, 1).choice(&[(0, "three points"), (1, "speed table")]).help("The start, middle, and top voltages (CV 2, 6, 5), or the 28-step speed table (CV 67 to 94)."),
        Setting::new("Address used", 29, "Configuration").bits(5, 1).choice(&[(0, "short"), (1, "long")]).help("Which address the decoder answers to: the short one (CV 1) or the long one (CV 17 and 18)."),
        Setting::new("Consist address", 19, "Consist").bits(0, 7).default(0).help("The consist address it also answers to (0 for none)."),
        Setting::new("Consist reversed", 19, "Consist").flag(7).help("Runs reversed in the consist."),
        Setting::new("Consist functions F1-F8", 21, "Consist").help("Which of F1 to F8 answer to the consist address: one bit each, F1 the lowest."),
        Setting::new("Consist functions FL, F9-F12", 22, "Consist").help("Bit 0 the light going forward, bit 1 in reverse, bits 2 to 5 F9 to F12, at the consist address."),
        Setting::new("Acceleration adjustment", 23, "Consist").help("Added to the acceleration while in a consist (-127 to 127)."),
        Setting::new("Deceleration adjustment", 24, "Consist").help("Added to the deceleration while in a consist (-127 to 127)."),
        Setting::new("Packet time-out", 11, "Advanced").help("How many seconds without a packet before the locomotive stops (0: never)."),
        Setting::new("Lock key", 15, "Decoder lock").help("The key: a decoder takes new values only while this equals its lock number."),
        Setting::new("Lock number", 16, "Decoder lock").help("This decoder's lock number (0: no lock), when a locomotive has more than one decoder."),
    ];
    for x in s.iter_mut() {
        match x.cvs[0] {
            17 => {
                x.cvs = vec![17, 18];
                x.kind = Kind::LongAddress;
            }
            23 | 24 => {
                x.kind = Kind::Signed;
                x.min = -127;
                x.max = 127;
            }
            15 | 16 => x.write_all = false,
            _ => {}
        }
    }
    for step in 1..=28u16 {
        s.push(Setting::new(&format!("Speed step {step}"), 66 + step, "Speed table").when("Speed curve", "speed table").help("The motor's voltage at this step, out of 255."));
    }
    s
}

/// A decoder definition: its settings, and which decoders it is for.
#[derive(Debug)]
pub struct Definition {
    pub name: String,
    pub maker: Option<u8>,
    pub versions: Vec<u8>,
    pub settings: Vec<Setting>,
}

impl Definition {
    pub fn setting(&self, name: &str) -> Option<&Setting> {
        self.settings.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Whether a setting applies, given the other settings' values (by name; one not known counts
    /// as applying).
    pub fn is_shown(&self, setting: &Setting, values: &dyn Fn(&str) -> Option<Value>) -> bool {
        let Some(c) = &setting.shown_when else { return true };
        let Some(v) = values(&c.setting) else { return true };
        compare(&v, &c.op, &c.value)
    }

    /// The groups, in the order their first settings come.
    pub fn groups(&self) -> Vec<String> {
        let mut groups: Vec<String> = Vec::new();
        for s in &self.settings {
            if !groups.contains(&s.group) {
                groups.push(s.group.clone());
            }
        }
        groups
    }

    /// Every CV the settings use, in order.
    pub fn cvs(&self) -> Vec<u16> {
        let mut cvs: Vec<u16> = self.settings.iter().flat_map(|s| s.cvs.iter().copied()).collect();
        cvs.sort_unstable();
        cvs.dedup();
        cvs
    }

    /// Its own settings over the standard ones: one of the same name takes the standard one's
    /// place; checked as a whole.
    fn build(name: String, maker: Option<u8>, versions: Vec<u8>, own: Vec<Setting>, with_standard: bool) -> Result<Definition, String> {
        let mut settings = if with_standard { standard() } else { Vec::new() };
        let mut named: Vec<String> = Vec::new();
        for s in own {
            if named.iter().any(|n| n.eq_ignore_ascii_case(&s.name)) {
                return Err(format!("two settings are named \"{}\"", s.name));
            }
            named.push(s.name.clone());
            match settings.iter_mut().find(|x| x.name.eq_ignore_ascii_case(&s.name)) {
                Some(place) => *place = s,
                None => settings.push(s),
            }
        }
        // (Two settings sharing a bit: one would undo the other.)
        // (One CV number on one index page only, for now: the programmer knows a CV by its number.)
        type Page = Option<(u8, u8)>;
        let mut pages: BTreeMap<u16, (Page, &str)> = BTreeMap::new();
        for s in &settings {
            for cv in &s.cvs {
                let (page, first) = *pages.entry(*cv).or_insert((s.index, &s.name));
                if page != s.index {
                    return Err(format!("the settings \"{first}\" and \"{}\" use CV {cv} on different index pages: one CV number on one page only, for now", s.name));
                }
            }
        }
        // (Each CV, with its index page: the bits taken, and by which setting last.)
        type Place = (u16, Option<(u8, u8)>);
        let mut taken: BTreeMap<Place, (u8, &str)> = BTreeMap::new();
        for s in &settings {
            for cv in &s.cvs {
                let mask = if s.cvs.len() > 1 { 0xFF } else { s.mask() };
                let place = taken.entry((*cv, s.index)).or_insert((0, &s.name));
                if place.0 & mask != 0 {
                    return Err(format!("the settings \"{}\" and \"{}\" both use bits of CV {cv}", place.1, s.name));
                }
                place.0 |= mask;
                place.1 = &s.name;
            }
            if let Some(c) = &s.shown_when
                && !settings.iter().any(|x| x.name.eq_ignore_ascii_case(&c.setting))
            {
                return Err(format!("the setting \"{}\" depends on \"{}\", but there is no setting named that", s.name, c.setting));
            }
        }
        Ok(Definition { name, maker, versions, settings })
    }
}

fn compare(a: &Value, op: &str, b: &Value) -> bool {
    let equal = match (a, b) {
        (Value::Text(x), Value::Text(y)) => x.eq_ignore_ascii_case(y),
        _ => match (a.as_number(), b.as_number()) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
    };
    match op {
        "==" => equal,
        "!=" => !equal,
        _ => match (a.as_number(), b.as_number()) {
            (Some(x), Some(y)) => match op {
                "<" => x < y,
                "<=" => x <= y,
                ">" => x > y,
                ">=" => x >= y,
                _ => false,
            },
            _ => false,
        },
    }
}

/// The built-in definitions: their names, and what they are for.
pub const BUILT_IN: &[(&str, &str)] = &[
    ("nmra", "The NMRA's standard settings, which every decoder has"),
    ("soundtraxx_tsunami2_diesel", "SoundTraxx Tsunami2 diesel sound decoders (CV 8 = 141, CV 7 = 71, a diesel's sounds)"),
];

fn built_in(name: &str) -> Option<Definition> {
    match name {
        "nmra" => Definition::build("nmra".into(), None, Vec::new(), Vec::new(), true).ok(),
        "soundtraxx_tsunami2_diesel" => Definition::build(name.into(), Some(141), vec![71], tsunami2_diesel(), true).ok(),
        _ => None,
    }
}

/// SoundTraxx's Tsunami2 diesel decoders: the settings modellers change most, from SoundTraxx's
/// "Tsunami2 Diesel Technical Reference". The sound lists (which horn number is which horn) are
/// SoundTraxx's, in its sound selection references, so those settings are numbers here.
fn tsunami2_diesel() -> Vec<Setting> {
    let mut s = vec![
        Setting::new("Master volume", 128, "Sound volumes").default(192).help("The volume of every sound: 0 silent, 255 the loudest."),
    ];
    let channels = [
        "Airhorn", "Bell", "Prime mover", "Air compressor", "Dynamic brake", "Radiator fans", "Alarm bell", "Gas turbine", "Coupler", "Train brake apply and release",
        "Independent brake apply", "Independent brake release", "GenSet prime mover 2", "GenSet prime mover 3", "Poppet valve", "Steam or HEP generator", "Cab doors", "",
        "Relay clicks", "Emergency brake", "Glad hand release", "All aboard and coach doors", "", "", "Clickety-clack", "Sander valve", "Fuel loading", "Air conditioner",
        "Wrenches", "Pneumatic oilers", "Toilet flush", "Cab chatter",
    ];
    for (i, name) in channels.iter().enumerate() {
        if !name.is_empty() {
            s.push(Setting::new(&format!("{name} volume"), 129 + i as u16, "Sound volumes").help("This sound's share of the master volume: 0 silent, 255 all of it."));
        }
    }
    s.extend([
        Setting::new("Airhorn", 120, "Sound choices").bits(0, 6).range(0, 43).help("Which airhorn: 0 is the first in SoundTraxx's list of this decoder's horns."),
        Setting::new("Alternate airhorn", 121, "Sound choices").bits(0, 6).range(0, 44).help("0 for none; another number puts that horn of SoundTraxx's list on the short-horn key."),
        Setting::new("Bell", 122, "Sound choices").bits(0, 6).help("Which bell: 0 is the first in SoundTraxx's list of this decoder's bells."),
        Setting::new("Grade-crossing bell", 122, "Sound choices").flag(7).help("The bell rings with the grade-crossing horn sequence."),
        Setting::new("Prime mover", 123, "Sound choices").help("Which engine sound: 0 is the first in SoundTraxx's list of this decoder's prime movers."),
        Setting::new("Air compressor", 124, "Sound choices").help("Which air compressor: 0 is the first in SoundTraxx's list."),
        Setting::new("Poppet valve", 125, "Sound choices").choice(&[(0, "poppet valve"), (1, "electronic air dryer")]),
        Setting::new("Coupler", 126, "Sound choices").bits(0, 2).choice(&[(0, "medium"), (1, "heavy"), (2, "link-and-pin")]),
        Setting::new("Uncouple when the function is turned off", 126, "Sound choices").flag(7).help("Couple when the coupler's function is turned on, uncouple when it is turned off."),
        Setting::new("Emergency stop idles the engine", 112, "Sound options").flag(0).help("After an emergency stop the engine idles, rather than shutting down."),
        Setting::new("Low-oil alarm bell", 112, "Sound options").flag(1).help("The alarm bell heard before the engine starts."),
        Setting::new("Generator", 112, "Sound options").bits(2, 1).choice(&[(0, "steam generator"), (1, "head end power")]),
        Setting::new("Idle notch", 112, "Sound options").flag(3).help("The engine stays at idle at speed step 1."),
        Setting::new("Quiet mode time-out", 113, "Sound options").help("Seconds stopped, with every function off, before the sounds go quiet (0: never)."),
        Setting::new("Auto-bell speed step", 193, "Sound options").bits(0, 7).help("The speed step at which the bell rings by itself (0: never)."),
        Setting::new("Auto-bell time", 194, "Sound options").help("How many seconds the automatic bell rings (0: never)."),
        Setting::new("Dynamic brake rate", 116, "Brakes").help("Added to the deceleration while the dynamic brake is on (-127 to 127; 0: no effect)."),
        Setting::new("Independent brake rate", 117, "Brakes").help("Added to the deceleration while the independent brake is on (-127 to 127; 0: no effect)."),
        Setting::new("Train brake rate", 118, "Brakes").help("Added to the deceleration while the train brake is on (-127 to 127; 0: no effect)."),
        Setting::new("Flash rate", 59, "Lights").bits(0, 4).help("How fast flashing lights flash: 0 the fastest, 15 the slowest."),
        Setting::new("Grade-crossing hold time", 60, "Lights").bits(0, 4).help("Seconds the grade-crossing lights keep flashing (0 to 15)."),
        Setting::new("Dimmer level", 63, "Lights").help("The headlight's brightness while dimmed: 0 dark, 255 full."),
        Setting::new("Master brightness", 64, "Lights").default(255).help("The brightness of every light: 0 dark, 255 full."),
        Setting::new("Back-EMF", 217, "Motor").flag(1).help("Load compensation: the motor keeps its speed up grades and round curves."),
        Setting::new("Auto-stop", 217, "Motor").flag(3).help("A change of direction stops the motor for half a second first."),
        Setting::new("Back-EMF intensity", 212, "Motor").default(255).help("How strongly the speed is held: 0 none, 255 the most."),
        Setting::new("Back-EMF Kp", 209, "Motor").default(48).help("The control loop's proportional gain."),
        Setting::new("Back-EMF Ki", 210, "Motor").default(16).help("The control loop's integral gain."),
    ]);
    for x in s.iter_mut() {
        if (116..=118).contains(&x.cvs[0]) {
            x.kind = Kind::Signed;
            x.min = -127;
            x.max = 127;
        }
    }
    s
}

/// The built-in definition for a decoder `identify_decoder()` described: its maker's, by CV 7 and the
/// kind of sound, or the NMRA's.
pub fn definition_for(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&[])?;
    let id = call.arg(0);
    if !matches!(id, Value::Array(_)) || id.get("maker_id").is_none() {
        return Err(Error::type_error("decoder_definition_for(): give what dcc_base.identify_decoder() gave"));
    }
    let maker = id.get("maker_id").and_then(Value::as_int);
    let version = id.get("version").and_then(Value::as_int);
    let sound = id.get("product").and_then(|p| p.get("sound")).and_then(Value::as_text);
    let name = match (maker, version, sound) {
        (Some(141), Some(71), Some("diesel")) => "soundtraxx_tsunami2_diesel",
        _ => "nmra",
    };
    Ok(call.new_object(DecoderDefinition(built_in(name).expect("a built-in definition"))))
}

/// A setting from a script's array (`decoder_definition(settings: ...)` or a definition file).
fn setting_from(v: &Value, at: &str) -> Result<Setting, String> {
    const KEYS: &[&str] = &["name", "cv", "bit", "bits", "kind", "min", "max", "choices", "group", "help", "default", "read_only", "write_all", "shown_when", "index"];
    if !matches!(v, Value::Array(_)) {
        return Err(format!("{at} is an array of a setting's parts, not {}", crate::base::describe(v)));
    }
    let name = v.get("name").and_then(Value::as_text).filter(|n| !n.trim().is_empty()).ok_or_else(|| format!("{at} needs a \"name\""))?.trim().to_string();
    let at = format!("the setting \"{name}\"");
    for (k, _) in v.entries() {
        let known = matches!(k, Key::Text(t) if KEYS.contains(&t.as_str()));
        if !known {
            let k = match k {
                Key::Text(t) => format!("\"{t}\""),
                Key::Int(n) => n.to_string(),
            };
            return Err(format!("{at} has the key {k}, which a setting doesn't have (it takes {})", KEYS.join(", ")));
        }
    }
    let cv_of = |c: &Value| -> Result<u16, String> {
        match c.as_int() {
            Some(n @ 1..=1024) => Ok(n as u16),
            _ => Err(format!("{at}'s \"cv\" is 1 to 1024, not {}", crate::base::describe(c))),
        }
    };
    let cvs: Vec<u16> = match v.get("cv") {
        None => return Err(format!("{at} needs a \"cv\"")),
        Some(Value::Array(items)) if !items.is_empty() => items.iter().map(|(_, c)| cv_of(c)).collect::<Result<_, _>>()?,
        Some(c) => vec![cv_of(c)?],
    };
    let mut s = Setting::new(&name, cvs[0], v.get("group").and_then(Value::as_text).unwrap_or("Settings"));
    s.cvs = cvs;
    let bit_of = |b: &Value| -> Result<u8, String> {
        match b.as_int() {
            Some(n @ 0..=7) => Ok(n as u8),
            _ => Err(format!("{at}'s bits are 0 to 7, not {}", crate::base::describe(b))),
        }
    };
    if s.cvs.len() > 1 && (v.get("bit").is_some() || v.get("bits").is_some()) {
        return Err(format!("{at} spreads over several CVs, so it takes them whole: no \"bit\" or \"bits\""));
    }
    if let Some(b) = v.get("bit") {
        s = s.flag(bit_of(b)?);
    } else if let Some(b) = v.get("bits") {
        let parts = b.entries();
        if parts.len() != 2 {
            return Err(format!("{at}'s \"bits\" is [first, last]: [0, 3]"));
        }
        let (first, last) = (bit_of(&parts[0].1)?, bit_of(&parts[1].1)?);
        if last < first {
            return Err(format!("{at}'s \"bits\" is [first, last], the first the lower"));
        }
        s = s.bits(first, last - first + 1);
    } else if s.cvs.len() > 1 {
        s.max = if s.cvs.len() >= 8 { i64::MAX } else { (1i64 << (8 * s.cvs.len())) - 1 };
    }
    if let Some(k) = v.get("kind") {
        s.kind = match k.as_text() {
            Some("number") => Kind::Number,
            Some("flag") if s.count == 1 => Kind::Flag,
            Some("flag") => return Err(format!("{at} is a \"flag\", so it is one bit: give \"bit\"")),
            Some("signed") if s.count == 8 && s.cvs.len() == 1 => {
                s.min = -127;
                s.max = 127;
                Kind::Signed
            }
            Some("signed") => return Err(format!("{at} is \"signed\": one whole CV, bit 7 the sign")),
            Some("long_address") if s.cvs.len() == 2 => {
                s.min = 1;
                s.max = 10239;
                Kind::LongAddress
            }
            Some("long_address") => return Err(format!("{at} is a \"long_address\": two CVs, as [17, 18]")),
            Some("choice") => Kind::Choice(Vec::new()),
            _ => return Err(format!("{at}'s \"kind\" is \"number\", \"choice\", \"flag\", \"signed\", or \"long_address\", not {}", crate::base::describe(k))),
        };
    } else if v.get("choices").is_some() {
        s.kind = Kind::Choice(Vec::new());
    }
    if let Some(c) = v.get("choices") {
        if !matches!(s.kind, Kind::Choice(_)) {
            return Err(format!("{at} has \"choices\", so its \"kind\" is \"choice\""));
        }
        let mut choices = Vec::new();
        for (k, w) in c.entries() {
            let (Key::Int(n), Some(w)) = (k, w.as_text()) else {
                return Err(format!("{at}'s \"choices\" are values with their words: [0: \"off\", 1: \"on\"]"));
            };
            if *n < 0 || *n > s.max {
                return Err(format!("{at}'s choice {n} doesn't fit in its bits (0 to {})", s.max));
            }
            choices.push((*n, w.to_string()));
        }
        s.kind = Kind::Choice(choices);
    }
    if matches!(&s.kind, Kind::Choice(c) if c.is_empty()) {
        return Err(format!("{at} is a \"choice\": give its \"choices\""));
    }
    let limit = |key: &str, now: i64| -> Result<i64, String> {
        match v.get(key) {
            None => Ok(now),
            Some(n) => n.as_int().ok_or_else(|| format!("{at}'s \"{key}\" is a whole number, not {}", crate::base::describe(n))),
        }
    };
    let (min, max) = (limit("min", s.min)?, limit("max", s.max)?);
    if matches!(s.kind, Kind::Number) && (min < 0 || max > s.max || min > max) {
        return Err(format!("{at}'s \"min\" and \"max\" are within 0 to {} and the min the lower", s.max));
    }
    s.min = min;
    s.max = max;
    if let Some(t) = v.get("help") {
        s.help = t.as_text().ok_or_else(|| format!("{at}'s \"help\" is text"))?.to_string();
    }
    for (key, place) in [("read_only", &mut s.read_only), ("write_all", &mut s.write_all)] {
        if let Some(b) = v.get(key) {
            *place = b.as_bool().ok_or_else(|| format!("{at}'s \"{key}\" is true or false"))?;
        }
    }
    if s.read_only {
        s.write_all = false;
    }
    if let Some(d) = v.get("default") {
        s.raw_of(d).map_err(|e| format!("its \"default\": {e}"))?;
        s.default = Some(d.clone());
    }
    if let Some(w) = v.get("shown_when") {
        let parts: Vec<&Value> = w.entries().iter().map(|(_, x)| x).collect();
        let (setting, op, value) = match parts.as_slice() {
            [n, x] => (n, "==", *x),
            [n, o, x] => (n, o.as_text().unwrap_or(""), *x),
            _ => return Err(format!("{at}'s \"shown_when\" is [setting, value] or [setting, \"==\", value]")),
        };
        if !["==", "!=", "<", "<=", ">", ">="].contains(&op) {
            return Err(format!("{at}'s \"shown_when\" compares with ==, !=, <, <=, >, or >="));
        }
        let setting = setting.as_text().ok_or_else(|| format!("{at}'s \"shown_when\" names a setting first"))?;
        s.shown_when = Some(Condition { setting: setting.into(), op: op.into(), value: value.clone() });
    }
    if let Some(i) = v.get("index") {
        let parts: Vec<Option<i64>> = i.entries().iter().map(|(_, x)| x.as_int()).collect();
        match parts.as_slice() {
            [Some(hi @ 0..=255), Some(lo @ 0..=255)] => s.index = Some((*hi as u8, *lo as u8)),
            _ => return Err(format!("{at}'s \"index\" is [CV 31, CV 32], each 0 to 255")),
        }
    }
    Ok(s)
}

/// `decoder_definition("nmra")`, or `decoder_definition(settings: [...], name:, maker:, versions:,
/// standard: true)`.
pub fn new_definition(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 1)?;
    call.only_options(&["settings", "name", "maker", "versions", "standard"])?;
    if call.option("settings").is_none() {
        if call.args.is_empty() {
            return Err(Error::type_error("decoder_definition(): give a built-in definition's name, such as \"nmra\", or settings: and a name:"));
        }
        let name = call.text(0)?.to_string();
        let Some(d) = built_in(&name) else {
            let names: Vec<String> = BUILT_IN.iter().map(|(n, _)| format!("\"{n}\"")).collect();
            return Err(Error::range_error(format!("decoder_definition(): there is no built-in definition named \"{name}\" (there are {})", names.join(", "))));
        };
        return Ok(call.new_object(DecoderDefinition(d)));
    }
    if call.args.len() == 1 {
        return Err(Error::type_error("decoder_definition(): give a built-in definition's name or settings:, not both"));
    }
    let d = definition_from(call.option("settings").unwrap(), call.option("name"), call.option("maker"), call.option("versions"), call.option("standard"))?;
    Ok(call.new_object(DecoderDefinition(d)))
}

/// A definition from its parts as a script or a file gives them.
fn definition_from(settings: &Value, name: Option<&Value>, maker: Option<&Value>, versions: Option<&Value>, standard: Option<&Value>) -> Result<Definition, Error> {
    let what = "decoder_definition()";
    let name = match name {
        None => return Err(Error::type_error(format!("{what}: give its name: (a few words, such as \"my sound decoder\")"))),
        Some(n) => n.as_text().filter(|t| !t.trim().is_empty()).ok_or_else(|| Error::type_error(format!("{what}'s `name:` is text")))?.trim().to_string(),
    };
    let maker = match maker {
        None | Some(Value::Null) => None,
        Some(m) => match m.as_int() {
            Some(n @ 0..=255) => Some(n as u8),
            _ => return Err(Error::range_error(format!("{what}'s `maker:` is the maker's ID, CV 8's value (0 to 255)"))),
        },
    };
    let versions = match versions {
        None | Some(Value::Null) => Vec::new(),
        Some(v) => v.entries().iter().map(|(_, x)| x.as_int().filter(|n| (0..=255).contains(n)).map(|n| n as u8)).collect::<Option<Vec<_>>>().ok_or_else(|| Error::range_error(format!("{what}'s `versions:` is a list of CV 7's values (each 0 to 255)")))?,
    };
    let with_standard = match standard {
        None => true,
        Some(b) => b.as_bool().ok_or_else(|| Error::type_error(format!("{what}'s `standard:` is true or false")))?,
    };
    if !matches!(settings, Value::Array(_)) {
        return Err(Error::type_error(format!("{what}'s `settings:` is a list of settings, each an array")));
    }
    let own = settings.entries().iter().enumerate().map(|(i, (_, s))| setting_from(s, &format!("setting {}", i + 1))).collect::<Result<Vec<_>, _>>().map_err(|e| Error::type_error(format!("{what}: {e}")))?;
    Definition::build(name, maker, versions, own, with_standard).map_err(|e| Error::type_error(format!("{what}: {e}")))
}

/// `read_decoder_definition_file($path)`: a definition kept as JSON (its name, maker, versions,
/// standard, and settings, as `decoder_definition()` takes them).
pub fn read_definition_file(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&[])?;
    let path = call.text(0)?.to_string();
    let text = call.builtin("read_text_file", vec![Value::from(path.as_str())], vec![])?;
    let data = call.builtin("json_decode", vec![text], vec![]).map_err(|e| Error::type_error(format!("read_decoder_definition_file(): {path} isn't JSON: {}", e.message)))?;
    if !matches!(data, Value::Array(_)) || data.get("settings").is_none() {
        return Err(Error::type_error(format!("read_decoder_definition_file(): {path} holds no definition (an object with \"name\" and \"settings\")")));
    }
    for (k, _) in data.entries() {
        if !matches!(k, Key::Text(t) if ["name", "maker", "versions", "standard", "settings"].contains(&t.as_str())) {
            return Err(Error::type_error(format!("read_decoder_definition_file(): {path} has a part a definition doesn't (it takes name, maker, versions, standard, settings)")));
        }
    }
    let d = definition_from(data.get("settings").unwrap(), data.get("name"), data.get("maker"), data.get("versions"), data.get("standard")).map_err(|e| Error::new(&e.kind, format!("{path}: {}", e.message.replacen("decoder_definition(): ", "", 1))))?;
    Ok(call.new_object(DecoderDefinition(d)))
}

/// `decoder_definitions()`: the built-in definitions, as name and what it is for.
pub fn definitions_list(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 0)?;
    call.only_options(&[])?;
    Ok(Value::list(BUILT_IN.iter().map(|(n, what)| Value::map([("name", Value::from(*n)), ("about", Value::from(*what))]))))
}

/// The object a script holds.
pub struct DecoderDefinition(pub Definition);

/// The CVs' bytes from a script's array (CV number → byte).
pub fn cvs_from(v: &Value, what: &str) -> Result<BTreeMap<u16, u8>, Error> {
    let mut cvs = BTreeMap::new();
    if v.is_null() {
        return Ok(cvs);
    }
    if !matches!(v, Value::Array(_)) {
        return Err(Error::type_error(format!("{what}: the CVs are an array of each CV's value by its number: [29: 6, 3: 10]")));
    }
    for (k, b) in v.entries() {
        match (k, b.as_int()) {
            (Key::Int(cv @ 1..=1024), Some(byte @ 0..=255)) => {
                cvs.insert(*cv as u16, byte as u8);
            }
            _ => return Err(Error::range_error(format!("{what}: the CVs are each CV's value (0 to 255) by its number (1 to 1024)"))),
        }
    }
    Ok(cvs)
}

pub fn cvs_value(cvs: &BTreeMap<u16, u8>) -> Value {
    Value::map(cvs.iter().map(|(cv, b)| (Key::Int(i64::from(*cv)), Value::from(i64::from(*b)))))
}

const DEF_PROPS: &[&str] = &["name", "maker", "versions", "settings", "groups", "cvs"];
const DEF_FUNCS: &[&str] = &["setting", "value_of", "cvs_of", "is_shown"];

impl DecoderDefinition {
    fn setting_arg(&self, call: &Call, what: &str) -> Result<&Setting, Error> {
        let name = call.text(0)?;
        self.0.setting(name).ok_or_else(|| Error::range_error(format!("{what}: the definition \"{}\" has no setting named \"{name}\"", self.0.name)))
    }
}

impl Object for DecoderDefinition {
    fn kind(&self) -> &str {
        "decoder_definition"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        let d = &self.0;
        Ok(match property {
            "name" => Value::from(d.name.as_str()),
            "maker" => d.maker.map_or(Value::Null, |m| Value::from(i64::from(m))),
            "versions" => Value::list(d.versions.iter().map(|v| Value::from(i64::from(*v)))),
            "settings" => Value::list(d.settings.iter().map(Setting::to_value)),
            "groups" => Value::list(d.groups().into_iter().map(Value::from)),
            "cvs" => Value::list(d.cvs().into_iter().map(|c| Value::from(i64::from(c)))),
            other => return Err(call.unknown_property("decoder_definition", other, DEF_PROPS)),
        })
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("decoder_definition.{function}()");
        match function {
            "setting" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                Ok(self.setting_arg(call, &what)?.to_value())
            }
            "value_of" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let s = self.setting_arg(call, &what)?;
                let cvs = cvs_from(call.arg(1), &what)?;
                Ok(s.value_from(&cvs).unwrap_or(Value::Null))
            }
            "cvs_of" => {
                call.count_between(2, 3)?;
                call.only_options(&[])?;
                let s = self.setting_arg(call, &what)?;
                let now = if call.args.len() == 3 { cvs_from(call.arg(2), &what)? } else { BTreeMap::new() };
                let bytes = s.bytes_for(call.arg(1), &now).map_err(|e| Error::range_error(format!("{what}: {e}")))?;
                Ok(cvs_value(&bytes.into_iter().collect()))
            }
            "is_shown" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let s = self.setting_arg(call, &what)?;
                let values = call.arg(1).clone();
                if !matches!(values, Value::Array(_)) {
                    return Err(Error::type_error(format!("{what}: the other settings' values are an array by name: [\"Speed curve\": \"speed table\"]")));
                }
                let found = |name: &str| values.entries().iter().find(|(k, _)| matches!(k, Key::Text(t) if t.eq_ignore_ascii_case(name))).map(|(_, v)| v.clone()).filter(|v| !v.is_null());
                Ok(Value::from(self.0.is_shown(s, &found)))
            }
            other => Err(call.unknown_function("decoder_definition", other, DEF_FUNCS)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nmra() -> Definition {
        built_in("nmra").unwrap()
    }

    #[test]
    fn the_standard_set_has_no_two_settings_on_one_bit() {
        let d = nmra();
        assert_eq!(d.settings.len(), 29 + 28);
        assert_eq!(d.groups(), ["Address", "Motor", "Identity", "Configuration", "Consist", "Advanced", "Decoder lock", "Speed table"]);
    }

    #[test]
    fn values_come_from_their_bits_and_go_back() {
        let d = nmra();
        let cvs: BTreeMap<u16, u8> = [(29, 38), (17, 194), (18, 218), (19, 0x85), (23, 0x83)].into_iter().collect();
        assert_eq!(d.setting("Address used").unwrap().value_from(&cvs), Some(Value::from("long")));
        assert_eq!(d.setting("Speed steps").unwrap().value_from(&cvs), Some(Value::from("28/128")));
        assert_eq!(d.setting("Analog running").unwrap().value_from(&cvs), Some(Value::from(true)));
        assert_eq!(d.setting("Long address").unwrap().value_from(&cvs), Some(Value::from(730)));
        assert_eq!(d.setting("Consist address").unwrap().value_from(&cvs), Some(Value::from(5)));
        assert_eq!(d.setting("Consist reversed").unwrap().value_from(&cvs), Some(Value::from(true)));
        assert_eq!(d.setting("Acceleration adjustment").unwrap().value_from(&cvs), Some(Value::from(-3)));
        assert_eq!(d.setting("Short address").unwrap().value_from(&cvs), None);
        // Back: only its own bits change.
        assert_eq!(d.setting("Address used").unwrap().bytes_for(&Value::from("short"), &cvs), Ok(vec![(29, 6)]));
        assert_eq!(d.setting("Speed steps").unwrap().bytes_for(&Value::from(0), &cvs), Ok(vec![(29, 36)]));
        assert_eq!(d.setting("Long address").unwrap().bytes_for(&Value::from(730), &cvs), Ok(vec![(17, 194), (18, 218)]));
        assert_eq!(d.setting("Acceleration adjustment").unwrap().bytes_for(&Value::from(-3), &cvs), Ok(vec![(23, 0x83)]));
        assert!(d.setting("Direction reversed").unwrap().bytes_for(&Value::from(true), &BTreeMap::new()).unwrap_err().contains("read it first"));
        assert!(d.setting("Short address").unwrap().bytes_for(&Value::from(200), &cvs).unwrap_err().contains("1 to 127"));
        assert!(d.setting("Speed curve").unwrap().bytes_for(&Value::from("fast"), &cvs).unwrap_err().contains("\"three points\" or \"speed table\""));
    }

    #[test]
    fn a_speed_step_is_shown_only_with_the_speed_table() {
        let d = nmra();
        let step = d.setting("Speed step 4").unwrap();
        assert!(!d.is_shown(step, &|n| (n == "Speed curve").then(|| Value::from("three points"))));
        assert!(d.is_shown(step, &|n| (n == "Speed curve").then(|| Value::from("speed table"))));
        assert!(d.is_shown(step, &|_| None));
    }

    #[test]
    fn the_tsunami2_diesel_definition_is_whole() {
        let d = built_in("soundtraxx_tsunami2_diesel").unwrap();
        assert_eq!(d.maker, Some(141));
        assert!(d.setting("Short address").is_some());
        assert_eq!(d.setting("Cab chatter volume").unwrap().cvs, [160]);
        assert_eq!(d.setting("Relay clicks volume").unwrap().cvs, [147]);
        let cvs: BTreeMap<u16, u8> = [(126, 0x81), (217, 10), (118, 0x85)].into_iter().collect();
        assert_eq!(d.setting("Coupler").unwrap().value_from(&cvs), Some(Value::from("heavy")));
        assert_eq!(d.setting("Uncouple when the function is turned off").unwrap().value_from(&cvs), Some(Value::from(true)));
        assert_eq!(d.setting("Back-EMF").unwrap().value_from(&cvs), Some(Value::from(true)));
        assert_eq!(d.setting("Train brake rate").unwrap().value_from(&cvs), Some(Value::from(-5)));
    }

    #[test]
    fn a_definition_refuses_two_settings_on_one_bit() {
        let mut own = Setting::new("Mute", 29, "Sound").flag(5);
        assert!(Definition::build("x".into(), None, vec![], vec![own.clone()], true).unwrap_err().contains("both use bits of CV 29"));
        own.cvs = vec![120];
        assert!(Definition::build("x".into(), None, vec![], vec![own.clone()], true).is_ok());
        // (One CV number on one index page only.)
        let mut a = Setting::new("Effect 1", 257, "Lights");
        a.index = Some((0, 1));
        let mut b = Setting::new("Volume 1", 257, "Sound");
        b.index = Some((0, 2));
        assert!(Definition::build("x".into(), None, vec![], vec![a, b], true).unwrap_err().contains("different index pages"));
    }
}
