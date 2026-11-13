//! The layout simulator (design/model_railroading.md, "Phase D, item 18", model-railroading#22): trains
//! that move through a track plan's blocks with no layout, so panels, routes, and automation are tried
//! on the simulated base station. Each train is a locomotive the script (or a phone) drives: ten times a
//! second, the simulator moves it by its speed (`top_speed` centimeters a second at full speed) and
//! direction, into the block its turnouts lead to (never back through the points it came through), and
//! stops it at the end of the track or at points set against it. Every block a train covers is
//! occupied, through its sensor, or as the script's `occupied` would say it for one without.
//!
//! A train is a line of blocks (its trail), each with where it starts along the train's way; the head
//! is in the last. Running the other way turns the trail round.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::{describe, Locomotive};
use crate::block::Block;
use crate::plan::TrackPlan;

const PROPS: &[&str] = &["plan", "trains", "top_speed", "block_length", "running"];
const FUNCS: &[&str] = &["add_train", "remove_train", "set_length", "length", "start", "stop"];
/// How often the trains move.
const TICK: Duration = Duration::from_millis(100);

struct Train {
    loco: Rc<Locomotive>,
    length: f64,
    /// Its blocks, the head's last, each with where it starts along the train's way.
    trail: Vec<(Rc<Block>, f64)>,
    /// Where its head is along its way.
    head: f64,
    /// The way it is running: true while its locomotive runs forward.
    forward: bool,
    /// The next block from its first, before it has moved (`toward:`).
    toward: Option<Rc<Block>>,
    /// Said stopped (at the end of the track), until it moves again.
    stopped: bool,
}

pub struct Simulator {
    plan: Rc<TrackPlan>,
    trains: RefCell<Vec<Train>>,
    top_speed: Cell<f64>,
    block_length: Cell<f64>,
    lengths: RefCell<Vec<(Rc<Block>, f64)>>,
    /// The blocks it has said occupied.
    occupied: RefCell<Vec<Rc<Block>>>,
    running: Cell<bool>,
    last: Cell<Option<Instant>>,
    runs: Arc<AtomicU64>,
    events: Events,
}

/// `layout_simulator($plan, top_speed: 40, block_length: 100)`.
pub fn new_simulator(call: &mut Call) -> Result<Value, Error> {
    let what = "layout_simulator()";
    call.count_between(1, 1)?;
    call.only_options(&["top_speed", "block_length"])?;
    let plan = call.own::<TrackPlan>(0).ok_or_else(|| Error::type_error(format!("{what}: the first value is the track_plan its trains run on, not {}", describe(call.arg(0)))))?;
    let number = |name: &str, default: f64, most: f64| -> Result<f64, Error> {
        match call.option(name) {
            None => Ok(default),
            Some(v) => match v.as_number() {
                Some(n) if n > 0.0 && n <= most => Ok(n),
                _ => Err(Error::range_error(format!("{what}'s `{name}:` is centimeters{}, more than 0 and up to {most}, not {}", if name == "top_speed" { " a second" } else { "" }, describe(v))).at(call.option_line(name))),
            },
        }
    };
    let top_speed = number("top_speed", 40.0, 1000.0)?;
    let block_length = number("block_length", 100.0, 100_000.0)?;
    Ok(call.new_object_with_events(|events| Simulator {
        plan,
        trains: RefCell::new(Vec::new()),
        top_speed: Cell::new(top_speed),
        block_length: Cell::new(block_length),
        lengths: RefCell::new(Vec::new()),
        occupied: RefCell::new(Vec::new()),
        running: Cell::new(false),
        last: Cell::new(None),
        runs: Arc::new(AtomicU64::new(0)),
        events,
    }))
}

impl Simulator {
    fn length_of(&self, b: &Rc<Block>) -> f64 {
        self.lengths.borrow().iter().find(|(x, _)| Rc::ptr_eq(x, b)).map_or(self.block_length.get(), |(_, l)| *l)
    }

