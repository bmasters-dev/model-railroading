//! `dcc_base()` and `locomotive($address)` (design/model_railroading.md, "Stage 1a" and "Stage 1b"):
//! the base station, simulated or Leah's own board on a USB port, and the locomotives it drives. Each
//! change sends its packet at once. On the simulated track the packet becomes its signal, which the
//! simulated decoders read back as real ones do; on the board it goes out as the real signal, and a
//! reader board (`reader:`) reads it back and tells Leah what it heard.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::{Rc, Weak};
use std::time::Duration;

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::board::{self, Link, Timing};
use crate::dcc::{self, DecoderState, Reader, Target};
use crate::makers;

const BASE_PROPS: &[&str] = &["simulate", "port", "reader", "power", "packets", "timing", "shield", "current", "current_limit", "locomotives"];
const BASE_FUNCS: &[&str] = &["emergency_stop", "stop_all", "decoder", "clear_packets", "close", "read_cv", "read_address", "write_cv", "write_address", "verify_cv", "read_cv_bit", "write_cv_bit", "identify_decoder", "reset_decoder", "find_decoder_locks", "unlock_decoder", "lock_decoder"];
const LOCO_PROPS: &[&str] = &["address", "long", "base", "speed", "direction", "speed_steps", "speed_percent", "speed_limit", "momentary", "labels", "entry"];
const LOCO_FUNCS: &[&str] = &["set_function", "function", "press_function", "release_function", "stop", "emergency_stop", "write_cv", "write_cv_bit"];
/// The least time between two writes on the main to one locomotive, unless `pause:` says otherwise:
/// some decoders miss a write that comes too soon after the last.
const WRITE_PAUSE_MS: i64 = 500;

/// The packets a base station keeps for `packets`.
const KEPT: usize = 200;
/// The places in the board's refresh table.
const PLACES: u8 = 32;
/// The current limit a base station starts with, and the least and most it takes (mA).
const DEFAULT_LIMIT: i64 = 1500;
const LIMITS: std::ops::RangeInclusive<i64> = 10..=5000;
/// How long a board may take to say it runs Leah's program (it starts again as its port opens).
const START_LIMIT: Duration = Duration::from_secs(5);

/// An address's places in the refresh table (its speed's, its F0 to F4's), and how many locomotive
/// objects share them.
type SharedPlaces = (Option<u8>, Option<u8>, usize);

/// A packet sent, as `packets` gives it.
struct Sent {
    bytes: Vec<u8>,
    meaning: String,
    heard: bool,
}

pub struct DccBase {
    power: Cell<bool>,
    sent: RefCell<VecDeque<Sent>>,
    /// The simulated track's reader (what its decoders read the signal with); and each decoder's
    /// state: the simulated ones', or, on a board, what the reader board heard.
    reader: RefCell<Reader>,
    decoders: RefCell<BTreeMap<(u16, bool), DecoderState>>,
    /// The locomotive objects made on it (for its emergency stop).
    locomotives: RefCell<Vec<Weak<Locomotive>>>,
    /// The board, when it isn't simulated (None again once closed); its port, its reader's port.
    link: RefCell<Option<Link>>,
    port: Option<String>,
    reader_port: Option<String>,
    /// The refresh table's places in use, and each address's (its speed's and its F0 to F4's), with
    /// how many locomotive objects share them.
    places: Cell<u32>,
    places_of: RefCell<BTreeMap<(u16, bool), SharedPlaces>>,
    timing: RefCell<Option<Timing>>,
    /// On a board: what each place in its refresh table keeps (to send again if the board starts
    /// again by itself), and how many hellos it had said by the time it was connected.
    kept: RefCell<BTreeMap<u8, Vec<u8>>>,
    hellos: Cell<u32>,
    /// On a board: whether the motor shield is on it, the current it last said (mA), and its limit.
    shield: Cell<bool>,
    current: Cell<i64>,
    limit: Cell<i64>,
    /// Why the board can't be used any more (its port went away), once it can't.
    lost: RefCell<Option<String>>,
    /// Its events (`on_read`), fired from a reading's thread.
    events: Option<Events>,
}

thread_local! {
    /// The base station made last: a locomotive's when none is named.
    static LAST: RefCell<Weak<DccBase>> = const { RefCell::new(Weak::new()) };
}

fn hardware(message: impl Into<String>) -> Error {
    Error::new("hardware_error", message)
}

/// A value as Leah's messages show it: text in quotes, anything else by its kind.
pub(crate) fn describe(v: &Value) -> String {
    match v {
        Value::Text(t) => format!("{t:?}"),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        other => other.kind_name(),
    }
}

/// An option given as true or false (`default` when not given).
fn bool_option(call: &Call, name: &str, default: bool) -> Result<bool, Error> {
    match call.option(name) {
        None => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(other) => Err(Error::type_error(format!("{}()'s `{name}:` is true or false, not {}", call.name, describe(other))).at(call.option_line(name))),
    }
}

/// What went wrong on the programming track, as an error: a `programming_error` for the decoder's
/// side (nothing answered, not confirmed, the programming track cut off), a `hardware_error` for the
/// base station's (unreachable, its port gone).
fn programming(what: &str, e: String) -> Error {
    let kind = if e.starts_with("can't talk") || e.starts_with("the base station didn't answer") || e.contains("went away") { "hardware_error" } else { "programming_error" };
    Error::new(kind, format!("{what}: {e}"))
}

/// `index: [high, low]`: CV 31 and 32 for an indexed setting (each 0 to 255), or None.
fn index_option(call: &Call, what: &str) -> Result<Option<(i64, i64)>, Error> {
    match call.option("index") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let parts: Vec<i64> = v.entries().iter().filter_map(|(_, x)| x.as_int()).collect();
            if parts.len() != 2 || v.entries().len() != 2 || parts.iter().any(|n| !(0..=255).contains(n)) {
                return Err(Error::type_error(format!("{what}'s `index:` is the two index settings' values, CV 31's and CV 32's, like [16, 0], not {}", describe(v))).at(call.option_line("index")));
            }
            Ok(Some((parts[0], parts[1])))
        }
    }
}

/// `tries:`, 1 to 10 (3 when not given).
fn tries_option(call: &Call, what: &str) -> Result<u32, Error> {
    match call.option("tries") {
        None => Ok(3),
        Some(v) => match v.as_int() {
            Some(n) if (1..=10).contains(&n) => Ok(n as u32),
            _ => Err(Error::range_error(format!("{what}'s `tries:` is 1 to 10, not {}", describe(v))).at(call.option_line("tries"))),
        },
    }
}

/// An option given as text, or null (None when not given or null).
fn text_option(call: &Call, name: &str) -> Result<Option<String>, Error> {
    match call.option(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Text(t)) => Ok(Some(t.clone())),
        Some(other) => Err(Error::type_error(format!("{}()'s `{name}:` is text, not {}", call.name, describe(other))).at(call.option_line(name))),
    }
}

/// `dcc_base($port, reader: $port)` or `dcc_base(simulate: true)`.
pub fn new_base(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 1)?;
    call.only_options(&["simulate", "reader"])?;
    let simulate = bool_option(call, "simulate", false)?;
    let reader_port = text_option(call, "reader")?;
    let mut base = DccBase {
        power: Cell::new(false),
        sent: RefCell::new(VecDeque::new()),
        reader: RefCell::new(Reader::default()),
        decoders: RefCell::new(BTreeMap::new()),
        locomotives: RefCell::new(Vec::new()),
        link: RefCell::new(None),
        port: None,
        reader_port: None,
        places: Cell::new(0),
        places_of: RefCell::new(BTreeMap::new()),
        timing: RefCell::new(None),
        kept: RefCell::new(BTreeMap::new()),
        hellos: Cell::new(0),
        shield: Cell::new(false),
        current: Cell::new(0),
        limit: Cell::new(DEFAULT_LIMIT),
        lost: RefCell::new(None),
        events: None,
    };
    if simulate {
        if !call.args.is_empty() || reader_port.is_some() {
            return Err(Error::type_error("dcc_base(simulate: true) takes no port and no reader: the simulated base station has no boards"));
        }
    } else {
        let port = match call.args.first() {
            Some(Value::Text(p)) => p.clone(),
            Some(other) => return Err(Error::type_error(format!("dcc_base()'s port is text, like \"/dev/ttyACM0\", not {}", describe(other)))),
            None => return Err(hardware("dcc_base(): give the base station board's port, like dcc_base(\"/dev/ttyACM0\") (dcc_ports() lists them), or simulate: true for a simulated base station")),
        };
        let link = Link::open(&port, reader_port.as_deref()).map_err(|e| hardware(format!("dcc_base(): {}", e.message)))?;
        link.wait_for_programs(START_LIMIT).map_err(|e| hardware(format!("dcc_base(): {e}")))?;
        // (A clean start: nothing kept from before, the track power off, the current limit the
        // usual one.)
        link.say("X").and_then(|_| link.say(&format!("L {DEFAULT_LIMIT}"))).map_err(|e| hardware(format!("dcc_base(): can't talk to the board on {port}: {e}")))?;
        base.hellos.set(link.heard.lock().unwrap_or_else(|p| p.into_inner()).base_hellos);
        base.port = Some(port);
        base.reader_port = reader_port;
        *base.link.borrow_mut() = Some(link);
    }
    let value = call.new_object_with_events(move |events| {
        base.events = Some(events);
        base
    });
    // (The kit keeps the object; the last one made is found again through it, by its value.)
    let made = call.own_value::<DccBase>(&value);
    LAST.with(|l| *l.borrow_mut() = made.as_ref().map(Rc::downgrade).unwrap_or_default());
    Ok(value)
}

