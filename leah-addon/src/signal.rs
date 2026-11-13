//! Signal heads (design/model_railroading.md, "Phase D, item 14", model-railroading#18, its first
//! part): a head of colored lamps, each a `light` (an accessory output), showing one aspect: "red",
//! "yellow", "green", or "dark". `held` keeps it red. `protect($blocks, turnouts:)` makes it automatic,
//! as automatic block signals work: red while the first block ahead is occupied (or not yet known), or
//! a turnout ahead isn't lying the way it needs; yellow while the next is; green otherwise. Rulebook masts and signal decoders' aspect numbers wait for the rest of #18.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::describe;
use crate::block::Block;
use crate::light::Light;
use crate::sensor::{as_follower, Follower};
use crate::turnout::Turnout;

const PROPS: &[&str] = &["name", "aspect", "held", "lamps", "automatic"];
const FUNCS: &[&str] = &["protect", "stop_protecting"];
const ASPECTS: &[&str] = &["red", "yellow", "green", "dark"];

pub struct SignalHead {
    name: String,
    /// Its lamps: red, yellow, green (any may be missing: a two-lamp head has no yellow).
    lamps: [Option<Rc<Light>>; 3],
    aspect: Cell<&'static str>,
    held: Cell<bool>,
    /// What it protects when automatic: the blocks ahead, in order, and the turnouts' ways.
    blocks: RefCell<Vec<Rc<Block>>>,
    turnouts: RefCell<Vec<(Rc<Turnout>, bool)>>,
    me: RefCell<Weak<SignalHead>>,
    events: Events,
}

/// `signal_head(name:, red:, yellow:, green:)`.
pub fn new_signal_head(call: &mut Call) -> Result<Value, Error> {
    let what = "signal_head()";
    call.count_between(0, 0)?;
    call.only_options(&["name", "red", "yellow", "green"])?;
    let name = match call.option("name") {
        None => "Signal".to_string(),
        Some(v) => v.as_text().filter(|t| !t.trim().is_empty()).map(|t| t.trim().to_string()).ok_or_else(|| Error::type_error(format!("{what}'s `name:` is text, not {}", describe(v))).at(call.option_line("name")))?,
    };
    let lamp = |n: &str| -> Result<Option<Rc<Light>>, Error> {
        match call.option(n) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => call.own_option::<Light>(n).map(Some).ok_or_else(|| Error::type_error(format!("{what}'s `{n}:` is the light of its {n} lamp, not {}", describe(v))).at(call.option_line(n))),
        }
    };
    let lamps = [lamp("red")?, lamp("yellow")?, lamp("green")?];
    if lamps[0].is_none() {
        return Err(Error::type_error(format!("{what}: a signal head needs its red lamp (`red:`, a light), at least")));
    }
    let value = call.new_object_with_events(|events| SignalHead { name, lamps, aspect: Cell::new("dark"), held: Cell::new(false), blocks: RefCell::new(Vec::new()), turnouts: RefCell::new(Vec::new()), me: RefCell::new(Weak::new()), events });
    if let Some(made) = call.own_value::<SignalHead>(&value) {
        *made.me.borrow_mut() = Rc::downgrade(&made);
    }
    Ok(value)
}

impl SignalHead {
    /// Shows an aspect: its lamp on, the others off. A head without that lamp shows the next more
    /// restrictive one it has (yellow without a yellow lamp: red).
    fn show(&self, wanted: &'static str, call: &Call) -> Result<(), Error> {
        let wanted = if self.held.get() && wanted != "dark" { "red" } else { wanted };
        let shown = match wanted {
            "green" if self.lamps[2].is_none() => if self.lamps[1].is_some() { "yellow" } else { "red" },
            "yellow" if self.lamps[1].is_none() => "red",
            other => other,
        };
        let on = match shown {
            "red" => Some(0),
            "yellow" => Some(1),
            "green" => Some(2),
            _ => None,
        };
        // (The lamp going off first, then the one coming on: never two lit.)
        for (i, lamp) in self.lamps.iter().enumerate() {
            if let Some(l) = lamp
                && Some(i) != on
                && !l.is_off()
            {
                l.switch_to(false, call)?;
            }
        }
        if let Some(i) = on
            && let Some(l) = &self.lamps[i]
            && !l.is_on()
        {
            l.switch_to(true, call)?;
        }
        if self.aspect.replace(shown) != shown {
            self.events.fire("on_changed", vec![Value::from(shown)]);
        }
        Ok(())
    }

