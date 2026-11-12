//! The phone throttle server (design/model_railroading.md, "Phase B, item 8", model-railroading#12):
//! phone throttle apps that speak the WiThrottle protocol (Engine Driver, WiThrottle, and others)
//! drive the base station's locomotives over the local network. "WiThrottle" is its author's
//! trademark, and the author publishes no specification: this is built from the protocol's
//! published descriptions, never from another program's code, and is never called "WiThrottle"
//! itself.
//!
//! Threads of the server's own listen, read each phone's lines, watch the heartbeats, and send the
//! fast clock; every command is posted (the addon kit's level 6, `Events::post`) to the server
//! object, which runs it on the script's thread with the base station and its locomotives. A phone
//! drives the base's own locomotive at an address when the script has one, so the two never
//! disagree; changes made elsewhere (the script, another phone) reach every phone holding that
//! locomotive.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use leah_addon_kit::{Call, Error, Events, Object, Value};

use crate::base::{describe, make_locomotive, DccBase, Locomotive};
use crate::consist::Consist;
use crate::dcc;
use crate::roster::Roster;

/// The protocol's separators: between a list's items, between an item's parts, and in commands.
const ITEMS: &str = "]\\[";
const PARTS: &str = "}|{";
const SPLIT: &str = "<;>";

/// The usual port for these servers, which most phone apps try first.
pub const USUAL_PORT: u16 = 12090;

const PROPS: &[&str] = &["name", "port", "running", "clients", "heartbeat", "turnouts", "routes", "clock", "base", "roster"];
const FUNCS: &[&str] = &["add_turnout", "set_turnout", "remove_turnout", "add_route", "set_route", "remove_route", "set_clock", "stop_clock", "message", "close"];

/// What the server's threads and the script's thread share.
struct Shared {
    stop: AtomicBool,
    /// Each phone's lines waiting to be written, by the phone's number: a thread of the phone's own
    /// writes them, so a phone that stops reading never holds up the script.
    writers: Mutex<HashMap<u64, std::sync::mpsc::Sender<String>>>,
    /// Each phone's connection, to close it.
    streams: Mutex<HashMap<u64, TcpStream>>,
    /// Each phone's heartbeat: whether it asked for one, and when it was last heard from.
    watches: Mutex<HashMap<u64, Arc<Watch>>>,
    /// The heartbeat's seconds (0: none).
    heartbeat: AtomicU64,
    clock: Mutex<Option<Clock>>,
}

struct Watch {
    on: AtomicBool,
    heard: Mutex<Instant>,
    /// Said quiet already, until it is heard from again.
    quiet: AtomicBool,
}

/// The fast clock: its time of day (seconds) when set, how fast it runs, and when it was set.
#[derive(Clone, Copy)]
struct Clock {
    seconds: f64,
    rate: f64,
    set_at: Instant,
}

impl Clock {
    fn now(&self) -> f64 {
        (self.seconds + self.set_at.elapsed().as_secs_f64() * self.rate).rem_euclid(86400.0)
    }

    fn line(&self) -> String {
        format!("PFT{}{SPLIT}{}", self.now() as i64, rate_words(self.rate))
    }
}

fn rate_words(rate: f64) -> String {
    if rate.fract() == 0.0 { format!("{rate:.1}") } else { format!("{rate}") }
}

impl Shared {
    /// A line to one phone, queued for its writing thread.
    fn send(&self, id: u64, line: &str) {
        if logging() {
            eprintln!("phone {id} <  {line}");
        }
        if let Some(w) = self.writers.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
            let _ = w.send(format!("{line}\n"));
        }
    }

    /// A phone's connection closed: its reading thread ends, which tells the server.
    fn hang_up(&self, id: u64) {
        if let Some(s) = self.streams.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
            let _ = s.shutdown(Shutdown::Both);
        }
    }

    /// The heartbeat's seconds as the phones are told, and how long one may go quiet: a little more,
    /// so a phone saying it right on time over a slow Wi-Fi is never stopped.
    fn quiet_after(&self) -> Option<Duration> {
        let seconds = self.heartbeat.load(Ordering::SeqCst);
        (seconds > 0).then(|| Duration::from_secs(seconds) + Duration::from_secs((seconds / 2).max(2)))
    }

    fn send_all(&self, line: &str) {
        let ids: Vec<u64> = self.writers.lock().unwrap_or_else(|e| e.into_inner()).keys().copied().collect();
        for id in ids {
            self.send(id, line);
        }
    }
}

/// What a phone was last told of a locomotive it holds: only differences are sent.
#[derive(Clone)]
struct Told {
    speed: u8,
    forward: bool,
    functions: [bool; dcc::FUNCTIONS],
}

/// A locomotive a phone holds on one of its throttles, by the key the phone gave it ("L341").
struct Held {
    loco: Rc<Locomotive>,
    /// The phone's own word on which keys are momentary (`m`), over the roster's.
    momentary: BTreeMap<u8, bool>,
    told: Told,
}

/// A connected phone.
struct Client {
    name: String,
    device: String,
    address: String,
    /// Its throttles ('0' to '9', 'T', 'S', 'G'), each holding locomotives by their keys.
    throttles: BTreeMap<char, BTreeMap<String, Held>>,
}

struct Turnout {
    name: String,
    label: String,
    /// 1 unknown, 2 closed, 4 thrown (the protocol's).
    state: u8,
}

struct Route {
    name: String,
    label: String,
    /// 2 active, 4 inactive.
    state: u8,
}

pub struct PhoneServer {
    base: Rc<DccBase>,
    roster: Option<Rc<Roster>>,
    name: String,
    port: u16,
    shared: Arc<Shared>,
    events: Events,
    open: Cell<bool>,
    clients: RefCell<BTreeMap<u64, Client>>,
    turnouts: RefCell<Vec<Turnout>>,
    routes: RefCell<Vec<Route>>,
    /// The phone and throttle whose command is being run, with the speed or direction it asked for:
    /// not told those back when they came out as asked (a slider being dragged would jump), but told
    /// when they didn't (a speed limit), and always told its functions (a latching key needs to hear).
    origin: Cell<Option<Origin>>,
    /// The locomotives phones made (no locomotive of the script's at their addresses), kept while the
    /// server serves: one let go of runs on as it was, and is the same one when taken again.
    made: RefCell<BTreeMap<(u16, bool), Rc<Locomotive>>>,
    /// The advanced consists phones made, by their consist addresses (`RC` commands).
    consists: RefCell<BTreeMap<u8, Rc<Consist>>>,
    announcer: RefCell<Option<mdns_sd::ServiceDaemon>>,
}

#[derive(Clone, Copy, PartialEq)]
struct Origin {
    id: u64,
    throttle: char,
    speed: Option<u8>,
    forward: Option<bool>,
}