impl DccBase {
    fn simulated(&self) -> bool {
        self.port.is_none()
    }

    /// The board's link, or the error saying why it can't be used.
    fn with_link<T>(&self, what: &str, f: impl FnOnce(&Link) -> std::io::Result<T>) -> Result<T, Error> {
        if let Some(lost) = self.lost.borrow().as_ref() {
            return Err(hardware(format!("{what}: {lost}")));
        }
        match self.link.borrow().as_ref() {
            Some(link) => f(link).map_err(|e| hardware(format!("{what}: can't talk to the board on {}: {e}", link.port))),
            None => Err(hardware(format!("{what}: the base station is closed"))),
        }
    }

    /// Takes in what the boards said since last time: the packets the reader heard (the decoders'
    /// states, and `heard` on the packets sent), its timing, and the base station's own words (said
    /// as warnings; its safety turning the power off among them).
    fn take_heard(&self, call: &Call) {
        let Some(link) = self.link.borrow().as_ref().map(|l| l.heard.clone()) else { return };
        let mut restarted = false;
        let (packets, said, timing, gone, reader_gone) = {
            let mut h = link.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(shield) = h.shield {
                self.shield.set(shield);
            }
            // (The limit is Leah's: set on connecting and by the script, never taken from the board.)
            h.limit = None;
            if let Some(current) = h.current.take() {
                self.current.set(current);
            }
            if h.base_hellos != self.hellos.get() {
                self.hellos.set(h.base_hellos);
                restarted = true;
            }
            (std::mem::take(&mut h.packets), std::mem::take(&mut h.base_said), h.timing.clone(), h.gone.clone(), h.reader_gone.take())
        };
        if timing.is_some() {
            *self.timing.borrow_mut() = timing;
        }
        if let Some(words) = reader_gone {
            call.warn(words);
            *self.timing.borrow_mut() = None;
        }
        for line in said {
            if line.starts_with("OFF reading a setting") {
                // (Reading turns the main track off by design, as read_cv() says: no warning.)
                self.power.set(false);
                self.current.set(0);
            } else if line.starts_with("OFF ") {
                self.power.set(false);
                self.current.set(0);
                call.warn(format!("the base station turned the track power off by itself: {}", line.trim_start_matches("OFF ")));
            } else {
                call.warn(format!("the base station on {} said: {line}", self.port.as_deref().unwrap_or("")));
            }
        }
        if !packets.is_empty() {
            let mut sent = self.sent.borrow_mut();
            let mut decoders = self.decoders.borrow_mut();
            for packet in packets {
                for s in sent.iter_mut().rev().filter(|s| s.bytes == packet && !s.heard) {
                    s.heard = true;
                }
                // (The reader's packets carry their error byte; a decoder reads the data before it.)
                let data = &packet[..packet.len().saturating_sub(1)];
                match dcc::target(data) {
                    (Target::Locomotive(address, long), instruction) => {
                        dcc::obey(decoders.entry((address, long)).or_default(), instruction);
                    }
                    (Target::Broadcast, instruction) => {
                        for state in decoders.values_mut() {
                            dcc::obey(state, instruction);
                        }
                    }
                    _ => {}
                }
            }
        }
        if restarted {
            // (It started again by itself, a brown-out or a reset: its power is off and its table and
            // limit are its first ones. They are sent again; the power stays off until the script
            // turns it on.)
            self.power.set(false);
            self.current.set(0);
            let mut lines = vec![format!("L {}", self.limit.get())];
            lines.extend(self.kept.borrow().iter().map(|(place, packet)| format!("K {place} {}", dcc::hex(packet))));
            if let Some(link) = self.link.borrow().as_ref() {
                for line in lines {
                    let _ = link.say(&line);
                }
            }
            call.warn(format!("the base station on {} started again by itself (its power dipped, or it was reset): the track power is off; its refresh table and current limit were sent again", self.port.as_deref().unwrap_or("")));
        }
        if let Some(gone) = gone
            && self.lost.borrow().is_none()
        {
            self.power.set(false);
            *self.lost.borrow_mut() = Some(gone);
        }
    }

    /// A free place in the board's refresh table (None when all are in use).
    fn take_place(&self) -> Option<u8> {
        let used = self.places.get();
        let free = (0..PLACES).find(|p| used & (1 << p) == 0)?;
        self.places.set(used | (1 << free));
        Some(free)
    }

    fn give_back_place(&self, place: u8) {
        self.places.set(self.places.get() & !(1 << place));
        self.kept.borrow_mut().remove(&place);
        if let Some(link) = self.link.borrow().as_ref() {
            let _ = link.say(&format!("D {place}"));
        }
    }

    /// The places an address's packets are kept in: the ones another locomotive object at the same
    /// address has (so the board doesn't send two speeds in turn), else new ones. With a warning when
    /// the table is full.
    fn places_for(&self, address: u16, long: bool, call: &Call) -> (Option<u8>, Option<u8>) {
        let mut shared = self.places_of.borrow_mut();
        if let Some(entry) = shared.get_mut(&(address, long)) {
            entry.2 += 1;
            return (entry.0, entry.1);
        }
        let speed = self.take_place();
        let lights = self.take_place();
        match (speed, lights) {
            (None, _) => call.warn(format!("the base station's refresh table is full ({PLACES} places): locomotive {address}'s speed and lights are sent once each time they change, not kept repeating")),
            (Some(_), None) => call.warn(format!("the base station's refresh table is full ({PLACES} places): locomotive {address}'s lights are sent once each time they change, not kept repeating")),
            _ => {}
        }
        shared.insert((address, long), (speed, lights, 1));
        (speed, lights)
    }

    /// A locomotive object let go of its address's places: given back once no other one shares them.
    fn release_places(&self, address: u16, long: bool) {
        let gone = {
            let mut shared = self.places_of.borrow_mut();
            match shared.get_mut(&(address, long)) {
                Some(entry) if entry.2 > 1 => {
                    entry.2 -= 1;
                    None
                }
                Some(_) => shared.remove(&(address, long)),
                None => None,
            }
        };
        if let Some((speed, lights, _)) = gone {
            for place in [speed, lights].into_iter().flatten() {
                self.give_back_place(place);
            }
        }
    }

    /// Sends a packet. Simulated: onto the track, where the decoders read its signal (with the track
    /// power on). On a board: kept in its refresh table's place `keep`, or sent now. Either way into
    /// `packets`.
    fn send(&self, packet: Vec<u8>, meaning: String, keep: Option<u8>, call: &Call) -> Result<(), Error> {
        self.send_times(packet, meaning, keep, 1, call)
    }

    /// As `send`, the packet going onto the simulated track `times` times (a write on the main needs
    /// two identical packets; the board sends every packet sent now three times anyway).
    fn send_times(&self, packet: Vec<u8>, meaning: String, keep: Option<u8>, times: usize, call: &Call) -> Result<(), Error> {
        let heard = if self.simulated() {
            self.power.get() && {
                let halves = dcc::timings(&dcc::bits(&packet));
                let halves: Vec<u32> = std::iter::repeat_n(halves, times).flatten().collect();
                let read = self.reader.borrow_mut().read(&halves);
                let mut decoders = self.decoders.borrow_mut();
                let mut any = false;
                for data in read {
                    match dcc::target(&data) {
                        (Target::Locomotive(address, long), instruction) => {
                            any |= dcc::obey(decoders.entry((address, long)).or_default(), instruction).is_some();
                        }
                        (Target::Broadcast, instruction) => {
                            // (Every decoder obeys; one that hasn't heard anything yet has nothing to stop.)
                            for state in decoders.values_mut() {
                                dcc::obey(state, instruction);
                            }
                            any = true;
                        }
                        _ => {}
                    }
                }
                any
            }
        } else {
            self.take_heard(call);
            let line = match keep {
                Some(place) => {
                    self.kept.borrow_mut().insert(place, packet.clone());
                    format!("K {place} {}", dcc::hex(&packet))
                }
                None => format!("S {}", dcc::hex(&packet)),
            };
            self.with_link("dcc_base", |l| l.say(&line))?;
            false
        };
        let mut sent = self.sent.borrow_mut();
        if sent.len() == KEPT {
            sent.pop_front();
        }
        sent.push_back(Sent { bytes: packet, meaning, heard });
        Ok(())
    }

