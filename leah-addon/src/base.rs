//! `dcc_base(simulate: true)` and `locomotive($address)` (design/model_railroading.md, "Stage 1a"):
//! the base station, simulated for now, and the locomotives it drives. Each change sends its packet
//! at once; on the simulated track the packet becomes its signal, which the simulated decoders read
//! back as real ones do.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::{Rc, Weak};

use leah_addon_kit::{Call, Error, Object, Value};

use crate::dcc::{self, DecoderState, Reader, Target};

const BASE_PROPS: &[&str] = &["simulate", "power", "packets"];
const BASE_FUNCS: &[&str] = &["emergency_stop", "decoder", "clear_packets"];
const LOCO_PROPS: &[&str] = &["address", "long", "base", "speed", "direction"];
const LOCO_FUNCS: &[&str] = &["set_function", "function", "stop", "emergency_stop"];

/// The packets a base station keeps for `packets`.
const KEPT: usize = 200;

/// A packet sent, as `packets` gives it.
struct Sent {
    bytes: Vec<u8>,
    meaning: String,
    heard: bool,
}

pub struct DccBase {
    power: Cell<bool>,
    sent: RefCell<VecDeque<Sent>>,
    /// The simulated track: what its decoders read the signal with, and each decoder's state.
    reader: RefCell<Reader>,
    decoders: RefCell<BTreeMap<(u16, bool), DecoderState>>,
    /// The locomotive objects made on it (for its emergency stop).
    locomotives: RefCell<Vec<Weak<Locomotive>>>,
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

/// `dcc_base(simulate: true)`.
pub fn new_base(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 1)?;
    call.only_options(&["simulate"])?;
    if !bool_option(call, "simulate", false)? {
        return Err(hardware(match call.args.first() {
            Some(port) => format!("dcc_base(): Leah's base station board isn't supported yet ({}): dcc_base(simulate: true) runs a simulated one", describe(port)),
            None => "dcc_base(): give the board's port, or simulate: true for a simulated base station (the board isn't supported yet)".to_string(),
        }));
    }
    if !call.args.is_empty() {
        return Err(Error::type_error("dcc_base(simulate: true) takes no port: the simulated base station has no board"));
    }
    let base = DccBase { power: Cell::new(false), sent: RefCell::new(VecDeque::new()), reader: RefCell::new(Reader::default()), decoders: RefCell::new(BTreeMap::new()), locomotives: RefCell::new(Vec::new()) };
    let value = call.new_object(base);
    // (The kit keeps the object; the last one made is found again through it, by its value.)
    let made = call.own_value::<DccBase>(&value);
    LAST.with(|l| *l.borrow_mut() = made.as_ref().map(Rc::downgrade).unwrap_or_default());
    Ok(value)
}

impl DccBase {
    /// Sends a packet: onto the simulated track, where the decoders read its signal (with the track
    /// power on), and into `packets`.
    fn send(&self, packet: Vec<u8>, meaning: String) {
        let heard = self.power.get() && {
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
        };
        let mut sent = self.sent.borrow_mut();
        if sent.len() == KEPT {
            sent.pop_front();
        }
        sent.push_back(Sent { bytes: packet, meaning, heard });
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
}

impl Object for DccBase {
    fn kind(&self) -> &str {
        "dcc_base"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        match property {
            "simulate" => Ok(true.into()),
            "power" => Ok(self.power.get().into()),
            "packets" => Ok(self.packets()),
            other => Err(call.unknown_property("dcc_base", other, BASE_PROPS)),
        }
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "power" => match value {
                Value::Bool(on) => {
                    self.power.set(on);
                    Ok(())
                }
                other => Err(Error::type_error(format!("a dcc_base's `power` is true or false, not {}", describe(&other)))),
            },
            "simulate" | "packets" => Err(Error::type_error(format!("a dcc_base's `{property}` can be read but not set"))),
            other => Err(call.unknown_property("dcc_base", other, BASE_PROPS)),
        }
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        match function {
            "emergency_stop" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.send(dcc::broadcast_emergency_stop(), "every locomotive: emergency stop".to_string());
                for loco in self.locomotives.borrow().iter().filter_map(Weak::upgrade) {
                    loco.speed.set(0);
                }
                Ok(Value::Null)
            }
            "decoder" => {
                call.count_between(1, 1)?;
                call.only_options(&["long"])?;
                let long = bool_option(call, "long", false)?;
                let address = address_of("dcc_base.decoder", call.arg(0), long)?;
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
        None => return Err(hardware("locomotive(): there is no base station: make one first, dcc_base(simulate: true)")),
    };
    let value = call.new_object(Locomotive { base: base.clone(), address, long, speed: Cell::new(0), forward: Cell::new(true), functions: Cell::new([false; dcc::FUNCTIONS]) });
    if let Some(loco) = call.own_value::<Locomotive>(&value) {
        let mut kept = base.locomotives.borrow_mut();
        kept.retain(|w| w.strong_count() > 0);
        kept.push(Rc::downgrade(&loco));
    }
    Ok(value)
}

impl Locomotive {
    fn name(&self) -> String {
        if self.long && self.address <= dcc::SHORT_MOST { format!("locomotive {} (long)", self.address) } else { format!("locomotive {}", self.address) }
    }

    fn send_speed(&self, emergency: bool) {
        let words = if emergency {
            "emergency stop".to_string()
        } else if self.speed.get() == 0 {
            "stop".to_string()
        } else {
            format!("speed {}", self.speed.get())
        };
        let direction = if self.forward.get() { "forward" } else { "reverse" };
        let packet = dcc::speed_128(self.address, self.long, self.speed.get(), self.forward.get(), emergency);
        self.base.send(packet, format!("{}: {words}, {direction}", self.name()));
    }

    fn send_functions(&self, group: usize) {
        let on = self.functions.get();
        let packet = dcc::functions(self.address, self.long, group, &on);
        let (first, last) = [(0, 4), (5, 8), (9, 12), (13, 20), (21, 28)][group];
        let lit: Vec<String> = (first..=last).filter(|n| on[*n]).map(|n| format!("F{n}")).collect();
        let which = if lit.is_empty() { "all off".to_string() } else { format!("{} on, the rest off", lit.join(", ")) };
        self.base.send(packet, format!("{}: functions F{first} to F{last}: {which}", self.name()));
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
                self.send_speed(false);
                Ok(())
            }
            "direction" => {
                let forward = match value.as_text() {
                    Some("forward") => true,
                    Some("reverse") => false,
                    _ => return Err(Error::type_error(format!("a locomotive's `direction` is \"forward\" or \"reverse\", not {}", describe(&value)))),
                };
                self.forward.set(forward);
                self.send_speed(false);
                Ok(())
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
                self.send_functions(dcc::group_of(n));
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
                self.send_speed(function == "emergency_stop");
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("locomotive", other, LOCO_FUNCS)),
        }
    }
}
