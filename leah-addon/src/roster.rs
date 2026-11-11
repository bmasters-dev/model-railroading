//! The roster (design/model_railroading.md, "Phase A, item 5"): a record for each locomotive (its
//! names, address, decoder, every CV, function labels, groups, picture, notes, the owner's own
//! fields, and a speed limit), kept in one JSON file the script names (by default in its data path),
//! saved when the script says so. `locomotive($entry)` drives one; the decoder programmer's CVs go
//! into an entry and back.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use leah_addon_kit::{Call, Error, Key, Object, Value};

use crate::base::describe;
use crate::definitions::{cvs_from, cvs_value};

/// One locomotive's record.
#[derive(Clone, Default)]
pub struct EntryData {
    id: u64,
    pub name: String,
    road_name: String,
    road_number: String,
    pub address: u16,
    pub long: bool,
    decoder: String,
    model: String,
    notes: String,
    picture: String,
    groups: Vec<String>,
    cvs: BTreeMap<u16, u8>,
    /// Each function's label, and whether it is momentary (on only while pressed).
    pub functions: BTreeMap<u8, (String, bool)>,
    /// The fastest speed step it may be given (1 to 126).
    pub speed_limit: u8,
    attributes: Vec<(String, Value)>,
}

pub struct RosterData {
    path: String,
    entries: Vec<EntryData>,
    next_id: u64,
    /// Changed since read or saved.
    changed: bool,
}

type Shared = Rc<RefCell<RosterData>>;

pub struct Roster(Shared);

pub struct RosterEntry {
    roster: Shared,
    id: u64,
}

impl RosterEntry {
    /// Its record now (a copy), or why there is none.
    pub fn data(&self) -> Result<EntryData, Error> {
        self.roster.borrow().entries.iter().find(|e| e.id == self.id).cloned().ok_or_else(|| Error::new("programming_error", "the roster entry was removed from its roster"))
    }

    fn change<T>(&self, f: impl FnOnce(&mut EntryData) -> T) -> Result<T, Error> {
        let mut r = self.roster.borrow_mut();
        let e = r.entries.iter_mut().find(|e| e.id == self.id).ok_or_else(|| Error::new("programming_error", "the roster entry was removed from its roster"))?;
        let t = f(e);
        r.changed = true;
        Ok(t)
    }
}

const ROSTER_PROPS: &[&str] = &["path", "names", "count", "groups", "changed"];
const ROSTER_FUNCS: &[&str] = &["add", "entry", "entries", "remove", "save", "export_entries", "import_entries"];
const ENTRY_PROPS: &[&str] = &["name", "road_name", "road_number", "address", "long", "decoder", "model", "notes", "picture", "groups", "cvs", "functions", "speed_limit", "attributes"];
const ENTRY_FUNCS: &[&str] = &["set_function", "attribute", "set_attribute"];
/// The entry's parts a file and `add()` take, besides its name.
const SETTABLE: &[&str] = &["road_name", "road_number", "address", "long", "decoder", "model", "notes", "picture", "groups", "cvs", "speed_limit", "attributes"];

fn text_of(v: &Value, what: &str) -> Result<String, Error> {
    match v {
        Value::Text(t) => Ok(t.clone()),
        Value::Int(n) => Ok(n.to_string()),
        Value::Null => Ok(String::new()),
        other => Err(Error::type_error(format!("{what} is text, not {}", describe(other)))),
    }
}

