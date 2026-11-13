//! Blocks (design/model_railroading.md, "Phase C, item 10", model-railroading#14): a stretch of track
//! whose occupancy comes from a sensor (a block detector), or from the script for one without, and the
//! train in it: a locomotive, a consist, or a name, given by the script (passing it along from block to
//! block comes with the track plan, #16). When the block clears, its train is given to `on_cleared`
//! and the block holds none.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::{describe, Locomotive};
use crate::consist::Consist;
use crate::sensor::{as_follower, Follower, Followers, Sensor};

const PROPS: &[&str] = &["name", "sensor", "state", "occupied", "train", "reserved", "reserved_by"];

/// The train in a block, as the script gave it.
enum Train {
    Locomotive(Rc<Locomotive>),
    Consist(Rc<Consist>),
    /// A name or a number.
    Plain(Value),
}

pub struct Block {
    name: String,
    sensor: Option<Rc<Sensor>>,
    /// Whether it is occupied (None: not known yet), as last said.
    occupied: Cell<Option<bool>>,
    train: RefCell<Option<Train>>,
    /// The lights and routes following it.
    pub(crate) followers: Followers,
    /// The route holding it for a train (route.rs), by name, while it does.
    pub(crate) reserved_by: RefCell<Option<String>>,
    events: Events,
}

/// `block($name, sensor:)`.
pub fn new_block(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&["sensor"])?;
    let what = "block()";
    let name = call.arg(0).as_text().filter(|t| !t.trim().is_empty() && !t.contains(['\n', '\r'])).map(|t| t.trim().to_string()).ok_or_else(|| Error::type_error(format!("{what}: a block's name is text on one line, like \"East siding\", not {}", describe(call.arg(0)))))?;
    let sensor = match call.option("sensor") {
        None | Some(Value::Null) => None,
        Some(v) => Some(call.own_option::<Sensor>("sensor").ok_or_else(|| Error::type_error(format!("{what}'s `sensor:` is the sensor its occupancy comes from, not {}", describe(v))).at(call.option_line("sensor")))?),
    };
    // (Without a sensor it is free until the script says otherwise.)
    let occupied = match &sensor {
        Some(s) => {
            s.catch_up(call);
            s.is_active()
        }
        None => Some(false),
    };
    let value = call.new_object_with_events(|events| Block { name, sensor, occupied: Cell::new(occupied), train: RefCell::new(None), followers: Followers::default(), reserved_by: RefCell::new(None), events });
    if let Some(made) = call.own_value::<Block>(&value)
        && let Some(s) = &made.sensor
    {
        s.follow(as_follower(&made));
        // (Its detector on a board: its events may come while the script waits.)
        if s.is_live() {
            made.events.keep_running(true);
        }
    }
    Ok(value)
}

impl Block {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// How it is made, for a layout file: its name and its sensor's.
    pub(crate) fn settings(&self) -> Value {
        let mut pairs = vec![("name", Value::from(self.name.as_str()))];
        if let Some(s) = &self.sensor {
            pairs.push(("sensor", Value::from(s.name())));
        }
        Value::map(pairs)
    }

    /// Occupied or free, said by the layout simulator: through its sensor, or, without one, as the
    /// script's `occupied` would say it.
    pub(crate) fn simulate_occupied(&self, occupied: bool, call: &Call) {
        match &self.sensor {
            Some(s) => s.simulate_now(occupied, call),
            None => self.now(Some(occupied), call),
        }
    }

    /// Held by a route, or let go: its followers (a track diagram) told.
    pub(crate) fn reserve_for(&self, route: Option<&str>, call: &Call) {
        *self.reserved_by.borrow_mut() = route.map(str::to_string);
        self.followers.tell(call);
    }

    pub(crate) fn sensor(&self) -> Option<&Rc<Sensor>> {
        self.sensor.as_ref()
    }

    /// Whether a board's detector changes it.
    pub(crate) fn is_live(&self) -> bool {
        self.sensor.as_ref().is_some_and(|s| s.is_live())
    }

    pub(crate) fn is_occupied(&self) -> Option<bool> {
        self.occupied.get()
    }

    fn word(occupied: Option<bool>) -> &'static str {
        match occupied {
            Some(true) => "occupied",
            Some(false) => "free",
            None => "unknown",
        }
    }

    fn train_value(&self, call: &Call) -> Value {
        match self.train.borrow().as_ref() {
            Some(Train::Locomotive(l)) => call.object_value(l),
            Some(Train::Consist(c)) => call.object_value(c),
            Some(Train::Plain(v)) => v.clone(),
            None => Value::Null,
        }
    }

    /// Occupied or free now: its events (a train leaving given to on_cleared), then its followers.
    fn now(&self, occupied: Option<bool>, call: &Call) {
        if self.occupied.replace(occupied) == occupied {
            return;
        }
        self.events.fire("on_changed", vec![Value::from(Self::word(occupied))]);
        match occupied {
            Some(true) => self.events.fire("on_occupied", vec![]),
            Some(false) => {
                let train = self.train_value(call);
                self.train.borrow_mut().take();
                self.events.fire("on_cleared", vec![train]);
            }
            None => {}
        }
        self.followers.tell(call);
    }
}

impl Follower for Block {
    fn source_closed(&self) {
        self.events.keep_running(false);
        self.followers.tell_closed();
    }

    fn followed_changed(&self, call: &Call) {
        let Some(s) = &self.sensor else { return };
        self.now(s.is_active(), call);
    }
}

impl Object for Block {
    fn kind(&self) -> &str {
        "block"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        if let Some(s) = &self.sensor {
            s.catch_up(call);
        }
        Ok(match property {
            "name" => Value::from(self.name.as_str()),
            "sensor" => match &self.sensor {
                Some(s) => call.object_value(s),
                None => Value::Null,
            },
            "state" => Value::from(Self::word(self.occupied.get())),
            "occupied" => Value::from(self.occupied.get() == Some(true)),
            "train" => self.train_value(call),
            "reserved" => Value::from(self.reserved_by.borrow().is_some()),
            "reserved_by" => self.reserved_by.borrow().as_deref().map_or(Value::Null, Value::from),
            other => return Err(call.unknown_property("block", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "occupied" => {
                if self.sensor.is_some() {
                    return Err(Error::type_error(format!("block {}'s `occupied` comes from its sensor: it can be read but not set", self.name)));
                }
                match value {
                    Value::Bool(b) => self.now(Some(b), call),
                    other => return Err(Error::type_error(format!("a block's `occupied` is true or false, not {}", describe(&other)))),
                }
            }
            "train" => {
                let train = if let Some(l) = call.own_value::<Locomotive>(&value) {
                    Some(Train::Locomotive(l))
                } else if let Some(c) = call.own_value::<Consist>(&value) {
                    Some(Train::Consist(c))
                } else {
                    match value {
                        Value::Null => None,
                        v @ (Value::Text(_) | Value::Int(_) | Value::Float(_)) => Some(Train::Plain(v)),
                        other => return Err(Error::type_error(format!("a block's `train` is a locomotive, a consist, a name, or a number (null: none), not {}", describe(&other)))),
                    }
                };
                *self.train.borrow_mut() = train;
            }
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a block's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("block", other, PROPS)),
        }
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        Err(call.unknown_function("block", function, &[]))
    }
}
