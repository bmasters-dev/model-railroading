//! Waiting helpers (design/model_railroading.md, "Phase C, item 11", model-railroading#15): nearly every
//! train script is "wait, then act". `wait_for($sensor, "active", timeout:)` waits until a sensor, a
//! turnout, a block, or a light is in a state; `wait_for_any([[$a, "active"], [$b, "occupied"]])` until
//! the first of several is. They wait as Leah's `sleep()` does, in small pieces with Leah's events run
//! between them (the board's pins heard, a sensor's delay passed), so they are for a script without
//! windows; a window's app uses the objects' events instead.

use std::rc::Rc;
use std::time::{Duration, Instant};

use leah_addon_kit::{Call, Error, Value};

use crate::base::describe;
use crate::block::Block;
use crate::light::Light;
use crate::sensor::Sensor;
use crate::turnout::Turnout;

/// How long each piece of waiting is: the board says a pin's change within 3 ms, and 10 ms more
/// keeps the computer quiet.
const PIECE_MS: i64 = 10;
/// The longest timeout (a day).
const MOST_TIMEOUT: i64 = 86_400_000;

/// One thing waited for, in the state wanted.
enum Wanted {
    Sensor(Rc<Sensor>, bool),
    Turnout(Rc<Turnout>, &'static str),
    Block(Rc<Block>, bool),
    Light(Rc<Light>, bool),
}

impl Wanted {
    fn of(call: &Call, thing: &Value, state: &Value, what: &str) -> Result<Wanted, Error> {
        let words = |allowed: &str| Error::range_error(format!("{what}: the state of {} is {allowed}, not {}", describe(thing), describe(state)));
        let word = state.as_text();
        if let Some(s) = call.own_value::<Sensor>(thing) {
            let active = match (word, state) {
                (Some("active"), _) | (_, Value::Bool(true)) => true,
                (Some("inactive"), _) | (_, Value::Bool(false)) => false,
                _ => return Err(words("\"active\" or \"inactive\"")),
            };
            Ok(Wanted::Sensor(s, active))
        } else if let Some(t) = call.own_value::<Turnout>(thing) {
            let w = match word {
                Some("closed") => "closed",
                Some("thrown") => "thrown",
                Some("moving") => "moving",
                Some("inconsistent") => "inconsistent",
                _ => return Err(words("\"closed\", \"thrown\", \"moving\", or \"inconsistent\"")),
            };
            Ok(Wanted::Turnout(t, w))
        } else if let Some(b) = call.own_value::<Block>(thing) {
            let occupied = match (word, state) {
                (Some("occupied"), _) | (_, Value::Bool(true)) => true,
                (Some("free"), _) | (_, Value::Bool(false)) => false,
                _ => return Err(words("\"occupied\" or \"free\"")),
            };
            Ok(Wanted::Block(b, occupied))
        } else if let Some(l) = call.own_value::<Light>(thing) {
            let on = match (word, state) {
                (Some("on"), _) | (_, Value::Bool(true)) => true,
                (Some("off"), _) | (_, Value::Bool(false)) => false,
                _ => return Err(words("\"on\" or \"off\"")),
            };
            Ok(Wanted::Light(l, on))
        } else {
            Err(Error::type_error(format!("{what} waits for a sensor, a turnout, a block, or a light, not {}", describe(thing))))
        }
    }

    /// Whether it is in the state wanted now (what the board said since taken in first).
    fn reached(&self, call: &Call) -> bool {
        match self {
            Wanted::Sensor(s, active) => {
                s.catch_up(call);
                s.is_active() == Some(*active)
            }
            Wanted::Turnout(t, w) => {
                for s in t.sensors() {
                    s.catch_up(call);
                }
                t.state_word() == *w
            }
            Wanted::Block(b, occupied) => {
                if let Some(s) = b.sensor() {
                    s.catch_up(call);
                }
                b.is_occupied() == Some(*occupied)
            }
            Wanted::Light(l, on) => l.is_on() == *on,
        }
    }
}

fn timeout_of(call: &Call, what: &str) -> Result<Option<Duration>, Error> {
    match call.option("timeout") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match v.as_int() {
            Some(n @ 0..=MOST_TIMEOUT) => Ok(Some(Duration::from_millis(n as u64))),
            _ => Err(Error::range_error(format!("{what}'s `timeout:` is milliseconds, 0 to {MOST_TIMEOUT} (null: no limit), not {}", describe(v))).at(call.option_line("timeout"))),
        },
    }
}

/// Waits until one of them is reached: its place in the list, or None when the timeout passed first.
fn wait(call: &mut Call, wanted: &[Wanted], timeout: Option<Duration>, what: &str) -> Result<Option<usize>, Error> {
    let start = Instant::now();
    loop {
        if let Some(i) = wanted.iter().position(|w| w.reached(call)) {
            return Ok(Some(i));
        }
        let left = match timeout {
            Some(t) => match t.checked_sub(start.elapsed()) {
                Some(left) if !left.is_zero() => Some(left),
                _ => return Ok(None),
            },
            None => None,
        };
        let piece = left.map_or(PIECE_MS, |l| (l.as_millis() as i64).clamp(1, PIECE_MS));
        call.builtin("sleep", vec![Value::Int(piece)], vec![]).map_err(|e| {
            if e.message.contains("freeze the window") {
                Error::new(e.kind.clone(), format!("{what} would freeze the window: in a window's app, use the objects' events (a sensor's on_active, a block's on_occupied) instead"))
            } else {
                e
            }
        })?;
    }
}

/// `wait_for($thing, $state, timeout:)`: true once it is in that state, false when the timeout passed
/// first.
pub fn wait_for(call: &mut Call) -> Result<Value, Error> {
    let what = "wait_for()";
    call.count_between(2, 2)?;
    call.only_options(&["timeout"])?;
    let wanted = Wanted::of(call, call.arg(0), call.arg(1), what)?;
    let timeout = timeout_of(call, what)?;
    Ok(Value::from(wait(call, &[wanted], timeout, what)?.is_some()))
}

/// `wait_for_any([[$thing, $state], ...], timeout:)`: the place in the list (from 0) of the first in its
/// state, or null when the timeout passed first.
pub fn wait_for_any(call: &mut Call) -> Result<Value, Error> {
    let what = "wait_for_any()";
    call.count_between(1, 1)?;
    call.only_options(&["timeout"])?;
    let wrong = |v: &Value| Error::type_error(format!("{what} takes a list of pairs, each a thing and the state waited for ([[$sensor, \"active\"], [$block, \"free\"]]), not {}", describe(v)));
    let Value::Array(pairs) = call.arg(0) else { return Err(wrong(call.arg(0))) };
    if pairs.is_empty() {
        return Err(wrong(call.arg(0)));
    }
    let mut wanted = Vec::new();
    for (_, pair) in pairs {
        let Value::Array(two) = pair else { return Err(wrong(pair)) };
        if two.len() != 2 {
            return Err(wrong(pair));
        }
        wanted.push(Wanted::of(call, &two[0].1, &two[1].1, what)?);
    }
    let timeout = timeout_of(call, what)?;
    Ok(match wait(call, &wanted, timeout, what)? {
        Some(i) => Value::Int(i as i64),
        None => Value::Null,
    })
}
