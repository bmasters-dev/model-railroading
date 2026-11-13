//! Turnouts (design/model_railroading.md, "Phase C, item 10", model-railroading#14): a turnout thrown
//! by its accessory decoder through the base station, by the NMRA's basic accessory packet (S-9.2.1
//! 2.4.1). Its output address 1 to 2044 is the one throttles and command stations show; closed is the
//! pair's output 1, thrown output 0 (RCN-213's numbering), `inverted: true` for one wired the other
//! way. Each move is sent switched on twice, then switched off (a decoder for coils times its own
//! pulse; the off lets go of one that doesn't).
//!
//! What it was told (`commanded`) and where it is known to lie (`state`) are apart when it has
//! feedback: one sensor (active: thrown), two sensors (one at each end of its throw), or only a
//! `move_time` for a slow motor or a servo ("moving" until it has passed). A move the feedback doesn't
//! confirm within `move_time` is said by `on_mismatch`. A `locked` turnout refuses to move (a route
//! holding it under a train).

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::{describe, DccBase};
use crate::dcc;
use crate::sensor::{as_follower, Follower, Followers, Sensor};

/// How long a turnout with sensors may take to move before its feedback is said not to agree, unless
/// `move_time:` says otherwise.
const SENSOR_MOVE_TIME: i64 = 3000;
const MOST_MOVE_TIME: i64 = 60_000;

pub struct Turnout {
    base: Rc<DccBase>,
    address: u16,
    name: String,
    inverted: bool,
    /// Its feedback sensors: none, one (active: thrown), or two (closed's, then thrown's).
    sensors: Vec<Rc<Sensor>>,
    move_time: i64,
    /// What it was last told (None: not yet).
    commanded: Cell<Option<bool>>,
    /// Where it is known to lie, as `state` says it.
    known: Cell<Known>,
    /// Counts the moves: a move's wait whose count is no longer the newest is over.
    moves: Cell<u64>,
    locked: Cell<bool>,
    /// The lights following it.
    pub(crate) followers: Followers,
    events: Events,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Known {
    Closed,
    Thrown,
    Unknown,
    /// On its way (a move_time not yet passed, or two sensors both inactive).
    Moving,
    /// Two sensors both active.
    Inconsistent,
}

impl Known {
    fn word(self) -> &'static str {
        match self {
            Known::Closed => "closed",
            Known::Thrown => "thrown",
            Known::Unknown => "unknown",
            Known::Moving => "moving",
            Known::Inconsistent => "inconsistent",
        }
    }

    fn of(closed: Option<bool>) -> Known {
        match closed {
            Some(true) => Known::Closed,
            Some(false) => Known::Thrown,
            None => Known::Unknown,
        }
    }
}

const PROPS: &[&str] = &["address", "name", "state", "commanded", "inverted", "feedback", "sensors", "move_time", "locked", "base"];
const FUNCS: &[&str] = &["close", "throw", "toggle"];