    fn packets(&self) -> Value {
        Value::list(self.sent.borrow().iter().map(|s| {
            Value::map([
                ("bytes", Value::from(dcc::hex(&s.bytes))),
                ("bits", Value::from(dcc::bits_text(&s.bytes))),
                ("meaning", Value::from(s.meaning.clone())),
                ("heard", Value::from(s.heard)),
            ])
        }))
    }

    fn timing_value(&self) -> Value {
        let range = |r: Option<(f64, f64)>| match r {
            Some((a, b)) => Value::list([Value::from(a), Value::from(b)]),
            None => Value::Null,
        };
        match self.timing.borrow().as_ref() {
            Some(t) => Value::map([
                ("one", range(t.one)),
                ("zero", range(t.zero)),
                ("packets", Value::from(t.packets)),
                ("outside", Value::from(t.outside)),
                ("errors", Value::from(t.errors)),
            ]),
            None => Value::Null,
        }
    }

    /// The programming track taken for readings, or the error saying why it can't be.
    pub(crate) fn reading_turn(&self, what: &str, call: &Call) -> Result<board::ReadingTurn, Error> {
        if self.simulated() {
            return Err(hardware(format!("{what}: reading a decoder's settings needs Leah's base station board with its motor shield (its channel B is the programming track)")));
        }
        self.take_heard(call);
        if !self.shield.get() {
            return Err(hardware(format!("{what}: the base station has no motor shield, whose channel B is the programming track")));
        }
        let reading = self.with_link(what, |l| Ok(l.reading()))?;
        reading.start().ok_or_else(|| hardware(format!("{what}: a reading or writing is going on already: one at a time (wait for its on_read or on_write)")))
    }

    /// Runs a reading on a thread of its own, firing `on_read(what, value, error)` when it is done (the
    /// value null on an error, the error null otherwise); a script without windows waits for it.
    fn read_in_background(&self, turn: board::ReadingTurn, what: Value, read: impl FnOnce(&board::ReadingTurn) -> Result<Value, String> + Send + 'static) {
        self.in_background("on_read", turn, what, read);
    }

    /// Runs a reading or a writing on a thread of its own, firing `event(what, value, error)` when it
    /// is done (the value null on an error, the error null otherwise); a script without windows waits.
    fn in_background(&self, event: &'static str, turn: board::ReadingTurn, what: Value, read: impl FnOnce(&board::ReadingTurn) -> Result<Value, String> + Send + 'static) {
        let Some(events) = self.events.clone() else { return };
        events.keep_running(true);
        // (The main track's power goes off for the reading, as the board says in its "OFF" line.)
        self.power.set(false);
        self.current.set(0);
        std::thread::spawn(move || {
            let done = read(&turn);
            let (value, error) = match done {
                Ok(v) => (v, Value::Null),
                Err(e) => (Value::Null, Value::from(e)),
            };
            // ("No more" first, then the programming track let go of, then the event: a reading
            // started after it (by the event's function, or any other) must be the newer word; Leah
            // keeps the script waiting for the event meanwhile.)
            events.keep_running(false);
            drop(turn);
            events.fire(event, vec![what, value, error]);
        });
    }

    /// The locomotives the script holds on this base station, in the order made.
    fn live_locomotives(&self) -> Vec<Rc<Locomotive>> {
        self.locomotives.borrow().iter().filter_map(Weak::upgrade).collect()
    }

    /// As a reading starts on a thread of its own: the main track's power goes off for it, as the
    /// board says in its "OFF" line.
    pub(crate) fn reading_starts(&self) {
        self.power.set(false);
        self.current.set(0);
    }

    /// After a reading on the script's thread: the main track's power was turned off by it.
    pub(crate) fn after_reading(&self, call: &Call) {
        self.take_heard(call);
        self.power.set(false);
        self.current.set(0);
    }

    fn close(&self) {
        if let Some(link) = self.link.borrow_mut().take() {
            link.close();
        }
        self.power.set(false);
    }
}

impl Drop for DccBase {
    fn drop(&mut self) {
        self.close();
    }
}