thread_local! {
    /// The servers serving: each kept until it is closed (or the program ends), held by the script or
    /// not, so one the script let go of never leaves phones driving with nothing to stop them.
    static SERVING: RefCell<Vec<Rc<PhoneServer>>> = const { RefCell::new(Vec::new()) };
}

/// `phone_throttle_server(port: 12090, name: "...", base: ..., roster: ..., heartbeat: 10, announce: true)`.
pub fn new_server(call: &mut Call) -> Result<Value, Error> {
    call.count_between(0, 0)?;
    call.only_options(&["port", "name", "base", "roster", "heartbeat", "announce"])?;
    let what = "phone_throttle_server()";
    // (Its threads hand the phones' commands over through the addon interface's level 6: a Leah
    // serving less would take phones' connections and never answer them.)
    if call.info("interface").as_int().unwrap_or(0) < 6 {
        return Err(Error::new("api_error", format!("{what} needs a newer Leah (one serving the addon interface's level 6: 0.1.0-alpha.50 or later): update Leah")));
    }
    let base = match call.option("base") {
        None => DccBase::last().ok_or_else(|| Error::new("hardware_error", format!("{what}: there is no base station: make one first, dcc_base(\"/dev/ttyACM0\") or dcc_base(simulate: true)")))?,
        Some(v) => call.own_option::<DccBase>("base").ok_or_else(|| Error::type_error(format!("{what}'s `base:` is a dcc_base, not {}", describe(v))).at(call.option_line("base")))?,
    };
    let roster = match call.option("roster") {
        None | Some(Value::Null) => None,
        Some(v) => Some(call.own_option::<Roster>("roster").ok_or_else(|| Error::type_error(format!("{what}'s `roster:` is a roster, not {}", describe(v))).at(call.option_line("roster")))?),
    };
    let port = match call.option("port") {
        None => USUAL_PORT,
        Some(v) => match v.as_int() {
            Some(n @ 0..=65535) => n as u16,
            _ => return Err(Error::range_error(format!("{what}'s `port:` is 1 to 65535 (12090, the usual one; 0 for any free one), not {}", describe(v))).at(call.option_line("port"))),
        },
    };
    let name = match call.option("name") {
        None => "Leah".to_string(),
        Some(v) => match v.as_text() {
            Some(t) if !t.trim().is_empty() && !t.contains(['\n', '\r']) => t.trim().to_string(),
            _ => return Err(Error::type_error(format!("{what}'s `name:` is the name phones show, text on one line, not {}", describe(v))).at(call.option_line("name"))),
        },
    };
    let heartbeat = match call.option("heartbeat") {
        None => 10,
        Some(v) => match v.as_int() {
            Some(n @ 0..=3600) => n as u64,
            _ => return Err(Error::range_error(format!("{what}'s `heartbeat:` is the seconds a phone may go quiet before its locomotives stop, 0 (never) to 3600, not {}", describe(v))).at(call.option_line("heartbeat"))),
        },
    };
    let announce = match call.option("announce") {
        None => true,
        Some(Value::Bool(b)) => *b,
        Some(other) => return Err(Error::type_error(format!("{what}'s `announce:` is true or false, not {}", describe(other))).at(call.option_line("announce"))),
    };
    // (Tests: any free port, never announced on the network the computer is on.)
    let (port, announce) = if std::env::var_os("LEAH_TEST_PHONE_SERVER").is_some() { (0, false) } else { (port, announce) };
    let listener = TcpListener::bind(("0.0.0.0", port)).map_err(|e| {
        let why = if e.kind() == std::io::ErrorKind::AddrInUse { format!("port {port} is in use (another server, perhaps this one started twice)") } else { e.to_string() };
        Error::new("network_error", format!("{what}: can't listen for phones: {why}"))
    })?;
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    listener.set_nonblocking(true).map_err(|e| Error::new("network_error", format!("{what}: {e}")))?;
    let shared = Arc::new(Shared {
        stop: AtomicBool::new(false),
        writers: Mutex::new(HashMap::new()),
        streams: Mutex::new(HashMap::new()),
        watches: Mutex::new(HashMap::new()),
        heartbeat: AtomicU64::new(heartbeat),
        clock: Mutex::new(None),
    });
    let announcer = if announce { announce_on_network(&name, port) } else { None };
    let value = call.new_object_with_events(|events| {
        events.keep_working(true);
        start_threads(listener, shared.clone(), events.clone());
        PhoneServer {
            base: base.clone(),
            roster,
            name,
            port,
            shared,
            events,
            open: Cell::new(true),
            clients: RefCell::new(BTreeMap::new()),
            turnouts: RefCell::new(Vec::new()),
            routes: RefCell::new(Vec::new()),
            origin: Cell::new(None),
            made: RefCell::new(BTreeMap::new()),
            consists: RefCell::new(BTreeMap::new()),
            announcer: RefCell::new(announcer),
        }
    });
    if let Some(made) = call.own_value::<PhoneServer>(&value) {
        base.phone_servers.borrow_mut().push(Rc::downgrade(&made));
        SERVING.with(|s| s.borrow_mut().push(made));
    }
    Ok(value)
}

/// The server announced on the local network (mDNS, `_withrottle._tcp`), so phones list it by its
/// name; None when that can't be done (the server works all the same: the phone is given the
/// computer's address and the port).
fn announce_on_network(name: &str, port: u16) -> Option<mdns_sd::ServiceDaemon> {
    let daemon = mdns_sd::ServiceDaemon::new().ok()?;
    let host = format!("{}.local.", host_name());
    let info = mdns_sd::ServiceInfo::new("_withrottle._tcp.local.", name, &host, "", port, None::<HashMap<String, String>>).ok()?.enable_addr_auto();
    daemon.register(info).ok()?;
    Some(daemon)
}

fn host_name() -> String {
    std::fs::read_to_string("/etc/hostname").ok().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).unwrap_or_else(|| "leah".to_string())
}