/// Sets one of an entry's parts from a value, checked.
fn set_part(e: &mut EntryData, part: &str, v: &Value, what: &str) -> Result<(), Error> {
    match part {
        "road_name" => e.road_name = text_of(v, &format!("{what}'s `road_name`"))?,
        "road_number" => e.road_number = text_of(v, &format!("{what}'s `road_number`"))?,
        "decoder" => e.decoder = text_of(v, &format!("{what}'s `decoder`"))?,
        "model" => e.model = text_of(v, &format!("{what}'s `model`"))?,
        "notes" => e.notes = text_of(v, &format!("{what}'s `notes`"))?,
        "picture" => e.picture = text_of(v, &format!("{what}'s `picture`"))?,
        "address" => match v.as_int() {
            Some(n @ 1..=10239) => {
                e.address = n as u16;
                if n > 127 {
                    e.long = true;
                }
            }
            _ => return Err(Error::range_error(format!("{what}'s `address` is 1 to 10239, not {}", describe(v)))),
        },
        "long" => match v {
            Value::Bool(b) => {
                if !b && e.address > 127 {
                    return Err(Error::range_error(format!("{what}: {} is a long address, so `long` is true", e.address)));
                }
                e.long = *b;
            }
            other => return Err(Error::type_error(format!("{what}'s `long` is true or false, not {}", describe(other)))),
        },
        "speed_limit" => match v.as_int() {
            Some(n @ 1..=126) => e.speed_limit = n as u8,
            _ => return Err(Error::range_error(format!("{what}'s `speed_limit` is a speed step, 1 to 126, not {}", describe(v)))),
        },
        "groups" => {
            let items: Vec<&Value> = match v {
                Value::Array(items) => items.iter().map(|(_, x)| x).collect(),
                Value::Null => Vec::new(),
                other => return Err(Error::type_error(format!("{what}'s `groups` is a list of groups' names, not {}", describe(other)))),
            };
            let mut groups: Vec<String> = Vec::new();
            for g in items {
                let g = g.as_text().map(str::trim).filter(|g| !g.is_empty()).ok_or_else(|| Error::type_error(format!("{what}'s `groups` are names, each text")))?;
                if !groups.iter().any(|x| x.eq_ignore_ascii_case(g)) {
                    groups.push(g.to_string());
                }
            }
            e.groups = groups;
        }
        "cvs" => {
            // (From a file, the CVs' numbers come as text: "29": 6.)
            let fixed = Value::map(v.entries().iter().map(|(k, x)| match k {
                Key::Text(t) => (t.parse::<i64>().map_or(Key::Text(t.clone()), Key::Int), x.clone()),
                k => (k.clone(), x.clone()),
            }));
            e.cvs = cvs_from(if v.is_null() { v } else { &fixed }, what)?;
        }
        "attributes" => match v {
            Value::Array(items) => {
                let mut attrs = Vec::new();
                for (k, x) in items {
                    let Key::Text(k) = k else {
                        return Err(Error::type_error(format!("{what}'s `attributes` are values by name: [\"Era\": \"1950s\"]")));
                    };
                    attrs.push((k.clone(), x.clone()));
                }
                e.attributes = attrs;
            }
            Value::Null => e.attributes.clear(),
            other => return Err(Error::type_error(format!("{what}'s `attributes` are values by name, not {}", describe(other)))),
        },
        _ => unreachable!("a part the entry has"),
    }
    Ok(())
}

fn functions_value(f: &BTreeMap<u8, (String, bool)>) -> Value {
    Value::map(f.iter().map(|(n, (label, momentary))| (Key::Int(i64::from(*n)), Value::map([("label", Value::from(label.as_str())), ("momentary", Value::from(*momentary))]))))
}

/// An entry as a file keeps it.
fn entry_value(e: &EntryData) -> Value {
    Value::map([
        ("name", Value::from(e.name.as_str())),
        ("road_name", Value::from(e.road_name.as_str())),
        ("road_number", Value::from(e.road_number.as_str())),
        ("address", Value::from(i64::from(e.address))),
        ("long", Value::from(e.long)),
        ("decoder", Value::from(e.decoder.as_str())),
        ("model", Value::from(e.model.as_str())),
        ("notes", Value::from(e.notes.as_str())),
        ("picture", Value::from(e.picture.as_str())),
        ("groups", Value::list(e.groups.iter().map(|g| Value::from(g.as_str())))),
        ("cvs", Value::map(e.cvs.iter().map(|(cv, b)| (Key::Text(cv.to_string()), Value::from(i64::from(*b)))))),
        ("functions", Value::map(e.functions.iter().map(|(n, (label, momentary))| (Key::Text(n.to_string()), Value::map([("label", Value::from(label.as_str())), ("momentary", Value::from(*momentary))]))))),
        ("speed_limit", Value::from(i64::from(e.speed_limit))),
        ("attributes", Value::map(e.attributes.iter().map(|(k, v)| (Key::Text(k.clone()), v.clone())))),
    ])
}

/// An entry from a file's object.
fn entry_from(v: &Value, what: &str) -> Result<EntryData, Error> {
    let name = v.get("name").and_then(Value::as_text).map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| Error::type_error(format!("{what}: an entry without a name")))?;
    let what = format!("{what}: the entry \"{name}\"");
    let mut e = EntryData { name: name.to_string(), address: 3, speed_limit: 126, ..EntryData::default() };
    for (k, x) in v.entries() {
        let Key::Text(k) = k else { continue };
        match k.as_str() {
            "name" => {}
            "functions" => {
                for (n, f) in x.entries() {
                    let n = match n {
                        Key::Int(n) => *n,
                        Key::Text(t) => t.parse().unwrap_or(-1),
                    };
                    if !(0..=68).contains(&n) {
                        return Err(Error::type_error(format!("{what}'s functions are F0 to F68")));
                    }
                    let label = f.get("label").and_then(Value::as_text).unwrap_or("").to_string();
                    let momentary = f.get("momentary").and_then(Value::as_bool).unwrap_or(false);
                    e.functions.insert(n as u8, (label, momentary));
                }
            }
            // (`long` after `address`, whatever the file's order.)
            part if SETTABLE.contains(&part) && part != "long" => set_part(&mut e, part, x, &what)?,
            _ => {}
        }
    }
    if let Some(l) = v.get("long") {
        set_part(&mut e, "long", l, &what)?;
    }
    Ok(e)
}