impl Object for DccBase {
    fn kind(&self) -> &str {
        "dcc_base"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        self.take_heard(call);
        match property {
            "simulate" => Ok(self.simulated().into()),
            "port" => Ok(self.port.clone().map_or(Value::Null, Value::from)),
            "reader" => Ok(self.reader_port.clone().map_or(Value::Null, Value::from)),
            "power" => Ok(self.power.get().into()),
            "packets" => Ok(self.packets()),
            "timing" => Ok(self.timing_value()),
            "shield" => Ok(self.shield.get().into()),
            "current" => Ok(if self.shield.get() { Value::from(if self.power.get() { self.current.get() } else { 0 }) } else { Value::Null }),
            "current_limit" => Ok(self.limit.get().into()),
            "locomotives" => {
                let all = self.live_locomotives();
                Ok(Value::list(all.iter().map(|l| call.object_value(l))))
            }
            other => Err(call.unknown_property("dcc_base", other, BASE_PROPS)),
        }
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "power" => match value {
                Value::Bool(on) => {
                    if !self.simulated() {
                        self.take_heard(call);
                        // (Off always works: with the board closed or its cable out, its power is off
                        // already, or its safety turns it off within 3 seconds.)
                        let reachable = self.lost.borrow().is_none() && self.link.borrow().is_some();
                        if on || reachable {
                            self.with_link("a dcc_base's `power`", |l| {
                                // (A reading said before the change is no longer the track's.)
                                l.heard.lock().unwrap_or_else(|p| p.into_inner()).current = None;
                                l.say(if on { "+" } else { "-" })
                            })?;
                        }
                    }
                    self.current.set(0);
                    self.power.set(on);
                    Ok(())
                }
                other => Err(Error::type_error(format!("a dcc_base's `power` is true or false, not {}", describe(&other)))),
            },
            "current_limit" => {
                let n = value.as_int().ok_or_else(|| Error::type_error(format!("a dcc_base's `current_limit` is a whole number of milliamps, not {}", describe(&value))))?;
                if !LIMITS.contains(&n) {
                    return Err(Error::range_error(format!("a dcc_base's `current_limit` is {} to {} mA, not {n}", LIMITS.start(), LIMITS.end())));
                }
                if !self.simulated() {
                    self.take_heard(call);
                    self.with_link("a dcc_base's `current_limit`", |l| l.say(&format!("L {n}")))?;
                }
                self.limit.set(n);
                Ok(())
            }
            "simulate" | "port" | "reader" | "packets" | "timing" | "shield" | "current" => Err(Error::type_error(format!("a dcc_base's `{property}` can be read but not set"))),
            other => Err(call.unknown_property("dcc_base", other, BASE_PROPS)),
        }
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        match function {
            "emergency_stop" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                if !self.simulated() {
                    self.take_heard(call);
                    if self.lost.borrow().is_some() || self.link.borrow().is_none() {
                        // (A board out of reach sends nothing: its power is off, or its safety turns it
                        // off within 3 seconds. The locomotives are stopped here all the same.)
                        for loco in self.locomotives.borrow().iter().filter_map(Weak::upgrade) {
                            loco.speed.set(0);
                        }
                        self.power.set(false);
                        return Ok(Value::Null);
                    }
                }
                self.send(dcc::broadcast_emergency_stop(), "every locomotive: emergency stop".to_string(), None, call)?;
                for loco in self.live_locomotives() {
                    loco.speed.set(0);
                    // (On a board each locomotive's kept speed becomes its emergency stop too, or the
                    // refresh table would start it again.)
                    if !self.simulated() {
                        loco.send_speed(true, call)?;
                    }
                    loco.changed(call);
                }
                Ok(Value::Null)
            }
            // (Every locomotive stopped as its decoder eases it, as each one's stop() does.)
            "stop_all" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                for loco in self.live_locomotives() {
                    loco.speed.set(0);
                    loco.send_speed(false, call)?;
                    loco.changed(call);
                }
                Ok(Value::Null)
            }
            "decoder" => {
                call.count_between(1, 1)?;
                call.only_options(&["long"])?;
                let long = bool_option(call, "long", false)?;
                let address = address_of("dcc_base.decoder", call.arg(0), long)?;
                self.take_heard(call);
                Ok(match self.decoders.borrow().get(&(address, long)) {
                    Some(state) if state.heard > 0 => decoder_value(address, long, state),
                    _ => Value::Null,
                })
            }
            "clear_packets" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.sent.borrow_mut().clear();
                Ok(Value::Null)
            }
            "close" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.close();
                Ok(Value::Null)
            }
            "read_cv" => {
                call.count_between(1, 1)?;
                call.only_options(&["in_background", "index", "tries"])?;
                let cv = whole("dcc_base.read_cv", "a setting's number", call.arg(0))?;
                if !(1..=1024).contains(&cv) {
                    return Err(Error::range_error(format!("dcc_base.read_cv(): a setting (CV) is 1 to 1024, not {cv}")));
                }
                let index = index_option(call, "dcc_base.read_cv()")?;
                let tries = tries_option(call, "dcc_base.read_cv()")?;
                let background = bool_option(call, "in_background", false)?;
                let turn = self.reading_turn("dcc_base.read_cv()", call)?;
                if background {
                    self.read_in_background(turn, Value::from(cv), move |t| t.select_index(index).and_then(|_| t.read_cv_tries(cv, tries)).map(Value::from));
                    return Ok(Value::Null);
                }
                let read = turn.select_index(index).and_then(|_| turn.read_cv_tries(cv, tries));
                drop(turn);
                self.after_reading(call);
                read.map(Value::from).map_err(|e| programming("dcc_base.read_cv()", e))
            }
            "read_address" => {
                call.count_between(0, 0)?;
                call.only_options(&["in_background"])?;
                let background = bool_option(call, "in_background", false)?;
                let turn = self.reading_turn("dcc_base.read_address()", call)?;
                let as_value = |(address, long): (i64, bool)| Value::map([("address", Value::from(address)), ("long", Value::from(long))]);
                if background {
                    self.read_in_background(turn, Value::from("address"), move |t| t.read_address().map(as_value));
                    return Ok(Value::Null);
                }
                let read = turn.read_address();
                drop(turn);
                self.after_reading(call);
                read.map(as_value).map_err(|e| programming("dcc_base.read_address()", e))
            }
            "write_cv" => {
                call.count_between(2, 2)?;
                call.only_options(&["in_background", "index"])?;
                let index = index_option(call, "dcc_base.write_cv()")?;
                let cv = whole("dcc_base.write_cv", "a setting's number", call.arg(0))?;
                let value = whole("dcc_base.write_cv", "a setting's value", call.arg(1))?;
                if !(1..=1024).contains(&cv) {
                    return Err(Error::range_error(format!("dcc_base.write_cv(): a setting (CV) is 1 to 1024, not {cv}")));
                }
                if !(0..=255).contains(&value) {
                    return Err(Error::range_error(format!("dcc_base.write_cv(): a setting's value is 0 to 255, not {value}")));
                }
                match cv {
                    7 => return Err(Error::range_error("dcc_base.write_cv(): CV 7 is the decoder's version, which can't be written")),
                    8 => return Err(Error::range_error("dcc_base.write_cv(): CV 8 isn't written here: writing it is how many decoders are reset to their factory settings, every setting lost")),
                    _ => {}
                }
                let background = bool_option(call, "in_background", false)?;
                let turn = self.reading_turn("dcc_base.write_cv()", call)?;
                if background {
                    self.in_background("on_write", turn, Value::from(cv), move |t| t.select_index(index).and_then(|_| t.write_cv(cv, value)).map(|_| Value::from(value)));
                    return Ok(Value::Null);
                }
                let done = turn.select_index(index).and_then(|_| turn.write_cv(cv, value));
                drop(turn);
                self.after_reading(call);
                done.map(|_| Value::Null).map_err(|e| programming("dcc_base.write_cv()", e))
            }
            "write_address" => {
                call.count_between(1, 1)?;
                call.only_options(&["long", "in_background"])?;
                let address = whole("dcc_base.write_address", "an address", call.arg(0))?;
                if !(1..=i64::from(dcc::LONG_MOST)).contains(&address) {
                    return Err(Error::range_error(format!("dcc_base.write_address(): a locomotive's address is 1 to {}, not {address}", dcc::LONG_MOST)));
                }
                let long = bool_option(call, "long", address > i64::from(dcc::SHORT_MOST))?;
                if !long && address > i64::from(dcc::SHORT_MOST) {
                    return Err(Error::range_error(format!("dcc_base.write_address(): {address} is a long address: a short one is 1 to {}", dcc::SHORT_MOST)).at(call.option_line("long")));
                }
                let background = bool_option(call, "in_background", false)?;
                let turn = self.reading_turn("dcc_base.write_address()", call)?;
                let as_value = move || Value::map([("address", Value::from(address)), ("long", Value::from(long))]);
                if background {
                    self.in_background("on_write", turn, Value::from("address"), move |t| t.write_address(address, long).map(|_| as_value()));
                    return Ok(Value::Null);
                }
                let done = turn.write_address(address, long);
                drop(turn);
                self.after_reading(call);
                done.map(|_| Value::Null).map_err(|e| programming("dcc_base.write_address()", e))
            }
            "identify_decoder" => {
                call.count_between(0, 0)?;
                call.only_options(&["in_background", "tries"])?;
                let tries = tries_option(call, "dcc_base.identify_decoder()")?;
                let background = bool_option(call, "in_background", false)?;
                let turn = self.reading_turn("dcc_base.identify_decoder()", call)?;
                let identify = move |t: &board::ReadingTurn| -> Result<Value, String> {
                    let maker = t.read_cv_tries(8, tries)?;
                    let version = t.read_cv_tries(7, tries)?;
                    let consist = t.read_cv_tries(19, tries)?;
                    let config = t.read_cv_tries(29, tries)?;
                    // (SoundTraxx's Tsunami2, Econami, and Blunami say which product they are in CV 253
                    // to 256: its own "product identification" guide.)
                    // (Every SoundTraxx decoder keeps its firmware in CV 105 and 106: the version as
                    // major and minor in a Tsunami2, Econami, or Blunami; in the others CV 105 is the
                    // version and CV 106 the set of factory settings, a fitted decoder's part number.)
                    let newer = maker == 141 && (70..=72).contains(&version);
                    let (firmware, product) = if maker == 141 {
                        let (major, minor) = (t.read_cv_tries(105, tries)?, t.read_cv_tries(106, tries)?);
                        if newer {
                            let mut v = Vec::new();
                            for cv in 253..=256 {
                                v.push(t.read_cv_tries(cv, tries)?);
                            }
                            (Value::from(format!("{major}.{minor}")), Value::map([
                                ("customer", Value::from(v[0])),
                                ("sound", makers::soundtraxx_sound(v[1]).map_or(Value::Null, Value::from)),
                                ("revision", Value::from(v[2] >> 3)),
                                ("id", Value::from(v[3])),
                            ]))
                        } else {
                            // (CV 105 and 106 are also the NMRA's user IDs: 0 there is a decoder that
                            // doesn't keep them, as the owner's Sound Value decoder (CV 7 = 82) gave,
                            // not firmware 0: unknown.)
                            let known = |v: i64| if v == 0 { Value::Null } else { Value::from(v) };
                            (if major == 0 { Value::Null } else { Value::from(major.to_string()) }, Value::map([("settings_set", known(minor))]))
                        }
                    } else {
                        (Value::Null, Value::Null)
                    };
                    Ok(identity_value(maker, version, consist, config, firmware, product))
                };
                if background {
                    self.read_in_background(turn, Value::from("identity"), identify);
                    return Ok(Value::Null);
                }
                let read = identify(&turn);
                drop(turn);
                self.after_reading(call);
                let value = read.map_err(|e| programming("dcc_base.identify_decoder()", e))?;
                if let Some(c) = value.get("consist").and_then(Value::as_int).filter(|c| *c != 0) {
                    call.warn(format!("the decoder's consist address (CV 19) is {c}: it answers to that address too, and ignores some commands at its own (clear CV 19 to take it out of the consist)"));
                }
                Ok(value)
            }
            "reset_decoder" => {
                call.count_between(0, 0)?;
                call.only_options(&["confirm", "cv", "value"])?;
                let what = "dcc_base.reset_decoder()";
                if !bool_option(call, "confirm", false)? {
                    return Err(Error::new("programming_error", format!("{what} sets every one of the decoder's settings back to the maker's: give confirm: true to do it")));
                }
                let given = match (call.option("cv"), call.option("value")) {
                    (None, None) => None,
                    (Some(c), Some(v)) => match (c.as_int(), v.as_int()) {
                        (Some(c @ 1..=1024), Some(v @ 0..=255)) => Some((c, v)),
                        _ => return Err(Error::range_error(format!("{what}'s `cv:` is 1 to 1024 and `value:` 0 to 255"))),
                    },
                    _ => return Err(Error::type_error(format!("{what}: give both `cv:` and `value:` (the maker's reset), or neither"))),
                };
                let turn = self.reading_turn(what, call)?;
                let done = (|| -> Result<Value, String> {
                    let maker = turn.read_cv(8)?;
                    let (cv, value) = match given.or_else(|| maker_reset(maker as u8)) {
                        Some(r) => r,
                        None => {
                            let name = makers::maker_name(maker as u8).unwrap_or("an unknown maker");
                            return Err(format!("no reset is known for {name} (CV 8 = {maker}): give the maker's own reset from its manual, as cv: and value:"));
                        }
                    };
                    // (The decoder resets as it takes the value, so it can't confirm it: not confirmed is
                    // what a reset looks like.)
                    match turn.write_cv(cv, value) {
                        Ok(()) => {}
                        Err(e) if e.starts_with("the decoder didn't confirm") => {}
                        Err(e) => return Err(e),
                    }
                    let (address, long) = turn.read_address()?;
                    Ok(Value::map([
                        ("maker", Value::from(makers::maker_name(maker as u8).unwrap_or(""))),
                        ("cv", Value::from(cv)),
                        ("value", Value::from(value)),
                        ("address", Value::from(address)),
                        ("long", Value::from(long)),
                    ]))
                })();
                drop(turn);
                self.after_reading(call);
                done.map_err(|e| programming(what, e))
            }
            "find_decoder_locks" => {
                let what = "dcc_base.find_decoder_locks()";
                call.count_between(0, 0)?;
                call.only_options(&["most"])?;
                let most = match call.option("most") {
                    None => 7,
                    Some(v) => match v.as_int() {
                        Some(n @ 1..=255) => n,
                        _ => return Err(Error::range_error(format!("{what}'s `most:` is 1 to 255, not {}", describe(v))).at(call.option_line("most"))),
                    },
                };
                let turn = self.reading_turn(what, call)?;
                // (The lock guards writes only: asking whether CV 16 holds each number finds every
                // decoder's lock, whatever CV 15 is, and changes nothing.)
                let found = (|| -> Result<Value, String> {
                    let mut found = Vec::new();
                    for id in 1..=most {
                        if turn.verify_cv(16, id)? {
                            found.push(Value::from(id));
                        }
                    }
                    Ok(Value::list(found))
                })();
                drop(turn);
                self.after_reading(call);
                found.map_err(|e| programming(what, e))
            }
            "unlock_decoder" | "lock_decoder" => {
                let what = format!("dcc_base.{function}()");
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let id = whole(&format!("dcc_base.{function}"), "a decoder's lock number", call.arg(0))?;
                if !(0..=255).contains(&id) {
                    return Err(Error::range_error(format!("{what}: a decoder's lock number (CV 16) is 0 to 255, not {id}")));
                }
                let turn = self.reading_turn(&what, call)?;
                // (Locking: CV 16 is the decoder's lock, then CV 15, the key, the same so it stays
                // open. Writes to CV 15 and 16 are always taken, whatever the lock.)
                let done = if function == "lock_decoder" { turn.write_cv(16, id).and_then(|()| turn.write_cv(15, id)) } else { turn.write_cv(15, id) };
                drop(turn);
                self.after_reading(call);
                done.map(|()| Value::Null).map_err(|e| programming(&what, e))
            }
            "verify_cv" | "read_cv_bit" | "write_cv_bit" => {
                let what = format!("dcc_base.{function}()");
                let counts = match function { "verify_cv" | "read_cv_bit" => 2, _ => 3 };
                call.count_between(counts, counts)?;
                call.only_options(&["index", "tries"])?;
                let cv = whole(&format!("dcc_base.{function}"), "a setting's number", call.arg(0))?;
                if !(1..=1024).contains(&cv) {
                    return Err(Error::range_error(format!("{what}: a setting (CV) is 1 to 1024, not {cv}")));
                }
                let second = whole(&format!("dcc_base.{function}"), if function == "verify_cv" { "a setting's value" } else { "a bit's number" }, call.arg(1))?;
                if function == "verify_cv" && !(0..=255).contains(&second) {
                    return Err(Error::range_error(format!("{what}: a setting's value is 0 to 255, not {second}")));
                }
                if function != "verify_cv" && !(0..=7).contains(&second) {
                    return Err(Error::range_error(format!("{what}: a bit is 0 to 7, not {second}")));
                }
                let on = if function == "write_cv_bit" {
                    match call.arg(2) {
                        Value::Bool(b) => *b,
                        Value::Int(0) => false,
                        Value::Int(1) => true,
                        other => return Err(Error::type_error(format!("{what}: the bit's value is true or false (or 1 or 0), not {}", describe(other)))),
                    }
                } else {
                    false
                };
                if function == "write_cv_bit" && matches!(cv, 7 | 8) {
                    return Err(Error::range_error(format!("{what}: CV {cv} isn't written here (CV 7 is the version; writing CV 8 resets many decoders)")));
                }
                let index = index_option(call, &what)?;
                let tries = tries_option(call, &what)?;
                let turn = self.reading_turn(&what, call)?;
                let done = turn.select_index(index).and_then(|_| match function {
                    "verify_cv" => turn.verify_cv(cv, second).map(Value::from),
                    "read_cv_bit" => turn.read_cv_bit(cv, second, tries).map(Value::from),
                    _ => turn.write_cv_bit(cv, second, on).map(|_| Value::Null),
                });
                drop(turn);
                self.after_reading(call);
                done.map_err(|e| programming(&what, e))
            }
            other => Err(call.unknown_function("dcc_base", other, BASE_FUNCS)),
        }
    }
}

