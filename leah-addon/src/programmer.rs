//! The decoder programmer (design/model_railroading.md, "Phase A, item 4"): a decoder's settings by
//! a definition's names, read and written as a whole. On the programming track (given a base
//! station), every reading and writing runs on a thread of its own, with `on_progress` and `on_done`;
//! on the main (given a locomotive), it can only write, through the locomotive's writes on the main.
//! Each setting's state says whether its value can be trusted: "unknown", "read", "written", "edited",
//! or "loaded".

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use leah_addon_kit::{Call, Error, Events, Key, Object, Value};

use crate::base::{describe, DccBase, Locomotive};
use crate::definitions::{cvs_from, cvs_value, DecoderDefinition, Setting};

/// Where the decoder is.
enum Track {
    Programming(Rc<DccBase>),
    Main(Rc<Locomotive>),
}

/// What a CV's known byte came from.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Read,
    Written,
}

/// What the programmer knows, shared with its thread.
#[derive(Default)]
struct Known {
    /// The decoder's bytes, as last read or written.
    decoder: BTreeMap<u16, (u8, Source)>,
    /// Bytes waiting to be written: edited by the script, or loaded.
    pending: BTreeMap<u16, u8>,
    /// The CVs loaded (`load_cvs`) and not yet written.
    loaded: BTreeSet<u16>,
    /// The settings the script changed, not yet written, with their new values (kept so a reading of
    /// the CV they share with others can put them back over what it read).
    edited: BTreeMap<String, Value>,
    /// The CVs the last reading couldn't read, with why.
    problems: BTreeMap<u16, String>,
    busy: bool,
}

impl Known {
    /// Every byte known: the decoder's, with the pending ones over them.
    fn bytes(&self) -> BTreeMap<u16, u8> {
        let mut b: BTreeMap<u16, u8> = self.decoder.iter().map(|(cv, (v, _))| (*cv, *v)).collect();
        b.extend(self.pending.iter().map(|(cv, v)| (*cv, *v)));
        b
    }
}

pub struct Programmer {
    definition: Rc<DecoderDefinition>,
    track: Track,
    known: Arc<Mutex<Known>>,
    cancel: Arc<AtomicBool>,
    events: Events,
}

const PROPS: &[&str] = &["definition", "main_track", "values", "states", "cvs", "busy", "problems"];
const FUNCS: &[&str] = &["value", "set_value", "state", "read_all", "read", "write_changes", "write_all", "load_cvs", "cancel"];

/// CVs a "write everything" never writes: the version and maker (CV 8 resets many decoders), and
/// the decoder lock's (a wrong key locks the decoder out).
const NEVER_ALL: &[u16] = &[7, 8, 15, 16];

/// `decoder_programmer($definition, $base)` on the programming track, or
/// `decoder_programmer($definition, $locomotive)` on the main.
pub fn new_programmer(call: &mut Call) -> Result<Value, Error> {
    let what = "decoder_programmer()";
    call.count_between(2, 2)?;
    call.only_options(&[])?;
    let Some(definition) = call.own::<DecoderDefinition>(0) else {
        return Err(Error::type_error(format!("{what}: the first value is a decoder_definition, not {}", describe(call.arg(0)))));
    };
    let track = if let Some(base) = call.own::<DccBase>(1) {
        Track::Programming(base)
    } else if let Some(loco) = call.own::<Locomotive>(1) {
        Track::Main(loco)
    } else {
        return Err(Error::type_error(format!("{what}: the second value is a dcc_base (for its programming track) or a locomotive (to write on the main), not {}", describe(call.arg(1)))));
    };
    Ok(call.new_object_with_events(|events| Programmer { definition, track, known: Arc::new(Mutex::new(Known::default())), cancel: Arc::new(AtomicBool::new(false)), events }))
}

fn lock(k: &Arc<Mutex<Known>>) -> std::sync::MutexGuard<'_, Known> {
    k.lock().unwrap_or_else(|p| p.into_inner())
}

/// One step of a reading or writing on the programming track.
#[derive(Clone)]
struct Step {
    cv: u16,
    index: Option<(u8, u8)>,
    /// The byte to write; None to read.
    write: Option<u8>,
}

/// The end of a run on its thread, however it ends: "no more events" first (a run started from the
/// script after it must be the newer word), then the programming track let go of and the programmer
/// not busy, then `on_done`.
struct Finish {
    known: Arc<Mutex<Known>>,
    events: Events,
    what: &'static str,
    error: Option<String>,
    turn: Option<crate::board::ReadingTurn>,
}

