//! The track diagram (design/model_railroading.md, "Phase C, item 13", model-railroading#17): a
//! window's control (the kit's level 5) drawing the layout as a panel does, on a grid the script
//! places it on: track colored by its block (free, occupied, held by a route), each turnout's legs (the
//! one it lies on bright), sensors' and lights' lamps, and labels. A click throws a turnout, switches a
//! light, and, with a track plan given, picks an entry block, then an exit one, and lines and holds the
//! path between them (entry and exit). It follows the objects it shows, and draws again as they change.
//!
//! GTK's clicks come with no call's context: each is posted to the diagram's own function
//! (`diagram_click`, the kit's level 6), which runs on the script's thread with one.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::describe;
use crate::block::Block;
use crate::light::Light;
use crate::plan::TrackPlan;
use crate::route::Route;
use crate::sensor::{as_follower, Follower, Sensor};
use crate::turnout::Turnout;

pub(crate) const PROPS: &[&str] = &["grid", "track_width", "show_names", "background_color", "plan", "entry"];
pub(crate) const FUNCS: &[&str] = &["add_track", "add_turnout", "add_sensor", "add_light", "add_label", "clear", "pick_block", "cancel_entry"];
pub(crate) const EVENTS: &[&str] = &["on_turnout_clicked", "on_block_clicked", "on_light_clicked", "on_sensor_clicked", "on_route_set", "on_route_refused"];

type Point = (f64, f64);

/// One thing drawn, with where (in grid units).
enum Piece {
    Track(Option<Rc<Block>>, Vec<Point>),
    Turnout { turnout: Rc<Turnout>, at: Point, closed_to: Point, thrown_to: Point, block: Option<Rc<Block>> },
    Sensor(Rc<Sensor>, Point),
    Light(Rc<Light>, Point),
    Label(String, Point),
}

/// What the drawing and the clicks share.
pub(crate) struct Shown {
    pieces: RefCell<Vec<Piece>>,
    grid: Cell<f64>,
    track_width: Cell<f64>,
    show_names: Cell<bool>,
    background: RefCell<Option<(f64, f64, f64)>>,
    /// The entry block picked (entry and exit), waiting for its exit.
    entry: RefCell<Option<Rc<Block>>>,
}

pub struct TrackDiagram {
    shown: Rc<Shown>,
    plan: RefCell<Option<Rc<TrackPlan>>>,
    /// The routes entry and exit lined, kept while they hold their blocks.
    routes: RefCell<Vec<Rc<Route>>>,
    me: RefCell<Weak<TrackDiagram>>,
    events: Events,
    #[cfg(feature = "windows")]
    area: gtk4::DrawingArea,
}

const FREE: (f64, f64, f64) = (0.78, 0.80, 0.84);
const OCCUPIED: (f64, f64, f64) = (0.88, 0.22, 0.24);
const RESERVED: (f64, f64, f64) = (0.18, 0.64, 0.31);
const UNKNOWN: (f64, f64, f64) = (0.48, 0.51, 0.56);
const DIM: (f64, f64, f64) = (0.33, 0.36, 0.41);
const LAMP_ON: (f64, f64, f64) = (0.96, 0.75, 0.17);
const ACTIVE: (f64, f64, f64) = (0.18, 0.64, 0.31);
const ENTRY: (f64, f64, f64) = (1.0, 1.0, 1.0);

fn block_color(b: Option<&Rc<Block>>) -> (f64, f64, f64) {
    match b {
        None => FREE,
        Some(b) if b.is_occupied() == Some(true) => OCCUPIED,
        Some(b) if b.reserved_by.borrow().is_some() => RESERVED,
        Some(b) if b.is_occupied().is_none() => UNKNOWN,
        Some(_) => FREE,
    }
}

/// The distance from a point to a line segment.
fn to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx * dx + dy * dy;
    let t = if len == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len).clamp(0.0, 1.0) };
    let (x, y) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - x).powi(2) + (p.1 - y).powi(2)).sqrt()
}

