//! Running trains automatically (design/model_railroading.md, "Phase D, item 17", model-railroading#21):
//! one `auto_train` runs a locomotive along a path a track plan found. `start()` holds the path for it
//! (or says `on_blocked`), lines its turnouts, and, once they lie its way, departs at `max_speed`; on
//! entering the block before the last it slows to `approach_speed`, and it stops as the train reaches
//! the last (`on_arrived`). Each block let go as the train clears it is the route's own work (route.rs).
//!
//! It follows its blocks and turnouts; each change is posted to its own function (`auto_check`, the
//! kit's level 6), which drives the locomotive with a call's context.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::{describe, Locomotive};
use crate::route::Route;
use crate::sensor::{as_follower, Follower};

const PROPS: &[&str] = &["locomotive", "route", "state", "block", "max_speed", "approach_speed", "direction"];
const FUNCS: &[&str] = &["start", "halt", "resume", "emergency_stop"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Waiting,
    Lining,
    Running,
    Approaching,
    Halted,
    Arrived,
    Blocked,
}

impl State {
    fn word(self) -> &'static str {
        match self {
            State::Waiting => "waiting",
            State::Lining => "lining",
            State::Running => "running",
            State::Approaching => "approaching",
            State::Halted => "halted",
            State::Arrived => "arrived",
            State::Blocked => "blocked",
        }
    }
}

pub struct AutoTrain {
    loco: Rc<Locomotive>,
    route: Rc<Route>,
    max_speed: Cell<f64>,
    approach_speed: Cell<f64>,
    forward: bool,
    state: Cell<State>,
    /// What it was doing when halted, to resume.
    before_halt: Cell<State>,
    /// The furthest of its blocks the train has reached.
    reached: Cell<usize>,
    /// Blocked, waiting for its path to be free (`start(wait: true)`).
    waiting: Cell<bool>,
    me: RefCell<Weak<AutoTrain>>,
    events: Events,
}

fn percent_of(v: &Value, what: &str, name: &str) -> Result<f64, Error> {
    match v.as_number() {
        Some(n) if n > 0.0 && n <= 100.0 => Ok(n),
        _ => Err(Error::range_error(format!("{what}'s `{name}` is a percent of full speed, more than 0 and up to 100, not {}", describe(v)))),
    }
}

/// `auto_train($loco, $route, max_speed: 60, approach_speed: 20, direction: "forward")`.
pub fn new_auto_train(call: &mut Call) -> Result<Value, Error> {
    let what = "auto_train()";
    call.count_between(2, 2)?;
    call.only_options(&["max_speed", "approach_speed", "direction"])?;
    let loco = call.own::<Locomotive>(0).ok_or_else(|| Error::type_error(format!("{what}: the first value is the locomotive it runs, not {}", describe(call.arg(0)))))?;
    let route = call.own::<Route>(1).ok_or_else(|| Error::type_error(format!("{what}: the second value is the path it runs, a route a track plan's find_path gave, not {}", describe(call.arg(1)))))?;
    if route.blocks().len() < 2 {
        return Err(Error::type_error(format!("{what}: route {} has no blocks to run through: give a path a track plan's find_path found", route.name())));
    }
    let max_speed = call.option("max_speed").map_or(Ok(60.0), |v| percent_of(v, what, "max_speed:").map_err(|e| e.at(call.option_line("max_speed"))))?;
    let approach_speed = call.option("approach_speed").map_or(Ok(20.0), |v| percent_of(v, what, "approach_speed:").map_err(|e| e.at(call.option_line("approach_speed"))))?;
    let forward = match call.option("direction").map(|v| (v.as_text(), v)) {
        None | Some((Some("forward"), _)) => true,
        Some((Some("reverse"), _)) => false,
        Some((_, v)) => return Err(Error::type_error(format!("{what}'s `direction:` is \"forward\" or \"reverse\", the way the locomotive runs, not {}", describe(v))).at(call.option_line("direction"))),
    };
    let value = call.new_object_with_events(|events| AutoTrain {
        loco,
        route,
        max_speed: Cell::new(max_speed),
        approach_speed: Cell::new(approach_speed),
        forward,
        state: Cell::new(State::Waiting),
        before_halt: Cell::new(State::Waiting),
        reached: Cell::new(0),
        waiting: Cell::new(false),
        me: RefCell::new(Weak::new()),
        events,
    });
    if let Some(made) = call.own_value::<AutoTrain>(&value) {
        *made.me.borrow_mut() = Rc::downgrade(&made);
        let me = as_follower(&made);
        for b in made.route.blocks() {
            b.followers.add(me.clone());
        }
        for (t, _) in made.route.settings() {
            t.followers.add(me.clone());
        }
    }
    Ok(value)
}

impl AutoTrain {
    fn drive(&self, percent: f64, call: &mut Call) -> Result<(), Error> {
        let steps = ((percent / 100.0) * 126.0).round().clamp(1.0, 126.0) as u8;
        self.loco.drive(steps, self.forward, call)
    }

