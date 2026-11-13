//! The fast clock (design/model_railroading.md, "Phase D, item 15", model-railroading#19): a layout's
//! own time of day, running faster than the real one (4: four layout minutes to each real one), for
//! timetables and operating sessions. It says each layout minute (`on_minute`) and the times listed in
//! `alarms` (`on_alarm`), from a thread of its own posting the clock's own function at each layout
//! minute (the kit's level 6). Phones (`set_clock($clock)`) and track diagrams (`add_clock`) follow it.

use std::cell::{Cell, RefCell};
use std::rc::Weak;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::describe;
use crate::sensor::{Follower, Followers};

const PROPS: &[&str] = &["time", "rate", "running", "alarms", "hour", "minute"];
const FUNCS: &[&str] = &["start", "stop", "set_time"];
const MOST_RATE: f64 = 60.0;
const LEAST_RATE: f64 = 0.1;

pub struct FastClock {
    /// Its time of day (seconds) when it was last set, started, or stopped; and when that was.
    seconds: Cell<f64>,
    set_at: Cell<Instant>,
    rate: Cell<f64>,
    running: Cell<bool>,
    alarms: RefCell<Vec<u32>>,
    /// The layout minute last said (to say each once).
    said: Cell<Option<u32>>,
    /// Counts its runs: the thread of an older run ends.
    runs: Arc<AtomicU64>,
    /// The phones and track diagrams showing it.
    pub(crate) followers: Followers,
    events: Events,
}

/// A time of day, "14:05" or "14:05:30", in seconds.
pub(crate) fn time_of_day(v: &Value, what: &str) -> Result<f64, Error> {
    let bad = || Error::type_error(format!("{what}: a time of day is like \"14:05\" or \"14:05:30\", not {}", describe(v)));
    let t = v.as_text().ok_or_else(bad)?;
    let parts: Vec<&str> = t.trim().split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return Err(bad());
    }
    let n: Vec<u32> = parts.iter().map(|p| p.parse::<u32>()).collect::<Result<_, _>>().map_err(|_| bad())?;
    if n[0] > 23 || n[1] > 59 || n.get(2).is_some_and(|s| *s > 59) {
        return Err(bad());
    }
    Ok(f64::from(n[0] * 3600 + n[1] * 60 + n.get(2).copied().unwrap_or(0)))
}

fn rate_of(v: &Value, what: &str) -> Result<f64, Error> {
    match v.as_number() {
        Some(r) if (LEAST_RATE..=MOST_RATE).contains(&r) => Ok(r),
        _ => Err(Error::range_error(format!("{what}: a fast clock's rate is how much faster it runs, {LEAST_RATE} to {MOST_RATE} (4: four minutes to each real one), not {}", describe(v)))),
    }
}