    /// The block a train leaves `head` for, coming from `came` (none at its start: `toward`), or None at
    /// the end of the track or at points set against it.
    fn next_block(&self, head: &Rc<Block>, came: Option<&Rc<Block>>, toward: Option<&Rc<Block>>) -> Option<Rc<Block>> {
        let ways = self.plan.ways_from(head);
        // (The turnout it came in through: never left through again, as it can't reverse at the points.)
        let came_through = came.and_then(|c| self.plan.ways_from(c).into_iter().find(|(b, _)| Rc::ptr_eq(b, head)).and_then(|(_, t)| t));
        let open = |(b, t): &crate::plan::Exit| -> bool {
            if came.is_some_and(|c| Rc::ptr_eq(c, b)) {
                return false;
            }
            match t {
                None => true,
                Some((turnout, closed)) => {
                    if came_through.as_ref().is_some_and(|(ct, _)| Rc::ptr_eq(ct, turnout)) {
                        return false;
                    }
                    turnout.state_word() == if *closed { "closed" } else { "thrown" }
                }
            }
        };
        match toward {
            Some(t) => ways.iter().find(|w| Rc::ptr_eq(&w.0, t) && open(w)).map(|w| w.0.clone()),
            None => ways.iter().find(|w| open(w)).map(|w| w.0.clone()),
        }
    }

    /// Moves every train by the time passed, then says the blocks' occupancy that changed.
    fn step(&self, seconds: f64, call: &Call) {
        let mut entered = Vec::new();
        let mut stopped = Vec::new();
        {
            let mut trains = self.trains.borrow_mut();
            for train in trains.iter_mut() {
                let speed = f64::from(train.loco.speed_126()) / 126.0 * self.top_speed.get();
                if speed <= 0.0 {
                    continue;
                }
                // (Running the other way: the train turned round, its tail its head.)
                if train.loco.is_forward() != train.forward {
                    let (last, start) = train.trail.last().cloned().expect("a train has a block");
                    let end = start + self.length_of(&last);
                    let tail = train.head - train.length;
                    train.trail = train.trail.iter().rev().map(|(b, s)| (b.clone(), end - (s + self.length_of(b)))).collect();
                    train.head = end - tail;
                    train.forward = !train.forward;
                    train.toward = None;
                    train.stopped = false;
                    // (The block it came from, behind its old tail, is ahead of it now, not yet entered:
                    // the way it goes next.)
                    while train.trail.len() > 1 && train.trail.last().is_some_and(|(_, s)| *s >= train.head - 1e-9) {
                        train.toward = train.trail.pop().map(|(b, _)| b);
                    }
                }
                let mut head = train.head + speed * seconds;
                loop {
                    let (last, start) = train.trail.last().cloned().expect("a train has a block");
                    let end = start + self.length_of(&last);
                    if head < end {
                        break;
                    }
                    let came = train.trail.len().checked_sub(2).map(|i| train.trail[i].0.clone());
                    match self.next_block(&last, came.as_ref(), train.toward.as_ref()) {
                        Some(next) => {
                            train.trail.push((next.clone(), end));
                            train.toward = None;
                            train.stopped = false;
                            entered.push((train.loco.clone(), next));
                        }
                        None => {
                            head = end;
                            if !train.stopped {
                                train.stopped = true;
                                stopped.push((train.loco.clone(), last.clone()));
                            }
                            break;
                        }
                    }
                }
                train.head = head;
                // (Blocks the tail has left: off the train, but the one just behind its head, which
                // says the way it came.)
                let tail = train.head - train.length;
                while train.trail.len() > 2 && train.trail[0].1 + self.length_of(&train.trail[0].0) <= tail {
                    train.trail.remove(0);
                }
            }
        }
        self.say_occupancy(call);
        for (loco, block) in entered {
            self.events.fire("on_train_entered", vec![call.object_value(&loco), call.object_value(&block)]);
        }
        for (loco, block) in stopped {
            self.events.fire("on_train_stopped", vec![call.object_value(&loco), call.object_value(&block)]);
        }
    }

