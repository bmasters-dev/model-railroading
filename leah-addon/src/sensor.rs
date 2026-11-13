//! Sensors (design/model_railroading.md, "Phase C, item 10", model-railroading#14): an input from the
//! layout, a block detector, a fascia pushbutton, a track contact, read on one of the base station
//! board's pins (its program watches the pin and says each change that lasts 3 ms), or an internal
//! one with no pin (a panel's button, a flag other programs see), set by `simulate()`.
//!
//! A pin is **active when it is low**: block detectors and pushbuttons pull it to ground against the
//! internal pull-up (`pull: true`, the default); `inverted: true` for one that is high when active.
//! `active_delay` and `inactive_delay` (milliseconds) hold a change back until it has lasted that
//! long: dirty track and lost wheels mustn't drop a block's occupancy for a moment.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::{describe, DccBase};

const PROPS: &[&str] = &["pin", "name", "state", "active", "inverted", "pull", "active_delay", "inactive_delay", "base"];
const FUNCS: &[&str] = &["simulate"];
/// The pins the base station's program uses itself (boards/dcc_base: 3, 8, 9, 11, 12, 13, A0, A1, A4, A5).
const OWN_PINS: [u8; 10] = [3, 8, 9, 11, 12, 13, 54, 55, 58, 59];
/// The longest delay a sensor takes (a minute).
const MOST_DELAY: i64 = 60_000;

/// What a layout object tells the objects that follow it (a turnout following its sensors, a block its
/// sensor, a light a sensor or a turnout), on the script's thread, after its own events: they read
/// its state again.
pub(crate) trait Follower {
    fn followed_changed(&self, call: &Call);
}

/// The objects following one, each told when it changes.
#[derive(Default)]
pub(crate) struct Followers(RefCell<Vec<Weak<dyn Follower>>>);

impl Followers {
    pub(crate) fn add(&self, who: Weak<dyn Follower>) {
        let mut f = self.0.borrow_mut();
        f.retain(|w| w.strong_count() > 0);
        f.push(who);
    }

    pub(crate) fn tell(&self, call: &Call) {
        let followers: Vec<_> = self.0.borrow().iter().filter_map(Weak::upgrade).collect();
        for f in followers {
            f.followed_changed(call);
        }
    }
}

/// An object as a follower another keeps (a weak hold: following keeps nothing alive).
pub(crate) fn as_follower<T: Follower + 'static>(made: &Rc<T>) -> Weak<dyn Follower> {
    let me: Rc<dyn Follower> = made.clone();
    Rc::downgrade(&me)
}

pub struct Sensor {
    base: Option<Rc<DccBase>>,
    pin: Option<u8>,
    name: String,
    inverted: bool,
    pull: bool,
    active_delay: Cell<i64>,
    inactive_delay: Cell<i64>,
    /// What the pin (or `simulate()`) says now, active or not, and what the sensor says: the same once
    /// a change has lasted its delay. None: nothing heard yet.
    raw: Cell<Option<bool>>,
    state: Cell<Option<bool>>,
    /// Counts the changes waiting out a delay: a wait whose count is no longer the newest is over.
    waits: Cell<u64>,
    followers: Followers,
    events: Events,
}

/// A pin's number, 2 to 69, or its analog name, "A0" to "A15" (54 to 69).
fn pin_of(v: &Value) -> Option<u8> {
    if let Some(n) = v.as_int() {
        return u8::try_from(n).ok().filter(|n| (2..70).contains(n));
    }
    let t = v.as_text()?.trim();
    let n: u8 = t.strip_prefix(['A', 'a'])?.parse().ok()?;
    (n < 16).then_some(54 + n)
}

fn delay_of(what: &str, name: &str, v: &Value) -> Result<i64, Error> {
    match v.as_int() {
        Some(n @ 0..=MOST_DELAY) => Ok(n),
        _ => Err(Error::range_error(format!("{what}'s `{name}` is milliseconds, 0 to {MOST_DELAY}, not {}", describe(v)))),
    }
}

fn bool_of(what: &str, name: &str, v: &Value) -> Result<bool, Error> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(Error::type_error(format!("{what}'s `{name}:` is true or false, not {}", describe(other)))),
    }
}

