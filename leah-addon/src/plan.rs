//! The track plan (design/model_railroading.md, "Phase C, item 12", model-railroading#16): which block
//! joins which, and through which turnout lying which way, written in code
//! (`$plan.connect($a, $b, turnout: $t, position: "thrown")`), with no window. `find_path` finds the
//! cheapest way from one block to another (each block costing 1 unless `set_cost` says otherwise),
//! around blocks to avoid (occupied ones too, unless told otherwise, and those another route holds),
//! through blocks it must pass, and gives it back as a route: its blocks in order and its turnouts'
//! ways, ready to `set()` and `reserve()`.
//!
//! A path never goes through one turnout twice: a train can't reverse at the points (in at one leg,
//! out at the other), and one turnout can't lie two ways at once.

use std::cell::RefCell;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use std::rc::Rc;

use leah_addon_kit::{Call, Error, Object, Value};

use crate::base::describe;
use crate::block::Block;
use crate::route::{self, Way};
use crate::turnout::Turnout;

const PROPS: &[&str] = &["blocks", "connections"];
const FUNCS: &[&str] = &["connect", "set_cost", "cost", "neighbours", "find_path"];
/// The most blocks a path may have unless `max_blocks:` says otherwise.
const MOST_BLOCKS: i64 = 64;
/// The most paths looked at before giving up (a plan with many loops).
const MOST_TRIED: usize = 200_000;

/// A way out of a block: the block reached, and the turnout (and the way it lies) the connection goes
/// through, if any.
pub(crate) type Exit = (Rc<Block>, Option<(Rc<Turnout>, bool)>);

/// One connection: two blocks (by their place in `blocks`), through a turnout lying a way, or plainly;
/// a one-way one only from `a` to `b`.
struct Joint {
    a: usize,
    b: usize,
    turnout: Option<(Rc<Turnout>, bool)>,
    one_way: bool,
}

#[derive(Default)]
pub struct TrackPlan {
    blocks: RefCell<Vec<Rc<Block>>>,
    costs: RefCell<HashMap<usize, i64>>,
    joints: RefCell<Vec<Joint>>,
}

/// `track_plan()`: an empty plan.
pub fn new_plan(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 0)?;
    call.only_options(&[])?;
    Ok(call.new_object(TrackPlan::default()))
}

impl TrackPlan {
    /// A block's place in the plan, added when it isn't there yet.
    fn place(&self, b: &Rc<Block>) -> usize {
        let mut blocks = self.blocks.borrow_mut();
        match blocks.iter().position(|x| Rc::ptr_eq(x, b)) {
            Some(i) => i,
            None => {
                blocks.push(b.clone());
                blocks.len() - 1
            }
        }
    }