impl Drop for Finish {
    fn drop(&mut self) {
        self.events.keep_running(false);
        drop(self.turn.take());
        lock(&self.known).busy = false;
        self.events.fire("on_done", vec![Value::from(self.what), self.error.take().map_or(Value::Null, Value::from)]);
    }
}

impl Programmer {
    fn setting(&self, call: &Call, what: &str) -> Result<&Setting, Error> {
        let name = call.text(0)?;
        self.definition.0.setting(name).ok_or_else(|| Error::range_error(format!("{what}: the definition \"{}\" has no setting named \"{name}\"", self.definition.0.name)))
    }

    fn state_of(&self, s: &Setting, k: &Known) -> &'static str {
        if k.edited.contains_key(&s.name) {
            "edited"
        } else if s.cvs.iter().any(|cv| k.loaded.contains(cv)) {
            "loaded"
        } else if s.cvs.iter().all(|cv| k.decoder.contains_key(cv)) {
            if s.cvs.iter().any(|cv| k.decoder[cv].1 == Source::Written) { "written" } else { "read" }
        } else {
            "unknown"
        }
    }

    fn value_of(&self, s: &Setting, bytes: &BTreeMap<u16, u8>) -> Value {
        s.value_from(bytes).unwrap_or(Value::Null)
    }

    /// The index page a CV is read and written in: its settings' (each CV has one).
    fn index_of(&self, cv: u16) -> Option<(u8, u8)> {
        self.definition.0.settings.iter().find(|s| s.cvs.contains(&cv)).and_then(|s| s.index)
    }

    fn not_busy(&self, what: &str) -> Result<(), Error> {
        if lock(&self.known).busy {
            return Err(Error::new("programming_error", format!("{what}: the programmer is busy reading or writing: one at a time (wait for its on_done, or cancel())")));
        }
        Ok(())
    }

    /// Runs the steps on the programming track on a thread of its own, firing `on_progress(done,
    /// total, cv)` after each and `on_done(what, error)` at the end. A reading that finds no decoder
    /// at all stops at once; a setting the decoder doesn't have is left unknown, listed in `problems`.
    /// `keep`: the edited settings a reading puts back over what it read (those it wasn't asked for).
    fn run(&self, base: &Rc<DccBase>, what: &'static str, steps: Vec<Step>, tries: u32, keep: Vec<(Setting, Value)>, call: &Call) -> Result<Value, Error> {
        // (Nothing to do, such as no changes to write: done, with nothing sent and the track as it is.)
        if steps.is_empty() {
            self.events.fire("on_done", vec![Value::from(what), Value::Null]);
            return Ok(Value::Null);
        }
        let turn = base.reading_turn(&format!("decoder_programmer.{what}()"), call)?;
        base.reading_starts();
        let known = self.known.clone();
        let cancel = self.cancel.clone();
        cancel.store(false, Ordering::SeqCst);
        let reading = what.starts_with("read");
        {
            let mut k = lock(&known);
            k.busy = true;
            if reading {
                k.problems.clear();
            }
        }
        let events = self.events.clone();
        let settings: Vec<(String, Vec<u16>)> = self.definition.0.settings.iter().map(|s| (s.name.clone(), s.cvs.clone())).collect();
        events.keep_running(true);
        let spawned = std::thread::Builder::new().name("programmer".into()).spawn(move || {
            // (However the thread ends, even by a panic: not busy, no more events, on_done.)
            let mut finish = Finish { known: known.clone(), events: events.clone(), what, error: Some("the programmer stopped unexpectedly".into()), turn: Some(turn) };
            let uses = |name: &str, cv: u16| settings.iter().any(|(n, cvs)| n == name && cvs.contains(&cv));
            let total = steps.len();
            let mut error: Option<String> = None;
            let mut any_answered = false;
            let mut page: Option<(u8, u8)> = None;
            for (done, step) in steps.into_iter().enumerate() {
                if cancel.load(Ordering::SeqCst) {
                    error = Some("cancelled".into());
                    break;
                }
                let turn = finish.turn.as_ref().expect("the reading turn");
                let mut result = (|| -> Result<u8, String> {
                    if step.index.is_some() && step.index != page {
                        // (A page change that failed half way leaves the decoder's page unknown.)
                        page = None;
                        turn.select_index(step.index.map(|(h, l)| (i64::from(h), i64::from(l))))?;
                        page = step.index;
                    }
                    match step.write {
                        Some(v) => turn.write_cv(i64::from(step.cv), i64::from(v)).map(|()| v),
                        None => turn.read_cv_tries(i64::from(step.cv), tries).and_then(|v| u8::try_from(v).map_err(|_| format!("the base station gave {v} for a setting (0 to 255)"))),
                    }
                })();
                // (Nothing has answered yet: a decoder without this CV, or no decoder at all? CV 8,
                // which every decoder has, tells.)
                if let Err(e) = &result
                    && e.starts_with("no decoder answered")
                    && !any_answered
                    && step.write.is_none()
                    && step.cv != 8
                    && let Ok(maker) = turn.read_cv_tries(8, 2)
                    && let Ok(maker) = u8::try_from(maker)
                {
                    any_answered = true;
                    lock(&known).decoder.insert(8, (maker, Source::Read));
                    result = Err(e.clone());
                }
                {
                    let mut k = lock(&known);
                    match result {
                        Ok(v) => {
                            any_answered = true;
                            if step.write.is_some() {
                                k.decoder.insert(step.cv, (v, Source::Written));
                                if k.pending.get(&step.cv) == Some(&v) {
                                    k.pending.remove(&step.cv);
                                    k.loaded.remove(&step.cv);
                                }
                                let k = &mut *k;
                                let (decoder, pending) = (&k.decoder, &k.pending);
                                k.edited.retain(|name, _| settings.iter().find(|(n, _)| n == name).is_none_or(|(_, cvs)| cvs.iter().any(|cv| pending.contains_key(cv) || !decoder.contains_key(cv))));
                            } else {
                                // (Read: what was read replaces what waited for this CV, but the
                                // changes to settings it wasn't asked for go back over it.)
                                k.decoder.insert(step.cv, (v, Source::Read));
                                k.pending.remove(&step.cv);
                                k.loaded.remove(&step.cv);
                                k.edited.retain(|name, _| !uses(name, step.cv) || keep.iter().any(|(s, _)| &s.name == name));
                                for (s, value) in keep.iter().filter(|(s, _)| s.cvs.contains(&step.cv)) {
                                    if let Ok(bytes) = s.bytes_for(value, &k.bytes()) {
                                        k.pending.extend(bytes);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            // (Nothing at all on the track, or the base station's own trouble: no use
                            // asking about the rest. A writing stops at its first failure.)
                            let nobody = e.starts_with("no decoder answered") && !any_answered;
                            let board = e.starts_with("can't talk") || e.starts_with("the base station didn't answer") || e.contains("went away") || e.contains("cut off");
                            if step.write.is_none() {
                                k.problems.insert(step.cv, e.clone());
                            }
                            if nobody || board || step.write.is_some() {
                                error = Some(format!("CV {}: {e}", step.cv));
                            }
                        }
                    }
                }
                events.fire("on_progress", vec![Value::from(done + 1), Value::from(total), Value::from(i64::from(step.cv))]);
                if error.is_some() {
                    break;
                }
            }
            let problems = {
                let mut k = lock(&known);
                // (With the decoder answering the others, one it didn't answer for is a setting it
                // doesn't have, not a missing locomotive.)
                if any_answered {
                    for e in k.problems.values_mut() {
                        if e.starts_with("no decoder answered") {
                            *e = "the decoder didn't answer for this CV (it may not have it)".into();
                        }
                    }
                }
                if reading { k.problems.len() } else { 0 }
            };
            if error.is_none() && problems > 0 {
                error = Some(format!("{problems} CV{} couldn't be read (the decoder may not have {}): see problems", if problems == 1 { "" } else { "s" }, if problems == 1 { "it" } else { "them" }));
            }
            finish.error = error;
            drop(finish);
        });
        if let Err(e) = spawned {
            // (The thread, and so its Finish, never started: undone here.)
            self.events.keep_running(false);
            lock(&self.known).busy = false;
            return Err(Error::new("programming_error", format!("decoder_programmer.{what}(): couldn't start its work: {e}")));
        }
        Ok(Value::Null)
    }

    /// Writes on the main, one after another (half a second apart, as the locomotive's writes are),
    /// with the same events.
    fn write_main(&self, loco: &Rc<Locomotive>, what: &'static str, steps: Vec<Step>, call: &Call) -> Result<Value, Error> {
        // (A write that changes the address the decoder answers to would leave every write after it
        // unheard, sent to the old address: refused, before anything is sent.)
        {
            let k = lock(&self.known);
            let byte = |cv: u16| steps.iter().find(|s| s.cv == cv).and_then(|s| s.write).or_else(|| k.decoder.get(&cv).map(|(b, _)| *b));
            let changes = |cv: u16| steps.iter().any(|s| s.cv == cv);
            let long_after = byte(29).map_or(loco.is_long(), |b| b & 0x20 != 0);
            let moves = if long_after != loco.is_long() {
                true
            } else if !long_after {
                changes(1) && byte(1) != Some(loco.address() as u8)
            } else if changes(17) || changes(18) {
                match (byte(17), byte(18)) {
                    (Some(a), Some(b)) => (i64::from(a) - 192) << 8 | i64::from(b) != i64::from(loco.address()),
                    _ => true,
                }
            } else {
                false
            };
            if moves {
                return Err(Error::new("programming_error", format!("decoder_programmer.{what}(): on the main, the locomotive's address can't change (the writes after it would go unheard): change it on the programming track")));
            }
        }
        let total = steps.len();
        let mut error = None;
        for (done, step) in steps.into_iter().enumerate() {
            let v = step.write.unwrap_or(0);
            if let Err(e) = loco.write_on_main(step.cv, v, step.index, call) {
                error = Some(e.message);
                break;
            }
            let mut k = lock(&self.known);
            k.decoder.insert(step.cv, (v, Source::Written));
            k.pending.remove(&step.cv);
            k.loaded.remove(&step.cv);
            drop(k);
            self.events.fire("on_progress", vec![Value::from(done + 1), Value::from(total), Value::from(i64::from(step.cv))]);
        }
        let mut k = lock(&self.known);
        let pending = k.pending.clone();
        let settings = &self.definition.0.settings;
        k.edited.retain(|name, _| settings.iter().find(|s| &s.name == name).is_none_or(|s| s.cvs.iter().any(|cv| pending.contains_key(cv))));
        drop(k);
        self.events.fire("on_done", vec![Value::from(what), error.map_or(Value::Null, Value::from)]);
        Ok(Value::Null)
    }

    fn tries(call: &Call, what: &str) -> Result<u32, Error> {
        match call.option("tries") {
            None => Ok(2),
            Some(v) => match v.as_int() {
                Some(n @ 1..=10) => Ok(n as u32),
                _ => Err(Error::range_error(format!("{what}'s `tries:` is 1 to 10, not {}", describe(v))).at(call.option_line("tries"))),
            },
        }
    }

    fn read_steps(&self, cvs: impl IntoIterator<Item = u16>) -> Vec<Step> {
        let mut steps: Vec<Step> = cvs.into_iter().map(|cv| Step { cv, index: self.index_of(cv), write: None }).collect();
        // (Each index page once: its CVs together.)
        steps.sort_by_key(|s| (s.index, s.cv));
        steps
    }
}

impl Object for Programmer {
    fn kind(&self) -> &str {
        "decoder_programmer"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        let k = lock(&self.known);
        let d = &self.definition.0;
        Ok(match property {
            "definition" => {
                drop(k);
                return Ok(call.object_value(&self.definition));
            }
            "main_track" => Value::from(matches!(self.track, Track::Main(_))),
            "values" => {
                let bytes = k.bytes();
                Value::map(d.settings.iter().map(|s| (Key::Text(s.name.clone()), self.value_of(s, &bytes))))
            }
            "states" => Value::map(d.settings.iter().map(|s| (Key::Text(s.name.clone()), Value::from(self.state_of(s, &k))))),
            "cvs" => cvs_value(&k.bytes()),
            "busy" => Value::from(k.busy),
            "problems" => Value::map(k.problems.iter().map(|(cv, e)| (Key::Int(i64::from(*cv)), Value::from(e.as_str())))),
            other => {
                drop(k);
                return Err(call.unknown_property("decoder_programmer", other, PROPS));
            }
        })
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("decoder_programmer.{function}()");
        match function {
            "value" | "state" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let s = self.setting(call, &what)?;
                let k = lock(&self.known);
                Ok(if function == "value" { self.value_of(s, &k.bytes()) } else { Value::from(self.state_of(s, &k)) })
            }
            "set_value" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                self.not_busy(&what)?;
                let s = self.setting(call, &what)?;
                if s.read_only {
                    return Err(Error::range_error(format!("{what}: the setting \"{}\" can only be read", s.name)));
                }
                let mut k = lock(&self.known);
                let bytes = s.bytes_for(call.arg(1), &k.bytes()).map_err(|e| Error::range_error(format!("{what}: {e}")))?;
                // (A loaded CV stays loaded: its other settings still hold what was loaded.)
                k.pending.extend(bytes);
                k.edited.insert(s.name.clone(), call.arg(1).clone());
                Ok(Value::Null)
            }
            "load_cvs" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                self.not_busy(&what)?;
                let cvs = cvs_from(call.arg(0), &what)?;
                let mut k = lock(&self.known);
                // (Never the version and maker, which can't be written (CV 8 resets many decoders),
                // nor the decoder lock's: they belong to the decoder in hand, not to a saved copy.)
                for (cv, b) in cvs.into_iter().filter(|(cv, _)| !NEVER_ALL.contains(cv)) {
                    k.pending.insert(cv, b);
                    k.loaded.insert(cv);
                }
                Ok(Value::Null)
            }
            "cancel" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.cancel.store(true, Ordering::SeqCst);
                Ok(Value::Null)
            }
            "read_all" | "read" => {
                let base = match &self.track {
                    Track::Programming(b) => b.clone(),
                    Track::Main(_) => return Err(Error::new("programming_error", format!("{what}: on the main, a decoder's settings can only be written: read them on the programming track"))),
                };
                let mut asked: BTreeSet<String> = BTreeSet::new();
                let cvs: Vec<u16> = if function == "read_all" {
                    call.count_between(0, 0)?;
                    asked.extend(self.definition.0.settings.iter().map(|s| s.name.clone()));
                    self.definition.0.cvs()
                } else {
                    call.count_between(1, 1)?;
                    let names: Vec<Value> = match call.arg(0) {
                        Value::Array(items) => items.iter().map(|(_, v)| v.clone()).collect(),
                        one => vec![one.clone()],
                    };
                    let mut cvs = BTreeSet::new();
                    for n in names {
                        let name = n.as_text().ok_or_else(|| Error::type_error(format!("{what}: give a setting's name, or a list of them, not {}", describe(&n))))?;
                        let s = self.definition.0.setting(name).ok_or_else(|| Error::range_error(format!("{what}: the definition \"{}\" has no setting named \"{name}\"", self.definition.0.name)))?;
                        cvs.extend(s.cvs.iter().copied());
                        asked.insert(s.name.clone());
                    }
                    cvs.into_iter().collect()
                };
                call.only_options(&["tries"])?;
                let tries = Self::tries(call, &what)?;
                self.not_busy(&what)?;
                let steps = self.read_steps(cvs);
                // (The changes to settings it wasn't asked to read stay, over what it reads.)
                let keep: Vec<(Setting, Value)> = lock(&self.known).edited.iter().filter(|(n, _)| !asked.contains(*n)).filter_map(|(n, v)| self.definition.0.setting(n).map(|s| (s.clone(), v.clone()))).collect();
                self.run(&base, if function == "read_all" { "read_all" } else { "read" }, steps, tries, keep, call)
            }
            "write_changes" | "write_all" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.not_busy(&what)?;
                let steps: Vec<Step> = {
                    let k = lock(&self.known);
                    if function == "write_changes" {
                        k.pending.iter().map(|(cv, b)| Step { cv: *cv, index: self.index_of(*cv), write: Some(*b) }).collect()
                    } else {
                        let bytes = k.bytes();
                        let mut cvs = BTreeSet::new();
                        for s in self.definition.0.settings.iter().filter(|s| s.write_all && !s.read_only) {
                            cvs.extend(s.cvs.iter().copied().filter(|cv| !NEVER_ALL.contains(cv)));
                        }
                        let missing: Vec<String> = cvs.iter().filter(|cv| !bytes.contains_key(cv)).map(|cv| cv.to_string()).collect();
                        if !missing.is_empty() {
                            return Err(Error::new("programming_error", format!("{what}: {} {} known: read everything first, or load the CVs (load_cvs)", if missing.len() == 1 { "CV" } else { "CVs" }, if missing.len() == 1 { format!("{} isn't", missing[0]) } else { format!("{} aren't", missing.join(", ")) })));
                        }
                        cvs.into_iter().map(|cv| Step { cv, index: self.index_of(cv), write: Some(bytes[&cv]) }).collect()
                    }
                };
                let mut steps = steps;
                steps.sort_by_key(|s| (s.index, s.cv));
                let name = if function == "write_all" { "write_all" } else { "write_changes" };
                match &self.track {
                    Track::Programming(b) => {
                        let b = b.clone();
                        self.run(&b, name, steps, 1, Vec::new(), call)
                    }
                    Track::Main(l) => {
                        let l = l.clone();
                        self.write_main(&l, name, steps, call)
                    }
                }
            }
            other => Err(call.unknown_function("decoder_programmer", other, FUNCS)),
        }
    }
}
