//! Routes (design/model_railroading.md, "Phase C, item 12", model-railroading#16): many turnouts set
//! at once, for a yard ladder, a main line cleared, the layout's start position. `set()` moves them one
//! after another, `delay` apart (so the power supply and the decoders cope), the first at once and the
//! rest by a thread of the route's own posting each step (the kit's level 6). `aligned` says when every
//! turnout lies the route's way (by its feedback, or where it was told).
//!
//! A route found by a track plan (`find_path`, plan.rs) also holds its blocks: `reserve()` holds them
//! for one train (refused while one is occupied or held by another route) and locks its turnouts;
//! each block is let go as the train clears it (occupied, then free), and the turnouts are unlocked
//! with the last one, or by `release()`.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::describe;
use crate::block::Block;
use crate::sensor::{as_follower, Follower};
use crate::turnout::Turnout;

const PROPS: &[&str] = &["name", "settings", "blocks", "delay", "busy", "aligned", "reserved"];
const FUNCS: &[&str] = &["set", "reserve", "release"];
const DEFAULT_DELAY: i64 = 250;
const MOST_DELAY: i64 = 10_000;

/// A turnout's place in a route: closed, thrown, or the other way from where it lies.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Way {
    Closed,
    Thrown,
    Toggle,
}

impl Way {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Way::Closed => "closed",
            Way::Thrown => "thrown",
            Way::Toggle => "toggle",
        }
    }
}

pub struct Route {
    name: String,
    settings: Vec<(Rc<Turnout>, Way)>,
    /// A path's blocks, in order (none for a route made by name).
    blocks: Vec<Rc<Block>>,
    delay: Cell<i64>,
    /// The step `set()` is on (the turnouts moved so far), and its count, so an older set's steps end.
    step: Cell<usize>,
    sets: Cell<u64>,
    busy: Cell<bool>,
    aligned: Cell<bool>,
    /// While reserved: each block's place, whether it is still held, and whether the train has been in it.
    held: RefCell<Vec<(bool, bool)>>,
    me: RefCell<Weak<Route>>,
    events: Events,
}

/// The settings as a script gives them: `[[$turnout, "closed"], ...]`.
fn settings_of(call: &Call, v: &Value, what: &str) -> Result<Vec<(Rc<Turnout>, Way)>, Error> {
    let wrong = |v: &Value| Error::type_error(format!("{what}: a route's turnouts are a list of pairs, each a turnout and \"closed\", \"thrown\", or \"toggle\" ([[$yard, \"thrown\"], [$main, \"closed\"]]), not {}", describe(v)));
    let Value::Array(pairs) = v else { return Err(wrong(v)) };
    let mut out: Vec<(Rc<Turnout>, Way)> = Vec::new();
    for (_, pair) in pairs {
        let Value::Array(two) = pair else { return Err(wrong(pair)) };
        if two.len() != 2 {
            return Err(wrong(pair));
        }
        let t = call.own_value::<Turnout>(&two[0].1).ok_or_else(|| wrong(&two[0].1))?;
        let way = match two[1].1.as_text() {
            Some("closed") => Way::Closed,
            Some("thrown") => Way::Thrown,
            Some("toggle") => Way::Toggle,
            _ => return Err(wrong(&two[1].1)),
        };
        if out.iter().any(|(o, _)| Rc::ptr_eq(o, &t)) {
            return Err(Error::type_error(format!("{what}: turnout {} is in the route twice", t.name())));
        }
        out.push((t, way));
    }
    Ok(out)
}

fn delay_of(v: &Value, what: &str) -> Result<i64, Error> {
    match v.as_int() {
        Some(n @ 0..=MOST_DELAY) => Ok(n),
        _ => Err(Error::range_error(format!("{what}'s `delay` is the milliseconds between two turnouts' moves, 0 to {MOST_DELAY}, not {}", describe(v)))),
    }
}

/// `route($name, $settings, delay: 250)`.
pub fn new_route(call: &mut Call) -> Result<Value, Error> {
    let what = "route()";
    call.count_between(2, 2)?;
    call.only_options(&["delay"])?;
    let name = call.arg(0).as_text().filter(|t| !t.trim().is_empty() && !t.contains(['\n', '\r'])).map(|t| t.trim().to_string()).ok_or_else(|| Error::type_error(format!("{what}: a route's name is text on one line, like \"Track 5\", not {}", describe(call.arg(0)))))?;
    let settings = settings_of(call, call.arg(1), what)?;
    let delay = match call.option("delay") {
        None => DEFAULT_DELAY,
        Some(v) => delay_of(v, what).map_err(|e| e.at(call.option_line("delay")))?,
    };
    Ok(make(call, name, settings, Vec::new(), delay))
}