    /// The blocks the trains cover now; those changed said occupied or free.
    fn say_occupancy(&self, call: &Call) {
        let mut now: Vec<Rc<Block>> = Vec::new();
        for train in self.trains.borrow().iter() {
            let tail = train.head - train.length;
            for (b, start) in &train.trail {
                let end = start + self.length_of(b);
                if end > tail && *start < train.head.max(start + 1e-9) && !now.iter().any(|x| Rc::ptr_eq(x, b)) {
                    now.push(b.clone());
                }
            }
        }
        let before = std::mem::replace(&mut *self.occupied.borrow_mut(), now.clone());
        for b in before.iter().filter(|b| !now.iter().any(|x| Rc::ptr_eq(x, b))) {
            b.simulate_occupied(false, call);
        }
        for b in now.iter().filter(|b| !before.iter().any(|x| Rc::ptr_eq(x, b))) {
            b.simulate_occupied(true, call);
        }
    }

    fn run_thread(&self) {
        let run = self.runs.fetch_add(1, Ordering::SeqCst) + 1;
        let runs = self.runs.clone();
        let events = self.events.clone();
        self.events.keep_working(true);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(TICK);
                if runs.load(Ordering::SeqCst) != run {
                    return;
                }
                events.post("sim_tick", vec![]);
            }
        });
    }
}

