//! The layout (design/model_railroading.md, "Phase C, item 10", model-railroading#14): the turnouts,
//! sensors, lights, and blocks of one railroad, kept by name, so panels, routes, and phones find them
//! by the names people use, and saved to a file and read back (`write_layout_file`,
//! `read_layout_file`): JSON, with each object's settings as its maker takes them, the objects it
//! follows or reads by name. Reading a file makes each object through its own maker, so it is checked
//! as the script's own call would be.

use std::cell::RefCell;
use std::rc::Rc;

use leah_addon_kit::{Call, Error, Object, Value};

use crate::base::{describe, DccBase};
use crate::block::Block;
use crate::light::Light;
use crate::sensor::Sensor;
use crate::turnout::Turnout;

const PROPS: &[&str] = &["names", "count", "turnouts", "sensors", "lights", "blocks"];
const FUNCS: &[&str] = &["add", "find", "remove"];
/// The layout file's format, written in it as `"format"`.
const FORMAT: i64 = 1;

/// One of a layout's objects.
#[derive(Clone)]
pub(crate) enum Item {
    Sensor(Rc<Sensor>),
    Turnout(Rc<Turnout>),
    Block(Rc<Block>),
    Light(Rc<Light>),
}

impl Item {
    fn name(&self) -> &str {
        match self {
            Item::Sensor(s) => s.name(),
            Item::Turnout(t) => t.name(),
            Item::Block(b) => b.name(),
            Item::Light(l) => l.name(),
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Item::Sensor(_) => "sensor",
            Item::Turnout(_) => "turnout",
            Item::Block(_) => "block",
            Item::Light(_) => "light",
        }
    }

    pub(crate) fn value(&self, call: &Call) -> Value {
        match self {
            Item::Sensor(s) => call.object_value(s),
            Item::Turnout(t) => call.object_value(t),
            Item::Block(b) => call.object_value(b),
            Item::Light(l) => call.object_value(l),
        }
    }

    fn of(call: &Call, v: &Value) -> Option<Item> {
        if let Some(s) = call.own_value::<Sensor>(v) {
            Some(Item::Sensor(s))
        } else if let Some(t) = call.own_value::<Turnout>(v) {
            Some(Item::Turnout(t))
        } else if let Some(b) = call.own_value::<Block>(v) {
            Some(Item::Block(b))
        } else {
            call.own_value::<Light>(v).map(Item::Light)
        }
    }