fn point_of(v: &Value, what: &str) -> Result<Point, Error> {
    if let Value::Array(two) = v
        && two.len() == 2
        && let (Some(x), Some(y)) = (two[0].1.as_number(), two[1].1.as_number())
    {
        return Ok((x, y));
    }
    Err(Error::type_error(format!("{what}: a place on the diagram's grid is [x, y], like [3, 2], not {}", describe(v))))
}

fn color_of(v: &Value, what: &str) -> Result<(f64, f64, f64), Error> {
    let bad = || Error::type_error(format!("{what}: a color is hex text, like \"#2B2F36\", not {}", describe(v)));
    let t = v.as_text().ok_or_else(bad)?.trim().trim_start_matches('#');
    if t.len() != 6 {
        return Err(bad());
    }
    let part = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).map(|n| f64::from(n) / 255.0).map_err(|_| bad());
    Ok((part(0)?, part(2)?, part(4)?))
}

impl Shown {
    /// What a click at (x, y) pixels lands on: the nearest piece within reach.
    fn hit(&self, x: f64, y: f64) -> Option<usize> {
        let g = self.grid.get();
        let p = (x / g, y / g);
        let reach = (self.track_width.get() / g).max(0.25) + 0.25;
        let mut best: Option<(f64, usize)> = None;
        for (i, piece) in self.pieces.borrow().iter().enumerate() {
            let d = match piece {
                Piece::Track(_, points) => points.windows(2).map(|w| to_segment(p, w[0], w[1])).fold(f64::INFINITY, f64::min),
                // (A turnout is reached near its points, ahead of the track around it.)
                Piece::Turnout { at, closed_to, thrown_to, .. } => {
                    let legs = to_segment(p, *at, *closed_to).min(to_segment(p, *at, *thrown_to));
                    let near = ((p.0 - at.0).powi(2) + (p.1 - at.1).powi(2)).sqrt();
                    if near < reach * 2.0 { near * 0.5 } else { legs }
                }
                Piece::Sensor(_, at) | Piece::Light(_, at) => ((p.0 - at.0).powi(2) + (p.1 - at.1).powi(2)).sqrt() * 0.5,
                Piece::Label(..) => f64::INFINITY,
            };
            if d <= reach && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }

    #[cfg(feature = "windows")]
    fn draw(&self, cr: &gtk4::cairo::Context, width: f64, height: f64) {
        let g = self.grid.get();
        let w = self.track_width.get();
        if let Some((r, gr, b)) = *self.background.borrow() {
            cr.set_source_rgb(r, gr, b);
            cr.rectangle(0.0, 0.0, width, height);
            let _ = cr.fill();
        }
        cr.set_line_cap(gtk4::cairo::LineCap::Round);
        cr.set_line_join(gtk4::cairo::LineJoin::Round);
        let line = |cr: &gtk4::cairo::Context, points: &[Point], color: (f64, f64, f64), width: f64| {
            if points.len() < 2 {
                return;
            }
            cr.set_source_rgb(color.0, color.1, color.2);
            cr.set_line_width(width);
            cr.move_to(points[0].0 * g, points[0].1 * g);
            for p in &points[1..] {
                cr.line_to(p.0 * g, p.1 * g);
            }
            let _ = cr.stroke();
        };
        let entry = self.entry.borrow().clone();
        let pieces = self.pieces.borrow();
        // Track first, then turnouts over it, then lamps and words.
        for piece in pieces.iter() {
            if let Piece::Track(block, points) = piece {
                if let (Some(e), Some(b)) = (&entry, block)
                    && Rc::ptr_eq(e, b)
                {
                    line(cr, points, ENTRY, w + 4.0);
                }
                line(cr, points, block_color(block.as_ref()), w);
            }
        }
        for piece in pieces.iter() {
            if let Piece::Turnout { turnout, at, closed_to, thrown_to, block } = piece {
                let color = block_color(block.as_ref());
                let (lying, other) = match turnout.state_word() {
                    "closed" => (Some(*closed_to), Some(*thrown_to)),
                    "thrown" => (Some(*thrown_to), Some(*closed_to)),
                    _ => (None, None),
                };
                match (lying, other) {
                    (Some(l), Some(o)) => {
                        line(cr, &[*at, o], DIM, w);
                        line(cr, &[*at, l], color, w);
                    }
                    _ => {
                        line(cr, &[*at, *closed_to], DIM, w);
                        line(cr, &[*at, *thrown_to], DIM, w);
                    }
                }
                if turnout.is_locked() {
                    cr.set_source_rgb(ENTRY.0, ENTRY.1, ENTRY.2);
                    cr.arc(at.0 * g, at.1 * g, w * 0.45, 0.0, std::f64::consts::TAU);
                    let _ = cr.fill();
                }
            }
        }
        for piece in pieces.iter() {
            let (at, lit, color) = match piece {
                Piece::Sensor(s, at) => (*at, s.is_active(), ACTIVE),
                Piece::Light(l, at) => (*at, Some(l.is_on()), LAMP_ON),
                _ => continue,
            };
            let r = (g * 0.3).max(4.0);
            cr.arc(at.0 * g, at.1 * g, r, 0.0, std::f64::consts::TAU);
            match lit {
                Some(true) => cr.set_source_rgb(color.0, color.1, color.2),
                Some(false) => cr.set_source_rgb(DIM.0, DIM.1, DIM.2),
                None => cr.set_source_rgb(UNKNOWN.0, UNKNOWN.1, UNKNOWN.2),
            }
            let _ = cr.fill_preserve();
            cr.set_source_rgb(0.1, 0.1, 0.12);
            cr.set_line_width(1.0);
            let _ = cr.stroke();
        }
        cr.set_font_size((g * 0.55).clamp(9.0, 16.0));
        for piece in pieces.iter() {
            let (text, at) = match piece {
                Piece::Label(t, at) => (t.clone(), *at),
                Piece::Track(Some(b), points) if self.show_names.get() && points.len() >= 2 => {
                    let (a, z) = (points[0], points[1]);
                    (b.name().to_string(), ((a.0 + z.0) / 2.0, (a.1 + z.1) / 2.0 - 0.6))
                }
                _ => continue,
            };
            let extents = cr.text_extents(&text).ok();
            let half = extents.map_or(0.0, |e| e.width() / 2.0);
            cr.set_source_rgb(0.86, 0.88, 0.91);
            cr.move_to(at.0 * g - half, at.1 * g);
            let _ = cr.show_text(&text);
        }
    }
}

/// `track_diagram`'s maker: Leah calls it as a window's control is made, then sets its settings.
pub fn new_diagram(call: &mut Call) -> Result<Value, Error> {
    let shown = Rc::new(Shown {
        pieces: RefCell::new(Vec::new()),
        grid: Cell::new(20.0),
        track_width: Cell::new(5.0),
        show_names: Cell::new(true),
        background: RefCell::new(Some((0.17, 0.18, 0.21))),
        entry: RefCell::new(None),
    });
    #[cfg(feature = "windows")]
    {
        use gtk4::prelude::*;
        // (Leah has started GTK: this library's own copy of GTK's bindings is told so.)
        unsafe { gtk4::set_initialized() };
        let area = gtk4::DrawingArea::new();
        let drawn = shown.clone();
        area.set_draw_func(move |_, cr, w, h| drawn.draw(cr, f64::from(w), f64::from(h)));
        let value = call.new_object_with_events(|events| {
            let click = gtk4::GestureClick::new();
            let poster = events.clone();
            click.connect_pressed(move |_, _, x, y| {
                poster.post("diagram_click", vec![Value::Float(x), Value::Float(y)]);
            });
            area.add_controller(click);
            TrackDiagram { shown, plan: RefCell::new(None), routes: RefCell::new(Vec::new()), me: RefCell::new(Weak::new()), events, area }
        });
        if let Some(made) = call.own_value::<TrackDiagram>(&value) {
            *made.me.borrow_mut() = Rc::downgrade(&made);
        }
        Ok(value)
    }
    #[cfg(not(feature = "windows"))]
    {
        let _ = shown;
        Err(Error::new("gui_error", "track_diagram: this build of the model railroading addon has no windows (its `windows` feature)"))
    }
}

impl TrackDiagram {
    fn redraw(&self) {
        #[cfg(feature = "windows")]
        {
            use gtk4::prelude::*;
            self.area.queue_draw();
        }
    }