/// Each maker's documented factory reset (CV, value), from its own manual (the private reference):
/// most write 8 into CV 8; TCS 2 into CV 8; NCE 2 into CV 30; MRC 1 into CV 125.
fn maker_reset(maker: u8) -> Option<(i64, i64)> {
    match maker {
        141 | 151 | 129 | 145 => Some((8, 8)), // SoundTraxx, ESU, Digitrax, ZIMO
        153 => Some((8, 2)),                   // TCS
        11 => Some((30, 2)),                   // NCE
        143 => Some((125, 1)),                 // MRC
        _ => None,
    }
}

/// What `identify_decoder` gives: the maker (by the NMRA's list), its version, the consist address,
/// and CV 29's parts.
fn identity_value(maker: i64, version: i64, consist: i64, config: i64, firmware: Value, product: Value) -> Value {
    Value::map([
        ("maker_id", Value::from(maker)),
        ("maker", makers::maker_name(maker as u8).map_or(Value::Null, Value::from)),
        ("version", Value::from(version)),
        ("family", makers::family_name(maker as u8, version as u8).map_or(Value::Null, Value::from)),
        ("firmware", firmware),
        ("product", product),
        ("consist", Value::from(consist & 0x7F)),
        ("consist_reversed", Value::from(consist & 0x80 != 0)),
        ("cv29", cv29_value(config)),
    ])
}

/// CV 29's bits by name (S-9.2.2).
fn cv29_value(v: i64) -> Value {
    Value::map([
        ("reversed", Value::from(v & 0x01 != 0)),
        ("speed_steps", Value::from(if v & 0x02 != 0 { 28 } else { 14 })),
        ("analog", Value::from(v & 0x04 != 0)),
        ("railcom", Value::from(v & 0x08 != 0)),
        ("speed_table", Value::from(v & 0x10 != 0)),
        ("long_address", Value::from(v & 0x20 != 0)),
        ("accessory", Value::from(v & 0x80 != 0)),
    ])
}

/// `decoder_maker_name($id)`: the maker's name for a CV 8 value, as the NMRA lists it, or null.
pub fn decoder_maker_name(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&[])?;
    let id = whole("decoder_maker_name", "a maker's ID (CV 8)", call.arg(0))?;
    if !(0..=255).contains(&id) {
        return Err(Error::range_error(format!("decoder_maker_name(): a maker's ID (CV 8) is 0 to 255, not {id}")));
    }
    Ok(makers::maker_name(id as u8).map_or(Value::Null, Value::from))
}

/// `cv29_parts($value)`: CV 29's bits by name.
pub fn cv29_parts(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&[])?;
    let v = whole("cv29_parts", "CV 29's value", call.arg(0))?;
    if !(0..=255).contains(&v) {
        return Err(Error::range_error(format!("cv29_parts(): CV 29's value is 0 to 255, not {v}")));
    }
    Ok(cv29_value(v))
}