/// "06:05" (or "06:05:30" with its seconds).
fn words(seconds: f64, with_seconds: bool) -> String {
    let s = seconds as u32 % 86400;
    if with_seconds { format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{:02}:{:02}", s / 3600, s / 60 % 60) }
}

/// `fast_clock(time: "06:00", rate: 4, alarms: [])`: stopped until `start()`.
pub fn new_clock(call: &mut Call) -> Result<Value, Error> {
    let what = "fast_clock()";
    call.count_between(0, 0)?;
    call.only_options(&["time", "rate", "alarms"])?;
    let seconds = match call.option("time") {
        None => 6.0 * 3600.0,
        Some(v) => time_of_day(v, what).map_err(|e| e.at(call.option_line("time")))?,
    };
    let rate = match call.option("rate") {
        None => 4.0,
        Some(v) => rate_of(v, what).map_err(|e| e.at(call.option_line("rate")))?,
    };
    let alarms = match call.option("alarms") {
        None => Vec::new(),
        Some(v) => alarms_of(v, what).map_err(|e| e.at(call.option_line("alarms")))?,
    };
    let value = call.new_object_with_events(|events| FastClock {
        seconds: Cell::new(seconds),
        set_at: Cell::new(Instant::now()),
        rate: Cell::new(rate),
        running: Cell::new(false),
        alarms: RefCell::new(alarms),
        said: Cell::new(Some((seconds / 60.0) as u32)),
        runs: Arc::new(AtomicU64::new(0)),
        followers: Followers::default(),
        events,
    });
    Ok(value)
}

fn alarms_of(v: &Value, what: &str) -> Result<Vec<u32>, Error> {
    let Value::Array(items) = v else {
        return Err(Error::type_error(format!("{what}: the alarms are a list of times of day, [\"07:15\", \"18:40\"], not {}", describe(v))));
    };
    items.iter().map(|(_, t)| time_of_day(t, what).map(|s| (s / 60.0) as u32)).collect()
}

impl FastClock {
    /// Its time of day now, in seconds.
    pub(crate) fn now(&self) -> f64 {
        let ran = if self.running.get() { self.set_at.get().elapsed().as_secs_f64() * self.rate.get() } else { 0.0 };
        (self.seconds.get() + ran).rem_euclid(86400.0)
    }

    pub(crate) fn rate(&self) -> f64 {
        self.rate.get()
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.get()
    }

    pub(crate) fn words(&self) -> String {
        words(self.now(), false)
    }

    /// Its time kept as it is now, from now on (before its rate or running changes).
    fn hold(&self) {
        self.seconds.set(self.now());
        self.set_at.set(Instant::now());
    }

    /// A thread of its own posting `clock_tick` at each layout minute while it runs.
    fn run_thread(&self) {
        let run = self.runs.fetch_add(1, Ordering::SeqCst) + 1;
        let runs = self.runs.clone();
        let events = self.events.clone();
        let (start, from, rate) = (Instant::now(), self.now(), self.rate.get());
        self.events.keep_working(true);
        std::thread::spawn(move || {
            loop {
                // (Until the next layout minute, in real time.)
                let at = from + start.elapsed().as_secs_f64() * rate;
                let next = (at / 60.0).floor() * 60.0 + 60.0;
                // (Until a moment after the minute's turn, so the clock's own reading is in the new one.)
                let wait = ((next - at) / rate).max(0.0) + 0.002;
                std::thread::sleep(Duration::from_secs_f64(wait.min(1.0)));
                if runs.load(Ordering::SeqCst) != run {
                    return;
                }
                if from + start.elapsed().as_secs_f64() * rate >= next {
                    events.post("clock_tick", vec![]);
                }
            }
        });
    }

    /// Each layout minute not yet said: on_minute, and on_alarm for an alarm's.
    /// Each layout minute not yet said (every one since the last said, when the script was busy for
    /// more than one): on_minute, and on_alarm for an alarm's.
    fn tick(&self, call: &Call) {
        let minute = (self.now() / 60.0) as u32 % 1440;
        if self.said.get() == Some(minute) {
            return;
        }
        let first = self.said.get().map_or(minute, |s| (s + 1) % 1440);
        let count = (minute + 1440 - first) % 1440 + 1;
        self.said.set(Some(minute));
        for k in 0..count {
            let m = (first + k) % 1440;
            let time = words(f64::from(m * 60), false);
            self.events.fire("on_minute", vec![Value::from(time.as_str())]);
            if self.alarms.borrow().contains(&m) {
                self.events.fire("on_alarm", vec![Value::from(time.as_str())]);
            }
        }
        self.followers.tell(call);
    }

    pub(crate) fn follow(&self, who: Weak<dyn Follower>) {
        self.followers.add(who);
    }
}

impl Object for FastClock {
    fn kind(&self) -> &str {
        "fast_clock"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        let now = self.now();
        Ok(match property {
            "time" => Value::from(words(now, false)),
            "hour" => Value::from(i64::from(now as u32 / 3600)),
            "minute" => Value::from(i64::from(now as u32 / 60 % 60)),
            "rate" => Value::Float(self.rate.get()),
            "running" => Value::from(self.running.get()),
            "alarms" => Value::list(self.alarms.borrow().iter().map(|m| Value::from(words(f64::from(m * 60), false))).collect::<Vec<_>>()),
            other => return Err(call.unknown_property("fast_clock", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "rate" => {
                let rate = rate_of(&value, "a fast_clock's `rate`")?;
                self.hold();
                self.rate.set(rate);
                if self.running.get() {
                    self.run_thread();
                }
            }
            "time" => {
                let seconds = time_of_day(&value, "a fast_clock's `time`")?;
                self.seconds.set(seconds);
                self.set_at.set(Instant::now());
                self.said.set(Some((seconds / 60.0) as u32));
                if self.running.get() {
                    self.run_thread();
                }
            }
            "alarms" => *self.alarms.borrow_mut() = alarms_of(&value, "a fast_clock's `alarms`")?,
            "running" => match value {
                Value::Bool(true) if !self.running.get() => {
                    self.hold();
                    self.running.set(true);
                    self.run_thread();
                }
                Value::Bool(false) if self.running.get() => {
                    self.hold();
                    self.running.set(false);
                    self.runs.fetch_add(1, Ordering::SeqCst);
                    self.events.keep_working(false);
                }
                Value::Bool(_) => {}
                other => return Err(Error::type_error(format!("a fast_clock's `running` is true or false, not {}", describe(&other)))),
            },
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a fast_clock's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("fast_clock", other, PROPS)),
        }
        self.followers.tell(call);
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        match function {
            "start" | "stop" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.set("running", Value::from(function == "start"), call)?;
                Ok(Value::Null)
            }
            "set_time" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let t = call.arg(0).clone();
                self.set("time", t, call)?;
                Ok(Value::Null)
            }
            // (Posted by the clock's thread at each layout minute, the kit's level 6: not listed for scripts.)
            "clock_tick" => {
                self.tick(call);
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("fast_clock", other, FUNCS)),
        }
    }
}