    /// The cheapest path, as a route (`Value::Null` when there is none): `find_path`'s work, for the
    /// track diagram's entry and exit too.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn find(&self, call: &mut Call, from: &Rc<Block>, to: &Rc<Block>, via: &[Rc<Block>], avoid: &[Rc<Block>], avoid_occupied: bool, most: usize, delay: i64) -> Result<Value, Error> {
        // (Blocks not in the plan: no path.)
        let mut stops = vec![from];
        stops.extend(via.iter());
        stops.push(to);
        let mut places = Vec::new();
        for b in &stops {
            match self.found(b) {
                Some(i) => places.push(i),
                None => return Ok(Value::Null),
            }
        }
        let blocks = self.blocks.borrow().clone();
        for b in &blocks {
            if let Some(s) = b.sensor() {
                s.catch_up(call);
            }
        }
        let mut banned: Vec<bool> = blocks.iter().map(|b| {
            avoid.iter().any(|a| Rc::ptr_eq(a, b)) || b.reserved_by.borrow().is_some() || (avoid_occupied && b.is_occupied() == Some(true))
        }).collect();
        // (The train's own block, where it starts, is occupied by it.)
        banned[places[0]] = false;
        let mut path_blocks = vec![places[0]];
        let mut path_joints = Vec::new();
        let mut ways: HashMap<*const Turnout, bool> = HashMap::new();
        for leg in places.windows(2) {
            banned[leg[1]] = avoid.iter().any(|a| Rc::ptr_eq(a, &blocks[leg[1]]));
            if banned[leg[1]] {
                return Ok(Value::Null);
            }
            let Some((bs, js)) = self.search(leg[0], leg[1], &banned, &ways, most.saturating_sub(path_blocks.len()) + 1) else {
                return Ok(Value::Null);
            };
            let joints = self.joints.borrow();
            for j in &js {
                if let Some((t, closed)) = &joints[*j].turnout
                    && ways.insert(Rc::as_ptr(t), *closed).is_some_and(|w| w != *closed)
                {
                    return Ok(Value::Null);
                }
            }
            // (The blocks passed can't be passed again on the next leg.)
            for b in &bs {
                banned[*b] = true;
            }
            path_blocks.extend(bs.into_iter().skip(1));
            path_joints.extend(js);
        }
        let joints = self.joints.borrow();
        let mut settings: Vec<(Rc<Turnout>, Way)> = Vec::new();
        // (Each turnout's place: the path's block before it, which the train clears once past it.)
        let mut before: Vec<usize> = Vec::new();
        for (k, j) in path_joints.iter().enumerate() {
            if let Some((t, closed)) = &joints[*j].turnout
                && !settings.iter().any(|(s, _)| Rc::ptr_eq(s, t))
            {
                settings.push((t.clone(), if *closed { Way::Closed } else { Way::Thrown }));
                before.push(k);
            }
        }
        let route_blocks: Vec<Rc<Block>> = path_blocks.iter().map(|i| blocks[*i].clone()).collect();
        let name = format!("{} to {}", from.name(), to.name());
        drop(joints);
        Ok(route::make(call, name, settings, route_blocks, before, delay))
    }

    /// The ways a train can leave a block: each block it reaches, and the turnout (and its way) the
    /// connection goes through, if any (a one-way connection only from its first block).
    pub(crate) fn ways_from(&self, b: &Rc<Block>) -> Vec<Exit> {
        let Some(i) = self.found(b) else { return Vec::new() };
        let blocks = self.blocks.borrow();
        let joints = self.joints.borrow();
        self.exits(i).into_iter().map(|(n, j)| (blocks[n].clone(), joints[j].turnout.clone())).collect()
    }

    fn found(&self, b: &Rc<Block>) -> Option<usize> {
        self.blocks.borrow().iter().position(|x| Rc::ptr_eq(x, b))
    }

    fn cost_of(&self, i: usize) -> i64 {
        self.costs.borrow().get(&i).copied().unwrap_or(1)
    }

    /// The ways out of block `from`: (the block reached, the joint's place).
    fn exits(&self, from: usize) -> Vec<(usize, usize)> {
        self.joints.borrow().iter().enumerate().filter_map(|(j, joint)| {
            if joint.a == from {
                Some((joint.b, j))
            } else if joint.b == from && !joint.one_way {
                Some((joint.a, j))
            } else {
                None
            }
        }).collect()
    }

    /// The cheapest path from `from` to `to` not through `banned`: its blocks and its joints, or None.
    fn search(&self, from: usize, to: usize, banned: &[bool], ways: &HashMap<*const Turnout, bool>, most: usize) -> Option<(Vec<usize>, Vec<usize>)> {
        // (Uniform-cost search over simple paths: each path keeps its blocks, joints, and turnouts'
        // ways, so none is passed twice.)
        struct Path {
            blocks: Vec<usize>,
            joints: Vec<usize>,
            ways: HashMap<*const Turnout, bool>,
        }
        let joints = self.joints.borrow();
        let mut heap = BinaryHeap::new();
        let mut paths = vec![Path { blocks: vec![from], joints: vec![], ways: ways.clone() }];
        heap.push(Reverse((self.cost_of(from), 0usize)));
        let mut tried = 0;
        while let Some(Reverse((cost, p))) = heap.pop() {
            tried += 1;
            if tried > MOST_TRIED {
                return None;
            }
            let here = *paths[p].blocks.last().unwrap_or(&from);
            if here == to {
                return Some((paths[p].blocks.clone(), paths[p].joints.clone()));
            }
            if paths[p].blocks.len() >= most {
                continue;
            }
            for (next, j) in self.exits(here) {
                if banned[next] || paths[p].blocks.contains(&next) {
                    continue;
                }
                let mut ways = paths[p].ways.clone();
                if let Some((t, closed)) = &joints[j].turnout {
                    let key = Rc::as_ptr(t);
                    // (Through a turnout once only on a leg, and one way for the whole path.)
                    if paths[p].joints.iter().any(|pj| joints[*pj].turnout.as_ref().is_some_and(|(pt, _)| Rc::ptr_eq(pt, t))) {
                        continue;
                    }
                    match ways.get(&key) {
                        Some(w) if w != closed => continue,
                        _ => {
                            ways.insert(key, *closed);
                        }
                    }
                }
                let mut blocks = paths[p].blocks.clone();
                blocks.push(next);
                let mut js = paths[p].joints.clone();
                js.push(j);
                paths.push(Path { blocks, joints: js, ways });
                heap.push(Reverse((cost + self.cost_of(next), paths.len() - 1)));
            }
        }
        None
    }
}