/// `make_cv29(reversed:, speed_steps:, analog:, railcom:, speed_table:, long_address:)`: CV 29's value
/// (a locomotive decoder's: 28/128 speed steps and analog running on unless given).
pub fn make_cv29(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 0)?;
    call.only_options(&["reversed", "speed_steps", "analog", "railcom", "speed_table", "long_address"])?;
    let steps = match call.option("speed_steps") {
        None => 28,
        Some(v) => match v.as_int() {
            Some(n @ (14 | 28 | 128)) => n,
            _ => return Err(Error::range_error(format!("make_cv29()'s `speed_steps:` is 14, 28, or 128 (28 and 128 share a bit), not {}", describe(v))).at(call.option_line("speed_steps"))),
        },
    };
    let mut v = 0;
    if bool_option(call, "reversed", false)? { v |= 0x01; }
    if steps != 14 { v |= 0x02; }
    if bool_option(call, "analog", true)? { v |= 0x04; }
    if bool_option(call, "railcom", false)? { v |= 0x08; }
    if bool_option(call, "speed_table", false)? { v |= 0x10; }
    if bool_option(call, "long_address", false)? { v |= 0x20; }
    Ok(Value::from(v))
}

/// A decoder's state, as `decoder()` gives it.
fn decoder_value(address: u16, long: bool, state: &DecoderState) -> Value {
    Value::map([
        ("address", Value::from(i64::from(address))),
        ("long", Value::from(long)),
        ("speed", Value::from(i64::from(state.speed))),
        ("speed_steps", Value::from(i64::from(state.steps))),
        ("direction", Value::from(if state.forward { "forward" } else { "reverse" })),
        ("functions", Value::list(state.functions.iter().map(|on| Value::from(*on)))),
        ("packets", Value::from(state.heard as i64)),
        // (The settings written to it on the main, by number: on the simulated track.)
        ("cvs", Value::map(state.cvs.iter().map(|(cv, v)| (i64::from(*cv), Value::from(i64::from(*v)))))),
    ])
}

/// A whole number from a value (a decimal with nothing after the point counts).
fn whole(name: &str, what: &str, v: &Value) -> Result<i64, Error> {
    v.as_int().ok_or_else(|| Error::type_error(format!("{name}(): {what} is a whole number, not {}", describe(v))))
}

/// A locomotive's address: 1 to 127 (short), or 128 to 10239 (long), 1 to 10239 with `long: true`.
fn address_of(name: &str, v: &Value, long: bool) -> Result<u16, Error> {
    let n = whole(name, "an address", v)?;
    let most = if long { dcc::LONG_MOST } else { dcc::SHORT_MOST };
    if n < 1 || n > i64::from(dcc::LONG_MOST) {
        return Err(Error::range_error(format!("{name}(): a locomotive's address is 1 to {}, not {n}", dcc::LONG_MOST)));
    }
    if n > i64::from(most) {
        return Err(Error::range_error(format!("{name}(): {n} is a long address: give long: true (a short address is 1 to {})", dcc::SHORT_MOST)));
    }
    Ok(n as u16)
}

pub struct Locomotive {
    base: Rc<DccBase>,
    address: u16,
    long: bool,
    speed: Cell<u8>,
    forward: Cell<bool>,
    functions: Cell<[bool; dcc::FUNCTIONS]>,
    /// On a board: the refresh table's places keeping its speed and its F0 to F4 (None: sent once).
    speed_place: Option<u8>,
    lights_place: Option<u8>,
    /// When the last write on the main to it went out.
    last_write: Cell<Option<std::time::Instant>>,
    /// Its decoder's speed steps (128, 28, or 14), and the fastest step it may be given.
    steps: u8,
    speed_limit: Cell<u8>,
    /// The functions on only while pressed, and the labels of its keys (from its roster entry).
    momentary: RefCell<std::collections::BTreeSet<u8>>,
    labels: RefCell<BTreeMap<u8, String>>,
    /// The roster entry it was made from.
    entry: Option<Rc<crate::roster::RosterEntry>>,
}

/// `locomotive($address, long: false, base: ...)`.
pub fn new_locomotive(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&["long", "base", "speed_steps"])?;
    let long = bool_option(call, "long", false)?;
    let base = match call.option("base") {
        None => None,
        Some(v) => match call.own_option::<DccBase>("base") {
            Some(b) => Some(b),
            None => return Err(Error::type_error(format!("locomotive()'s `base:` is a dcc_base, not {}", describe(v))).at(call.option_line("base"))),
        },
    };
    // (A roster entry: its address, whether it is long, its keys' labels and momentary ones, its
    // speed limit, and its speed steps when its CV 29 is kept (bit 1 clear: 14).)
    let entry = call.own::<crate::roster::RosterEntry>(0);
    let data = match &entry {
        Some(e) => {
            if call.option("long").is_some() {
                return Err(Error::type_error("locomotive(): a roster entry says whether its address is long: no `long:`").at(call.option_line("long")));
            }
            Some(e.data()?)
        }
        None => None,
    };
    let (address, long) = match &data {
        Some(e) => (e.address, e.long),
        None => (address_of("locomotive", call.arg(0), long)?, long),
    };
    let steps = match call.option("speed_steps") {
        None => match data.as_ref().and_then(|e| e.cv(29)) {
            Some(c) if c & 0x02 == 0 => 14,
            _ => 128,
        },
        Some(v) => match v.as_int() {
            Some(n @ (14 | 28 | 128)) => n as u8,
            _ => return Err(Error::range_error(format!("locomotive()'s `speed_steps:` is 14, 28, or 128, as its decoder is set (CV 29), not {}", describe(v))).at(call.option_line("speed_steps"))),
        },
    };
    let top = top_step(steps);
    let speed_limit = match &data {
        // (The roster's limit is in 126 steps; in fewer steps, the step at or above it.)
        Some(e) => ((u32::from(e.speed_limit) * u32::from(top)).div_ceil(126)).clamp(1, u32::from(top)) as u8,
        None => top,
    };
    let momentary: std::collections::BTreeSet<u8> = data.as_ref().map(|e| e.functions.iter().filter(|(_, (_, m))| *m).map(|(n, _)| *n).collect()).unwrap_or_default();
    let labels: BTreeMap<u8, String> = data.as_ref().map(|e| e.functions.iter().filter(|(_, (l, _))| !l.is_empty()).map(|(n, (l, _))| (*n, l.clone())).collect()).unwrap_or_default();
    let base = match base.or_else(|| LAST.with(|l| l.borrow().upgrade())) {
        Some(b) => b,
        None => return Err(hardware("locomotive(): there is no base station: make one first, dcc_base(\"/dev/ttyACM0\") or dcc_base(simulate: true)")),
    };
    let (speed_place, lights_place) = if base.simulated() { (None, None) } else { base.places_for(address, long, call) };
    let value = call.new_object(Locomotive {
        base: base.clone(),
        address,
        long,
        speed: Cell::new(0),
        forward: Cell::new(true),
        functions: Cell::new([false; dcc::FUNCTIONS]),
        speed_place,
        lights_place,
        last_write: Cell::new(None),
        steps,
        speed_limit: Cell::new(speed_limit),
        momentary: RefCell::new(momentary),
        labels: RefCell::new(labels),
        entry,
    });
    if let Some(loco) = call.own_value::<Locomotive>(&value) {
        let mut kept = base.locomotives.borrow_mut();
        kept.retain(|w| w.strong_count() > 0);
        kept.push(Rc::downgrade(&loco));
    }
    Ok(value)
}

impl Drop for Locomotive {
    fn drop(&mut self) {
        // (A locomotive the script let go of is no longer kept repeating on the board, once no other
        // object at its address is.)
        if !self.base.simulated() {
            self.base.release_places(self.address, self.long);
        }
    }
}

/// The top speed step of a decoder's speed steps.
fn top_step(steps: u8) -> u8 {
    match steps {
        14 => 14,
        28 => 28,
        _ => 126,
    }
}

impl Locomotive {
    /// Tells the script's `on_locomotive_changed` this locomotive changed.
    fn changed(&self, call: &mut Call) {
        let Some(events) = self.base.events.clone() else { return };
        let me = self.base.locomotives.borrow().iter().filter_map(Weak::upgrade).find(|l| std::ptr::eq(Rc::as_ptr(l), self));
        if let Some(me) = me {
            events.fire("on_locomotive_changed", vec![call.object_value(&me)]);
        }
    }

    /// Sets a function and sends it (in 14 steps F0 goes in the speed packet too).
    fn turn_function(&self, n: usize, on: bool, call: &mut Call) -> Result<(), Error> {
        let mut all = self.functions.get();
        all[n] = on;
        self.functions.set(all);
        self.send_functions(dcc::group_of(n), call)?;
        if n == 0 && self.steps == 14 {
            self.send_speed(false, call)?;
        }
        self.changed(call);
        Ok(())
    }

    fn name(&self) -> String {
        if self.long && self.address <= dcc::SHORT_MOST { format!("locomotive {} (long)", self.address) } else { format!("locomotive {}", self.address) }
    }

