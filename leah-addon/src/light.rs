//! Lights (design/model_railroading.md, "Phase C, item 10", model-railroading#14): an accessory
//! decoder's output switched on and off, for scenery lamps, fascia indicators, uncouplers, and
//! animations, through the base station by the NMRA's basic accessory packet (S-9.2.1 2.4.1). On is the
//! pair's output 1 (as a turnout's closed), off its output 0, each sent switched on twice and never let
//! go (a lamp's decoder keeps its output as last set); `inverted: true` swaps them. A light can follow
//! a sensor, a turnout, or a block, and can be switched on for a while (`pulse`).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::{describe, DccBase};
use crate::block::Block;
use crate::dcc;
use crate::sensor::{as_follower, Follower, Followers, Sensor};
use crate::turnout::Turnout;

const PROPS: &[&str] = &["address", "name", "on", "inverted", "following", "base"];
const FUNCS: &[&str] = &["toggle", "pulse", "follow"];
/// The longest pulse (ten minutes).
const MOST_PULSE: i64 = 600_000;

/// What a light follows, and the state of it that turns the light on.
enum Following {
    Sensor(Rc<Sensor>, bool),
    Turnout(Rc<Turnout>, &'static str),
    Block(Rc<Block>, bool),
}

pub struct Light {
    base: Rc<DccBase>,
    address: u16,
    name: String,
    inverted: bool,
    /// Whether it is on (None: not yet set).
    on: Cell<Option<bool>>,
    following: RefCell<Option<Following>>,
    /// Counts the pulses: a pulse's end whose count is no longer the newest is over.
    pulses: Cell<u64>,
    /// Itself, for following (set as it is made).
    me: RefCell<Weak<Light>>,
    /// The track diagrams showing it.
    pub(crate) followers: Followers,
    events: Events,
}

/// `light($address, name:, inverted: false, base:)`.
pub fn new_light(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&["name", "inverted", "base"])?;
    let what = "light()";
    let address = match call.arg(0).as_int() {
        Some(n @ 1..=2044) => n as u16,
        _ => return Err(Error::range_error(format!("{what}: a light's address is its accessory decoder's output, 1 to {}, not {}", dcc::ACCESSORY_MOST, describe(call.arg(0))))),
    };
    let name = match call.option("name") {
        None => format!("L{address}"),
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
    let value = call.new_object_with_events(|events| Light { base, address, name, inverted, on: Cell::new(None), following: RefCell::new(None), pulses: Cell::new(0), me: RefCell::new(Weak::new()), followers: Followers::default(), events });
    if let Some(made) = call.own_value::<Light>(&value) {
        *made.me.borrow_mut() = Rc::downgrade(&made);
    }
    Ok(value)
}

impl Light {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// How it is made, for a layout file: its constructor's settings, and what it follows by name.
    pub(crate) fn settings(&self) -> Value {
        let mut pairs = vec![("name", Value::from(self.name.as_str())), ("address", Value::from(i64::from(self.address))), ("inverted", Value::from(self.inverted))];
        if let Some((name, when)) = self.followed() {
            pairs.push(("follow", Value::from(name)));
            pairs.push(("when", Value::from(when)));
        }
        Value::map(pairs)
    }

    /// Switched the other way, as `toggle()` does (a pulse under way is over).
    pub(crate) fn toggle_now(&self, call: &Call) -> Result<(), Error> {
        self.pulses.set(self.pulses.get() + 1);
        self.events.keep_working(false);
        self.switch(self.on.get() != Some(true), call)
    }

    /// Switched on or off, as setting `on` does (a signal head's lamps).
    pub(crate) fn switch_to(&self, on: bool, call: &Call) -> Result<(), Error> {
        self.pulses.set(self.pulses.get() + 1);
        self.switch(on, call)
    }

    /// Known to be off (never switched: not known, maybe lit on the layout).
    pub(crate) fn is_off(&self) -> bool {
        self.on.get() == Some(false)
    }

