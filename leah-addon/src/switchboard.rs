//! The switchboard (model-railroading#34, panels' part 2): a window's control (the kit's level 5) with a
//! button for each turnout, light, route, and sensor, in rows `columns` wide, each saying its name and
//! state and colored by it; a click throws the turnout, switches the light, sets the route (a sensor's
//! button only says it was clicked). The bench and club tool when no diagram is drawn: given a
//! `layout`, it shows every turnout, light, and sensor of it. It follows what it shows.
//!
//! GTK's clicks come with no call's context: each is posted to the switchboard's own function
//! (`switchboard_click`, the kit's level 6).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::describe;
use crate::layout::Layout;
use crate::light::Light;
use crate::route::Route;
use crate::sensor::{as_follower, Follower, Sensor};
use crate::turnout::Turnout;

pub(crate) const PROPS: &[&str] = &["columns", "count"];
pub(crate) const FUNCS: &[&str] = &["add", "add_layout", "clear"];
pub(crate) const EVENTS: &[&str] = &["on_clicked"];

#[derive(Clone)]
enum Shown {
    Turnout(Rc<Turnout>),
    Light(Rc<Light>),
    Route(Rc<Route>),
    Sensor(Rc<Sensor>),
}

impl Shown {
    /// Its words on the button, and whether it is "on" (thrown, lit, aligned, active).
    fn words(&self) -> (String, Option<bool>) {
        match self {
            Shown::Turnout(t) => {
                let w = t.state_word();
                (format!("{}\n{w}", t.name()), match w {
                    "thrown" => Some(true),
                    "closed" => Some(false),
                    _ => None,
                })
            }
            Shown::Light(l) => (format!("{}\n{}", l.name(), if l.is_on() { "on" } else { "off" }), Some(l.is_on())),
            Shown::Route(r) => (format!("{}\n{}", r.name(), if r.is_aligned() { "set" } else { "not set" }), Some(r.is_aligned())),
            Shown::Sensor(s) => (format!("{}\n{}", s.name(), match s.is_active() {
                Some(true) => "active",
                Some(false) => "inactive",
                None => "unknown",
            }), s.is_active()),
        }
    }
}

pub struct Switchboard {
    shown: RefCell<Vec<Shown>>,
    columns: Cell<i64>,
    me: RefCell<Weak<Switchboard>>,
    events: Events,
    #[cfg(feature = "windows")]
    grid: gtk4::Grid,
    #[cfg(feature = "windows")]
    buttons: RefCell<Vec<gtk4::Button>>,
}