/// `turnout($address, name:, inverted: false, sensor:, sensors:, move_time:, base:)`.
pub fn new_turnout(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&["name", "inverted", "base", "sensor", "sensors", "move_time"])?;
    let what = "turnout()";
    let address = match call.arg(0).as_int() {
        Some(n @ 1..=2044) => n as u16,
        _ => return Err(Error::range_error(format!("{what}: a turnout's address is its accessory decoder's output, 1 to {}, not {}", dcc::ACCESSORY_MOST, describe(call.arg(0))))),
    };
    let name = match call.option("name") {
        None => format!("T{address}"),
        Some(v) => v.as_text().filter(|t| !t.trim().is_empty() && !t.contains(['\n', '\r'])).map(|t| t.trim().to_string()).ok_or_else(|| Error::type_error(format!("{what}'s `name:` is text on one line, not {}", describe(v))).at(call.option_line("name")))?,
    };
    let inverted = match call.option("inverted") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(other) => return Err(Error::type_error(format!("{what}'s `inverted:` is true or false, not {}", describe(other))).at(call.option_line("inverted"))),
    };
    let base = match call.option("base") {
        None => DccBase::last().ok_or_else(|| Error::new("hardware_error", format!("{what}: there is no base station: make one first, dcc_base(\"/dev/ttyACM0\") or dcc_base(simulate: true)")))?,
        Some(v) => call.own_option::<DccBase>("base").ok_or_else(|| Error::type_error(format!("{what}'s `base:` is a dcc_base, not {}", describe(v))).at(call.option_line("base")))?,
    };
    let sensors = match (call.option("sensor"), call.option("sensors")) {
        (Some(_), Some(_)) => return Err(Error::type_error(format!("{what}: give `sensor:` (one, active when thrown) or `sensors:` (two: closed's, then thrown's), not both"))),
        (Some(v), None) => vec![call.own_option::<Sensor>("sensor").ok_or_else(|| Error::type_error(format!("{what}'s `sensor:` is a sensor, not {}", describe(v))).at(call.option_line("sensor")))?],
        (None, Some(v)) => {
            let wrong = || Error::type_error(format!("{what}'s `sensors:` is a list of two sensors, the one active when it is closed, then the one active when it is thrown, not {}", describe(v))).at(call.option_line("sensors"));
            let Value::Array(items) = v else { return Err(wrong()) };
            if items.len() != 2 {
                return Err(wrong());
            }
            let mut two = Vec::new();
            for (_, item) in items {
                two.push(call.own_value::<Sensor>(item).ok_or_else(wrong)?);
            }
            if Rc::ptr_eq(&two[0], &two[1]) {
                return Err(Error::type_error(format!("{what}'s `sensors:` are two different sensors, one at each end of the throw")).at(call.option_line("sensors")));
            }
            two
        }
        (None, None) => Vec::new(),
    };
    let move_time = match call.option("move_time") {
        None if sensors.is_empty() => 0,
        None => SENSOR_MOVE_TIME,
        Some(v) => match v.as_int() {
            Some(n @ 0..=MOST_MOVE_TIME) => n,
            _ => return Err(Error::range_error(format!("{what}'s `move_time:` is milliseconds, 0 to {MOST_MOVE_TIME}, not {}", describe(v))).at(call.option_line("move_time"))),
        },
    };
    let value = call.new_object_with_events(|events| Turnout {
        base,
        address,
        name,
        inverted,
        sensors,
        move_time,
        commanded: Cell::new(None),
        known: Cell::new(Known::Unknown),
        moves: Cell::new(0),
        locked: Cell::new(false),
        followers: Followers::default(),
        events,
    });
    if let Some(made) = call.own_value::<Turnout>(&value)
        && !made.sensors.is_empty()
    {
        for s in &made.sensors {
            s.catch_up(call);
        }
        made.known.set(made.sensed());
        let me = as_follower(&made);
        for s in &made.sensors {
            s.follow(me.clone());
        }
    }
    Ok(value)
}

impl Turnout {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// How it is made, for a layout file: its constructor's settings, its sensors by name.
    pub(crate) fn settings(&self) -> Value {
        let mut pairs = vec![("name", Value::from(self.name.as_str())), ("address", Value::from(i64::from(self.address))), ("inverted", Value::from(self.inverted))];
        match self.sensors.as_slice() {
            [one] => pairs.push(("sensor", Value::from(one.name()))),
            [closed, thrown] => pairs.push(("sensors", Value::list([Value::from(closed.name()), Value::from(thrown.name())]))),
            _ => {}
        }
        pairs.push(("move_time", Value::from(self.move_time)));
        Value::map(pairs)
    }

    pub(crate) fn sensors(&self) -> &[Rc<Sensor>] {
        &self.sensors
    }