impl Object for Simulator {
    fn kind(&self) -> &str {
        "layout_simulator"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "plan" => call.object_value(&self.plan),
            "top_speed" => Value::Float(self.top_speed.get()),
            "block_length" => Value::Float(self.block_length.get()),
            "running" => Value::from(self.running.get()),
            "trains" => Value::list(self.trains.borrow().iter().map(|t| {
                let (head, start) = t.trail.last().cloned().expect("a train has a block");
                let tail = t.head - t.length;
                let covered: Vec<Value> = t.trail.iter().filter(|(b, s)| s + self.length_of(b) > tail).map(|(b, _)| call.object_value(b)).collect();
                Value::map([
                    ("locomotive", call.object_value(&t.loco)),
                    ("block", call.object_value(&head)),
                    ("blocks", Value::list(covered)),
                    ("position", Value::Float(((t.head - start) * 10.0).round() / 10.0)),
                    ("stopped", Value::from(t.stopped)),
                ])
            }).collect::<Vec<_>>()),
            other => return Err(call.unknown_property("layout_simulator", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "top_speed" => match value.as_number() {
                Some(n) if n > 0.0 && n <= 1000.0 => self.top_speed.set(n),
                _ => return Err(Error::range_error(format!("a layout_simulator's `top_speed` is centimeters a second at full speed, more than 0 and up to 1000, not {}", describe(&value)))),
            },
            "block_length" => match value.as_number() {
                Some(n) if n > 0.0 && n <= 100_000.0 => self.block_length.set(n),
                _ => return Err(Error::range_error(format!("a layout_simulator's `block_length` is centimeters, more than 0 and up to 100000, not {}", describe(&value)))),
            },
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a layout_simulator's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("layout_simulator", other, PROPS)),
        }
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("layout_simulator.{function}()");
        match function {
            "add_train" => {
                call.count_between(2, 2)?;
                call.only_options(&["toward", "length"])?;
                let loco = call.own::<Locomotive>(0).ok_or_else(|| Error::type_error(format!("{what}: the first value is the train's locomotive, not {}", describe(call.arg(0)))))?;
                let block = call.own::<Block>(1).ok_or_else(|| Error::type_error(format!("{what}: the second value is the block it starts in, not {}", describe(call.arg(1)))))?;
                if self.trains.borrow().iter().any(|t| Rc::ptr_eq(&t.loco, &loco)) {
                    return Err(Error::range_error(format!("{what}: that locomotive is a train already")));
                }
                if !self.plan_has(&block) {
                    return Err(Error::range_error(format!("{what}: block {} isn't in the simulator's track plan", block.name())));
                }
                let toward = match call.option("toward") {
                    None | Some(Value::Null) => None,
                    Some(v) => {
                        let t = call.own_option::<Block>("toward").ok_or_else(|| Error::type_error(format!("{what}'s `toward:` is the block next to it that the locomotive faces, not {}", describe(v))).at(call.option_line("toward")))?;
                        if !self.plan.ways_from(&block).iter().any(|(b, _)| Rc::ptr_eq(b, &t)) {
                            return Err(Error::range_error(format!("{what}: block {} isn't joined to {}", t.name(), block.name())).at(call.option_line("toward")));
                        }
                        Some(t)
                    }
                };
                let length = match call.option("length") {
                    None => 30.0,
                    Some(v) => match v.as_number() {
                        Some(n) if n > 0.0 && n <= 10_000.0 => n,
                        _ => return Err(Error::range_error(format!("{what}'s `length:` is the train's length in centimeters, more than 0 and up to 10000, not {}", describe(v))).at(call.option_line("length"))),
                    },
                };
                // (Its head in the middle of its block, or at the block's end for a train longer than half of it.)
                let head = (self.length_of(&block) / 2.0).max(length.min(self.length_of(&block)));
                let forward = loco.is_forward();
                self.trains.borrow_mut().push(Train { loco, length, trail: vec![(block, 0.0)], head, forward, toward, stopped: false });
                self.say_occupancy(call);
                Ok(Value::Null)
            }
            "remove_train" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let loco = call.own::<Locomotive>(0).ok_or_else(|| Error::type_error(format!("{what}: the value is a train's locomotive, not {}", describe(call.arg(0)))))?;
                let before = self.trains.borrow().len();
                self.trains.borrow_mut().retain(|t| !Rc::ptr_eq(&t.loco, &loco));
                self.say_occupancy(call);
                Ok(Value::from(self.trains.borrow().len() != before))
            }
            "set_length" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let b = call.own::<Block>(0).ok_or_else(|| Error::type_error(format!("{what}: the first value is a block, not {}", describe(call.arg(0)))))?;
                let cm = match call.arg(1).as_number() {
                    Some(n) if n > 0.0 && n <= 100_000.0 => n,
                    _ => return Err(Error::range_error(format!("{what}: a block's length is centimeters, more than 0 and up to 100000, not {}", describe(call.arg(1))))),
                };
                let mut lengths = self.lengths.borrow_mut();
                lengths.retain(|(x, _)| !Rc::ptr_eq(x, &b));
                lengths.push((b, cm));
                Ok(Value::Null)
            }
            "length" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let b = call.own::<Block>(0).ok_or_else(|| Error::type_error(format!("{what}: the value is a block, not {}", describe(call.arg(0)))))?;
                Ok(Value::Float(self.length_of(&b)))
            }
            "start" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                if !self.running.replace(true) {
                    self.last.set(Some(Instant::now()));
                    self.run_thread();
                }
                Ok(Value::Null)
            }
            "stop" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                if self.running.replace(false) {
                    self.runs.fetch_add(1, Ordering::SeqCst);
                    self.events.keep_working(false);
                }
                Ok(Value::Null)
            }
            // (Posted by its thread ten times a second, the kit's level 6: not listed for scripts.)
            "sim_tick" => {
                if self.running.get() {
                    let now = Instant::now();
                    let seconds = self.last.replace(Some(now)).map_or(0.0, |l| now.duration_since(l).as_secs_f64()).min(1.0);
                    self.step(seconds, call);
                }
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("layout_simulator", other, FUNCS)),
        }
    }
}

impl Simulator {
    fn plan_has(&self, b: &Rc<Block>) -> bool {
        !self.plan.ways_from(b).is_empty()
    }
}