/// The listening thread (each phone's reading thread from it), the heartbeat's watch, and the clock.
fn start_threads(listener: TcpListener, shared: Arc<Shared>, events: Events) {
    let next = Arc::new(AtomicU64::new(1));
    {
        let shared = shared.clone();
        let events = events.clone();
        std::thread::spawn(move || {
            while !shared.stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, from)) => {
                        let id = next.fetch_add(1, Ordering::SeqCst);
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_nodelay(true);
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                        let (Ok(mut writer), Ok(closer)) = (stream.try_clone(), stream.try_clone()) else { continue };
                        let watch = Arc::new(Watch { on: AtomicBool::new(false), heard: Mutex::new(Instant::now()), quiet: AtomicBool::new(false) });
                        let (tx, rx) = std::sync::mpsc::channel::<String>();
                        shared.writers.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);
                        shared.streams.lock().unwrap_or_else(|e| e.into_inner()).insert(id, closer);
                        // (Its lines written by a thread of its own, in order; one it won't take ends
                        // its connection.)
                        std::thread::spawn(move || {
                            for line in rx {
                                if writer.write_all(line.as_bytes()).is_err() {
                                    let _ = writer.shutdown(Shutdown::Both);
                                    break;
                                }
                            }
                        });
                        shared.watches.lock().unwrap_or_else(|e| e.into_inner()).insert(id, watch.clone());
                        events.post("phone_joined", vec![Value::from(id as i64), Value::from(from.ip().to_string())]);
                        let (shared, events) = (shared.clone(), events.clone());
                        std::thread::spawn(move || read_phone(stream, id, watch, shared, events));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                    Err(_) => std::thread::sleep(Duration::from_millis(200)),
                }
            }
        });
    }
    // (The heartbeat and the clock, checked five times a second.)
    std::thread::spawn(move || {
        let mut last_minute: Option<i64> = None;
        while !shared.stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(200));
            if let Some(limit) = shared.quiet_after() {
                let watches: Vec<(u64, Arc<Watch>)> = shared.watches.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(i, w)| (*i, w.clone())).collect();
                for (id, w) in watches {
                    let quiet_for = w.heard.lock().unwrap_or_else(|e| e.into_inner()).elapsed();
                    if w.on.load(Ordering::SeqCst) && quiet_for > limit && !w.quiet.swap(true, Ordering::SeqCst) {
                        events.post("phone_quiet", vec![Value::from(id as i64)]);
                    }
                }
            }
            // (The fast clock said to every phone at each fast minute.)
            let clock = *shared.clock.lock().unwrap_or_else(|e| e.into_inner());
            match clock {
                Some(c) if c.rate > 0.0 => {
                    let minute = (c.now() / 60.0) as i64;
                    if last_minute != Some(minute) {
                        if last_minute.is_some() {
                            shared.send_all(&c.line());
                        }
                        last_minute = Some(minute);
                    }
                }
                _ => last_minute = None,
            }
        }
    });
}

/// A phone's lines, each posted to the server; its end posted too. Any of `\n`, `\r`, or `\r\n` ends
/// a line.
fn read_phone(mut stream: TcpStream, id: u64, watch: Arc<Watch>, shared: Arc<Shared>, events: Events) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let mut line: Vec<u8> = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                *watch.heard.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
                watch.quiet.store(false, Ordering::SeqCst);
                for &b in &buf[..n] {
                    if b == b'\n' || b == b'\r' {
                        if !line.is_empty() {
                            let text = String::from_utf8_lossy(&line).into_owned();
                            line.clear();
                            events.post("phone_line", vec![Value::from(id as i64), Value::from(text)]);
                        }
                    } else if line.len() < 4096 {
                        line.push(b);
                    }
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted) => {}
            Err(_) => break,
        }
    }
    shared.writers.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    shared.streams.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    shared.watches.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    if !shared.stop.load(Ordering::SeqCst) {
        events.post("phone_left", vec![Value::from(id as i64)]);
    }
}

/// The track power changed (the script, a phone, or the board): every phone told.
pub(crate) fn power_changed(base: &DccBase, on: bool) {
    for s in base.phone_servers.borrow().iter().filter_map(Weak::upgrade) {
        s.shared.send_all(&format!("PPA{}", u8::from(on)));
    }
}

/// Whether each phone's lines and the server's answers are said on the error output
/// (`LEAH_PHONE_LOG`: tools/bench.sh sets it, to see what a phone and the server say).
fn logging() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LEAH_PHONE_LOG").is_some_and(|v| !v.is_empty()))
}

/// A locomotive on this base station changed (its speed, direction, or a function): every phone
/// holding it is told what changed (base.rs's `changed`).
pub(crate) fn locomotive_changed(base: &DccBase, loco: &Rc<Locomotive>) {
    let servers: Vec<Rc<PhoneServer>> = base.phone_servers.borrow().iter().filter_map(Weak::upgrade).collect();
    for s in servers {
        s.tell_changed(loco);
    }
}

fn told_now(loco: &Locomotive) -> Told {
    let mut functions = [false; dcc::FUNCTIONS];
    for (n, f) in functions.iter_mut().enumerate() {
        *f = loco.function_on(n);
    }
    Told { speed: loco.speed_126(), forward: loco.is_forward(), functions }
}

/// Text a phone is sent inside a list: on one line, without the protocol's separators (a roster name
/// or a label holding one would split the list).
fn plain(text: &str) -> String {
    let mut t = text.replace(['\n', '\r'], " ");
    for s in [ITEMS, PARTS, SPLIT] {
        t = t.replace(s, " ");
    }
    t
}

/// A locomotive's key as the protocol writes it: "S3", "L341".
fn key_of(loco: &Locomotive) -> String {
    format!("{}{}", if loco.is_long() { 'L' } else { 'S' }, loco.address())
}

/// "S3" or "L341": the address and whether it is long, or None.
fn address_in(text: &str) -> Option<(u16, bool)> {
    let long = match text.chars().next()? {
        'S' | 's' => false,
        'L' | 'l' => true,
        _ => return None,
    };
    let n: u16 = text[1..].trim().parse().ok()?;
    let most = if long { 10239 } else { dcc::SHORT_MOST };
    (1..=most).contains(&n).then_some((n, long))
}

fn steps_word(steps: u8) -> u8 {
    match steps {
        28 => 2,
        14 => 8,
        _ => 1,
    }
}

impl PhoneServer {
    fn send(&self, id: u64, line: &str) {
        self.shared.send(id, line);
    }

    fn alert(&self, id: u64, words: &str) {
        self.send(id, &format!("HM{}", words.replace(['\n', '\r'], " ")));
    }

    fn roster_line(&self) -> String {
        let entries = self.roster.as_ref().map(|r| r.entries_now()).unwrap_or_default();
        let mut line = format!("RL{}", entries.len());
        for e in entries {
            line.push_str(&format!("{ITEMS}{}{PARTS}{}{PARTS}{}", plain(&e.name), e.address, if e.long { 'L' } else { 'S' }));
        }
        line
    }

    fn turnout_lines(&self) -> Vec<String> {
        let turnouts = self.turnouts.borrow();
        if turnouts.is_empty() {
            return Vec::new();
        }
        let mut list = "PTL".to_string();
        for t in turnouts.iter() {
            list.push_str(&format!("{ITEMS}{}{PARTS}{}{PARTS}{}", t.name, t.label, t.state));
        }
        vec![format!("PTT{ITEMS}Turnouts{PARTS}Turnout{ITEMS}Closed{PARTS}2{ITEMS}Thrown{PARTS}4{ITEMS}Unknown{PARTS}1{ITEMS}Inconsistent{PARTS}8"), list]
    }