fn block_of(call: &Call, v: &Value, what: &str) -> Result<Rc<Block>, Error> {
    call.own_value::<Block>(v).ok_or_else(|| Error::type_error(format!("{what} takes blocks, not {}", describe(v))))
}

fn blocks_option(call: &Call, name: &str, what: &str) -> Result<Vec<Rc<Block>>, Error> {
    match call.option(name) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items.iter().map(|(_, v)| block_of(call, v, &format!("{what}'s `{name}:`")).map_err(|e| e.at(call.option_line(name)))).collect(),
        Some(v) => Ok(vec![block_of(call, v, &format!("{what}'s `{name}:`")).map_err(|e| e.at(call.option_line(name)))?]),
    }
}

impl Object for TrackPlan {
    fn kind(&self) -> &str {
        "track_plan"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "blocks" => Value::list(self.blocks.borrow().iter().map(|b| call.object_value(b)).collect::<Vec<_>>()),
            "connections" => {
                let blocks = self.blocks.borrow();
                Value::list(self.joints.borrow().iter().map(|j| {
                    let mut pairs = vec![("from", call.object_value(&blocks[j.a])), ("to", call.object_value(&blocks[j.b]))];
                    if let Some((t, closed)) = &j.turnout {
                        pairs.push(("turnout", call.object_value(t)));
                        pairs.push(("position", Value::from(if *closed { "closed" } else { "thrown" })));
                    }
                    pairs.push(("one_way", Value::from(j.one_way)));
                    Value::map(pairs)
                }).collect::<Vec<_>>())
            }
            other => return Err(call.unknown_property("track_plan", other, PROPS)),
        })
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("track_plan.{function}()");
        match function {
            "connect" => {
                call.count_between(2, 2)?;
                call.only_options(&["turnout", "position", "one_way"])?;
                let a = block_of(call, call.arg(0), &what)?;
                let b = block_of(call, call.arg(1), &what)?;
                if Rc::ptr_eq(&a, &b) {
                    return Err(Error::type_error(format!("{what}: a block isn't joined to itself")));
                }
                let turnout = match call.option("turnout") {
                    None | Some(Value::Null) => None,
                    Some(v) => {
                        let t = call.own_option::<Turnout>("turnout").ok_or_else(|| Error::type_error(format!("{what}'s `turnout:` is a turnout, not {}", describe(v))).at(call.option_line("turnout")))?;
                        let closed = match call.option("position").and_then(Value::as_text) {
                            Some("closed") => true,
                            Some("thrown") => false,
                            _ => return Err(Error::type_error(format!("{what}: with a turnout, `position:` says which way it lies for this connection, \"closed\" or \"thrown\"")).at(call.option_line("position"))),
                        };
                        Some((t, closed))
                    }
                };
                if turnout.is_none() && call.option("position").is_some() {
                    return Err(Error::type_error(format!("{what}: `position:` goes with `turnout:`")).at(call.option_line("position")));
                }
                let one_way = match call.option("one_way") {
                    None => false,
                    Some(Value::Bool(b)) => *b,
                    Some(v) => return Err(Error::type_error(format!("{what}'s `one_way:` is true or false, not {}", describe(v))).at(call.option_line("one_way"))),
                };
                let (a, b) = (self.place(&a), self.place(&b));
                self.joints.borrow_mut().push(Joint { a, b, turnout, one_way });
                Ok(Value::Null)
            }
            "set_cost" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let b = block_of(call, call.arg(0), &what)?;
                let cost = match call.arg(1).as_int() {
                    Some(n @ 1..=1_000_000) => n,
                    _ => return Err(Error::range_error(format!("{what}: a block's cost is 1 to 1000000 (1 for the main line, more for a siding a path should keep out of), not {}", describe(call.arg(1))))),
                };
                let i = self.place(&b);
                self.costs.borrow_mut().insert(i, cost);
                Ok(Value::Null)
            }
            "cost" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let b = block_of(call, call.arg(0), &what)?;
                Ok(Value::from(self.found(&b).map_or(1, |i| self.cost_of(i))))
            }
            "neighbours" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let b = block_of(call, call.arg(0), &what)?;
                let Some(i) = self.found(&b) else { return Ok(Value::list(Vec::<Value>::new())) };
                let mut seen: Vec<usize> = Vec::new();
                for (n, _) in self.exits(i) {
                    if !seen.contains(&n) {
                        seen.push(n);
                    }
                }
                let blocks = self.blocks.borrow().clone();
                Ok(Value::list(seen.into_iter().map(|n| call.object_value(&blocks[n])).collect::<Vec<_>>()))
            }
            "find_path" => {
                call.count_between(2, 2)?;
                call.only_options(&["via", "avoid", "avoid_occupied", "max_blocks", "delay"])?;
                let from = block_of(call, call.arg(0), &what)?;
                let to = block_of(call, call.arg(1), &what)?;
                let via = blocks_option(call, "via", &what)?;
                let avoid = blocks_option(call, "avoid", &what)?;
                let avoid_occupied = match call.option("avoid_occupied") {
                    None => true,
                    Some(Value::Bool(b)) => *b,
                    Some(v) => return Err(Error::type_error(format!("{what}'s `avoid_occupied:` is true or false, not {}", describe(v))).at(call.option_line("avoid_occupied"))),
                };
                let most = match call.option("max_blocks") {
                    None => MOST_BLOCKS,
                    Some(v) => match v.as_int() {
                        Some(n @ 1..=1000) => n,
                        _ => return Err(Error::range_error(format!("{what}'s `max_blocks:` is 1 to 1000, not {}", describe(v))).at(call.option_line("max_blocks"))),
                    },
                } as usize;
                let delay = match call.option("delay") {
                    None => 250,
                    Some(v) => match v.as_int() {
                        Some(n @ 0..=10_000) => n,
                        _ => return Err(Error::range_error(format!("{what}'s `delay:` is the milliseconds between two turnouts' moves, 0 to 10000, not {}", describe(v))).at(call.option_line("delay"))),
                    },
                };
                self.find(call, &from, &to, &via, &avoid, avoid_occupied, most, delay)
            }
            other => Err(call.unknown_function("track_plan", other, FUNCS)),
        }
    }
}