    pub(crate) fn is_on(&self) -> bool {
        self.on.get() == Some(true)
    }

    /// What it follows, by name, and the state of it that turns it on.
    pub(crate) fn followed(&self) -> Option<(String, &'static str)> {
        Some(match self.following.borrow().as_ref()? {
            Following::Sensor(s, on) => (s.name().to_string(), if *on { "active" } else { "inactive" }),
            Following::Turnout(t, when) => (t.name().to_string(), when),
            Following::Block(b, on) => (b.name().to_string(), if *on { "occupied" } else { "free" }),
        })
    }

    /// Follows a sensor, a turnout, or a block (`Value::Null`: nothing), on while it is `when`.
    pub(crate) fn follow(&self, target: &Value, when: Option<&str>, call: &Call) -> Result<(), Error> {
        let what = "light.follow()";
        let bad_when = |allowed: &str| Error::range_error(format!("{what}'s `when:` is {allowed}, not {:?}", when.unwrap_or_default()));
        let following = if matches!(target, Value::Null) {
            None
        } else if let Some(s) = call.own_value::<Sensor>(target) {
            let on = match when {
                None | Some("active") => true,
                Some("inactive") => false,
                _ => return Err(bad_when("\"active\" or \"inactive\"")),
            };
            Some(Following::Sensor(s, on))
        } else if let Some(t) = call.own_value::<Turnout>(target) {
            let on = match when {
                None | Some("thrown") => "thrown",
                Some("closed") => "closed",
                _ => return Err(bad_when("\"thrown\" or \"closed\"")),
            };
            Some(Following::Turnout(t, on))
        } else if let Some(b) = call.own_value::<Block>(target) {
            let on = match when {
                None | Some("occupied") => true,
                Some("free") => false,
                _ => return Err(bad_when("\"occupied\" or \"free\"")),
            };
            Some(Following::Block(b, on))
        } else {
            return Err(Error::type_error(format!("{what} follows a sensor, a turnout, or a block (null: nothing), not {}", describe(target))));
        };
        let me = self.me.borrow().upgrade().map(|me| as_follower(&me));
        if let (Some(f), Some(me)) = (&following, me) {
            match f {
                Following::Sensor(s, _) => s.follow(me),
                Following::Turnout(t, _) => t.followers.add(me),
                Following::Block(b, _) => b.followers.add(me),
            }
        }
        // (Following a board's detector: it may switch while the script waits.)
        let live = match &following {
            Some(Following::Sensor(s, _)) => s.is_live(),
            Some(Following::Turnout(t, _)) => t.is_live(),
            Some(Following::Block(b, _)) => b.is_live(),
            None => false,
        };
        self.events.keep_running(live);
        *self.following.borrow_mut() = following;
        // (At once, as what it follows is now.)
        if let Some(on) = self.followed_says() {
            self.switch(on, call)?;
        }
        Ok(())
    }

    /// Switched on or off: the packets sent, its state kept, its event fired when it changed.
    fn switch(&self, on: bool, call: &Call) -> Result<(), Error> {
        let output = on != self.inverted;
        self.base.send_now(dcc::accessory(self.address, output, true), format!("light {} ({}): {}", self.address, self.name, if on { "on" } else { "off" }), 2, call)?;
        if self.on.replace(Some(on)) != Some(on) {
            self.events.fire("on_changed", vec![Value::from(on)]);
            self.followers.tell(call);
        }
        Ok(())
    }