    fn me(&self) -> Option<Weak<dyn Follower>> {
        self.me.borrow().upgrade().map(|me| as_follower(&me))
    }

    /// A click, posted from GTK's handler: what it lands on is thrown, switched, or picked.
    fn clicked(&self, x: f64, y: f64, call: &mut Call) -> Result<(), Error> {
        let Some(i) = self.shown.hit(x, y) else { return Ok(()) };
        enum Hit {
            Turnout(Rc<Turnout>),
            Block(Rc<Block>),
            Light(Rc<Light>),
            Sensor(Rc<Sensor>),
        }
        let hit = match &self.shown.pieces.borrow()[i] {
            Piece::Turnout { turnout, .. } => Hit::Turnout(turnout.clone()),
            Piece::Track(Some(b), _) => Hit::Block(b.clone()),
            Piece::Light(l, _) => Hit::Light(l.clone()),
            Piece::Sensor(s, _) => Hit::Sensor(s.clone()),
            _ => return Ok(()),
        };
        match hit {
            Hit::Turnout(t) => {
                let closed = t.state_word() != "closed";
                if let Err(e) = t.set(closed, call) {
                    call.warn(e.message.clone());
                }
                self.events.fire("on_turnout_clicked", vec![call.object_value(&t)]);
            }
            Hit::Light(l) => {
                l.toggle_now(call)?;
                self.events.fire("on_light_clicked", vec![call.object_value(&l)]);
            }
            Hit::Sensor(s) => self.events.fire("on_sensor_clicked", vec![call.object_value(&s)]),
            Hit::Block(b) => {
                self.events.fire("on_block_clicked", vec![call.object_value(&b)]);
                let plan = self.plan.borrow().clone();
                if let Some(plan) = plan {
                    self.entry_exit(&plan, b, call)?;
                }
            }
        }
        self.redraw();
        Ok(())
    }