/// `roster(path)`: the roster kept in a JSON file (by default `roster.json` in the script's data path),
/// read now if it is there.
pub fn new_roster(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 1)?;
    call.only_options(&[])?;
    let path = if call.args.is_empty() {
        // (Shared by every app: beside the scripts' own data folders, not inside one.)
        let mine = call.builtin("data_path", vec![], vec![])?;
        let data = call.builtin("file_path", vec![mine], vec![])?;
        call.builtin("join_path", vec![data, Value::from("model_railroading"), Value::from("roster.json")], vec![])?.as_text().unwrap_or("roster.json").to_string()
    } else {
        call.text(0)?.to_string()
    };
    let mut data = RosterData { path: path.clone(), entries: Vec::new(), next_id: 1, changed: false };
    let there = call.builtin("file_exists", vec![Value::from(path.as_str())], vec![])?;
    if there == Value::Bool(true) {
        for e in read_entries(call, &path, "roster()")? {
            if data.entries.iter().any(|x| x.name.eq_ignore_ascii_case(&e.name)) {
                return Err(Error::type_error(format!("roster(): {path} has two entries named \"{}\"", e.name)));
            }
            let id = data.next_id;
            data.next_id += 1;
            data.entries.push(EntryData { id, ..e });
        }
    }
    Ok(call.new_object(Roster(Rc::new(RefCell::new(data)))))
}

/// The entries in a roster file (or an exported one).
fn read_entries(call: &mut Call, path: &str, what: &str) -> Result<Vec<EntryData>, Error> {
    let text = call.builtin("read_text_file", vec![Value::from(path)], vec![])?;
    let data = call.builtin("json_decode", vec![text], vec![]).map_err(|e| Error::type_error(format!("{what}: {path} isn't JSON: {}", e.message)))?;
    let Some(entries) = data.get("entries").filter(|e| matches!(e, Value::Array(_))) else {
        return Err(Error::type_error(format!("{what}: {path} holds no roster (an object with \"entries\")")));
    };
    entries.entries().iter().map(|(_, e)| entry_from(e, &format!("{what}: {path}"))).collect()
}

fn write_entries(call: &mut Call, path: &str, entries: &[EntryData]) -> Result<(), Error> {
    let file = Value::map([("format", Value::from(1)), ("entries", Value::list(entries.iter().map(entry_value)))]);
    let text = call.builtin("json_encode", vec![file], vec![("pretty", Value::from(true))])?;
    let folder = call.builtin("file_path", vec![Value::from(path)], vec![])?;
    if folder.as_text().is_some_and(|f| !f.is_empty()) && call.builtin("path_exists", vec![folder.clone()], vec![])? != Value::Bool(true) {
        call.builtin("make_path", vec![folder], vec![])?;
    }
    call.builtin("write_text_file", vec![Value::from(path), text], vec![])?;
    Ok(())
}

impl Roster {
    fn entry_object(&self, call: &mut Call, id: u64) -> Value {
        call.new_object(RosterEntry { roster: self.0.clone(), id })
    }

    fn named(&self, name: &str) -> Option<u64> {
        self.0.borrow().entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).map(|e| e.id)
    }

    fn add_entry(&self, mut e: EntryData) -> u64 {
        let mut r = self.0.borrow_mut();
        e.id = r.next_id;
        r.next_id += 1;
        let id = e.id;
        r.entries.push(e);
        r.changed = true;
        id
    }
}

fn name_arg(call: &Call, i: usize, what: &str) -> Result<String, Error> {
    let name = call.text(i)?.trim().to_string();
    if name.is_empty() {
        return Err(Error::range_error(format!("{what}: an entry's name isn't empty")));
    }
    Ok(name)
}