    fn send_speed(&self, emergency: bool, call: &Call) -> Result<(), Error> {
        let words = if emergency {
            "emergency stop".to_string()
        } else if self.speed.get() == 0 {
            "stop".to_string()
        } else {
            format!("speed {}", self.speed.get())
        };
        let direction = if self.forward.get() { "forward" } else { "reverse" };
        let packet = if self.steps == 128 {
            dcc::speed_128(self.address, self.long, self.speed.get(), self.forward.get(), emergency)
        } else {
            dcc::speed_basic(self.address, self.long, self.speed.get(), self.forward.get(), emergency, self.steps, self.functions.get()[0])
        };
        self.base.send(packet, format!("{}: {words}, {direction}", self.name()), self.speed_place, call)
    }

    fn send_functions(&self, group: usize, call: &Call) -> Result<(), Error> {
        let on = self.functions.get();
        let packet = dcc::functions(self.address, self.long, group, &on);
        let (first, last) = [(0, 4), (5, 8), (9, 12), (13, 20), (21, 28)][group];
        let lit: Vec<String> = (first..=last).filter(|n| on[*n]).map(|n| format!("F{n}")).collect();
        let which = if lit.is_empty() { "all off".to_string() } else { format!("{} on, the rest off", lit.join(", ")) };
        // (F0 to F4, the lights among them, are kept repeating; the others are sent now.)
        let keep = if group == 0 { self.lights_place } else { None };
        self.base.send(packet, format!("{}: functions F{first} to F{last}: {which}", self.name()), keep, call)
    }
}

fn function_number(name: &str, v: &Value) -> Result<usize, Error> {
    let n = whole(name, "a function's number", v)?;
    if !(0..dcc::FUNCTIONS as i64).contains(&n) {
        return Err(Error::range_error(format!("{name}(): a function's number is 0 (the headlight) to {}, not {n}", dcc::FUNCTIONS - 1)));
    }
    Ok(n as usize)
}

impl Locomotive {
    /// Sends writes on the main, each twice (S-9.2.1), spaced `pause` ms from the last write to this
    /// locomotive: some decoders miss one sent sooner.
    fn send_writes(&self, writes: Vec<(Vec<u8>, String)>, pause: i64, call: &Call) -> Result<(), Error> {
        for (packet, words) in writes {
            if let Some(last) = self.last_write.get() {
                let wait = std::time::Duration::from_millis(pause as u64).saturating_sub(last.elapsed());
                if !wait.is_zero() {
                    std::thread::sleep(wait);
                }
            }
            self.base.send_times(packet, format!("{}: {words}", self.name()), None, 2, call)?;
            self.last_write.set(Some(std::time::Instant::now()));
        }
        Ok(())
    }

    pub(crate) fn address(&self) -> u16 {
        self.address
    }

    pub(crate) fn is_long(&self) -> bool {
        self.long
    }

    /// Writes a setting on the main, as `write_cv` does (CV 31 and 32 first for an indexed one).
    pub(crate) fn write_on_main(&self, cv: u16, value: u8, index: Option<(u8, u8)>, call: &Call) -> Result<(), Error> {
        let mut writes = Vec::new();
        if let Some((high, low)) = index {
            for (n, v) in [(31u16, high), (32u16, low)] {
                writes.push((dcc::ops_write_byte(self.address, self.long, n, v), format!("CV {n} = {v} (on the main)")));
            }
        }
        writes.push((dcc::ops_write_byte(self.address, self.long, cv, value), format!("CV {cv} = {value} (on the main)")));
        self.send_writes(writes, WRITE_PAUSE_MS, call)
    }
}

impl Object for Locomotive {
    fn kind(&self) -> &str {
        "locomotive"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        match property {
            "address" => Ok(i64::from(self.address).into()),
            "long" => Ok(self.long.into()),
            "base" => Ok(call.object_value(&self.base)),
            "speed" => Ok(i64::from(self.speed.get()).into()),
            "direction" => Ok(if self.forward.get() { "forward" } else { "reverse" }.into()),
            "speed_steps" => Ok(i64::from(self.steps).into()),
            "speed_percent" => {
                let top = i64::from(top_step(self.steps));
                Ok(((i64::from(self.speed.get()) * 100 + top / 2) / top).into())
            }
            "speed_limit" => Ok(i64::from(self.speed_limit.get()).into()),
            "momentary" => Ok(Value::list(self.momentary.borrow().iter().map(|n| Value::from(i64::from(*n))))),
            "labels" => Ok(Value::map(self.labels.borrow().iter().map(|(n, l)| (leah_addon_kit::Key::Int(i64::from(*n)), Value::from(l.as_str()))))),
            "entry" => Ok(match &self.entry {
                Some(e) => call.object_value(e),
                None => Value::Null,
            }),
            other => Err(call.unknown_property("locomotive", other, LOCO_PROPS)),
        }
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "speed" | "speed_percent" => {
                let top = i64::from(top_step(self.steps));
                let most = if property == "speed" { top } else { 100 };
                let n = value.as_int().ok_or_else(|| Error::type_error(format!("a locomotive's `{property}` is a whole number from 0 to {most}, not {}", describe(&value))))?;
                if !(0..=most).contains(&n) {
                    return Err(Error::range_error(if property == "speed" {
                        format!("a locomotive's `speed` is 0 to {top} (its {} speed steps), not {n}", self.steps)
                    } else {
                        format!("a locomotive's `speed_percent` is 0 to 100, not {n}")
                    }));
                }
                let step = if property == "speed" { n } else { (n * top + 50) / 100 };
                // (Never past its speed limit.)
                self.speed.set((step as u8).min(self.speed_limit.get()));
                self.send_speed(false, call)?;
                self.changed(call);
                Ok(())
            }
            "speed_limit" => {
                let top = top_step(self.steps);
                match value.as_int() {
                    Some(n) if (1..=i64::from(top)).contains(&n) => {
                        self.speed_limit.set(n as u8);
                        if self.speed.get() > n as u8 {
                            self.speed.set(n as u8);
                            self.send_speed(false, call)?;
                            self.changed(call);
                        }
                        Ok(())
                    }
                    _ => Err(Error::range_error(format!("a locomotive's `speed_limit` is a speed step, 1 to {top}, not {}", describe(&value)))),
                }
            }
            "momentary" => {
                let items: Vec<Value> = match &value {
                    Value::Array(items) => items.iter().map(|(_, v)| v.clone()).collect(),
                    Value::Null => Vec::new(),
                    other => return Err(Error::type_error(format!("a locomotive's `momentary` is a list of function numbers, not {}", describe(other)))),
                };
                let mut set = std::collections::BTreeSet::new();
                for v in items {
                    set.insert(function_number("a locomotive's `momentary`", &v)? as u8);
                }
                *self.momentary.borrow_mut() = set;
                Ok(())
            }
            "direction" => {
                let forward = match value.as_text() {
                    Some("forward") => true,
                    Some("reverse") => false,
                    _ => return Err(Error::type_error(format!("a locomotive's `direction` is \"forward\" or \"reverse\", not {}", describe(&value)))),
                };
                self.forward.set(forward);
                self.send_speed(false, call)?;
                self.changed(call);
                Ok(())
            }
            "speed_steps" | "labels" | "entry" | "address" | "long" | "base" => Err(Error::type_error(format!("a locomotive's `{property}` can be read but not set"))),
            other => Err(call.unknown_property("locomotive", other, LOCO_PROPS)),
        }
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("locomotive.{function}");
        match function {
            "set_function" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let n = function_number(&what, call.arg(0))?;
                let on = call.arg(1).as_bool().ok_or_else(|| Error::type_error(format!("{what}(): on or off is true or false, not {}", describe(call.arg(1)))))?;
                self.turn_function(n, on, call)?;
                Ok(Value::Null)
            }
            // (A key pressed and let go of: a momentary function is on only while pressed; a
            // latching one turns on or off at each press.)
            "press_function" | "release_function" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let n = function_number(&what, call.arg(0))?;
                let momentary = self.momentary.borrow().contains(&(n as u8));
                let now = self.functions.get()[n];
                match (function == "press_function", momentary) {
                    (true, true) => self.turn_function(n, true, call)?,
                    (true, false) => self.turn_function(n, !now, call)?,
                    (false, true) => self.turn_function(n, false, call)?,
                    (false, false) => {}
                }
                Ok(Value::Null)
            }
            "function" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let n = function_number(&what, call.arg(0))?;
                Ok(self.functions.get()[n].into())
            }
            "stop" | "emergency_stop" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.speed.set(0);
                self.send_speed(function == "emergency_stop", call)?;
                self.changed(call);
                Ok(Value::Null)
            }
            "write_cv" | "write_cv_bit" => {
                let bit = function == "write_cv_bit";
                call.count_between(if bit { 3 } else { 2 }, if bit { 3 } else { 2 })?;
                call.only_options(&["pause", "index"])?;
                let index = index_option(call, &format!("{what}()"))?;
                let cv = whole(&what, "a setting's number", call.arg(0))?;
                if !(1..=1024).contains(&cv) {
                    return Err(Error::range_error(format!("{what}(): a setting (CV) is 1 to 1024, not {cv}")));
                }
                match cv {
                    7 => return Err(Error::range_error(format!("{what}(): CV 7 is the decoder's version, which can't be written"))),
                    8 => return Err(Error::range_error(format!("{what}(): CV 8 isn't written here: writing it is how many decoders are reset to their factory settings, every setting lost"))),
                    _ => {}
                }
                let pause = match call.option("pause") {
                    None => WRITE_PAUSE_MS,
                    Some(v) => match v.as_int() {
                        Some(ms) if (0..=10_000).contains(&ms) => ms,
                        _ => return Err(Error::range_error(format!("{what}()'s `pause:` is 0 to 10000 milliseconds, not {}", describe(v))).at(call.option_line("pause"))),
                    },
                };
                let (packet, words) = if bit {
                    let b = whole(&what, "a bit's number", call.arg(1))?;
                    if !(0..=7).contains(&b) {
                        return Err(Error::range_error(format!("{what}(): a bit is 0 to 7, not {b}")));
                    }
                    let on = match call.arg(2) {
                        Value::Bool(on) => *on,
                        Value::Int(0) => false,
                        Value::Int(1) => true,
                        other => return Err(Error::type_error(format!("{what}(): the bit's value is true or false (or 1 or 0), not {}", describe(other)))),
                    };
                    (dcc::ops_write_bit(self.address, self.long, cv as u16, b as u8, on), format!("CV {cv} bit {b} = {} (on the main)", u8::from(on)))
                } else {
                    let value = whole(&what, "a setting's value", call.arg(1))?;
                    if !(0..=255).contains(&value) {
                        return Err(Error::range_error(format!("{what}(): a setting's value is 0 to 255, not {value}")));
                    }
                    (dcc::ops_write_byte(self.address, self.long, cv as u16, value as u8), format!("CV {cv} = {value} (on the main)"))
                };
                // (An indexed setting: CV 31 and 32 written first, each a write of its own.)
                let mut writes = Vec::new();
                if let Some((high, low)) = index {
                    for (n, v) in [(31u16, high as u8), (32u16, low as u8)] {
                        writes.push((dcc::ops_write_byte(self.address, self.long, n, v), format!("CV {n} = {v} (on the main)")));
                    }
                }
                writes.push((packet, words));
                self.send_writes(writes, pause, call)?;
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("locomotive", other, LOCO_FUNCS)),
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The boards' ports, and loading their programs

