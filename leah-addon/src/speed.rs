//! Speed (design/model_railroading.md, "Phase D, item 16", model-railroading#20): a train's speed
//! measured between two sensors a known distance apart (a `speedometer`), in centimeters a second and
//! as the scale's real-world speed, and `scale_speed` for any model speed. The scales' ratios: HO
//! 1:87.1, N 1:160, O 1:48, S 1:64, TT 1:120, and Z 1:220 as the NMRA's S-1.2 gives them; OO 1:76.2,
//! G 1:32 (gauge 1), and large 1:22.5 as the hobby usually takes them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::describe;
use crate::sensor::{as_follower, Follower, Sensor};

const PROPS: &[&str] = &["sensors", "distance", "scale", "last_speed", "last_scale_kmh", "last_scale_mph"];
const SCALES: &[(&str, f64)] = &[("HO", 87.1), ("N", 160.0), ("O", 48.0), ("S", 64.0), ("TT", 120.0), ("Z", 220.0), ("OO", 76.2), ("G", 32.0), ("large", 22.5)];

/// A scale by its name (or a ratio's number, 87.1 for 1:87.1).
fn ratio_of(v: &Value, what: &str) -> Result<f64, Error> {
    if let Some(n) = v.as_number().filter(|n| (1.0..=1000.0).contains(n)) {
        return Ok(n);
    }
    let names: Vec<&str> = SCALES.iter().map(|(n, _)| *n).collect();
    v.as_text()
        .and_then(|t| SCALES.iter().find(|(n, _)| n.eq_ignore_ascii_case(t.trim())))
        .map(|(_, r)| *r)
        .ok_or_else(|| Error::range_error(format!("{what}: a scale is one of {} (or its ratio, 87.1 for 1:87.1), not {}", names.join(", "), describe(v))))
}

/// Centimeters a second on the model, as kilometers an hour at full size.
fn scale_kmh(cm_per_second: f64, ratio: f64) -> f64 {
    cm_per_second * ratio * 0.036
}

fn rounded(n: f64) -> f64 {
    (n * 10.0).round() / 10.0
}

/// `scale_speed($cm_per_second, scale: "HO")`: `["kmh": ..., "mph": ...]`.
pub fn scale_speed(call: &mut Call) -> Result<Value, Error> {
    let what = "scale_speed()";
    call.count_between(1, 1)?;
    call.only_options(&["scale"])?;
    let cm = call.arg(0).as_number().filter(|n| *n >= 0.0).ok_or_else(|| Error::range_error(format!("{what}: a model's speed is centimeters a second, 0 or more, not {}", describe(call.arg(0)))))?;
    let ratio = call.option("scale").map_or(Ok(87.1), |v| ratio_of(v, what).map_err(|e| e.at(call.option_line("scale"))))?;
    let kmh = scale_kmh(cm, ratio);
    Ok(Value::map([("kmh", Value::Float(rounded(kmh))), ("mph", Value::Float(rounded(kmh / 1.609_344)))]))
}

pub struct Speedometer {
    first: Rc<Sensor>,
    second: Rc<Sensor>,
    distance: f64,
    ratio: f64,
    /// Which sensor a train reached first, and when (true: the first one).
    started: Cell<Option<(bool, Instant)>>,
    /// What each sensor said last (to see it become active).
    was: RefCell<(Option<bool>, Option<bool>)>,
    last: Cell<Option<f64>>,
    events: Events,
}

/// `speedometer($first, $second, distance: 50, scale: "HO")`.
pub fn new_speedometer(call: &mut Call) -> Result<Value, Error> {
    let what = "speedometer()";
    call.count_between(2, 2)?;
    call.only_options(&["distance", "scale"])?;
    let sensor = |i: usize| call.own::<Sensor>(i).ok_or_else(|| Error::type_error(format!("{what}: its two values are the sensors a train passes, not {}", describe(call.arg(i)))));
    let (first, second) = (sensor(0)?, sensor(1)?);
    if Rc::ptr_eq(&first, &second) {
        return Err(Error::type_error(format!("{what}: two different sensors, a known distance apart")));
    }
    let distance = match call.option("distance") {
        Some(v) => v.as_number().filter(|n| *n > 0.0 && *n <= 100_000.0).ok_or_else(|| Error::range_error(format!("{what}'s `distance:` is the centimeters between the two sensors, more than 0, not {}", describe(v))).at(call.option_line("distance")))?,
        None => return Err(Error::type_error(format!("{what}: give `distance:`, the centimeters between the two sensors"))),
    };
    let ratio = call.option("scale").map_or(Ok(87.1), |v| ratio_of(v, what).map_err(|e| e.at(call.option_line("scale"))))?;
    let was = (first.is_active(), second.is_active());
    let value = call.new_object_with_events(|events| Speedometer { first, second, distance, ratio, started: Cell::new(None), was: RefCell::new(was), last: Cell::new(None), events });
    if let Some(made) = call.own_value::<Speedometer>(&value) {
        let me = as_follower(&made);
        made.first.follow(me.clone());
        made.second.follow(me);
        if made.first.is_live() || made.second.is_live() {
            made.events.keep_running(true);
        }
    }
    Ok(value)
}

impl Follower for Speedometer {
    fn source_closed(&self) {
        self.events.keep_running(self.first.is_live() || self.second.is_live());
    }

    fn followed_changed(&self, _call: &Call) {
        let now = (self.first.is_active(), self.second.is_active());
        let before = std::mem::replace(&mut *self.was.borrow_mut(), now);
        let rose = |b: Option<bool>, n: Option<bool>| n == Some(true) && b != Some(true);
        for (is_first, rising) in [(true, rose(before.0, now.0)), (false, rose(before.1, now.1))] {
            if !rising {
                continue;
            }
            // (A start older than a minute was a train that backed away: forgotten.)
            if self.started.get().is_some_and(|(_, at)| at.elapsed().as_secs_f64() > 60.0) {
                self.started.set(None);
            }
            match self.started.get() {
                // (The other sensor reached after this one's: a speed, either way. Under 20 ms, both
                // changes were taken in together, the script busy: no true time, so none is said.)
                Some((from_first, at)) if from_first != is_first => {
                    self.started.set(None);
                    let seconds = at.elapsed().as_secs_f64();
                    if seconds >= 0.02 {
                        let cm = self.distance / seconds;
                        self.last.set(Some(cm));
                        let kmh = scale_kmh(cm, self.ratio);
                        self.events.fire("on_measured", vec![Value::Float(rounded(cm)), Value::Float(rounded(kmh)), Value::Float(rounded(kmh / 1.609_344))]);
                    }
                }
                _ => self.started.set(Some((is_first, Instant::now()))),
            }
        }
    }
}

impl Object for Speedometer {
    fn kind(&self) -> &str {
        "speedometer"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        let last = self.last.get();
        Ok(match property {
            "sensors" => Value::list([call.object_value(&self.first), call.object_value(&self.second)]),
            "distance" => Value::Float(self.distance),
            "scale" => Value::Float(self.ratio),
            "last_speed" => last.map_or(Value::Null, |c| Value::Float(rounded(c))),
            "last_scale_kmh" => last.map_or(Value::Null, |c| Value::Float(rounded(scale_kmh(c, self.ratio)))),
            "last_scale_mph" => last.map_or(Value::Null, |c| Value::Float(rounded(scale_kmh(c, self.ratio) / 1.609_344))),
            other => return Err(call.unknown_property("speedometer", other, PROPS)),
        })
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        Err(call.unknown_function("speedometer", function, &[]))
    }
}