impl Object for Roster {
    fn kind(&self) -> &str {
        "roster"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        let r = self.0.borrow();
        Ok(match property {
            "path" => Value::from(r.path.as_str()),
            "names" => Value::list(r.entries.iter().map(|e| Value::from(e.name.as_str()))),
            "count" => Value::from(r.entries.len()),
            "groups" => {
                let mut groups: Vec<&str> = Vec::new();
                for g in r.entries.iter().flat_map(|e| e.groups.iter()) {
                    if !groups.iter().any(|x| x.eq_ignore_ascii_case(g)) {
                        groups.push(g);
                    }
                }
                groups.sort_by_key(|g| g.to_lowercase());
                Value::list(groups.into_iter().map(Value::from))
            }
            "changed" => Value::from(r.changed),
            other => {
                drop(r);
                return Err(call.unknown_property("roster", other, ROSTER_PROPS));
            }
        })
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("roster.{function}()");
        match function {
            "add" => {
                call.count_between(1, 1)?;
                let mut known = vec!["functions"];
                known.extend_from_slice(SETTABLE);
                call.only_options(&known)?;
                let name = name_arg(call, 0, &what)?;
                if self.named(&name).is_some() {
                    return Err(Error::range_error(format!("{what}: the roster has an entry named \"{name}\" already")));
                }
                let mut e = EntryData { name, address: 3, speed_limit: 126, ..EntryData::default() };
                for part in SETTABLE.iter().filter(|p| **p != "long") {
                    if let Some(v) = call.option(part) {
                        set_part(&mut e, part, v, &what).map_err(|err| err.at(call.option_line(part)))?;
                    }
                }
                if let Some(v) = call.option("long") {
                    set_part(&mut e, "long", v, &what).map_err(|err| err.at(call.option_line("long")))?;
                }
                if call.option("functions").is_some() {
                    return Err(Error::type_error(format!("{what}: give the functions' labels with the entry's set_function()")).at(call.option_line("functions")));
                }
                let id = self.add_entry(e);
                Ok(self.entry_object(call, id))
            }
            "entry" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let name = call.text(0)?.trim().to_string();
                Ok(match self.named(&name) {
                    Some(id) => self.entry_object(call, id),
                    None => Value::Null,
                })
            }
            "entries" => {
                call.count_between(0, 0)?;
                call.only_options(&["group", "address"])?;
                let group = match call.option("group") {
                    None => None,
                    Some(g) => Some(g.as_text().ok_or_else(|| Error::type_error(format!("{what}'s `group:` is a group's name")))?.to_string()),
                };
                let address = match call.option("address") {
                    None => None,
                    Some(a) => Some(a.as_int().ok_or_else(|| Error::type_error(format!("{what}'s `address:` is a number")))?),
                };
                let ids: Vec<u64> = self
                    .0
                    .borrow()
                    .entries
                    .iter()
                    .filter(|e| group.as_ref().is_none_or(|g| e.groups.iter().any(|x| x.eq_ignore_ascii_case(g))))
                    .filter(|e| address.is_none_or(|a| i64::from(e.address) == a))
                    .map(|e| e.id)
                    .collect();
                Ok(Value::list(ids.into_iter().map(|id| self.entry_object(call, id))))
            }
            "remove" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let name = call.text(0)?.trim().to_string();
                let mut r = self.0.borrow_mut();
                let before = r.entries.len();
                r.entries.retain(|e| !e.name.eq_ignore_ascii_case(&name));
                if r.entries.len() == before {
                    return Err(Error::range_error(format!("{what}: the roster has no entry named \"{name}\"")));
                }
                r.changed = true;
                Ok(Value::Null)
            }
            "save" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                let (path, entries) = {
                    let r = self.0.borrow();
                    (r.path.clone(), r.entries.clone())
                };
                write_entries(call, &path, &entries)?;
                self.0.borrow_mut().changed = false;
                Ok(Value::Null)
            }
            "export_entries" => {
                call.count_between(1, 1)?;
                call.only_options(&["names"])?;
                let path = call.text(0)?.to_string();
                let entries: Vec<EntryData> = match call.option("names") {
                    None => self.0.borrow().entries.clone(),
                    Some(Value::Array(items)) => {
                        let mut picked = Vec::new();
                        for (_, n) in items {
                            let n = n.as_text().ok_or_else(|| Error::type_error(format!("{what}'s `names:` is a list of entries' names")))?;
                            let e = self.0.borrow().entries.iter().find(|e| e.name.eq_ignore_ascii_case(n)).cloned().ok_or_else(|| Error::range_error(format!("{what}: the roster has no entry named \"{n}\"")))?;
                            picked.push(e);
                        }
                        picked
                    }
                    Some(other) => return Err(Error::type_error(format!("{what}'s `names:` is a list of entries' names, not {}", describe(other)))),
                };
                write_entries(call, &path, &entries)?;
                Ok(Value::from(entries.len()))
            }
            "import_entries" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let path = call.text(0)?.to_string();
                let mut added = Vec::new();
                for mut e in read_entries(call, &path, &what)? {
                    // (A name the roster has already: the imported one gets a number after it.)
                    if self.named(&e.name).is_some() {
                        let mut n = 2;
                        while self.named(&format!("{} ({n})", e.name)).is_some() {
                            n += 1;
                        }
                        e.name = format!("{} ({n})", e.name);
                    }
                    added.push(Value::from(e.name.as_str()));
                    self.add_entry(e);
                }
                Ok(Value::list(added))
            }
            other => Err(call.unknown_function("roster", other, ROSTER_FUNCS)),
        }
    }
}