    fn same(&self, other: &Item) -> bool {
        match (self, other) {
            (Item::Sensor(a), Item::Sensor(b)) => Rc::ptr_eq(a, b),
            (Item::Turnout(a), Item::Turnout(b)) => Rc::ptr_eq(a, b),
            (Item::Block(a), Item::Block(b)) => Rc::ptr_eq(a, b),
            (Item::Light(a), Item::Light(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct Layout {
    items: RefCell<Vec<Item>>,
}

/// `layout()`: an empty layout.
pub fn new_layout(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 0)?;
    call.only_options(&[])?;
    Ok(call.new_object(Layout::default()))
}

impl Layout {
    /// Its objects, in the order they were added.
    pub(crate) fn items(&self) -> Vec<Item> {
        self.items.borrow().clone()
    }

    /// Adds an object, its name not yet used by another in the layout (the same object again: nothing).
    fn add(&self, item: Item, what: &str) -> Result<(), Error> {
        let mut items = self.items.borrow_mut();
        if let Some(there) = items.iter().find(|i| i.name() == item.name()) {
            if there.same(&item) {
                return Ok(());
            }
            return Err(Error::range_error(format!("{what}: the layout has a {} named {:?} already: each name is used once", there.kind(), item.name())));
        }
        items.push(item);
        Ok(())
    }

    /// One of the objects already read, by its name in the file.
    fn named(&self, name: &Value, call: &Call, at: &str) -> Result<Value, Error> {
        let name = name.as_text().ok_or_else(|| Error::type_error(format!("{at} names another object by its name, not {}", describe(name))))?;
        self.items.borrow().iter().find(|i| i.name() == name).map(|i| i.value(call)).ok_or_else(|| Error::range_error(format!("{at} reads {name:?}, which the file doesn't make before it")))
    }

    fn of_kind(&self, kind: &str, call: &Call) -> Value {
        Value::list(self.items.borrow().iter().filter(|i| i.kind() == kind).map(|i| i.value(call)).collect::<Vec<_>>())
    }

    /// The layout's objects as a file holds them: each kind's settings, in the order they are made
    /// (sensors, then turnouts and blocks reading them, then lights following any of them).
    fn file_value(&self, what: &str) -> Result<Value, Error> {
        let items = self.items.borrow();
        let has = |name: &str| items.iter().any(|i| i.name() == name);
        let missing = |owner: &Item, name: &str| Error::range_error(format!("{what}: the {} {:?} reads {:?}, which isn't in the layout: add it too", owner.kind(), owner.name(), name));
        for item in items.iter() {
            let needs: Vec<String> = match item {
                Item::Turnout(t) => t.sensors().iter().map(|s| s.name().to_string()).collect(),
                Item::Block(b) => b.sensor().map(|s| s.name().to_string()).into_iter().collect(),
                Item::Light(l) => l.followed().map(|(n, _)| n).into_iter().collect(),
                Item::Sensor(_) => vec![],
            };
            if let Some(name) = needs.iter().find(|n| !has(n)) {
                return Err(missing(item, name));
            }
        }
        let kind = |k: &str| Value::list(items.iter().filter(|i| i.kind() == k).map(|i| match i {
            Item::Sensor(s) => s.settings(),
            Item::Turnout(t) => t.settings(),
            Item::Block(b) => b.settings(),
            Item::Light(l) => l.settings(),
        }).collect::<Vec<_>>());
        Ok(Value::map([("format", Value::from(FORMAT)), ("sensors", kind("sensor")), ("turnouts", kind("turnout")), ("blocks", kind("block")), ("lights", kind("light"))]))
    }
}

impl Object for Layout {
    fn kind(&self) -> &str {
        "layout"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "names" => Value::list(self.items.borrow().iter().map(|i| Value::from(i.name())).collect::<Vec<_>>()),
            "count" => Value::from(self.items.borrow().len() as i64),
            "turnouts" => self.of_kind("turnout", call),
            "sensors" => self.of_kind("sensor", call),
            "lights" => self.of_kind("light", call),
            "blocks" => self.of_kind("block", call),
            other => return Err(call.unknown_property("layout", other, PROPS)),
        })
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("layout.{function}()");
        match function {
            "add" => {
                if call.args.is_empty() {
                    return Err(Error::type_error(format!("{what}: give the turnouts, sensors, lights, and blocks to add")));
                }
                call.only_options(&[])?;
                for v in call.args.clone() {
                    let item = Item::of(call, &v).ok_or_else(|| Error::type_error(format!("{what} takes turnouts, sensors, lights, and blocks, not {}", describe(&v))))?;
                    self.add(item, &what)?;
                }
                Ok(Value::Null)
            }
            "find" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let name = call.text(0)?.trim().to_string();
                let found = self.items.borrow().iter().find(|i| i.name() == name).cloned();
                Ok(match found {
                    Some(i) => i.value(call),
                    None => {
                        let names: Vec<String> = self.items.borrow().iter().map(|i| i.name().to_string()).collect();
                        let near = nearest(&name, &names).map(|n| format!(": did you mean {n:?}?")).unwrap_or_default();
                        call.warn(format!("the layout has nothing named {name:?}{near}"));
                        Value::Null
                    }
                })
            }
            "remove" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let name = call.text(0)?.trim().to_string();
                let mut items = self.items.borrow_mut();
                let before = items.len();
                items.retain(|i| i.name() != name);
                Ok(Value::from(items.len() != before))
            }
            other => Err(call.unknown_function("layout", other, FUNCS)),
        }
    }
}

/// The name nearest to one not found, when one is close (a letter or two apart, or the same but for
/// case).
fn nearest<'a>(name: &str, names: &'a [String]) -> Option<&'a String> {
    fn distance(a: &str, b: &str) -> usize {
        let b: Vec<char> = b.chars().collect();
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut prev = row[0];
            row[0] = i + 1;
            for (j, cb) in b.iter().enumerate() {
                let here = row[j + 1];
                row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(here + 1);
                prev = here;
            }
        }
        row[b.len()]
    }
    let lower = name.to_lowercase();
    names.iter().map(|n| (distance(&lower, &n.to_lowercase()), n)).filter(|(d, n)| *d <= 2.max(n.chars().count() / 5)).min_by_key(|(d, _)| *d).map(|(_, n)| n)
}

/// `write_layout_file($layout, $path)`: the layout's objects to a JSON file (written safely: the old
/// file stays until the new one is whole).
pub fn write_layout_file(call: &mut Call) -> Result<Value, Error> {
    call.count_between(2, 2)?;
    call.only_options(&[])?;
    let what = "write_layout_file()";
    let layout = call.own::<Layout>(0).ok_or_else(|| Error::type_error(format!("{what}: the first value is a layout, not {}", describe(call.arg(0)))))?;
    let path = call.text(1)?.to_string();
    let file = layout.file_value(what)?;
    let text = call.builtin("json_encode", vec![file], vec![("pretty", Value::from(true))])?;
    call.builtin("write_text_file", vec![Value::from(path), text], vec![])?;
    Ok(Value::Null)
}

/// One object made by its own maker, as the script's call `kind(args, options)` would make it: on this
/// call, its values swapped for the maker's and given back after (made here, the object is the
/// addon's at once; through Leah's `builtin` it would cross Leah and back).
fn make(call: &mut Call, kind: &'static str, args: Vec<Value>, options: Vec<(&str, Value)>) -> Result<Value, Error> {
    let options = options.into_iter().map(|(n, v)| (n.to_string(), v, call.line)).collect();
    let saved = (std::mem::replace(&mut call.name, kind), std::mem::replace(&mut call.args, args), std::mem::replace(&mut call.options, options));
    let made = match kind {
        "sensor" => crate::sensor::new_sensor(call),
        "turnout" => crate::turnout::new_turnout(call),
        "block" => crate::block::new_block(call),
        _ => crate::light::new_light(call),
    };
    (call.name, call.args, call.options) = saved;
    made
}

