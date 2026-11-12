//! Consists (design/model_railroading.md, "Phase A, item 7"): several locomotives run as one train.
//! A software consist: Leah sends each member its own packets (each in its own speed steps and
//! within its limit, a reversed one the other way). An advanced consist (`address:`): each member's
//! decoder is told the consist address by the NMRA's consist control (CV 19), and the speed goes to
//! that address, which every member obeys (S-9.2.1 2.3.1.4); each member's own address keeps its
//! functions. A member's own speed is set to 0 as it joins or leaves, so a stale one repeating at its
//! own address can't start it when it leaves.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leah_addon_kit::{Call, Error, Object, Value};

use crate::base::{describe, function_number, make_locomotive, DccBase, Locomotive};
use crate::dcc;

pub struct Consist {
    base: Rc<DccBase>,
    name: String,
    /// An advanced consist's address, and the locomotive at it (what the speed is sent to).
    address: Option<u8>,
    carrier: Option<Rc<Locomotive>>,
    /// The members, the lead first, each with whether it runs reversed.
    members: RefCell<Vec<(Rc<Locomotive>, bool)>>,
    speed: Cell<u8>,
    forward: Cell<bool>,
}

const PROPS: &[&str] = &["name", "address", "kind", "members", "speed", "direction", "base"];
const FUNCS: &[&str] = &["add", "remove", "is_reversed", "set_function", "stop", "emergency_stop", "break_up"];

/// `consist(name: "", address: ..., base: ...)`.
pub fn new_consist(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 0)?;
    call.only_options(&["name", "address", "base"])?;
    let name = match call.option("name") {
        None => String::new(),
        Some(v) => v.as_text().ok_or_else(|| Error::type_error(format!("consist()'s `name:` is text, not {}", describe(v))))?.to_string(),
    };
    let base = match call.option("base") {
        None => DccBase::last().ok_or_else(|| Error::new("hardware_error", "consist(): there is no base station: make one first, dcc_base(\"/dev/ttyACM0\") or dcc_base(simulate: true)"))?,
        Some(v) => call.own_option::<DccBase>("base").ok_or_else(|| Error::type_error(format!("consist()'s `base:` is a dcc_base, not {}", describe(v))).at(call.option_line("base")))?,
    };
    let address = match call.option("address") {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_int() {
            Some(n @ 1..=127) => Some(n as u8),
            _ => return Err(Error::range_error(format!("consist()'s `address:` is a consist address, 1 to 127 (a decoder keeps it in CV 19), not {}", describe(v))).at(call.option_line("address"))),
        },
    };
    Ok(Consist::make(&base, name, address, call).0)
}