    /// Looks again at its blocks and turnouts: departs once lined, slows on the block before the last,
    /// stops on the last.
    fn check(&self, call: &mut Call) -> Result<(), Error> {
        let blocks = self.route.blocks();
        // (Waiting for its path: held as soon as it is free, then lined.)
        if self.state.get() == State::Blocked && self.waiting.get() && self.route.reserve_silently(call) {
            self.waiting.set(false);
            self.depart_lining(call)?;
        }
        let state = self.state.get();
        if state == State::Lining && self.route.is_aligned() {
            self.state.set(State::Running);
            self.drive(self.max_speed.get(), call)?;
            self.events.fire("on_departed", vec![]);
        }
        if !matches!(self.state.get(), State::Running | State::Approaching | State::Halted) {
            return Ok(());
        }
        // (The furthest block occupied ahead of the one reached: the train has come into it.)
        let mut newly = Vec::new();
        for (i, b) in blocks.iter().enumerate().skip(self.reached.get() + 1) {
            if b.is_occupied() == Some(true) {
                for j in self.reached.get() + 1..=i {
                    newly.push(j);
                }
                self.reached.set(i);
            }
        }
        for j in newly {
            self.events.fire("on_block_entered", vec![call.object_value(&blocks[j])]);
        }
        let last = blocks.len() - 1;
        if self.reached.get() >= last {
            if self.state.get() != State::Halted {
                self.loco.halt(false, call)?;
            }
            self.state.set(State::Arrived);
            self.events.keep_working(false);
            self.events.fire("on_arrived", vec![call.object_value(&blocks[last])]);
        } else if self.reached.get() + 1 >= last && self.state.get() == State::Running {
            self.state.set(State::Approaching);
            self.drive(self.approach_speed.get(), call)?;
        }
        Ok(())
    }
}

impl AutoTrain {
    /// Its path held: lined, to depart once its turnouts lie its way.
    fn depart_lining(&self, call: &mut Call) -> Result<(), Error> {
        self.reached.set(0);
        self.state.set(State::Lining);
        self.events.keep_working(true);
        self.route.set(call)
    }
}

impl Follower for AutoTrain {
    fn followed_changed(&self, _call: &Call) {
        if matches!(self.state.get(), State::Lining | State::Running | State::Approaching) || (self.state.get() == State::Blocked && self.waiting.get()) {
            self.events.post("auto_check", vec![]);
        }
    }
}

impl Object for AutoTrain {
    fn kind(&self) -> &str {
        "auto_train"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "locomotive" => call.object_value(&self.loco),
            "route" => call.object_value(&self.route),
            "state" => Value::from(self.state.get().word()),
            "block" => call.object_value(&self.route.blocks()[self.reached.get()]),
            "max_speed" => Value::Float(self.max_speed.get()),
            "approach_speed" => Value::Float(self.approach_speed.get()),
            "direction" => Value::from(if self.forward { "forward" } else { "reverse" }),
            other => return Err(call.unknown_property("auto_train", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "max_speed" => {
                self.max_speed.set(percent_of(&value, "an auto_train", "max_speed")?);
                if self.state.get() == State::Running {
                    self.drive(self.max_speed.get(), call)?;
                }
            }
            "approach_speed" => {
                self.approach_speed.set(percent_of(&value, "an auto_train", "approach_speed")?);
                if self.state.get() == State::Approaching {
                    self.drive(self.approach_speed.get(), call)?;
                }
            }
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("an auto_train's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("auto_train", other, PROPS)),
        }
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("auto_train.{function}()");
        match function {
            "start" => {
                call.count_between(0, 0)?;
                call.only_options(&["wait"])?;
                let wait = match call.option("wait") {
                    None => false,
                    Some(Value::Bool(b)) => *b,
                    Some(v) => return Err(Error::type_error(format!("{what}'s `wait:` is true or false, not {}", describe(v))).at(call.option_line("wait"))),
                };
                if !matches!(self.state.get(), State::Waiting | State::Blocked | State::Arrived) {
                    return Err(Error::type_error(format!("{what}: it is {} already: halt() and resume() it, or wait for on_arrived", self.state.get().word())));
                }
                let held = self.route.is_reserved() || if wait { self.route.reserve_silently(call) } else { self.route.reserve(call) };
                if !held {
                    self.state.set(State::Blocked);
                    self.waiting.set(wait);
                    // (Waiting, a script without windows waits with it.)
                    self.events.keep_working(wait);
                    self.events.fire("on_blocked", vec![]);
                    return Ok(Value::from(false));
                }
                self.depart_lining(call)?;
                self.check(call)?;
                Ok(Value::from(true))
            }
            "halt" | "emergency_stop" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                let state = self.state.get();
                if matches!(state, State::Lining | State::Running | State::Approaching) {
                    self.before_halt.set(state);
                    self.state.set(State::Halted);
                }
                if function == "halt" { self.loco.halt(false, call)? } else { self.loco.halt(true, call)? }
                Ok(Value::Null)
            }
            "resume" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                if self.state.get() == State::Halted {
                    let before = self.before_halt.get();
                    self.state.set(before);
                    match before {
                        State::Running => self.drive(self.max_speed.get(), call)?,
                        State::Approaching => self.drive(self.approach_speed.get(), call)?,
                        _ => {}
                    }
                    self.check(call)?;
                }
                Ok(Value::Null)
            }
            // (Posted when one of its blocks or turnouts changed, the kit's level 6: not listed for scripts.)
            "auto_check" => {
                self.check(call)?;
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("auto_train", other, FUNCS)),
        }
    }
}