/// `read_layout_file($path, base:)`: a layout made from a file `write_layout_file` wrote (or one
/// written by hand in the same form), each object made by its own maker on the base station given (by
/// default the one made last).
pub fn read_layout_file(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&["base"])?;
    let what = "read_layout_file()";
    let path = call.text(0)?.to_string();
    let base = match call.option("base") {
        None => None,
        Some(v) => Some(call.own_option::<DccBase>("base").ok_or_else(|| Error::type_error(format!("{what}'s `base:` is a dcc_base, not {}", describe(v))).at(call.option_line("base")))?),
    };
    let text = call.builtin("read_text_file", vec![Value::from(path.as_str())], vec![])?;
    let data = call.builtin("json_decode", vec![text], vec![]).map_err(|e| Error::type_error(format!("{what}: {path} isn't JSON: {}", e.message)))?;
    if data.get("format").and_then(Value::as_int).is_none() || !matches!(data, Value::Array(_)) {
        return Err(Error::type_error(format!("{what}: {path} holds no layout (an object with \"format\", \"sensors\", \"turnouts\", \"blocks\", and \"lights\")")));
    }
    if data.get("format").and_then(Value::as_int).is_some_and(|f| f > FORMAT) {
        return Err(Error::type_error(format!("{what}: {path} is a newer layout file (format {}): this addon reads format {FORMAT}; a newer addon reads it", data.get("format").and_then(Value::as_int).unwrap_or(0))));
    }
    let layout = Layout::default();
    let base_value = base.as_ref().map(|b| call.object_value(b));
    for (kind, list) in [("sensor", "sensors"), ("turnout", "turnouts"), ("block", "blocks"), ("light", "lights")] {
        let entries = match data.get(list) {
            None | Some(Value::Null) => continue,
            Some(Value::Array(items)) => items.clone(),
            Some(other) => return Err(Error::type_error(format!("{what}: {path}'s \"{list}\" is a list, not {}", describe(other)))),
        };
        for (n, (_, entry)) in entries.iter().enumerate() {
            let at = format!("{what}: {path}'s {} {}", list, n + 1);
            let Value::Array(_) = entry else {
                return Err(Error::type_error(format!("{at} isn't an object of settings")));
            };
            let mut args = Vec::new();
            let mut options: Vec<(&str, Value)> = Vec::new();
            let take = |options: &mut Vec<(&'static str, Value)>, key: &'static str| {
                if let Some(v) = entry.get(key) {
                    options.push((key, v.clone()));
                }
            };
            let mut follow = None;
            match kind {
                "sensor" => {
                    if let Some(pin) = entry.get("pin") {
                        args.push(pin.clone());
                        take(&mut options, "pull");
                        if let Some(b) = &base_value {
                            options.push(("base", b.clone()));
                        }
                    }
                    for key in ["name", "inverted", "active_delay", "inactive_delay"] {
                        take(&mut options, key);
                    }
                }
                "turnout" | "light" => {
                    args.push(entry.get("address").cloned().unwrap_or(Value::Null));
                    take(&mut options, "name");
                    take(&mut options, "inverted");
                    if let Some(b) = &base_value {
                        options.push(("base", b.clone()));
                    }
                    if kind == "turnout" {
                        if let Some(name) = entry.get("sensor") {
                            options.push(("sensor", layout.named(name, call, &at)?));
                        }
                        if let Some(Value::Array(two)) = entry.get("sensors") {
                            let mut both = Vec::new();
                            for (_, name) in two {
                                both.push(layout.named(name, call, &at)?);
                            }
                            options.push(("sensors", Value::list(both)));
                        }
                        take(&mut options, "move_time");
                    } else if let Some(name) = entry.get("follow") {
                        let when = match entry.get("when") {
                            None | Some(Value::Null) => None,
                            Some(w) => Some(w.as_text().ok_or_else(|| Error::type_error(format!("{at}'s \"when\" is text")))?.to_string()),
                        };
                        follow = Some((layout.named(name, call, &at)?, when));
                    }
                }
                _ => {
                    args.push(entry.get("name").cloned().unwrap_or(Value::Null));
                    if let Some(name) = entry.get("sensor") {
                        options.push(("sensor", layout.named(name, call, &at)?));
                    }
                }
            }
            let made = make(call, kind, args, options).map_err(|e| Error::new(e.kind.clone(), format!("{at}: {}", e.message)))?;
            if let Some((target, when)) = follow {
                let light = call.own_value::<Light>(&made).ok_or_else(|| Error::type_error(format!("{at} wasn't made")))?;
                light.follow(&target, when.as_deref(), call).map_err(|e| Error::new(e.kind.clone(), format!("{at}: {}", e.message)))?;
            }
            let item = Item::of(call, &made).ok_or_else(|| Error::type_error(format!("{at} wasn't made")))?;
            layout.add(item, &at)?;
        }
    }
    Ok(call.new_object(layout))
}