    /// The aspect what it protects calls for now.
    fn automatic_aspect(&self) -> &'static str {
        let blocks = self.blocks.borrow();
        let against = self.turnouts.borrow().iter().any(|(t, closed)| t.state_word() != if *closed { "closed" } else { "thrown" });
        let taken = |b: &Rc<Block>| b.is_occupied() != Some(false);
        match blocks.first() {
            Some(first) if against || taken(first) => "red",
            Some(_) if blocks.get(1).is_some_and(taken) => "yellow",
            Some(_) => "green",
            None => "red",
        }
    }
}

impl Follower for SignalHead {
    fn followed_changed(&self, call: &Call) {
        if self.blocks.borrow().is_empty() {
            return;
        }
        let aspect = self.automatic_aspect();
        if let Err(e) = self.show(aspect, call) {
            call.warn(format!("signal {} couldn't change: {}", self.name, e.message));
        }
    }
}

impl Object for SignalHead {
    fn kind(&self) -> &str {
        "signal_head"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "name" => Value::from(self.name.as_str()),
            "aspect" => Value::from(self.aspect.get()),
            "held" => Value::from(self.held.get()),
            "automatic" => Value::from(!self.blocks.borrow().is_empty()),
            "lamps" => Value::list(self.lamps.iter().map(|l| l.as_ref().map_or(Value::Null, |l| call.object_value(l))).collect::<Vec<_>>()),
            other => return Err(call.unknown_property("signal_head", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "aspect" => {
                if !self.blocks.borrow().is_empty() {
                    return Err(Error::type_error(format!("signal {} is automatic: its aspect comes from what it protects (stop_protecting() first, or hold it with held)", self.name)));
                }
                let a = value.as_text().and_then(|t| ASPECTS.iter().find(|a| **a == t)).ok_or_else(|| Error::range_error(format!("a signal head's `aspect` is \"red\", \"yellow\", \"green\", or \"dark\", not {}", describe(&value))))?;
                self.show(a, call)?;
            }
            "held" => match value {
                Value::Bool(b) => {
                    self.held.set(b);
                    let aspect = if self.blocks.borrow().is_empty() { if b { "red" } else { self.aspect.get() } } else { self.automatic_aspect() };
                    self.show(aspect, call)?;
                }
                other => return Err(Error::type_error(format!("a signal head's `held` is true or false, not {}", describe(&other)))),
            },
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a signal head's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("signal_head", other, PROPS)),
        }
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("signal_head.{function}()");
        match function {
            "protect" => {
                call.count_between(1, 1)?;
                call.only_options(&["turnouts"])?;
                let wrong = |v: &Value| Error::type_error(format!("{what}: the blocks ahead of it, in order, are a list of blocks (the first the one it guards), not {}", describe(v)));
                let Value::Array(items) = call.arg(0) else { return Err(wrong(call.arg(0))) };
                if items.is_empty() {
                    return Err(wrong(call.arg(0)));
                }
                let blocks = items.iter().map(|(_, v)| call.own_value::<Block>(v).ok_or_else(|| wrong(v))).collect::<Result<Vec<_>, _>>()?;
                let turnouts = match call.option("turnouts") {
                    None | Some(Value::Null) => Vec::new(),
                    Some(Value::Array(pairs)) => {
                        let bad = |v: &Value| Error::type_error(format!("{what}'s `turnouts:` are pairs, a turnout and the way it must lie for the signal to clear ([[$points, \"closed\"]]), not {}", describe(v))).at(call.option_line("turnouts"));
                        let mut out = Vec::new();
                        for (_, pair) in pairs {
                            let Value::Array(two) = pair else { return Err(bad(pair)) };
                            let t = two.first().and_then(|(_, v)| call.own_value::<Turnout>(v)).ok_or_else(|| bad(pair))?;
                            let closed = match two.get(1).and_then(|(_, v)| v.as_text()) {
                                Some("closed") => true,
                                Some("thrown") => false,
                                _ => return Err(bad(pair)),
                            };
                            out.push((t, closed));
                        }
                        out
                    }
                    Some(v) => return Err(Error::type_error(format!("{what}'s `turnouts:` is a list of pairs, not {}", describe(v))).at(call.option_line("turnouts"))),
                };
                if let Some(me) = self.me.borrow().upgrade().map(|m| as_follower(&m)) {
                    for b in &blocks {
                        b.followers.add(me.clone());
                    }
                    for (t, _) in &turnouts {
                        t.followers.add(me.clone());
                    }
                }
                *self.blocks.borrow_mut() = blocks;
                *self.turnouts.borrow_mut() = turnouts;
                self.show(self.automatic_aspect(), call)?;
                Ok(Value::Null)
            }
            "stop_protecting" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.blocks.borrow_mut().clear();
                self.turnouts.borrow_mut().clear();
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("signal_head", other, FUNCS)),
        }
    }
}