impl Consist {
    /// A consist made on a base station (`consist()`, and the phone throttle server's): its value and
    /// the object.
    pub(crate) fn make(base: &Rc<DccBase>, name: String, address: Option<u8>, call: &mut Call) -> (Value, Option<Rc<Consist>>) {
        // (The consist address's speed sent as a locomotive's: 128 steps, kept repeating on a board.)
        let carrier = match address {
            Some(a) => make_locomotive(call, base, u16::from(a), false, 128, 126, Default::default(), Default::default(), None).1,
            None => None,
        };
        let value = call.new_object(Consist { base: base.clone(), name, address, carrier, members: RefCell::new(Vec::new()), speed: Cell::new(0), forward: Cell::new(true) });
        let made = call.own_value::<Consist>(&value);
        (value, made)
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Its members, the lead first, each with whether it runs reversed.
    pub(crate) fn members_now(&self) -> Vec<(Rc<Locomotive>, bool)> {
        self.members.borrow().clone()
    }

    /// A locomotive added at the end (consist.add()).
    pub(crate) fn add_member(&self, loco: Rc<Locomotive>, reversed: bool, what: &str, call: &mut Call) -> Result<(), Error> {
        if !Rc::ptr_eq(loco.base(), &self.base) {
            return Err(Error::type_error(format!("{what}: the locomotive runs on another base station")));
        }
        if self.position(&loco).is_some() {
            return Err(Error::range_error(format!("{what}: locomotive {} is in this consist already", loco.address())));
        }
        if self.address.is_some_and(|a| !loco.is_long() && loco.address() == u16::from(a)) {
            return Err(Error::range_error(format!("{what}: locomotive {} has the consist's own address", loco.address())));
        }
        // (Stopped at its own address first: what repeats there must not start it later.)
        loco.halt(false, call)?;
        if let Some(a) = self.address {
            self.tell(&loco, a, reversed, call)?;
        }
        self.members.borrow_mut().push((loco.clone(), reversed));
        // (A software consist's new member takes the consist's speed and direction at once.)
        if self.carrier.is_none() && self.speed.get() > 0 {
            loco.drive(self.speed.get(), self.forward.get() != reversed, call)?;
        }
        Ok(())
    }

    /// A member taken out, stopped (consist.remove()).
    pub(crate) fn remove_member(&self, loco: &Rc<Locomotive>, what: &str, call: &mut Call) -> Result<(), Error> {
        let Some(i) = self.position(loco) else {
            return Err(Error::range_error(format!("{what}: locomotive {} isn't in this consist", loco.address())));
        };
        self.members.borrow_mut().remove(i);
        loco.halt(false, call)?;
        if self.address.is_some() {
            self.tell(loco, 0, false, call)?;
        }
        Ok(())
    }

    /// Every member taken out, the train stopped (consist.break_up()).
    pub(crate) fn break_up_all(&self, call: &mut Call) -> Result<(), Error> {
        self.speed.set(0);
        if let Some(c) = &self.carrier {
            c.halt(false, call)?;
        }
        let members = std::mem::take(&mut *self.members.borrow_mut());
        for (m, _) in members {
            m.halt(false, call)?;
            if self.address.is_some() {
                self.tell(&m, 0, false, call)?;
            }
        }
        Ok(())
    }

    /// Its members put in this order (each by its address and whether it is long); any not named
    /// keep their places after them.
    pub(crate) fn reorder(&self, order: &[(u16, bool)]) {
        let mut members = self.members.borrow_mut();
        let mut sorted: Vec<(Rc<Locomotive>, bool)> = Vec::new();
        for (a, l) in order {
            if let Some(i) = members.iter().position(|(m, _)| m.address() == *a && m.is_long() == *l) {
                sorted.push(members.remove(i));
            }
        }
        sorted.append(&mut members);
        *members = sorted;
    }

    fn position(&self, loco: &Rc<Locomotive>) -> Option<usize> {
        self.members.borrow().iter().position(|(m, _)| Rc::ptr_eq(m, loco))
    }

    fn member_arg(&self, call: &Call, i: usize, what: &str) -> Result<Rc<Locomotive>, Error> {
        call.own::<Locomotive>(i).ok_or_else(|| Error::type_error(format!("{what}: give a locomotive, not {}", describe(call.arg(i)))))
    }

    /// The speed and direction sent: to the consist address (advanced), or to each member as its
    /// own (software).
    fn drive(&self, call: &mut Call) -> Result<(), Error> {
        let (speed, forward) = (self.speed.get(), self.forward.get());
        if let Some(carrier) = &self.carrier {
            return carrier.drive(speed, forward, call);
        }
        let members = self.members.borrow().clone();
        for (m, reversed) in members {
            m.drive(speed, forward != reversed, call)?;
        }
        Ok(())
    }

    /// Consist control to a member: into this consist (reversed or not), or out (0). Sent twice.
    fn tell(&self, loco: &Locomotive, consist: u8, reversed: bool, call: &Call) -> Result<(), Error> {
        let words = if consist == 0 { "out of its consist".to_string() } else { format!("into consist {consist}{}", if reversed { ", reversed" } else { "" }) };
        let name = if loco.is_long() && loco.address() <= 127 { format!("locomotive {} (long)", loco.address()) } else { format!("locomotive {}", loco.address()) };
        self.base.send_now(dcc::consist_control(loco.address(), loco.is_long(), consist, reversed), format!("{name}: {words}"), 2, call)
    }
}

impl Object for Consist {
    fn kind(&self) -> &str {
        "consist"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "name" => Value::from(self.name.as_str()),
            "address" => self.address.map_or(Value::Null, |a| Value::from(i64::from(a))),
            "kind" => Value::from(if self.address.is_some() { "advanced" } else { "software" }),
            "members" => {
                let members = self.members.borrow().clone();
                Value::list(members.iter().map(|(m, _)| call.object_value(m)))
            }
            "speed" => Value::from(i64::from(self.speed.get())),
            "direction" => Value::from(if self.forward.get() { "forward" } else { "reverse" }),
            "base" => call.object_value(&self.base),
            other => return Err(call.unknown_property("consist", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "speed" => {
                match value.as_int() {
                    Some(n @ 0..=126) => self.speed.set(n as u8),
                    _ => return Err(Error::range_error(format!("a consist's `speed` is 0 to 126 (each member's own speed steps made to fit), not {}", describe(&value)))),
                }
                self.drive(call)
            }
            "direction" => {
                self.forward.set(match value.as_text() {
                    Some("forward") => true,
                    Some("reverse") => false,
                    _ => return Err(Error::type_error(format!("a consist's `direction` is \"forward\" or \"reverse\", not {}", describe(&value)))),
                });
                self.drive(call)
            }
            other if PROPS.contains(&other) => Err(Error::type_error(format!("a consist's `{other}` can be read but not set"))),
            other => Err(call.unknown_property("consist", other, PROPS)),
        }
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("consist.{function}()");
        match function {
            "add" => {
                call.count_between(1, 1)?;
                call.only_options(&["reversed"])?;
                let loco = self.member_arg(call, 0, &what)?;
                let reversed = match call.option("reversed") {
                    None => false,
                    Some(Value::Bool(b)) => *b,
                    Some(other) => return Err(Error::type_error(format!("{what}'s `reversed:` is true or false, not {}", describe(other))).at(call.option_line("reversed"))),
                };
                self.add_member(loco, reversed, &what, call)?;
                Ok(Value::Null)
            }
            "remove" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let loco = self.member_arg(call, 0, &what)?;
                self.remove_member(&loco, &what, call)?;
                Ok(Value::Null)
            }
            "is_reversed" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let loco = self.member_arg(call, 0, &what)?;
                let i = self.position(&loco).ok_or_else(|| Error::range_error(format!("{what}: locomotive {} isn't in this consist", loco.address())))?;
                Ok(Value::from(self.members.borrow()[i].1))
            }
            "set_function" => {
                call.count_between(2, 2)?;
                call.only_options(&["to"])?;
                let n = function_number(&format!("consist.{function}"), call.arg(0))?;
                let on = call.arg(1).as_bool().ok_or_else(|| Error::type_error(format!("{what}: on or off is true or false, not {}", describe(call.arg(1)))))?;
                let members = self.members.borrow().clone();
                let first: Vec<Rc<Locomotive>> = members.first().map(|(m, _)| m.clone()).into_iter().collect();
                let targets: Vec<Rc<Locomotive>> = match call.option("to").cloned() {
                    None => first,
                    Some(Value::Text(t)) => match t.as_str() {
                        "lead" => first,
                        "all" => members.iter().map(|(m, _)| m.clone()).collect(),
                        "consist" => match &self.carrier {
                            Some(c) => vec![c.clone()],
                            None => return Err(Error::type_error(format!("{what}: `to: \"consist\"` is for an advanced consist (one with an address)")).at(call.option_line("to"))),
                        },
                        _ => return Err(Error::type_error(format!("{what}'s `to:` is \"lead\", \"all\", \"consist\", or one of its locomotives, not {t:?}")).at(call.option_line("to"))),
                    },
                    Some(other) => match call.own_option::<Locomotive>("to") {
                        Some(l) if self.position(&l).is_some() => vec![l],
                        Some(l) => return Err(Error::range_error(format!("{what}: locomotive {} isn't in this consist", l.address())).at(call.option_line("to"))),
                        None => return Err(Error::type_error(format!("{what}'s `to:` is \"lead\", \"all\", \"consist\", or one of its locomotives, not {}", describe(&other))).at(call.option_line("to"))),
                    },
                };
                for t in targets {
                    t.set_function_now(n, on, call)?;
                }
                Ok(Value::Null)
            }
            "stop" | "emergency_stop" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.speed.set(0);
                let emergency = function == "emergency_stop";
                if let Some(c) = &self.carrier {
                    c.halt(emergency, call)?;
                }
                // (Each member too: on its own address in a software consist; an emergency stop in an
                // advanced one as well, should a member have missed its consist.)
                let members = self.members.borrow().clone();
                if self.carrier.is_none() || emergency {
                    for (m, _) in members {
                        m.halt(emergency, call)?;
                    }
                }
                Ok(Value::Null)
            }
            "break_up" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.break_up_all(call)?;
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("consist", other, FUNCS)),
        }
    }
}