    fn route_lines(&self) -> Vec<String> {
        let routes = self.routes.borrow();
        if routes.is_empty() {
            return Vec::new();
        }
        let mut list = "PRL".to_string();
        for r in routes.iter() {
            list.push_str(&format!("{ITEMS}{}{PARTS}{}{PARTS}{}", r.name, r.label, r.state));
        }
        vec![format!("PRT{ITEMS}Routes{PARTS}Route{ITEMS}Active{PARTS}2{ITEMS}Inactive{PARTS}4{ITEMS}Unknown{PARTS}0{ITEMS}Inconsistent{PARTS}8"), list]
    }

    fn power_line(&self, call: &mut Call) -> String {
        let on = self.base.get("power", call).ok().and_then(|v| v.as_bool()).unwrap_or(false);
        format!("PPA{}", u8::from(on))
    }

    /// A phone connected: what the server is told first (the protocol's version first).
    fn joined(&self, id: u64, address: String, call: &mut Call) {
        self.clients.borrow_mut().insert(id, Client { name: String::new(), device: String::new(), address, throttles: BTreeMap::new() });
        let mut lines = vec!["VN2.0".to_string(), self.roster_line(), self.power_line(call)];
        lines.extend(self.turnout_lines());
        lines.extend(self.route_lines());
        lines.extend(self.consist_lines());
        lines.push(format!("*{}", self.shared.heartbeat.load(Ordering::SeqCst)));
        lines.push("HTLeah".to_string());
        lines.push(format!("Ht{}", self.name));
        if let Some(c) = *self.shared.clock.lock().unwrap_or_else(|e| e.into_inner()) {
            lines.push(c.line());
        }
        for l in lines {
            self.send(id, &l);
        }
    }

    /// A phone gone (it closed, or its connection broke): its locomotives stopped, eased by their
    /// decoders, and the script told.
    fn left(&self, id: u64, call: &mut Call) {
        let Some(client) = self.clients.borrow_mut().remove(&id) else { return };
        for held in client.throttles.values().flat_map(|t| t.values()) {
            let _ = held.loco.halt(false, call);
        }
        self.events.fire("on_client_left", vec![Value::from(if client.name.is_empty() { client.address } else { client.name })]);
    }

    /// A phone quiet past the heartbeat: its locomotives stopped at once, and the phone told.
    fn quiet(&self, id: u64, call: &mut Call) {
        let held: Vec<Rc<Locomotive>> = self.clients.borrow().get(&id).map(|c| c.throttles.values().flat_map(|t| t.values().map(|h| h.loco.clone())).collect()).unwrap_or_default();
        for loco in &held {
            let _ = loco.halt(true, call);
        }
        if !held.is_empty() {
            self.alert(id, &format!("Leah: nothing from this phone for {} seconds, so its locomotives were stopped", self.shared.heartbeat.load(Ordering::SeqCst)));
        }
    }

    fn line(&self, id: u64, line: &str, call: &mut Call) {
        if logging() {
            eprintln!("phone {id}  > {line}");
        }
        if !self.clients.borrow().contains_key(&id) {
            return;
        }
        let line = line.trim_end();
        match line.as_bytes().first() {
            Some(b'N') => {
                let name = line[1..].trim().to_string();
                if let Some(c) = self.clients.borrow_mut().get_mut(&id) {
                    c.name = name.clone();
                }
                self.send(id, &format!("*{}", self.shared.heartbeat.load(Ordering::SeqCst)));
                self.events.fire("on_client", vec![Value::from(name)]);
            }
            Some(b'H') if line.starts_with("HU") => {
                if let Some(c) = self.clients.borrow_mut().get_mut(&id) {
                    c.device = line[2..].trim().to_string();
                }
            }
            Some(b'*') => {
                let on = match &line[1..] {
                    "+" => Some(true),
                    "-" => Some(false),
                    _ => None,
                };
                if let Some(on) = on
                    && let Some(w) = self.shared.watches.lock().unwrap_or_else(|e| e.into_inner()).get(&id)
                {
                    w.on.store(on, Ordering::SeqCst);
                }
            }
            Some(b'Q') => {
                // (The phone is going: its connection closed, which stops its locomotives.)
                self.shared.hang_up(id);
            }
            Some(b'P') => self.panel(id, line, call),
            Some(b'R') if line.starts_with("RC") => self.consist_command(id, line, call),
            Some(b'M') => self.throttle(id, line, call),
            // (Anything else, as the protocol asks, is left alone.)
            _ => {}
        }
    }

    /// Track power, turnouts, and routes.
    fn panel(&self, id: u64, line: &str, call: &mut Call) {
        if let Some(state) = line.strip_prefix("PPA") {
            let on = state.trim() == "1";
            if let Err(e) = self.base.set("power", Value::from(on), call) {
                self.alert(id, &format!("Leah: the track power wasn't switched: {}", e.message));
            }
            self.shared.send_all(&self.power_line(call));
        } else if let Some(rest) = line.strip_prefix("PTA") {
            let mut chars = rest.chars();
            let action = chars.next().unwrap_or(' ');
            let name = chars.as_str().trim().to_string();
            let known = self.turnouts.borrow().iter().position(|t| t.name == name);
            let Some(i) = known else {
                self.alert(id, &format!("Leah: there is no turnout {name}"));
                return;
            };
            let now = self.turnouts.borrow()[i].state;
            let state = match action {
                'C' => 2,
                'T' => 4,
                '2' => if now == 4 { 2 } else { 4 },
                _ => return,
            };
            self.set_turnout_state(i, state);
            self.events.fire("on_turnout_request", vec![Value::from(name), Value::from(if state == 2 { "closed" } else { "thrown" })]);
        } else if let Some(rest) = line.strip_prefix("PRA") {
            let name = rest.get(1..).unwrap_or("").trim().to_string();
            let known = self.routes.borrow().iter().position(|r| r.name == name);
            let Some(i) = known else {
                self.alert(id, &format!("Leah: there is no route {name}"));
                return;
            };
            self.set_route_state(i, 2);
            self.events.fire("on_route_request", vec![Value::from(name)]);
        }
    }

    fn set_turnout_state(&self, i: usize, state: u8) {
        let name = {
            let mut turnouts = self.turnouts.borrow_mut();
            turnouts[i].state = state;
            turnouts[i].name.clone()
        };
        self.shared.send_all(&format!("PTA{state}{name}"));
    }

    fn set_route_state(&self, i: usize, state: u8) {
        let name = {
            let mut routes = self.routes.borrow_mut();
            routes[i].state = state;
            routes[i].name.clone()
        };
        self.shared.send_all(&format!("PRA{state}{name}"));
    }

    /// A multi-throttle's command: `M<throttle><+|-|A|S><key><;><more>`.
    fn throttle(&self, id: u64, line: &str, call: &mut Call) {
        let mut chars = line.chars();
        chars.next();
        let Some(throttle) = chars.next() else { return };
        let Some(command) = chars.next() else { return };
        let rest = chars.as_str();
        let (key, more) = match rest.split_once(SPLIT) {
            Some((k, m)) => (k.trim().to_string(), m.to_string()),
            None => (rest.trim().to_string(), String::new()),
        };
        match command {
            '+' | 'S' => self.take(id, throttle, &key, &more, call),
            '-' => self.release(id, throttle, &key, call),
            'A' => self.action(id, throttle, &key, &more, call),
            _ => {}
        }
    }