/// A route, following its turnouts (for `aligned`) and its blocks (for releasing them).
pub(crate) fn make(call: &mut Call, name: String, settings: Vec<(Rc<Turnout>, Way)>, blocks: Vec<Rc<Block>>, delay: i64) -> Value {
    let value = call.new_object_with_events(|events| Route {
        name,
        settings,
        blocks,
        delay: Cell::new(delay),
        step: Cell::new(0),
        sets: Cell::new(0),
        busy: Cell::new(false),
        aligned: Cell::new(false),
        held: RefCell::new(Vec::new()),
        me: RefCell::new(Weak::new()),
        events,
    });
    if let Some(made) = call.own_value::<Route>(&value) {
        *made.me.borrow_mut() = Rc::downgrade(&made);
        let me = as_follower(&made);
        for (t, _) in &made.settings {
            t.followers.add(me.clone());
        }
        for b in &made.blocks {
            b.followers.add(me.clone());
        }
        made.aligned.set(made.lies_its_way());
    }
    value
}

impl Route {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn is_aligned(&self) -> bool {
        self.aligned.get()
    }

    /// Whether every turnout lies the route's way (a "toggle" one counts once it was moved by it).
    fn lies_its_way(&self) -> bool {
        self.settings.iter().all(|(t, way)| match way {
            Way::Closed => t.state_word() == "closed",
            Way::Thrown => t.state_word() == "thrown",
            Way::Toggle => matches!(t.state_word(), "closed" | "thrown") && t.commanded().is_some_and(|c| (c && t.state_word() == "closed") || (!c && t.state_word() == "thrown")),
        })
    }

    /// The way one turnout is moved by this route now.
    fn closed_for(t: &Turnout, way: Way) -> bool {
        match way {
            Way::Closed => true,
            Way::Thrown => false,
            Way::Toggle => t.commanded().map_or(t.state_word() != "closed", |c| !c),
        }
    }

    /// Starts setting it: the first turnout now, the rest `delay` apart.
    pub(crate) fn set(&self, call: &mut Call) -> Result<(), Error> {
        // (A turnout locked by another (or the script) that the route would move: none moves. Its own,
        // locked while it holds its blocks, it moves.)
        let mine = !self.held.borrow().is_empty();
        if let Some((t, _)) = self.settings.iter().find(|(t, way)| !mine && t.is_locked() && t.commanded() != Some(Self::closed_for(t, *way))) {
            return Err(Error::new("hardware_error", format!("route {} can't be set: turnout {} is locked", self.name, t.name())));
        }
        let set = self.sets.get() + 1;
        self.sets.set(set);
        self.step.set(0);
        self.busy.set(true);
        self.events.keep_working(true);
        let started = self.next_step(set, call);
        if started.is_err() {
            self.busy.set(false);
            self.events.keep_working(false);
        }
        started
    }