    /// Whether what it follows says it is on now (None: it follows nothing, or that isn't known).
    fn followed_says(&self) -> Option<bool> {
        match self.following.borrow().as_ref()? {
            Following::Sensor(s, when) => s.is_active().map(|a| a == *when),
            Following::Turnout(t, when) => match t.state_word() {
                "closed" | "thrown" => Some(t.state_word() == *when),
                _ => None,
            },
            Following::Block(b, when) => b.is_occupied().map(|o| o == *when),
        }
    }
}

impl Follower for Light {
    fn source_closed(&self) {
        let live = match self.following.borrow().as_ref() {
            Some(Following::Sensor(s, _)) => s.is_live(),
            Some(Following::Turnout(t, _)) => t.is_live(),
            Some(Following::Block(b, _)) => b.is_live(),
            None => false,
        };
        self.events.keep_running(live);
        self.followers.tell_closed();
    }

    fn followed_changed(&self, call: &Call) {
        // (Only a change: what it followed before still tells it of its own changes.)
        // (What it follows has the last word: a pulse under way is over.)
        if self.followed_says().is_some() {
            self.pulses.set(self.pulses.get() + 1);
            self.events.keep_working(false);
        }
        if let Some(on) = self.followed_says().filter(|on| self.on.get() != Some(*on))
            && let Err(e) = self.switch(on, call)
        {
            call.warn(format!("light {} ({}) couldn't follow: {}", self.address, self.name, e.message));
        }
    }
}

impl Object for Light {
    fn kind(&self) -> &str {
        "light"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "address" => Value::from(i64::from(self.address)),
            "name" => Value::from(self.name.as_str()),
            "on" => Value::from(self.on.get() == Some(true)),
            "inverted" => Value::from(self.inverted),
            "following" => match self.following.borrow().as_ref() {
                Some(Following::Sensor(s, _)) => call.object_value(s),
                Some(Following::Turnout(t, _)) => call.object_value(t),
                Some(Following::Block(b, _)) => call.object_value(b),
                None => Value::Null,
            },
            "base" => call.object_value(&self.base),
            other => return Err(call.unknown_property("light", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "on" => match value {
                Value::Bool(b) => {
                    // (Set by the script: a pulse under way is over.)
                    self.pulses.set(self.pulses.get() + 1);
                    self.events.keep_working(false);
                    self.switch(b, call)?;
                }
                other => return Err(Error::type_error(format!("a light's `on` is true or false, not {}", describe(&other)))),
            },
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a light's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("light", other, PROPS)),
        }
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("light.{function}()");
        match function {
            "toggle" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.pulses.set(self.pulses.get() + 1);
                self.events.keep_working(false);
                self.switch(self.on.get() != Some(true), call)?;
                Ok(Value::Null)
            }
            "pulse" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let ms = match call.arg(0).as_int() {
                    Some(n @ 1..=MOST_PULSE) => n,
                    _ => return Err(Error::range_error(format!("{what}: how long it stays on is milliseconds, 1 to {MOST_PULSE}, not {}", describe(call.arg(0))))),
                };
                self.switch(true, call)?;
                let pulse = self.pulses.get() + 1;
                self.pulses.set(pulse);
                let events = self.events.clone();
                self.events.keep_working(true);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(ms as u64));
                    events.post("light_pulse_over", vec![Value::Int(pulse as i64)]);
                });
                Ok(Value::Null)
            }
            "follow" => {
                call.count_between(1, 1)?;
                call.only_options(&["when"])?;
                let when = call.option("when").map(|v| v.as_text().map(str::to_string).ok_or_else(|| Error::type_error(format!("{what}'s `when:` is text, not {}", describe(v))).at(call.option_line("when")))).transpose()?;
                let target = call.arg(0).clone();
                self.follow(&target, when.as_deref(), call).map_err(|e| if e.message.contains("`when:`") { e.at(call.option_line("when")) } else { e })?;
                Ok(Value::Null)
            }
            // (Posted by the thread a pulse waits on, the kit's level 6: not listed for scripts.)
            "light_pulse_over" => {
                let n = call.int(0)? as u64;
                if n == self.pulses.get() {
                    self.events.keep_working(false);
                    self.switch(false, call)?;
                }
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("light", other, FUNCS)),
        }
    }
}