    /// A locomotive taken onto a throttle: by its address ("S3", "L341") or a roster entry's name
    /// ("ERGS 41"). The base station's own at that address is used when there is one.
    fn take(&self, id: u64, throttle: char, key: &str, what: &str, call: &mut Call) {
        let what = what.trim();
        let (address, long, entry) = if let Some(name) = what.strip_prefix('E') {
            let found = self.roster.as_ref().and_then(|r| r.entry_named(name.trim(), call));
            match found.as_ref().and_then(|e| e.data().ok()) {
                Some(d) => (d.address, d.long, found),
                None => {
                    self.alert(id, &format!("Leah: there is no roster entry {}", name.trim()));
                    return;
                }
            }
        } else {
            match address_in(if what.is_empty() { key } else { what }) {
                Some((a, l)) => (a, l, None),
                None => {
                    self.alert(id, &format!("Leah: {} isn't a locomotive's address (S1 to S127, or L1 to L10239)", if what.is_empty() { key } else { what }));
                    return;
                }
            }
        };
        let Some(loco) = self.locomotive_at(address, long, entry, call) else { return };
        let key = if key.is_empty() { key_of(&loco) } else { key.to_string() };
        let told = told_now(&loco);
        if let Some(c) = self.clients.borrow_mut().get_mut(&id) {
            c.throttles.entry(throttle).or_default().insert(key.clone(), Held { loco: loco.clone(), momentary: BTreeMap::new(), told: told.clone() });
        }
        // (What the phone shows: the keys' labels, each function's state, the speed, the direction,
        // and the speed steps.)
        self.send(id, &format!("M{throttle}+{key}{SPLIT}"));
        let labels = loco.key_labels();
        if !labels.is_empty() {
            let all: Vec<String> = (0..dcc::FUNCTIONS as u8).map(|n| plain(labels.get(&n).map(String::as_str).unwrap_or(""))).collect();
            self.send(id, &format!("M{throttle}L{key}{SPLIT}{ITEMS}{}", all.join(ITEMS)));
        }
        for (n, on) in told.functions.iter().enumerate() {
            self.send(id, &format!("M{throttle}A{key}{SPLIT}F{}{n}", u8::from(*on)));
        }
        self.send(id, &format!("M{throttle}A{key}{SPLIT}V{}", told.speed));
        self.send(id, &format!("M{throttle}A{key}{SPLIT}R{}", u8::from(told.forward)));
        self.send(id, &format!("M{throttle}A{key}{SPLIT}s{}", steps_word(loco.steps())));
    }

    /// The locomotive at an address: the script's own when it has one, else the one a phone made
    /// before, else one made now (from its roster entry when there is one) and kept.
    fn locomotive_at(&self, address: u16, long: bool, entry: Option<Rc<crate::roster::RosterEntry>>, call: &mut Call) -> Option<Rc<Locomotive>> {
        let existing = self.base.live_locomotives().into_iter().find(|l| l.address() == address && l.is_long() == long).or_else(|| self.made.borrow().get(&(address, long)).cloned());
        if existing.is_some() {
            return existing;
        }
        // (As locomotive() makes one: from its roster entry when there is one.)
        let data = entry.as_ref().and_then(|e| e.data().ok()).or_else(|| self.roster.as_ref().and_then(|r| r.entries_now().into_iter().find(|e| e.address == address && e.long == long)));
        let made = crate::base::from_entry(data.as_ref(), None);
        let entry = entry.or_else(|| {
            let name = data.as_ref()?.name.clone();
            self.roster.as_ref()?.entry_named(&name, call)
        });
        let loco = make_locomotive(call, &self.base, address, long, made.steps, made.speed_limit, made.momentary, made.labels, entry).1?;
        self.made.borrow_mut().insert((address, long), loco.clone());
        Some(loco)
    }

    /// The consists phones made, as the protocol lists them: their count, then one line each, its
    /// address, its name, and each member's address and facing (`true` the consist's way).
    fn consist_lines(&self) -> Vec<String> {
        let consists = self.consists.borrow();
        let mut lines = vec![format!("RCC{}", consists.len())];
        for (address, c) in consists.iter() {
            let mut line = format!("RCD{PARTS}{address}(S){PARTS}{}", plain(c.name()));
            for (m, reversed) in c.members_now() {
                line.push_str(&format!("{ITEMS}{}({}){PARTS}{}", m.address(), if m.is_long() { 'L' } else { 'S' }, !reversed));
            }
            lines.push(line);
        }
        lines
    }

    fn tell_consists(&self) {
        for l in self.consist_lines() {
            self.shared.send_all(&l);
        }
    }

    /// The phones' advanced (decoder-assisted) consists: each member's decoder told the consist
    /// address (its CV 19), as Leah's `consist(address: ...)` does. `RC+` makes one or adds to it (or
    /// turns a member round), `RC-` takes one out, `RCR` breaks it up, `RCP` puts its members in
    /// order. A phone then drives it by taking the consist address as a locomotive.
    fn consist_command(&self, id: u64, line: &str, call: &mut Call) {
        let (head, rest) = line.split_once(SPLIT).unwrap_or((line, ""));
        let (left, right) = rest.split_once("<:>").unwrap_or((rest, ""));
        let (consist_part, name) = left.split_once(SPLIT).unwrap_or((left, ""));
        let Some((address, false)) = address_in(consist_part.trim()) else {
            self.alert(id, &format!("Leah: {} isn't a consist address (S1 to S127: a decoder keeps it in seven bits of its CV 19)", consist_part.trim()));
            return;
        };
        let address = address as u8;
        let what = "the phone's consist";
        let result = match head {
            "RC+" => {
                let (loco_part, facing) = right.split_once(SPLIT).unwrap_or((right, "true"));
                match address_in(loco_part.trim()) {
                    None => Err(Error::range_error(format!("{} isn't a locomotive's address", loco_part.trim()))),
                    Some((a, l)) => self.add_to_consist(address, name.trim(), a, l, facing.trim() != "true", what, call),
                }
            }
            "RC-" => match address_in(right.trim()) {
                None => Err(Error::range_error(format!("{} isn't a locomotive's address", right.trim()))),
                Some((a, l)) => {
                    let found = self.consists.borrow().get(&address).cloned();
                    match found {
                        None => Err(Error::range_error(format!("there is no consist {address}"))),
                        Some(c) => match c.members_now().into_iter().find(|(m, _)| m.address() == a && m.is_long() == l) {
                            None => Err(Error::range_error(format!("locomotive {a} isn't in consist {address}"))),
                            Some((m, _)) => c.remove_member(&m, what, call),
                        },
                    }
                }
            },
            "RCR" => {
                let gone = self.consists.borrow_mut().remove(&address);
                match gone {
                    None => Err(Error::range_error(format!("there is no consist {address}"))),
                    Some(c) => c.break_up_all(call),
                }
            }
            "RCP" => {
                let order: Vec<(u16, bool)> = right.split(SPLIT).filter_map(|p| address_in(p.trim())).collect();
                match self.consists.borrow().get(&address) {
                    None => Err(Error::range_error(format!("there is no consist {address}"))),
                    Some(c) => {
                        c.reorder(&order);
                        Ok(())
                    }
                }
            }
            _ => return,
        };
        if let Err(e) = result {
            self.alert(id, &format!("Leah: {}", e.message));
        }
        self.tell_consists();
    }