/// `switchboard`'s maker: Leah calls it as a window's control is made, then sets its settings.
pub fn new_switchboard(call: &mut Call) -> Result<Value, Error> {
    #[cfg(feature = "windows")]
    {
        use gtk4::prelude::*;
        // (Leah has started GTK: this library's own copy of GTK's bindings is told so.)
        unsafe { gtk4::set_initialized() };
        let grid = gtk4::Grid::new();
        grid.set_row_spacing(6);
        grid.set_column_spacing(6);
        grid.set_row_homogeneous(true);
        grid.set_column_homogeneous(true);
        // (The colors of a button's state, once per program.)
        thread_local! {
            static STYLED: Cell<bool> = const { Cell::new(false) };
        }
        if !STYLED.with(|s| s.replace(true))
            && let Some(display) = gtk4::gdk::Display::default()
        {
            let css = gtk4::CssProvider::new();
            css.load_from_string(".leah-switch-on { background: #2DA44E; color: white; } .leah-switch-off { background: #5B6779; color: white; } .leah-switch-unknown { background: #A9B3C1; color: #20242B; }");
            gtk4::style_context_add_provider_for_display(&display, &css, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
        let value = call.new_object_with_events(|events| Switchboard { shown: RefCell::new(Vec::new()), columns: Cell::new(4), me: RefCell::new(Weak::new()), events, grid, buttons: RefCell::new(Vec::new()) });
        if let Some(made) = call.own_value::<Switchboard>(&value) {
            *made.me.borrow_mut() = Rc::downgrade(&made);
        }
        Ok(value)
    }
    #[cfg(not(feature = "windows"))]
    Err(Error::new("gui_error", "switchboard: this build of the model railroading addon has no windows (its `windows` feature)"))
}

impl Switchboard {
    /// Its buttons made again for what it shows, in rows `columns` wide.
    fn rebuild(&self) {
        #[cfg(feature = "windows")]
        {
            use gtk4::prelude::*;
            for b in self.buttons.borrow_mut().drain(..) {
                self.grid.remove(&b);
            }
            let columns = self.columns.get().max(1) as i32;
            for (i, _) in self.shown.borrow().iter().enumerate() {
                let button = gtk4::Button::new();
                let events = self.events.clone();
                button.connect_clicked(move |_| {
                    events.post("switchboard_click", vec![Value::Int(i as i64)]);
                });
                self.grid.attach(&button, i as i32 % columns, i as i32 / columns, 1, 1);
                self.buttons.borrow_mut().push(button);
            }
        }
        self.refresh();
    }

    /// Each button's words and color, as what it shows is now.
    fn refresh(&self) {
        #[cfg(feature = "windows")]
        {
            use gtk4::prelude::*;
            for (shown, button) in self.shown.borrow().iter().zip(self.buttons.borrow().iter()) {
                let (words, on) = shown.words();
                button.set_label(&words);
                for c in ["leah-switch-on", "leah-switch-off", "leah-switch-unknown"] {
                    button.remove_css_class(c);
                }
                button.add_css_class(match on {
                    Some(true) => "leah-switch-on",
                    Some(false) => "leah-switch-off",
                    None => "leah-switch-unknown",
                });
            }
        }
    }

    fn add_one(&self, call: &Call, v: &Value, what: &str) -> Result<(), Error> {
        let me = self.me.borrow().upgrade().map(|m| as_follower(&m));
        let shown = if let Some(t) = call.own_value::<Turnout>(v) {
            if let Some(me) = me {
                t.followers.add(me);
            }
            Shown::Turnout(t)
        } else if let Some(l) = call.own_value::<Light>(v) {
            if let Some(me) = me {
                l.followers.add(me);
            }
            Shown::Light(l)
        } else if let Some(r) = call.own_value::<Route>(v) {
            if let Some(me) = me {
                r.followers.add(me);
            }
            Shown::Route(r)
        } else if let Some(s) = call.own_value::<Sensor>(v) {
            if let Some(me) = me {
                s.follow(me);
            }
            Shown::Sensor(s)
        } else {
            return Err(Error::type_error(format!("{what} takes turnouts, lights, routes, sensors, and layouts, not {}", describe(v))));
        };
        self.shown.borrow_mut().push(shown);
        Ok(())
    }

    fn clicked(&self, i: usize, call: &mut Call) -> Result<(), Error> {
        let Some(shown) = self.shown.borrow().get(i).cloned() else { return Ok(()) };
        let value = match &shown {
            Shown::Turnout(t) => {
                if let Err(e) = t.set(t.state_word() != "closed", call) {
                    call.warn(e.message.clone());
                }
                call.object_value(t)
            }
            Shown::Light(l) => {
                l.toggle_now(call)?;
                call.object_value(l)
            }
            Shown::Route(r) => {
                if let Err(e) = r.set(call) {
                    call.warn(e.message.clone());
                }
                call.object_value(r)
            }
            Shown::Sensor(s) => call.object_value(s),
        };
        self.events.fire("on_clicked", vec![value]);
        self.refresh();
        Ok(())
    }
}

impl Follower for Switchboard {
    fn followed_changed(&self, _call: &Call) {
        self.refresh();
    }
}

impl Object for Switchboard {
    fn kind(&self) -> &str {
        "switchboard"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "columns" => Value::from(self.columns.get()),
            "count" => Value::from(self.shown.borrow().len() as i64),
            other => return Err(call.unknown_property("switchboard", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "columns" => match value.as_int() {
                Some(n @ 1..=50) => {
                    self.columns.set(n);
                    self.rebuild();
                }
                _ => return Err(Error::range_error(format!("a switchboard's `columns` is 1 to 50, not {}", describe(&value)))),
            },
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a switchboard's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("switchboard", other, PROPS)),
        }
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("switchboard.{function}()");
        match function {
            "add" => {
                if call.args.is_empty() {
                    return Err(Error::type_error(format!("{what}: give the turnouts, lights, routes, and sensors to show")));
                }
                call.only_options(&[])?;
                for v in call.args.clone() {
                    self.add_one(call, &v, &what)?;
                }
                self.rebuild();
            }
            "add_layout" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let layout = call.own::<Layout>(0).ok_or_else(|| Error::type_error(format!("{what}: the value is a layout, not {}", describe(call.arg(0)))))?;
                // (Its turnouts, then its lights, then its sensors; blocks have no button.)
                let items = layout.items();
                for kind in ["turnout", "light", "sensor"] {
                    for item in items.iter().filter(|i| i.kind() == kind) {
                        let v = item.value(call);
                        self.add_one(call, &v, &what)?;
                    }
                }
                self.rebuild();
            }
            "clear" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.shown.borrow_mut().clear();
                self.rebuild();
            }
            // (Posted by GTK's click handler, the kit's level 6: not listed for scripts.)
            "switchboard_click" => {
                let i = call.int(0)? as usize;
                self.clicked(i, call)?;
            }
            other => return Err(call.unknown_function("switchboard", other, FUNCS)),
        }
        Ok(Value::Null)
    }

    #[cfg(feature = "windows")]
    fn widget(&self) -> *mut std::ffi::c_void {
        use gtk4::prelude::*;
        self.grid.upcast_ref::<gtk4::Widget>().as_ptr().cast()
    }
}
