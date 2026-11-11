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

const BASE_PROPS: &[&str] = &["simulate", "port", "reader", "power", "packets", "timing", "shield", "current", "current_limit"];
const BASE_FUNCS: &[&str] = &["emergency_stop", "decoder", "clear_packets", "close", "read_cv", "read_address", "write_cv", "write_address"];
const LOCO_PROPS: &[&str] = &["address", "long", "base", "speed", "direction"];
const LOCO_FUNCS: &[&str] = &["set_function", "function", "stop", "emergency_stop"];

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
fn describe(v: &Value) -> String {
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
        let heard = if self.simulated() {
            self.power.get() && {
                let halves = dcc::timings(&dcc::bits(&packet));
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
    fn reading_turn(&self, what: &str, call: &Call) -> Result<board::ReadingTurn, Error> {
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
            drop(turn);
            let (value, error) = match done {
                Ok(v) => (v, Value::Null),
                Err(e) => (Value::Null, Value::from(e)),
            };
            // ("No more" first, then the event: its function may start another reading, whose "yes"
            // must be the newer word; Leah keeps the script waiting for the event meanwhile.)
            events.keep_running(false);
            events.fire(event, vec![what, value, error]);
        });
    }

    /// After a reading on the script's thread: the main track's power was turned off by it.
    fn after_reading(&self, call: &Call) {
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
                for loco in self.locomotives.borrow().iter().filter_map(Weak::upgrade) {
                    loco.speed.set(0);
                    // (On a board each locomotive's kept speed becomes its emergency stop too, or the
                    // refresh table would start it again.)
                    if !self.simulated() {
                        loco.send_speed(true, call)?;
                    }
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
                call.only_options(&["in_background"])?;
                let cv = whole("dcc_base.read_cv", "a setting's number", call.arg(0))?;
                if !(1..=1024).contains(&cv) {
                    return Err(Error::range_error(format!("dcc_base.read_cv(): a setting (CV) is 1 to 1024, not {cv}")));
                }
                let background = bool_option(call, "in_background", false)?;
                let turn = self.reading_turn("dcc_base.read_cv()", call)?;
                if background {
                    self.read_in_background(turn, Value::from(cv), move |t| t.read_cv(cv).map(Value::from));
                    return Ok(Value::Null);
                }
                let read = turn.read_cv(cv);
                drop(turn);
                self.after_reading(call);
                read.map(Value::from).map_err(|e| hardware(format!("dcc_base.read_cv(): {e}")))
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
                read.map(as_value).map_err(|e| hardware(format!("dcc_base.read_address(): {e}")))
            }
            "write_cv" => {
                call.count_between(2, 2)?;
                call.only_options(&["in_background"])?;
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
                    self.in_background("on_write", turn, Value::from(cv), move |t| t.write_cv(cv, value).map(|_| Value::from(value)));
                    return Ok(Value::Null);
                }
                let done = turn.write_cv(cv, value);
                drop(turn);
                self.after_reading(call);
                done.map(|_| Value::Null).map_err(|e| hardware(format!("dcc_base.write_cv(): {e}")))
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
                done.map(|_| Value::Null).map_err(|e| hardware(format!("dcc_base.write_address(): {e}")))
            }
            other => Err(call.unknown_function("dcc_base", other, BASE_FUNCS)),
        }
    }
}

/// A decoder's state, as `decoder()` gives it.
fn decoder_value(address: u16, long: bool, state: &DecoderState) -> Value {
    Value::map([
        ("address", Value::from(i64::from(address))),
        ("long", Value::from(long)),
        ("speed", Value::from(i64::from(state.speed))),
        ("direction", Value::from(if state.forward { "forward" } else { "reverse" })),
        ("functions", Value::list(state.functions.iter().map(|on| Value::from(*on)))),
        ("packets", Value::from(state.heard as i64)),
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
}

/// `locomotive($address, long: false, base: ...)`.
pub fn new_locomotive(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&["long", "base"])?;
    let long = bool_option(call, "long", false)?;
    let base = match call.option("base") {
        None => None,
        Some(v) => match call.own_option::<DccBase>("base") {
            Some(b) => Some(b),
            None => return Err(Error::type_error(format!("locomotive()'s `base:` is a dcc_base, not {}", describe(v))).at(call.option_line("base"))),
        },
    };
    let address = address_of("locomotive", call.arg(0), long)?;
    let base = match base.or_else(|| LAST.with(|l| l.borrow().upgrade())) {
        Some(b) => b,
        None => return Err(hardware("locomotive(): there is no base station: make one first, dcc_base(\"/dev/ttyACM0\") or dcc_base(simulate: true)")),
    };
    let (speed_place, lights_place) = if base.simulated() { (None, None) } else { base.places_for(address, long, call) };
    let value = call.new_object(Locomotive { base: base.clone(), address, long, speed: Cell::new(0), forward: Cell::new(true), functions: Cell::new([false; dcc::FUNCTIONS]), speed_place, lights_place });
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

impl Locomotive {
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
        let packet = dcc::speed_128(self.address, self.long, self.speed.get(), self.forward.get(), emergency);
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
            other => Err(call.unknown_property("locomotive", other, LOCO_PROPS)),
        }
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "speed" => {
                let n = value.as_int().ok_or_else(|| Error::type_error(format!("a locomotive's `speed` is a whole number from 0 to 126, not {}", describe(&value))))?;
                if !(0..=126).contains(&n) {
                    return Err(Error::range_error(format!("a locomotive's `speed` is 0 to 126 (the 128 speed steps), not {n}")));
                }
                self.speed.set(n as u8);
                self.send_speed(false, call)
            }
            "direction" => {
                let forward = match value.as_text() {
                    Some("forward") => true,
                    Some("reverse") => false,
                    _ => return Err(Error::type_error(format!("a locomotive's `direction` is \"forward\" or \"reverse\", not {}", describe(&value)))),
                };
                self.forward.set(forward);
                self.send_speed(false, call)
            }
            "address" | "long" | "base" => Err(Error::type_error(format!("a locomotive's `{property}` can be read but not set"))),
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
                let mut all = self.functions.get();
                all[n] = on;
                self.functions.set(all);
                self.send_functions(dcc::group_of(n), call)?;
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