    #[allow(clippy::too_many_arguments)]
    fn add_to_consist(&self, address: u8, name: &str, loco_address: u16, long: bool, reversed: bool, what: &str, call: &mut Call) -> Result<(), Error> {
        let existing = self.consists.borrow().get(&address).cloned();
        let consist = match existing {
            Some(c) => c,
            None => {
                let name = if name.is_empty() { address.to_string() } else { name.to_string() };
                let made = Consist::make(&self.base, name, Some(address), call).1.ok_or_else(|| Error::new("programming_error", "the consist wasn't made"))?;
                self.consists.borrow_mut().insert(address, made.clone());
                made
            }
        };
        let loco = self.locomotive_at(loco_address, long, None, call).ok_or_else(|| Error::new("programming_error", "the locomotive wasn't made"))?;
        // (One in it already, facing the other way: taken out and added again, turned round.)
        if let Some((_, was)) = consist.members_now().into_iter().find(|(m, _)| Rc::ptr_eq(m, &loco)) {
            if was == reversed {
                return Ok(());
            }
            consist.remove_member(&loco, what, call)?;
        }
        consist.add_member(loco, reversed, what, call)
    }

    /// Locomotives let go of (one, or `*` all on the throttle): they run on as they were.
    fn release(&self, id: u64, throttle: char, key: &str, _call: &mut Call) {
        let gone: Vec<String> = {
            let mut clients = self.clients.borrow_mut();
            let Some(held) = clients.get_mut(&id).and_then(|c| c.throttles.get_mut(&throttle)) else { return };
            if key == "*" {
                std::mem::take(held).into_keys().collect()
            } else {
                held.remove(key).map(|_| vec![key.to_string()]).unwrap_or_default()
            }
        };
        for k in gone {
            self.send(id, &format!("M{throttle}-{k}{SPLIT}"));
        }
    }

    /// A throttle's action on one of its locomotives, or on all of them (`*`).
    fn action(&self, id: u64, throttle: char, key: &str, what: &str, call: &mut Call) {
        let targets: Vec<(String, Rc<Locomotive>)> = self.clients.borrow().get(&id).and_then(|c| c.throttles.get(&throttle)).map(|t| t.iter().filter(|(k, _)| key == "*" || k.as_str() == key).map(|(k, h)| (k.clone(), h.loco.clone())).collect()).unwrap_or_default();
        let mut chars = what.trim().chars();
        let Some(letter) = chars.next() else { return };
        let value = chars.as_str();
        if matches!(letter, 'r' | 'd') {
            self.release(id, throttle, key, call);
            return;
        }
        let asked_speed = (letter == 'V').then(|| value.trim().parse::<i64>().ok().map(|v| v.clamp(0, 126) as u8)).flatten();
        let asked_forward = (letter == 'R').then(|| value.trim() != "0");
        self.origin.set(Some(Origin { id, throttle, speed: asked_speed, forward: asked_forward }));
        for (k, loco) in targets {
            let result = match letter {
                'V' => match value.trim().parse::<i64>() {
                    Ok(v) => loco.drive(v.clamp(0, 126) as u8, loco.is_forward(), call),
                    Err(_) => Ok(()),
                },
                'R' => loco.drive(loco.speed_126(), value.trim() != "0", call),
                'X' => loco.halt(true, call),
                'I' => loco.halt(false, call),
                'F' | 'f' | 'm' => {
                    let (state, number) = value.split_at(value.len().min(1));
                    match number.trim().parse::<usize>() {
                        Ok(n) if n < dcc::FUNCTIONS => self.function(id, throttle, &k, &loco, letter, state == "1", n, call),
                        _ => Ok(()),
                    }
                }
                'q' => {
                    let told = told_now(&loco);
                    match value.trim() {
                        "V" => self.send(id, &format!("M{throttle}A{k}{SPLIT}V{}", told.speed)),
                        "R" => self.send(id, &format!("M{throttle}A{k}{SPLIT}R{}", u8::from(told.forward))),
                        _ => {}
                    }
                    Ok(())
                }
                's' => {
                    // (Its speed steps are its decoder's, from its roster entry: said back as they are.)
                    self.send(id, &format!("M{throttle}A{k}{SPLIT}s{}", steps_word(loco.steps())));
                    Ok(())
                }
                _ => Ok(()),
            };
            if let Err(e) = result {
                self.alert(id, &format!("Leah: {}", e.message));
            }
        }
        self.origin.set(None);
    }

    /// A function key: pressed or let go of (`F`), set (`f`), or made momentary or latching (`m`).
    #[allow(clippy::too_many_arguments)]
    fn function(&self, id: u64, throttle: char, key: &str, loco: &Rc<Locomotive>, letter: char, on: bool, n: usize, call: &mut Call) -> Result<(), Error> {
        if letter == 'm' {
            if let Some(h) = self.clients.borrow_mut().get_mut(&id).and_then(|c| c.throttles.get_mut(&throttle)).and_then(|t| t.get_mut(key)) {
                h.momentary.insert(n as u8, on);
            }
            return Ok(());
        }
        if letter == 'f' {
            return loco.set_function_now(n, on, call);
        }
        let momentary = self.clients.borrow().get(&id).and_then(|c| c.throttles.get(&throttle)).and_then(|t| t.get(key)).and_then(|h| h.momentary.get(&(n as u8)).copied()).unwrap_or_else(|| loco.is_momentary(n));
        let now = loco.function_on(n);
        match (on, momentary) {
            (true, true) => loco.set_function_now(n, true, call),
            (true, false) => loco.set_function_now(n, !now, call),
            (false, true) => loco.set_function_now(n, false, call),
            (false, false) => Ok(()),
        }
    }