/// `sensor($pin, name:, inverted: false, pull: true, active_delay: 0, inactive_delay: 0, base:)`, or
/// `sensor(name: "Reset")` for an internal one.
pub fn new_sensor(call: &mut Call) -> Result<Value, Error> {
    let what = "sensor()";
    call.count_between(0, 1)?;
    call.only_options(&["name", "inverted", "pull", "active_delay", "inactive_delay", "base"])?;
    let pin = match call.args.first() {
        None => None,
        Some(v) => Some(pin_of(v).ok_or_else(|| Error::range_error(format!("{what}: a sensor's pin is the base station board's pin, 2 to 69 or \"A0\" to \"A15\", not {}", describe(v))))?),
    };
    if let Some(p) = pin.filter(|p| OWN_PINS.contains(p)) {
        return Err(Error::range_error(format!("{what}: pin {} is the base station program's own (the track's signal, power, brakes, current, and faults: 3, 8, 9, 11, 12, 13, A0, A1, A4, A5)", if p >= 54 { format!("A{}", p - 54) } else { p.to_string() })));
    }
    let name = match call.option("name") {
        None => match pin {
            Some(p) => format!("S{p}"),
            None => return Err(Error::type_error(format!("{what}: give the board's pin the sensor is wired to, sensor(22), or a name for one without a pin, sensor(name: \"Reset\")"))),
        },
        Some(v) => v.as_text().filter(|t| !t.trim().is_empty() && !t.contains(['\n', '\r'])).map(|t| t.trim().to_string()).ok_or_else(|| Error::type_error(format!("{what}'s `name:` is text on one line, not {}", describe(v))).at(call.option_line("name")))?,
    };
    let flag = |call: &Call, n: &str, default: bool| call.option(n).map_or(Ok(default), |v| bool_of(what, n, v).map_err(|e| e.at(call.option_line(n))));
    let inverted = flag(call, "inverted", false)?;
    let pull = flag(call, "pull", true)?;
    let delay = |call: &Call, n: &str| call.option(n).map_or(Ok(0), |v| delay_of(what, &format!("{n}:"), v).map_err(|e| e.at(call.option_line(n))));
    let (active_delay, inactive_delay) = (delay(call, "active_delay")?, delay(call, "inactive_delay")?);
    let base = match (pin, call.option("base")) {
        (_, Some(v)) => Some(call.own_option::<DccBase>("base").ok_or_else(|| Error::type_error(format!("{what}'s `base:` is a dcc_base, not {}", describe(v))).at(call.option_line("base")))?),
        (Some(_), None) => Some(DccBase::last().ok_or_else(|| Error::new("hardware_error", format!("{what}: there is no base station for its pin: make one first, dcc_base(\"/dev/ttyACM0\") or dcc_base(simulate: true)")))?),
        (None, None) => None,
    };
    if pin.is_none() && (call.option("pull").is_some() || base.is_some()) {
        let given = if base.is_some() { "base:" } else { "pull:" };
        return Err(Error::type_error(format!("{what}: a sensor without a pin takes no `{given}`")));
    }
    // (The simulated base station's pins, and an internal sensor, start inactive: nothing pulls them.)
    let starts = match &base {
        Some(b) if !b.simulated() => None,
        _ => Some(false),
    };
    let value = call.new_object_with_events(|events| Sensor {
        base: base.clone(),
        pin,
        name,
        inverted,
        pull,
        active_delay: Cell::new(active_delay),
        inactive_delay: Cell::new(inactive_delay),
        raw: Cell::new(starts),
        state: Cell::new(starts),
        waits: Cell::new(0),
        followers: Followers::default(),
        events,
    });
    if let (Some(base), Some(pin)) = (&base, pin) {
        let made = call.own_value::<Sensor>(&value).ok_or_else(|| Error::new("hardware_error", format!("{what}: the sensor couldn't be kept")))?;
        base.watch_pin(pin, pull, Rc::downgrade(&made), what)?;
        // (Its pin's changes come while the script waits: a script without windows waits for them
        // when it set one of its events' functions.)
        if !base.simulated() {
            made.events.keep_running(true);
        }
    }
    Ok(value)
}

impl Sensor {
    pub(crate) fn pin(&self) -> Option<u8> {
        self.pin
    }

    pub(crate) fn is_active(&self) -> Option<bool> {
        self.state.get()
    }