    /// Entry and exit: the first block clicked is the entry; the second, the exit, gets the path between
    /// them lined and held, or refused.
    fn entry_exit(&self, plan: &Rc<TrackPlan>, b: Rc<Block>, call: &mut Call) -> Result<(), Error> {
        let entry = self.shown.entry.borrow_mut().take();
        let Some(entry) = entry else {
            *self.shown.entry.borrow_mut() = Some(b);
            return Ok(());
        };
        if Rc::ptr_eq(&entry, &b) {
            return Ok(());
        }
        self.routes.borrow_mut().retain(|r| r.is_reserved());
        let found = plan.find(call, &entry, &b, &[], &[], true, 64, 250)?;
        let route = call.own_value::<Route>(&found);
        let lined = match &route {
            Some(r) if r.reserve(call) => {
                r.set(call)?;
                true
            }
            _ => false,
        };
        match route {
            Some(r) if lined => {
                self.routes.borrow_mut().push(r.clone());
                self.events.fire("on_route_set", vec![call.object_value(&r)]);
            }
            _ => self.events.fire("on_route_refused", vec![call.object_value(&entry), call.object_value(&b)]),
        }
        Ok(())
    }

    fn follow_block(&self, b: &Rc<Block>) {
        if let Some(me) = self.me() {
            b.followers.add(me);
        }
    }
}

impl Follower for TrackDiagram {
    fn followed_changed(&self, _call: &Call) {
        self.redraw();
    }
}

impl Object for TrackDiagram {
    fn kind(&self) -> &str {
        "track_diagram"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "grid" => Value::Float(self.shown.grid.get()),
            "track_width" => Value::Float(self.shown.track_width.get()),
            "show_names" => Value::from(self.shown.show_names.get()),
            "background_color" => match *self.shown.background.borrow() {
                Some((r, g, b)) => Value::from(format!("#{:02X}{:02X}{:02X}", (r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)),
                None => Value::Null,
            },
            "plan" => match self.plan.borrow().as_ref() {
                Some(p) => call.object_value(p),
                None => Value::Null,
            },
            "entry" => match self.shown.entry.borrow().as_ref() {
                Some(b) => call.object_value(b),
                None => Value::Null,
            },
            other => return Err(call.unknown_property("track_diagram", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        let what = format!("a track_diagram's `{property}`");
        match property {
            "grid" => match value.as_number() {
                Some(n) if (4.0..=200.0).contains(&n) => self.shown.grid.set(n),
                _ => return Err(Error::range_error(format!("{what} is the pixels of one grid step, 4 to 200, not {}", describe(&value)))),
            },
            "track_width" => match value.as_number() {
                Some(n) if (1.0..=40.0).contains(&n) => self.shown.track_width.set(n),
                _ => return Err(Error::range_error(format!("{what} is pixels, 1 to 40, not {}", describe(&value)))),
            },
            "show_names" => match value {
                Value::Bool(b) => self.shown.show_names.set(b),
                other => return Err(Error::type_error(format!("{what} is true or false, not {}", describe(&other)))),
            },
            "background_color" => {
                *self.shown.background.borrow_mut() = match value {
                    Value::Null => None,
                    other => Some(color_of(&other, &what)?),
                }
            }
            "plan" => {
                *self.plan.borrow_mut() = match &value {
                    Value::Null => None,
                    other => Some(call.own_value::<TrackPlan>(other).ok_or_else(|| Error::type_error(format!("{what} is a track_plan (for entry and exit: a block clicked, then another), not {}", describe(other))))?),
                };
                self.shown.entry.borrow_mut().take();
            }
            other if PROPS.contains(&other) => return Err(Error::type_error(format!("a track_diagram's `{other}` can be read but not set"))),
            other => return Err(call.unknown_property("track_diagram", other, PROPS)),
        }
        self.redraw();
        Ok(())
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("track_diagram.{function}()");
        match function {
            "add_track" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let block = match call.arg(0) {
                    Value::Null => None,
                    v => Some(call.own_value::<Block>(v).ok_or_else(|| Error::type_error(format!("{what}: the track's block is a block (null for track in none), not {}", describe(v))))?),
                };
                let Value::Array(items) = call.arg(1) else {
                    return Err(Error::type_error(format!("{what}: the track is a list of places, [[1, 3], [6, 3]], not {}", describe(call.arg(1)))));
                };
                let points = items.iter().map(|(_, v)| point_of(v, &what)).collect::<Result<Vec<_>, _>>()?;
                if points.len() < 2 {
                    return Err(Error::type_error(format!("{what}: the track goes through two places at least")));
                }
                if let Some(b) = &block {
                    self.follow_block(b);
                }
                self.shown.pieces.borrow_mut().push(Piece::Track(block, points));
            }
            "add_turnout" => {
                call.count_between(2, 2)?;
                call.only_options(&["closed_to", "thrown_to", "block"])?;
                let turnout = call.own::<Turnout>(0).ok_or_else(|| Error::type_error(format!("{what}: the first value is a turnout, not {}", describe(call.arg(0)))))?;
                let at = point_of(call.arg(1), &what)?;
                let leg = |name: &str| -> Result<Point, Error> {
                    let v = call.option(name).ok_or_else(|| Error::type_error(format!("{what}: give `closed_to:` and `thrown_to:`, the places its two legs lead to")))?;
                    point_of(v, &what).map_err(|e| e.at(call.option_line(name)))
                };
                let (closed_to, thrown_to) = (leg("closed_to")?, leg("thrown_to")?);
                let block = match call.option("block") {
                    None | Some(Value::Null) => None,
                    Some(v) => Some(call.own_option::<Block>("block").ok_or_else(|| Error::type_error(format!("{what}'s `block:` is the block its points are in, not {}", describe(v))).at(call.option_line("block")))?),
                };
                if let Some(me) = self.me() {
                    turnout.followers.add(me);
                }
                if let Some(b) = &block {
                    self.follow_block(b);
                }
                self.shown.pieces.borrow_mut().push(Piece::Turnout { turnout, at, closed_to, thrown_to, block });
            }
            "add_sensor" | "add_light" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let at = point_of(call.arg(1), &what)?;
                let me = self.me();
                let piece = if function == "add_sensor" {
                    let s = call.own::<Sensor>(0).ok_or_else(|| Error::type_error(format!("{what}: the first value is a sensor, not {}", describe(call.arg(0)))))?;
                    if let Some(me) = me {
                        s.follow(me);
                    }
                    Piece::Sensor(s, at)
                } else {
                    let l = call.own::<Light>(0).ok_or_else(|| Error::type_error(format!("{what}: the first value is a light, not {}", describe(call.arg(0)))))?;
                    if let Some(me) = me {
                        l.followers.add(me);
                    }
                    Piece::Light(l, at)
                };
                self.shown.pieces.borrow_mut().push(piece);
            }
            "add_label" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let text = call.text(0)?.to_string();
                let at = point_of(call.arg(1), &what)?;
                self.shown.pieces.borrow_mut().push(Piece::Label(text, at));
            }
            "clear" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.shown.pieces.borrow_mut().clear();
                self.shown.entry.borrow_mut().take();
            }
            "pick_block" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let b = call.own::<Block>(0).ok_or_else(|| Error::type_error(format!("{what}: the value is a block, not {}", describe(call.arg(0)))))?;
                let plan = self.plan.borrow().clone().ok_or_else(|| Error::type_error(format!("{what}: entry and exit need the diagram's `plan`, a track_plan")))?;
                self.entry_exit(&plan, b, call)?;
            }
            "cancel_entry" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                self.shown.entry.borrow_mut().take();
            }
            // (Posted by GTK's click handler, the kit's level 6: not listed for scripts.)
            "diagram_click" => {
                let (x, y) = (call.number(0)?, call.number(1)?);
                self.clicked(x, y, call)?;
                return Ok(Value::Null);
            }
            other => return Err(call.unknown_function("track_diagram", other, FUNCS)),
        }
        self.redraw();
        Ok(Value::Null)
    }

    #[cfg(feature = "windows")]
    fn widget(&self) -> *mut std::ffi::c_void {
        use gtk4::prelude::*;
        self.area.upcast_ref::<gtk4::Widget>().as_ptr().cast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(pieces: Vec<Piece>) -> Shown {
        Shown { pieces: RefCell::new(pieces), grid: Cell::new(20.0), track_width: Cell::new(5.0), show_names: Cell::new(true), background: RefCell::new(None), entry: RefCell::new(None) }
    }

    #[test]
    fn a_click_lands_on_the_nearest_track_within_reach() {
        let s = shown(vec![Piece::Track(None, vec![(1.0, 4.0), (6.0, 4.0)]), Piece::Track(None, vec![(1.0, 2.0), (6.0, 2.0)]), Piece::Label("words".into(), (3.0, 3.0))]);
        assert_eq!(s.hit(60.0, 82.0), Some(0));
        assert_eq!(s.hit(60.0, 38.0), Some(1));
        // (Between the two, out of reach of both; a label is never clicked.)
        assert_eq!(s.hit(60.0, 60.0), None);
        assert_eq!(s.hit(400.0, 80.0), None);
    }

    #[test]
    fn a_segments_distance_is_to_its_nearest_point() {
        assert_eq!(to_segment((3.0, 1.0), (0.0, 0.0), (6.0, 0.0)), 1.0);
        assert_eq!(to_segment((9.0, 4.0), (0.0, 0.0), (6.0, 0.0)), 5.0);
    }
}