    /// What changed in a locomotive, told to each phone holding it (only what differs from what it
    /// was told last; the phone whose command it was isn't told its own speed and direction back).
    fn tell_changed(&self, loco: &Rc<Locomotive>) {
        let now = told_now(loco);
        let origin = self.origin.get();
        let mut lines: Vec<(u64, String)> = Vec::new();
        for (id, client) in self.clients.borrow_mut().iter_mut() {
            for (throttle, held) in client.throttles.iter_mut() {
                for (key, h) in held.iter_mut() {
                    if !Rc::ptr_eq(&h.loco, loco) {
                        continue;
                    }
                    // (Its own phone and throttle: not told back what came out as it asked.)
                    let own = origin.filter(|o| o.id == *id && o.throttle == *throttle);
                    if h.told.speed != now.speed && own.is_none_or(|o| o.speed != Some(now.speed)) {
                        lines.push((*id, format!("M{throttle}A{key}{SPLIT}V{}", now.speed)));
                    }
                    if h.told.forward != now.forward && own.is_none_or(|o| o.forward != Some(now.forward)) {
                        lines.push((*id, format!("M{throttle}A{key}{SPLIT}R{}", u8::from(now.forward))));
                    }
                    for n in 0..dcc::FUNCTIONS {
                        if h.told.functions[n] != now.functions[n] {
                            lines.push((*id, format!("M{throttle}A{key}{SPLIT}F{}{n}", u8::from(now.functions[n]))));
                        }
                    }
                    h.told = now.clone();
                }
            }
        }
        for (id, line) in lines {
            self.send(id, &line);
        }
    }

    fn close_now(&self) {
        if !self.open.replace(false) {
            return;
        }
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.writers.lock().unwrap_or_else(|e| e.into_inner()).clear();
        for (_, s) in self.shared.streams.lock().unwrap_or_else(|e| e.into_inner()).drain() {
            let _ = s.shutdown(Shutdown::Both);
        }
        if let Some(d) = self.announcer.borrow_mut().take() {
            let _ = d.shutdown();
        }
        self.events.keep_working(false);
    }

    fn turnout_index(&self, name: &str) -> Option<usize> {
        self.turnouts.borrow().iter().position(|t| t.name == name)
    }

    fn route_index(&self, name: &str) -> Option<usize> {
        self.routes.borrow().iter().position(|r| r.name == name)
    }
}

impl Drop for PhoneServer {
    fn drop(&mut self) {
        self.close_now();
    }
}

/// A name a phone is given for a turnout or route: text on one line, without the protocol's
/// separators.
fn plain_name(call: &Call, i: usize, what: &str) -> Result<String, Error> {
    let t = call.text(i)?.trim().to_string();
    if t.is_empty() || t.contains(['\n', '\r']) || [ITEMS, PARTS, SPLIT].iter().any(|s| t.contains(s)) {
        return Err(Error::type_error(format!("{what}: a name is text on one line, not {}", describe(call.arg(i)))));
    }
    Ok(t)
}

fn label_option(call: &Call, what: &str, name: &str) -> Result<String, Error> {
    match call.option("label") {
        None => Ok(name.to_string()),
        Some(v) => match v.as_text() {
            Some(t) if !t.contains(['\n', '\r']) && ![ITEMS, PARTS, SPLIT].iter().any(|s| t.contains(s)) => Ok(t.to_string()),
            _ => Err(Error::type_error(format!("{what}'s `label:` is text on one line, not {}", describe(v))).at(call.option_line("label"))),
        },
    }
}

fn turnout_state(v: &Value, what: &str) -> Result<u8, Error> {
    match v.as_text() {
        Some("closed") => Ok(2),
        Some("thrown") => Ok(4),
        Some("unknown") => Ok(1),
        _ => Err(Error::type_error(format!("{what}: a turnout's state is \"closed\", \"thrown\", or \"unknown\", not {}", describe(v)))),
    }
}

fn turnout_word(state: u8) -> &'static str {
    match state {
        2 => "closed",
        4 => "thrown",
        _ => "unknown",
    }
}