    /// "closed", "thrown", "unknown", "moving", or "inconsistent".
    pub(crate) fn state_word(&self) -> &'static str {
        self.known.get().word()
    }

    /// Its state as the phones' protocol numbers it: 1 unknown, 2 closed, 4 thrown, 8 inconsistent
    /// (on its way too).
    pub(crate) fn wire_state(&self) -> u8 {
        match self.known.get() {
            Known::Closed => 2,
            Known::Thrown => 4,
            Known::Unknown => 1,
            Known::Moving | Known::Inconsistent => 8,
        }
    }

    /// Where its sensors say it lies (without sensors: where it was told).
    fn sensed(&self) -> Known {
        match self.sensors.as_slice() {
            [one] => match one.is_active() {
                Some(active) => Known::of(Some(!active)),
                None => Known::Unknown,
            },
            [closed, thrown] => match (closed.is_active(), thrown.is_active()) {
                (Some(true), Some(false)) => Known::Closed,
                (Some(false), Some(true)) => Known::Thrown,
                (Some(true), Some(true)) => Known::Inconsistent,
                (Some(false), Some(false)) => Known::Moving,
                _ => Known::Unknown,
            },
            _ => Known::of(self.commanded.get()),
        }
    }

    /// Where it lies now: said by its events, to the phones, and to the lights following it when it
    /// changed.
    fn now(&self, known: Known, call: &Call) {
        if self.known.replace(known) != known {
            crate::phone::turnout_changed(&self.base, self);
            self.events.fire("on_changed", vec![Value::from(known.word())]);
            self.followers.tell(call);
        }
    }

    /// Moved: the packets sent, what it was told kept, and where it lies worked out (at once without
    /// feedback, else once its sensors say so or its move_time has passed).
    pub(crate) fn set(&self, closed: bool, call: &mut Call) -> Result<(), Error> {
        let words = if closed { "closed" } else { "thrown" };
        if self.locked.get() {
            return Err(Error::new("hardware_error", format!("turnout {} ({}) is locked: it isn't {words} until it is unlocked (locked = false)", self.address, self.name)));
        }
        let output = closed != self.inverted;
        self.base.send_now(dcc::accessory(self.address, output, true), format!("turnout {} ({}): {words}", self.address, self.name), 2, call)?;
        self.base.send_now(dcc::accessory(self.address, output, false), format!("turnout {} ({}): let go", self.address, self.name), 1, call)?;
        self.commanded.set(Some(closed));
        let wanted = Known::of(Some(closed));
        let known = if self.sensors.is_empty() {
            if self.move_time > 0 && self.known.get() != wanted { Known::Moving } else { wanted }
        } else {
            // (Taken in first: what the board said since the script's last call.)
            for s in &self.sensors {
                s.catch_up(call);
            }
            self.sensed()
        };
        let move_number = self.moves.get() + 1;
        self.moves.set(move_number);
        if known != wanted && self.move_time > 0 {
            let events = self.events.clone();
            self.events.keep_working(true);
            let wait = self.move_time as u64;
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(wait));
                events.post("turnout_moved", vec![Value::Int(move_number as i64)]);
            });
        } else {
            // (Lying its new way already, or nothing to wait for: no move waits.)
            self.events.keep_working(false);
        }
        self.now(known, call);
        Ok(())
    }

    /// Its move_time has passed: without sensors it lies its new way now; with them, sensors that
    /// don't agree are said by `on_mismatch`.
    fn moved(&self, move_number: u64, call: &Call) {
        if move_number != self.moves.get() {
            return;
        }
        self.events.keep_working(false);
        let Some(closed) = self.commanded.get() else { return };
        let wanted = Known::of(Some(closed));
        if self.sensors.is_empty() {
            self.now(wanted, call);
        } else if self.known.get() != wanted {
            self.events.fire("on_mismatch", vec![Value::from(wanted.word()), Value::from(self.known.get().word())]);
        }
    }
}

impl Follower for Turnout {
    fn followed_changed(&self, call: &Call) {
        let known = self.sensed();
        self.now(known, call);
        // (Lying its new way: the move's wait is over.)
        if self.commanded.get().is_some_and(|c| Known::of(Some(c)) == known) {
            self.moves.set(self.moves.get() + 1);
            self.events.keep_working(false);
        }
    }
}

impl Object for Turnout {
    fn kind(&self) -> &str {
        "turnout"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "address" => Value::from(i64::from(self.address)),
            "name" => Value::from(self.name.as_str()),
            "state" => {
                for s in &self.sensors {
                    s.catch_up(call);
                }
                Value::from(self.state_word())
            }
            "commanded" => Value::from(Known::of(self.commanded.get()).word()),
            "inverted" => Value::from(self.inverted),
            "feedback" => Value::from(match (self.sensors.len(), self.move_time) {
                (1, _) => "one_sensor",
                (2, _) => "two_sensors",
                (_, 0) => "none",
                _ => "delay",
            }),
            "sensors" => Value::list(self.sensors.iter().map(|s| call.object_value(s)).collect::<Vec<_>>()),
            "move_time" => Value::from(self.move_time),
            "locked" => Value::from(self.locked.get()),
            "base" => call.object_value(&self.base),
            other => return Err(call.unknown_property("turnout", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "locked" => match value {
                Value::Bool(b) => self.locked.set(b),
                other => return Err(Error::type_error(format!("a turnout's `locked` is true or false, not {}", describe(&other)))),
            },
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a turnout's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("turnout", other, PROPS)),
        }
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        match function {
            "close" | "throw" | "toggle" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                let closed = match function {
                    "close" => true,
                    "throw" => false,
                    // (The other way from what it was told, or, never told, from where it lies.)
                    _ => match self.commanded.get() {
                        Some(c) => !c,
                        None => self.known.get() != Known::Closed,
                    },
                };
                self.set(closed, call)?;
                Ok(Value::Null)
            }
            // (Posted by the thread a move_time waits on, the kit's level 6: not listed for scripts.)
            "turnout_moved" => {
                let n = call.int(0)? as u64;
                self.moved(n, call);
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("turnout", other, FUNCS)),
        }
    }
}