    /// Moves the next turnout, then waits `delay` for the one after (posted by a thread of its own).
    fn next_step(&self, set: u64, call: &mut Call) -> Result<(), Error> {
        if set != self.sets.get() {
            return Ok(());
        }
        let i = self.step.get();
        let Some((t, way)) = self.settings.get(i) else {
            self.busy.set(false);
            self.events.keep_working(false);
            self.events.fire("on_set", vec![]);
            return Ok(());
        };
        self.step.set(i + 1);
        let closed = Self::closed_for(t, *way);
        // (One already told this way, locked or not, isn't sent again; one locked by this route's
        // hold is unlocked for its move.)
        if !(t.is_locked() && t.commanded() == Some(closed)) {
            let mine = t.is_locked() && !self.held.borrow().is_empty();
            if mine {
                t.lock(false);
            }
            let moved = t.set(closed, call);
            if mine {
                t.lock(true);
            }
            moved?;
        }
        if self.step.get() >= self.settings.len() || self.delay.get() == 0 {
            return self.next_step(set, call);
        }
        let events = self.events.clone();
        let wait = self.delay.get() as u64;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(wait));
            events.post("route_step", vec![Value::Int(set as i64)]);
        });
        Ok(())
    }

    /// Holds its blocks for one train and locks its turnouts: false (and a warning saying why) when a
    /// block is occupied or held by another route.
    pub(crate) fn is_reserved(&self) -> bool {
        self.held.borrow().iter().any(|(still, _)| *still)
    }

    pub(crate) fn reserve(&self, call: &Call) -> bool {
        if self.blocks.is_empty() {
            call.warn(format!("route {} has no blocks to hold (a route found by a track plan's find_path has)", self.name));
            return false;
        }
        if !self.held.borrow().is_empty() {
            return true;
        }
        for b in &self.blocks {
            if let Some(s) = b.sensor() {
                s.catch_up(call);
            }
            if b.is_occupied() == Some(true) {
                call.warn(format!("route {} can't be reserved: block {} is occupied", self.name, b.name()));
                return false;
            }
            if let Some(other) = b.reserved_by.borrow().as_ref() {
                call.warn(format!("route {} can't be reserved: block {} is held by route {other}", self.name, b.name()));
                return false;
            }
        }
        if let Some((t, _)) = self.settings.iter().find(|(t, way)| t.is_locked() && t.commanded() != Some(Self::closed_for(t, *way))) {
            call.warn(format!("route {} can't be reserved: turnout {} is locked the other way", self.name, t.name()));
            return false;
        }
        *self.held.borrow_mut() = vec![(true, false); self.blocks.len()];
        for (t, _) in &self.settings {
            t.lock(true);
        }
        for b in &self.blocks {
            b.reserve_for(Some(&self.name), call);
        }
        true
    }

    /// Lets go of its blocks still held and unlocks its turnouts.
    fn release(&self, call: &Call) {
        let held = std::mem::take(&mut *self.held.borrow_mut());
        if held.is_empty() {
            return;
        }
        for (b, (still, _)) in self.blocks.iter().zip(held) {
            if still && b.reserved_by.borrow().as_deref() == Some(self.name.as_str()) {
                b.reserve_for(None, call);
            }
        }
        for (t, _) in &self.settings {
            t.lock(false);
        }
        self.events.fire("on_released", vec![]);
    }
}

impl Follower for Route {
    fn followed_changed(&self, call: &Call) {
        let aligned = self.lies_its_way();
        if self.aligned.replace(aligned) != aligned {
            // (The phones showing it are told: active while it is aligned.)
            if let Some((t, _)) = self.settings.first() {
                crate::phone::route_changed(t.base(), self);
            }
            if aligned {
                self.events.fire("on_aligned", vec![]);
            }
        }
        // (Each block held is let go once the train has been in it and left it.)
        let mut released = Vec::new();
        {
            let mut held = self.held.borrow_mut();
            for (i, (b, (still, been))) in self.blocks.iter().zip(held.iter_mut()).enumerate() {
                if !*still {
                    continue;
                }
                match b.is_occupied() {
                    Some(true) => *been = true,
                    Some(false) if *been => {
                        *still = false;
                        released.push(i);
                    }
                    _ => {}
                }
            }
        }
        for i in &released {
            let b = &self.blocks[*i];
            if b.reserved_by.borrow().as_deref() == Some(self.name.as_str()) {
                b.reserve_for(None, call);
            }
        }
        if !released.is_empty() && self.held.borrow().iter().all(|(still, _)| !still) {
            self.release(call);
        }
    }
}

impl Object for Route {
    fn kind(&self) -> &str {
        "route"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "name" => Value::from(self.name.as_str()),
            "settings" => Value::list(self.settings.iter().map(|(t, way)| Value::list([call.object_value(t), Value::from(way.word())])).collect::<Vec<_>>()),
            "blocks" => Value::list(self.blocks.iter().map(|b| call.object_value(b)).collect::<Vec<_>>()),
            "delay" => Value::from(self.delay.get()),
            "busy" => Value::from(self.busy.get()),
            "aligned" => {
                for (t, _) in &self.settings {
                    for s in t.sensors() {
                        s.catch_up(call);
                    }
                }
                Value::from(self.lies_its_way())
            }
            "reserved" => Value::from(self.is_reserved()),
            other => return Err(call.unknown_property("route", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "delay" => self.delay.set(delay_of(&value, "a route")?),
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a route's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("route", other, PROPS)),
        }
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        match function {
            "set" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.set(call)?;
                Ok(Value::Null)
            }
            "reserve" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                Ok(Value::from(self.reserve(call)))
            }
            "release" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.release(call);
                Ok(Value::Null)
            }
            // (Posted by the thread a step waits on, the kit's level 6: not listed for scripts.)
            "route_step" => {
                let set = call.int(0)? as u64;
                if let Err(e) = self.next_step(set, call) {
                    // (A turnout that couldn't move: the route stops there.)
                    self.busy.set(false);
                    self.events.keep_working(false);
                    return Err(e);
                }
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("route", other, FUNCS)),
        }
    }
}