    /// How it is made, for a layout file: its constructor's settings.
    pub(crate) fn settings(&self) -> Value {
        let mut pairs = vec![("name", Value::from(self.name.as_str()))];
        if let Some(p) = self.pin {
            pairs.push(("pin", Value::from(i64::from(p))));
            pairs.push(("pull", Value::from(self.pull)));
        }
        pairs.push(("inverted", Value::from(self.inverted)));
        pairs.push(("active_delay", Value::from(self.active_delay.get())));
        pairs.push(("inactive_delay", Value::from(self.inactive_delay.get())));
        Value::map(pairs)
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Its base station was closed: nothing more comes from its pin.
    pub(crate) fn base_closed(&self) {
        self.events.keep_running(false);
    }

    pub(crate) fn follow(&self, who: Weak<dyn Follower>) {
        self.followers.add(who);
    }

    fn word(state: Option<bool>) -> &'static str {
        match state {
            Some(true) => "active",
            Some(false) => "inactive",
            None => "unknown",
        }
    }

    /// The pin's level the board said (true: high).
    pub(crate) fn heard(&self, high: bool, call: &Call) {
        self.raw_now(high == self.inverted, call);
    }

    /// What the pin (or `simulate()`) says now: taken at once, or once it has lasted its delay.
    fn raw_now(&self, active: bool, call: &Call) {
        self.raw.set(Some(active));
        // (A change still waiting is over: this one is newer.)
        let wait = self.waits.get() + 1;
        self.waits.set(wait);
        let delay = if active { self.active_delay.get() } else { self.inactive_delay.get() };
        // (The first word heard is taken at once: the delays are for changes. A change that went back
        // within its delay, or one taken at once, leaves nothing waiting: a wait still on its way is
        // no longer the newest.)
        if self.state.get() == Some(active) || delay == 0 || self.state.get().is_none() {
            self.events.keep_working(false);
            self.settle(active, call);
            return;
        }
        let events = self.events.clone();
        self.events.keep_working(true);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(delay as u64));
            events.post("sensor_waited", vec![Value::Int(wait as i64)]);
        });
    }

    /// Its state changed: its events, then the objects that follow it.
    fn settle(&self, active: bool, call: &Call) {
        if self.state.replace(Some(active)) == Some(active) {
            return;
        }
        let word = Self::word(Some(active));
        self.events.fire("on_changed", vec![Value::from(word)]);
        self.events.fire(if active { "on_active" } else { "on_inactive" }, vec![]);
        self.followers.tell(call);
    }

    /// What the board said since the script's last call, taken in (its posts may still be on the way).
    pub(crate) fn catch_up(&self, call: &Call) {
        if let Some(base) = &self.base {
            base.take_heard(call);
        }
    }
}

impl Object for Sensor {
    fn kind(&self) -> &str {
        "sensor"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "pin" => self.pin.map_or(Value::Null, |p| Value::from(i64::from(p))),
            "name" => Value::from(self.name.as_str()),
            "state" => {
                self.catch_up(call);
                Value::from(Self::word(self.state.get()))
            }
            "active" => {
                self.catch_up(call);
                Value::from(self.state.get() == Some(true))
            }
            "inverted" => Value::from(self.inverted),
            "pull" => Value::from(self.pull),
            "active_delay" => Value::from(self.active_delay.get()),
            "inactive_delay" => Value::from(self.inactive_delay.get()),
            "base" => match &self.base {
                Some(b) => call.object_value(b),
                None => Value::Null,
            },
            other => return Err(call.unknown_property("sensor", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "active_delay" => self.active_delay.set(delay_of("a sensor", "active_delay", &value)?),
            "inactive_delay" => self.inactive_delay.set(delay_of("a sensor", "inactive_delay", &value)?),
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a sensor's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("sensor", other, PROPS)),
        }
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        match function {
            "simulate" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let active = match call.arg(0) {
                    Value::Text(t) if t == "active" => true,
                    Value::Text(t) if t == "inactive" => false,
                    Value::Bool(b) => *b,
                    other => return Err(Error::type_error(format!("sensor.simulate() takes \"active\" or \"inactive\" (or true or false), not {}", describe(other)))),
                };
                self.catch_up(call);
                self.raw_now(active, call);
                Ok(Value::Null)
            }
            // (Posted by the thread a delay waits on, the kit's level 6: not listed for scripts.)
            "sensor_waited" => {
                let wait = call.int(0)? as u64;
                if wait == self.waits.get() {
                    self.events.keep_working(false);
                    if let Some(active) = self.raw.get() {
                        self.settle(active, call);
                    }
                }
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("sensor", other, FUNCS)),
        }
    }
}