/// `dcc_ports()`: the USB serial ports plugged in, each `["port", "name", "vendor_id", "product_id",
/// "allowed"]`.
pub fn dcc_ports(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 0)?;
    call.only_options(&[])?;
    Ok(Value::list(board::ports().into_iter().map(|p| {
        Value::map([
            ("port", Value::from(p.port)),
            ("name", Value::from(p.name)),
            ("vendor_id", Value::from(p.vendor_id)),
            ("product_id", Value::from(p.product_id)),
            ("allowed", Value::from(p.allowed)),
        ])
    })))
}

const JOB_PROPS: &[&str] = &["port", "role", "board", "finished", "succeeded", "message"];
const JOB_FUNCS: &[&str] = &["wait"];

/// Loading a program onto a board, in the background: `load_dcc_program()`'s object.
pub struct DccProgramJob {
    port: String,
    role: String,
    board: String,
    state: std::sync::Arc<std::sync::Mutex<board::JobState>>,
}

/// `load_dcc_program($port, "base" or "reader", board: ..., arduino_cli: ...)`.
pub fn load_dcc_program(call: &mut Call) -> Result<Value, Error> {
    call.count_between(2, 2)?;
    call.only_options(&["board", "arduino_cli"])?;
    let port = match call.arg(0) {
        Value::Text(p) if p.starts_with('/') => p.clone(),
        Value::Text(p) => return Err(Error::type_error(format!("load_dcc_program(): {p:?} isn't a port: give its whole path, like \"/dev/ttyACM0\" (dcc_ports() lists them)"))),
        other => return Err(Error::type_error(format!("load_dcc_program()'s port is text, like \"/dev/ttyACM0\", not {}", describe(other)))),
    };
    let role = match call.arg(1).as_text() {
        Some(r @ ("base" | "reader")) => r.to_string(),
        _ => return Err(Error::type_error(format!("load_dcc_program()'s program is \"base\" (the base station) or \"reader\" (the board reading the signal back), not {}", describe(call.arg(1))))),
    };
    let board = text_option(call, "board")?.unwrap_or_else(|| if role == "base" { "arduino:avr:mega".into() } else { "arduino:avr:uno".into() });
    if board.starts_with('-') || board.split(':').count() < 3 {
        return Err(Error::type_error(format!("load_dcc_program()'s `board:` is arduino-cli's name for the board, like \"arduino:avr:mega\", \"arduino:avr:uno\", or \"arduino:avr:nano\", not {board:?}")).at(call.option_line("board")));
    }
    // (The base station's signal comes from Timer1's OC1B, which is pin 12 on a Mega 2560 only; the
    // reader's input capture is pin 8 on the ATmega328P boards.)
    let kind = board.split(':').nth(2).unwrap_or("");
    let fits = if role == "base" { matches!(kind, "mega" | "megaADK") } else { matches!(kind, "uno" | "nano" | "pro") };
    if !fits {
        let wanted = if role == "base" { "an Arduino Mega 2560 (\"arduino:avr:mega\")" } else { "an Arduino Uno, Nano, or Pro Mini (\"arduino:avr:uno\", \"arduino:avr:nano\", \"arduino:avr:pro\")" };
        return Err(Error::type_error(format!("load_dcc_program(): Leah's {} program runs on {wanted}, not {board:?}", if role == "base" { "base station" } else { "reader" })).at(call.option_line("board")));
    }
    let cli = board::find_arduino_cli(text_option(call, "arduino_cli")?.as_deref()).map_err(|e| Error::new("shell_error", format!("load_dcc_program(): {e}")))?;
    // (The program's text, its build, and arduino-cli's cache go in a temporary folder of the
    // script's own, which Leah removes at the end.)
    let folder = match call.builtin("make_temp_path", vec![], vec![])? {
        Value::Text(t) => std::path::PathBuf::from(t),
        other => return Err(Error::new("file_error", format!("load_dcc_program(): no temporary folder ({})", describe(&other)))),
    };
    let state = std::sync::Arc::new(std::sync::Mutex::new(board::JobState::default()));
    let thread_state = state.clone();
    let (p, r, b) = (port.clone(), role.clone(), board.clone());
    Ok(call.new_object_with_events(move |events| {
        events.keep_running(true);
        let spawned = std::thread::Builder::new().name("leah-dcc-load".into()).spawn({
            let events = events.clone();
            move || {
                let done = board::load_program(&cli, &folder, &p, &r, &b);
                let (succeeded, message) = match done {
                    Ok(m) => (true, m),
                    Err(m) => (false, m),
                };
                {
                    let mut s = thread_state.lock().unwrap_or_else(|x| x.into_inner());
                    *s = board::JobState { finished: true, succeeded, message: message.clone() };
                }
                events.fire("on_finished", vec![Value::from(succeeded), Value::from(message)]);
                events.keep_running(false);
            }
        });
        if let Err(e) = spawned {
            let mut s = state.lock().unwrap_or_else(|x| x.into_inner());
            *s = board::JobState { finished: true, succeeded: false, message: format!("can't start loading: {e}") };
            events.keep_running(false);
        }
        DccProgramJob { port, role, board, state }
    }))
}

impl Object for DccProgramJob {
    fn kind(&self) -> &str {
        "dcc_program_job"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        let s = self.state.lock().unwrap_or_else(|x| x.into_inner()).clone();
        match property {
            "port" => Ok(self.port.clone().into()),
            "role" => Ok(self.role.clone().into()),
            "board" => Ok(self.board.clone().into()),
            "finished" => Ok(s.finished.into()),
            "succeeded" => Ok(s.succeeded.into()),
            "message" => Ok(if s.finished { Value::from(s.message) } else { Value::Null }),
            other => Err(call.unknown_property("dcc_program_job", other, JOB_PROPS)),
        }
    }

    fn set(&self, property: &str, _value: Value, call: &mut Call) -> Result<(), Error> {
        if JOB_PROPS.contains(&property) {
            Err(Error::type_error(format!("a dcc_program_job's `{property}` can be read but not set")))
        } else {
            Err(call.unknown_property("dcc_program_job", property, JOB_PROPS))
        }
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        match function {
            "wait" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                loop {
                    let s = self.state.lock().unwrap_or_else(|x| x.into_inner()).clone();
                    if s.finished {
                        return Ok(s.succeeded.into());
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            other => Err(call.unknown_function("dcc_program_job", other, JOB_FUNCS)),
        }
    }
}