/// A time of day as "14:05" or "14:05:30", in seconds.
fn time_of_day(v: &Value, what: &str) -> Result<f64, Error> {
    let bad = || Error::type_error(format!("{what}: the clock's time is a time of day like \"14:05\" or \"14:05:30\", not {}", describe(v)));
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

impl Object for PhoneServer {
    fn kind(&self) -> &str {
        "phone_throttle_server"
    }

    fn get(&self, property: &str, call: &mut Call) -> Result<Value, Error> {
        Ok(match property {
            "name" => Value::from(self.name.as_str()),
            "port" => Value::from(i64::from(self.port)),
            "running" => Value::from(self.open.get()),
            "heartbeat" => Value::from(self.shared.heartbeat.load(Ordering::SeqCst) as i64),
            "clients" => Value::list(self.clients.borrow().values().map(|c| {
                Value::map([
                    ("name", Value::from(c.name.as_str())),
                    ("device", Value::from(c.device.as_str())),
                    ("address", Value::from(c.address.as_str())),
                    ("locomotives", Value::from(c.throttles.values().map(|t| t.len()).sum::<usize>() as i64)),
                ])
            })),
            "turnouts" => Value::list(self.turnouts.borrow().iter().map(|t| Value::map([("name", Value::from(t.name.as_str())), ("label", Value::from(t.label.as_str())), ("state", Value::from(turnout_word(t.state)))]))),
            "routes" => Value::list(self.routes.borrow().iter().map(|r| Value::map([("name", Value::from(r.name.as_str())), ("label", Value::from(r.label.as_str())), ("state", Value::from(if r.state == 2 { "active" } else { "inactive" }))]))),
            "clock" => match *self.shared.clock.lock().unwrap_or_else(|e| e.into_inner()) {
                Some(c) => {
                    let s = c.now() as i64;
                    Value::map([("time", Value::from(format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60))), ("rate", Value::from(c.rate))])
                }
                None => Value::Null,
            },
            "base" => call.object_value(&self.base),
            "roster" => match &self.roster {
                Some(r) => call.object_value(r),
                None => Value::Null,
            },
            other => return Err(call.unknown_property("phone_throttle_server", other, PROPS)),
        })
    }

    fn set(&self, property: &str, value: Value, call: &mut Call) -> Result<(), Error> {
        match property {
            "heartbeat" => {
                match value.as_int() {
                    Some(n @ 0..=3600) => self.shared.heartbeat.store(n as u64, Ordering::SeqCst),
                    _ => return Err(Error::range_error(format!("a phone_throttle_server's `heartbeat` is 0 (never) to 3600 seconds, not {}", describe(&value)))),
                }
                // (The phones connected told, so they say it as often as now asked.)
                self.shared.send_all(&format!("*{}", self.shared.heartbeat.load(Ordering::SeqCst)));
                Ok(())
            }
            other if PROPS.contains(&other) => Err(Error::type_error(format!("a phone_throttle_server's `{other}` can be read but not set"))),
            other => Err(call.unknown_property("phone_throttle_server", other, PROPS)),
        }
    }

    fn call(&self, function: &str, call: &mut Call) -> Result<Value, Error> {
        let what = format!("phone_throttle_server.{function}()");
        match function {
            // (The server's own threads post these, level 6: not listed for scripts.)
            "phone_joined" => {
                let (id, address) = (call.int(0)? as u64, call.text(1)?.to_string());
                self.joined(id, address, call);
                Ok(Value::Null)
            }
            "phone_line" => {
                let (id, line) = (call.int(0)? as u64, call.text(1)?.to_string());
                self.line(id, &line, call);
                Ok(Value::Null)
            }
            "phone_left" => {
                let id = call.int(0)? as u64;
                self.left(id, call);
                Ok(Value::Null)
            }
            "phone_quiet" => {
                let id = call.int(0)? as u64;
                self.quiet(id, call);
                Ok(Value::Null)
            }
            "add_turnout" => {
                call.count_between(1, 1)?;
                call.only_options(&["label", "state"])?;
                let name = plain_name(call, 0, &what)?;
                if self.turnout_index(&name).is_some() {
                    return Err(Error::range_error(format!("{what}: there is a turnout {name} already")));
                }
                let label = label_option(call, &what, &name)?;
                let state = match call.option("state") {
                    None => 2,
                    Some(v) => turnout_state(v, &what).map_err(|e| e.at(call.option_line("state")))?,
                };
                self.turnouts.borrow_mut().push(Turnout { name, label, state });
                // (Phones list turnouts as they connect: those connected are told the list again.)
                for l in self.turnout_lines() {
                    self.shared.send_all(&l);
                }
                Ok(Value::Null)
            }
            "set_turnout" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let name = call.text(0)?.trim().to_string();
                let i = self.turnout_index(&name).ok_or_else(|| Error::range_error(format!("{what}: there is no turnout {name}")))?;
                let state = turnout_state(call.arg(1), &what)?;
                self.set_turnout_state(i, state);
                Ok(Value::Null)
            }
            "remove_turnout" | "remove_route" => {
                call.count_between(1, 1)?;
                call.only_options(&[])?;
                let name = call.text(0)?.trim().to_string();
                let turnout = function == "remove_turnout";
                let i = if turnout { self.turnout_index(&name) } else { self.route_index(&name) };
                let i = i.ok_or_else(|| Error::range_error(format!("{what}: there is no {} {name}", if turnout { "turnout" } else { "route" })))?;
                if turnout {
                    self.turnouts.borrow_mut().remove(i);
                    let lines = self.turnout_lines();
                    for l in if lines.is_empty() { vec!["PTL".to_string()] } else { lines } {
                        self.shared.send_all(&l);
                    }
                } else {
                    self.routes.borrow_mut().remove(i);
                    let lines = self.route_lines();
                    for l in if lines.is_empty() { vec!["PRL".to_string()] } else { lines } {
                        self.shared.send_all(&l);
                    }
                }
                Ok(Value::Null)
            }
            "add_route" => {
                call.count_between(1, 1)?;
                call.only_options(&["label"])?;
                let name = plain_name(call, 0, &what)?;
                if self.route_index(&name).is_some() {
                    return Err(Error::range_error(format!("{what}: there is a route {name} already")));
                }
                let label = label_option(call, &what, &name)?;
                self.routes.borrow_mut().push(Route { name, label, state: 4 });
                for l in self.route_lines() {
                    self.shared.send_all(&l);
                }
                Ok(Value::Null)
            }
            "set_route" => {
                call.count_between(2, 2)?;
                call.only_options(&[])?;
                let name = call.text(0)?.trim().to_string();
                let i = self.route_index(&name).ok_or_else(|| Error::range_error(format!("{what}: there is no route {name}")))?;
                let state = match call.arg(1).as_text() {
                    Some("active") => 2,
                    Some("inactive") => 4,
                    _ => return Err(Error::type_error(format!("{what}: a route's state is \"active\" or \"inactive\", not {}", describe(call.arg(1))))),
                };
                self.set_route_state(i, state);
                Ok(Value::Null)
            }
            "set_clock" => {
                call.count_between(1, 1)?;
                call.only_options(&["rate"])?;
                let seconds = time_of_day(call.arg(0), &what)?;
                let rate = match call.option("rate") {
                    None => 1.0,
                    Some(v) => match v.as_number() {
                        Some(r) if (0.1..=60.0).contains(&r) => r,
                        _ => return Err(Error::range_error(format!("{what}'s `rate:` is how much faster the clock runs, 0.1 to 60 (4: four minutes to each real one), not {}", describe(v))).at(call.option_line("rate"))),
                    },
                };
                let clock = Clock { seconds, rate, set_at: Instant::now() };
                *self.shared.clock.lock().unwrap_or_else(|e| e.into_inner()) = Some(clock);
                self.shared.send_all(&clock.line());
                Ok(Value::Null)
            }
            "stop_clock" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                let stopped = self.shared.clock.lock().unwrap_or_else(|e| e.into_inner()).as_mut().map(|c| {
                    *c = Clock { seconds: c.now(), rate: 0.0, set_at: Instant::now() };
                    *c
                });
                if let Some(c) = stopped {
                    self.shared.send_all(&c.line());
                }
                Ok(Value::Null)
            }
            "message" => {
                call.count_between(1, 1)?;
                call.only_options(&["alert"])?;
                let text = call.text(0)?.replace(['\n', '\r'], " ");
                let alert = matches!(call.option("alert"), Some(Value::Bool(true)));
                self.shared.send_all(&format!("{}{text}", if alert { "HM" } else { "Hm" }));
                Ok(Value::Null)
            }
            "close" => {
                call.count_between(0, 0)?;
                call.only_options(&[])?;
                // (Phones connected are let go of: their locomotives stopped, eased.)
                let ids: Vec<u64> = self.clients.borrow().keys().copied().collect();
                for id in ids {
                    self.left(id, call);
                }
                self.close_now();
                self.made.borrow_mut().clear();
                // (No longer kept serving: the script's hold, if any, is the last.)
                SERVING.with(|s| s.borrow_mut().retain(|k| !std::ptr::eq(Rc::as_ptr(k), self)));
                Ok(Value::Null)
            }
            other => Err(call.unknown_function("phone_throttle_server", other, FUNCS)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_read_as_the_protocol_writes_them() {
        assert_eq!(address_in("S3"), Some((3, false)));
        assert_eq!(address_in("L341"), Some((341, true)));
        assert_eq!(address_in("S200"), None);
        assert_eq!(address_in("L10240"), None);
        assert_eq!(address_in("X3"), None);
        assert_eq!(address_in("S0"), None);
    }

    #[test]
    fn the_clock_runs_at_its_rate_round_the_day() {
        let c = Clock { seconds: 86390.0, rate: 4.0, set_at: Instant::now() - Duration::from_secs(5) };
        assert!((c.now() - 10.0).abs() < 1.0, "{}", c.now());
        assert_eq!(rate_words(4.0), "4.0");
        assert_eq!(rate_words(1.5), "1.5");
    }
}
