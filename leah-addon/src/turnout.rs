//! Turnouts (design/model_railroading.md, "Phase C, item 10", model-railroading#14, its first part):
//! a turnout thrown by its accessory decoder through the base station, by the NMRA's basic accessory
//! packet (S-9.2.1 2.4.1). Its output address 1 to 2044 is the one throttles and command stations
//! show; closed is the pair's output 1, thrown output 0 (RCN-213's numbering), `inverted: true` for
//! one wired the other way. Each move is sent switched on twice, then switched off (a decoder for
//! coils times its own pulse; the off lets go of one that doesn't).

use std::cell::Cell;
use std::rc::Rc;

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::{describe, DccBase};
use crate::dcc;

pub struct Turnout {
    base: Rc<DccBase>,
    address: u16,
    name: String,
    inverted: bool,
    /// What it was last told (None: not yet, "unknown").
    closed: Cell<Option<bool>>,
    events: Events,
}

const PROPS: &[&str] = &["address", "name", "state", "inverted", "base"];
const FUNCS: &[&str] = &["close", "throw", "toggle"];

/// `turnout($address, name: "", inverted: false, base: ...)`.
pub fn new_turnout(call: &mut Call) -> Result<Value, Error> {
    call.count_between(1, 1)?;
    call.only_options(&["name", "inverted", "base"])?;
    let what = "turnout()";
    let address = match call.arg(0).as_int() {
        Some(n @ 1..=2044) => n as u16,
        _ => return Err(Error::range_error(format!("{what}: a turnout's address is its accessory decoder's output, 1 to {}, not {}", dcc::ACCESSORY_MOST, describe(call.arg(0))))),
    };
    let name = match call.option("name") {
        None => format!("T{address}"),
        Some(v) => v.as_text().filter(|t| !t.trim().is_empty() && !t.contains(['\n', '\r'])).map(|t| t.trim().to_string()).ok_or_else(|| Error::type_error(format!("{what}'s `name:` is text on one line, not {}", describe(v))).at(call.option_line("name")))?,
    };
    let inverted = match call.option("inverted") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(other) => return Err(Error::type_error(format!("{what}'s `inverted:` is true or false, not {}", describe(other))).at(call.option_line("inverted"))),
    };
    let base = match call.option("base") {
        None => DccBase::last().ok_or_else(|| Error::new("hardware_error", format!("{what}: there is no base station: make one first, dcc_base(\"/dev/ttyACM0\") or dcc_base(simulate: true)")))?,
        Some(v) => call.own_option::<DccBase>("base").ok_or_else(|| Error::type_error(format!("{what}'s `base:` is a dcc_base, not {}", describe(v))).at(call.option_line("base")))?,
    };
    Ok(call.new_object_with_events(|events| Turnout { base, address, name, inverted, closed: Cell::new(None), events }))
}

impl Turnout {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// "closed", "thrown", or "unknown".
    pub(crate) fn state_word(&self) -> &'static str {
        match self.closed.get() {
            Some(true) => "closed",
            Some(false) => "thrown",
            None => "unknown",
        }
    }

    /// Moved: the packets sent, its state kept, the script and the phone servers told.
    pub(crate) fn set(&self, closed: bool, call: &mut Call) -> Result<(), Error> {
        let output = closed != self.inverted;
        let words = if closed { "closed" } else { "thrown" };
        self.base.send_now(dcc::accessory(self.address, output, true), format!("turnout {} ({}): {words}", self.address, self.name), 2, call)?;
        self.base.send_now(dcc::accessory(self.address, output, false), format!("turnout {} ({}): let go", self.address, self.name), 1, call)?;
        let changed = self.closed.replace(Some(closed)) != Some(closed);
        if changed {
            crate::phone::turnout_changed(&self.base, self);
            self.events.fire("on_changed", vec![Value::from(words)]);
        }
        Ok(())
    }
}

impl Object for Turnout {
    fn kind(&self) -> &str {
        "turnout"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "address" => Value::from(i64::from(self.address)),
            "name" => Value::from(self.name.as_str()),
            "state" => Value::from(self.state_word()),
            "inverted" => Value::from(self.inverted),
            "base" => call.object_value(&self.base),
            other => return Err(call.unknown_property("turnout", other, PROPS)),
        })
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        match function {
            "close" | "throw" | "toggle" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                let closed = match function {
                    "close" => true,
                    "throw" => false,
                    _ => self.closed.get() != Some(true),
                };
                self.set(closed, call)?;
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("turnout", other, FUNCS)),
        }
    }
}