impl Object for RosterEntry {
    fn kind(&self) -> &str {
        "roster_entry"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        if !ENTRY_PROPS.contains(&property) {
            return Err(call.unknown_property("roster_entry", property, ENTRY_PROPS));
        }
        let e = self.data()?;
        Ok(match property {
            "name" => Value::from(e.name),
            "road_name" => Value::from(e.road_name),
            "road_number" => Value::from(e.road_number),
            "address" => Value::from(i64::from(e.address)),
            "long" => Value::from(e.long),
            "decoder" => Value::from(e.decoder),
            "model" => Value::from(e.model),
            "notes" => Value::from(e.notes),
            "picture" => Value::from(e.picture),
            "groups" => Value::list(e.groups.into_iter().map(Value::from)),
            "cvs" => cvs_value(&e.cvs),
            "functions" => functions_value(&e.functions),
            "speed_limit" => Value::from(i64::from(e.speed_limit)),
            _ => Value::map(e.attributes.into_iter().map(|(k, v)| (Key::Text(k), v))),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        let what = "a roster_entry";
        match property {
            "name" => {
                let name = text_of(&value, "a roster_entry's `name`")?.trim().to_string();
                if name.is_empty() {
                    return Err(Error::range_error("a roster_entry's `name` isn't empty"));
                }
                let taken = self.roster.borrow().entries.iter().any(|e| e.id != self.id && e.name.eq_ignore_ascii_case(&name));
                if taken {
                    return Err(Error::range_error(format!("the roster has an entry named \"{name}\" already")));
                }
                self.change(|e| e.name = name)
            }
            "functions" => Err(Error::type_error("a roster_entry's `functions` are set one at a time, with set_function()")),
            p if SETTABLE.contains(&p) => {
                let mut copy = self.data()?;
                set_part(&mut copy, p, &value, what)?;
                self.change(|e| *e = copy)
            }
            other if ENTRY_PROPS.contains(&other) => Err(Error::type_error(format!("a roster_entry's `{other}` can be read but not set"))),
            other => Err(call.unknown_property("roster_entry", other, ENTRY_PROPS)),
        }
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("roster_entry.{function}()");
        match function {
            "set_function" => {
                call.count_between(2, 2)?;
                call.only_options(&["momentary"])?;
                let n = match call.arg(0).as_int() {
                    Some(n @ 0..=68) => n as u8,
                    _ => return Err(Error::range_error(format!("{what}: a function is 0 to 68 (F0 the lights), not {}", describe(call.arg(0))))),
                };
                let label = text_of(call.arg(1), &format!("{what}'s label"))?.trim().to_string();
                let momentary = match call.option("momentary") {
                    None => false,
                    Some(Value::Bool(b)) => *b,
                    Some(other) => return Err(Error::type_error(format!("{what}'s `momentary:` is true or false, not {}", describe(other))).at(call.option_line("momentary"))),
                };
                // (No label and latching: the function is plain again.)
                self.change(|e| {
                    if label.is_empty() && !momentary {
                        e.functions.remove(&n);
                    } else {
                        e.functions.insert(n, (label, momentary));
                    }
                })?;
                Ok(Value::Null)
            }
            "attribute" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let name = call.text(0)?.to_string();
                Ok(self.data()?.attributes.into_iter().find(|(k, _)| k.eq_ignore_ascii_case(&name)).map_or(Value::Null, |(_, v)| v))
            }
            "set_attribute" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let name = call.text(0)?.trim().to_string();
                if name.is_empty() {
                    return Err(Error::range_error(format!("{what}: an attribute's name isn't empty")));
                }
                let value = call.arg(1).clone();
                self.change(|e| match e.attributes.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(&name)) {
                    Some(place) if value.is_null() => {
                        let k = place.0.clone();
                        e.attributes.retain(|(x, _)| *x != k);
                    }
                    Some(place) => place.1 = value,
                    None if value.is_null() => {}
                    None => e.attributes.push((name, value)),
                })?;
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("roster_entry", other, ENTRY_FUNCS)),
        }
    }
}
