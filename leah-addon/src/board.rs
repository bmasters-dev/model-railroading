//! Leah's base station board and its reader (design/model_railroading.md, "Stage 1b"): the boards'
//! USB serial ports, the link to them (a thread of its own reading both and telling the base station
//! the PC is still there), the ports plugged in (`dcc_ports()`), and loading the boards' programs
//! (`load_dcc_program()`) through `arduino-cli`.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The base station's program and the reader's, as they are loaded onto the boards.
pub const BASE_PROGRAM: &str = include_str!("../boards/dcc_base/dcc_base.ino");
pub const READER_PROGRAM: &str = include_str!("../boards/dcc_reader/dcc_reader.ino");

/// What each program says as it starts.
pub const BASE_HELLO: &str = "LEAH DCC BASE 6";
pub const READER_HELLO: &str = "LEAH DCC READER 1";

/// How a port couldn't be opened, in words a person can act on.
pub struct OpenError {
    pub message: String,
}

/// Opens a board's serial port at 115200 baud, raw. (Opening it starts the board's program again, as
/// an Arduino's USB port does.)
pub fn open(port: &str) -> Result<File, OpenError> {
    if !port.starts_with('/') {
        return Err(OpenError { message: format!("{port:?} isn't a port: give its whole path, like \"/dev/ttyACM0\" (dcc_ports() lists them)") });
    }
    let file = std::fs::OpenOptions::new().read(true).write(true).custom_flags_noctty().open(port).map_err(|e| words(port, &e))?;
    let fd = file.as_raw_fd();
    // (Only this program may open the port while it has it: two programs talking to one board at once
    // would mix their commands. A port another program has made its own the same way says "busy".)
    // SAFETY: an ioctl without arguments on the descriptor just opened.
    unsafe { libc::ioctl(fd, libc::TIOCEXCL) };
    // SAFETY: a termios read and write on the descriptor just opened.
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) != 0 {
            return Err(OpenError { message: format!("{port} isn't a serial port") });
        }
        libc::cfmakeraw(&mut t);
        t.c_cflag |= libc::CLOCAL | libc::CREAD;
        t.c_cc[libc::VMIN] = 0;
        t.c_cc[libc::VTIME] = 0;
        libc::cfsetispeed(&mut t, libc::B115200);
        libc::cfsetospeed(&mut t, libc::B115200);
        libc::tcsetattr(fd, libc::TCSANOW, &t);
        libc::tcflush(fd, libc::TCIOFLUSH);
    }
    Ok(file)
}

trait NoCtty {
    fn custom_flags_noctty(&mut self) -> &mut Self;
}

impl NoCtty for std::fs::OpenOptions {
    fn custom_flags_noctty(&mut self) -> &mut Self {
        use std::os::unix::fs::OpenOptionsExt;
        self.custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
    }
}

/// Why a port couldn't be opened, as the person at the screen can act on it.
fn words(port: &str, e: &std::io::Error) -> OpenError {
    match e.kind() {
        std::io::ErrorKind::NotFound => OpenError { message: format!("there is no board on {port}: is it plugged in? (dcc_ports() lists the ports there are)") },
        std::io::ErrorKind::PermissionDenied => OpenError {
            message: format!("you may not open {port} yet: on Linux, apps may use a board's USB port only once your account is in the `dialout` group (`sudo usermod -aG dialout $USER`, then log out and back in once)"),
        },
        _ if e.raw_os_error() == Some(libc::EBUSY) => OpenError { message: format!("{port} is in use by another program (a serial monitor, or a program being loaded onto the board)") },
        _ => OpenError { message: format!("can't open {port}: {e}") },
    }
}

/// The reader's measurements over its last second.
#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    pub one: Option<(f64, f64)>,
    pub zero: Option<(f64, f64)>,
    pub packets: i64,
    pub outside: i64,
    pub errors: i64,
}

/// With `LEAH_DCC_TRACE` set, every line to and from the boards on standard error (for working on the
/// boards' programs).
fn trace(line: &str) {
    if std::env::var_os("LEAH_DCC_TRACE").is_some() {
        eprintln!("{line}");
    }
}

/// Reads a reader's "T one 57.5-58.0 zero 99.5-100.0 packets 172 bad 0 errors 0" line.
pub fn timing_of(line: &str) -> Option<Timing> {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.first() != Some(&"T") || words.len() < 11 {
        return None;
    }
    let range = |w: &str| -> Option<(f64, f64)> {
        let (a, b) = w.split_once('-')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    };
    let number = |key: &str| -> Option<i64> { words.iter().position(|w| *w == key).and_then(|i| words.get(i + 1)).and_then(|w| w.parse().ok()) };
    let after = |key: &str| words.iter().position(|w| *w == key).and_then(|i| words.get(i + 1)).copied();
    Some(Timing { one: after("one").and_then(range), zero: after("zero").and_then(range), packets: number("packets")?, outside: number("bad")?, errors: number("errors")? })
}

/// Reads a "P 03 3F A9 95" line's bytes.
pub fn packet_of(line: &str) -> Option<Vec<u8>> {
    let rest = line.strip_prefix("P ")?;
    rest.split_whitespace().map(|b| u8::from_str_radix(b, 16).ok()).collect()
}

/// What the link's thread has heard, for the script's thread to take.
#[derive(Default)]
pub struct Heard {
    /// The base station's answers that weren't "OK" (an "ERR ...", "OFF ...").
    pub base_said: VecDeque<String>,
    /// The packets the reader heard, oldest first, not yet taken.
    pub packets: VecDeque<Vec<u8>>,
    pub timing: Option<Timing>,
    /// From the base station: whether the motor shield is on it, its current limit (mA), and the
    /// current it last said (mA, said every half second while the power is on).
    pub shield: Option<bool>,
    pub limit: Option<i64>,
    pub current: Option<i64>,
    /// The base station's answers to reading or writing a setting ("CV 1 3", "CV 1 none", "CV 1 off
    /// ...", "W 3 10 ok", "W 3 10 unconfirmed (...)"), oldest first.
    pub cv_answers: VecDeque<String>,
    /// The first line each board said as it started (or "STATE ..." / "T ..." when it said nothing
    /// as it started).
    pub base_hello: Option<String>,
    pub reader_hello: Option<String>,
    /// How many times the base station said its hello, and answered "?" with "STATE ...".
    pub base_hellos: u32,
    pub states: u32,
    /// The base station's port went away (its cable pulled): the link is over.
    pub gone: Option<String>,
    /// The reader's port went away: the base station goes on, without its reading back.
    pub reader_gone: Option<String>,
}

/// The link to the base station (and its reader): a thread reading both ports, saying "." to the
/// base station every second (its safety: three seconds without a word turn the track power off),
/// and handing what it heard over in `heard`.
pub struct Link {
    pub port: String,
    pub reader_port: Option<String>,
    base: Arc<Mutex<File>>,
    pub heard: Arc<Mutex<Heard>>,
    stop: Arc<AtomicBool>,
    /// Whether a reading on the programming track is going on (one at a time).
    busy: Arc<AtomicBool>,
}

/// What reading a decoder's settings needs from the link, for the script's thread or one of its own:
/// the way to the base station, what it heard, and whether a reading is going on.
#[derive(Clone)]
pub struct Reading {
    base: Arc<Mutex<File>>,
    heard: Arc<Mutex<Heard>>,
    busy: Arc<AtomicBool>,
}

/// How long the base station may take over one setting (it needs about 3 seconds).
const READ_LIMIT: Duration = Duration::from_secs(15);

impl Reading {
    /// Takes the programming track for one reading after another; None when a reading is going on.
    pub fn start(&self) -> Option<ReadingTurn> {
        (!self.busy.swap(true, Ordering::SeqCst)).then(|| ReadingTurn(self.clone()))
    }
}

/// The programming track taken for readings: given back when dropped.
pub struct ReadingTurn(Reading);

impl Drop for ReadingTurn {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}

impl ReadingTurn {
    /// Reads a setting (CV 1 to 1024): its value, or what went wrong in words. One no decoder answered
    /// is tried again, up to 3 times all told (a wheel losing contact for the 6 ms of an answer is
    /// enough to miss it, as other command stations find too).
    pub fn read_cv(&self, cv: i64) -> Result<i64, String> {
        self.read_cv_tries(cv, 3)
    }

    /// As `read_cv`, tried up to `tries` times all told.
    pub fn read_cv_tries(&self, cv: i64, tries: u32) -> Result<i64, String> {
        let mut tried = 0;
        loop {
            tried += 1;
            match self.read_cv_once(cv) {
                Err(e) if tried < tries && e.starts_with("no decoder answered") => continue,
                done => return done,
            }
        }
    }

    /// Selects an indexed page first (CV 31 and 32, as sound decoders keep their higher settings), when
    /// one is given.
    pub fn select_index(&self, index: Option<(i64, i64)>) -> Result<(), String> {
        if let Some((high, low)) = index {
            self.write_cv(31, high)?;
            self.write_cv(32, low)?;
        }
        Ok(())
    }

    /// Whether a setting holds a value (one question: much faster than reading it). Nothing answering
    /// means another value, or no decoder: the two can't be told apart.
    pub fn verify_cv(&self, cv: i64, value: i64) -> Result<bool, String> {
        let answer = self.ask(&format!("V {cv} {value}"))?;
        if let Some((_, why)) = answer.split_once(" off ") {
            return Err(why.to_string());
        }
        Ok(answer.ends_with(" yes"))
    }

    /// Reads one bit of a setting (0 or 1), tried up to `tries` times when nothing answered.
    pub fn read_cv_bit(&self, cv: i64, bit: i64, tries: u32) -> Result<i64, String> {
        let mut tried = 0;
        loop {
            tried += 1;
            let answer = self.ask(&format!("U {cv} {bit}"))?;
            if let Some((_, why)) = answer.split_once(" off ") {
                return Err(why.to_string());
            }
            match answer.rsplit(' ').next() {
                Some("1") => return Ok(1),
                Some("0") => return Ok(0),
                _ if tried < tries => continue,
                _ => return Err("no decoder answered on the programming track (the shield's B screws): is a locomotive on it, with clean wheels? A decoder answers by pulsing its motor, so it needs one".to_string()),
            }
        }
    }

    /// Writes one bit of a setting and has it confirmed; one not confirmed is written once more.
    pub fn write_cv_bit(&self, cv: i64, bit: i64, on: bool) -> Result<(), String> {
        for tried in 1..=2 {
            let answer = self.ask(&format!("Y {cv} {bit} {}", u8::from(on)))?;
            if let Some((_, why)) = answer.split_once(" off ") {
                return Err(why.to_string());
            }
            if answer.ends_with(" ok") {
                return Ok(());
            }
            if tried == 2 {
                return Err("the decoder didn't confirm the new value: it may not have taken it (is the locomotive on the programming track, with clean wheels?)".to_string());
            }
        }
        unreachable!()
    }

    fn read_cv_once(&self, cv: i64) -> Result<i64, String> {
        let answer = self.ask(&format!("R {cv}"))?;
        let rest = answer.trim_start_matches("CV ").split_once(' ').map(|(_, r)| r.trim().to_string()).unwrap_or_default();
        if let Some(seen) = rest.strip_prefix("none") {
            let seen = seen.trim();
            let seen = if seen.is_empty() { String::new() } else { format!(" {seen}") };
            return Err(format!("no decoder answered on the programming track (the shield's B screws){seen}: is a locomotive on it, with clean wheels? A decoder answers by pulsing its motor, so it needs one"));
        }
        if let Some(why) = rest.strip_prefix("off ") {
            return Err(why.to_string());
        }
        rest.parse::<i64>().map_err(|_| format!("the base station answered {answer:?}"))
    }

    /// Writes a setting and has it confirmed (the base station reads it back): Ok, or what went wrong.
    /// One the decoder didn't confirm is written once more.
    pub fn write_cv(&self, cv: i64, value: i64) -> Result<(), String> {
        let mut tries = 0;
        loop {
            tries += 1;
            let answer = self.ask(&format!("W {cv} {value}"))?;
            let rest = answer.splitn(4, ' ').nth(3).unwrap_or("").trim().to_string();
            if rest == "ok" {
                return Ok(());
            }
            if let Some(why) = answer.split_once(" off ").map(|(_, w)| w.to_string()) {
                return Err(why);
            }
            if tries == 2 {
                let seen = rest.strip_prefix("unconfirmed").unwrap_or("").trim();
                let seen = if seen.is_empty() { String::new() } else { format!(" {seen}") };
                return Err(format!("the decoder didn't confirm the new value{seen}: it may not have taken it (is the locomotive on the programming track, with clean wheels?)"));
            }
        }
    }

    /// Sends the base station a reading's or writing's line and waits for its answer.
    fn ask(&self, line: &str) -> Result<String, String> {
        let r = &self.0;
        r.heard.lock().unwrap_or_else(|p| p.into_inner()).cv_answers.clear();
        {
            let mut base = r.base.lock().unwrap_or_else(|p| p.into_inner());
            trace(&format!("-> {line}"));
            write_all(&mut base, format!("{line}\n").as_bytes()).map_err(|e| format!("can't talk to the base station: {e}"))?;
        }
        let start = Instant::now();
        loop {
            let mut h = r.heard.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(a) = h.cv_answers.pop_front() {
                return Ok(a);
            }
            if let Some(gone) = &h.gone {
                return Err(gone.clone());
            }
            drop(h);
            if start.elapsed() > READ_LIMIT {
                return Err(format!("the base station didn't answer within {} seconds", READ_LIMIT.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Gives a decoder an address: a long one (128 to 10239, or any with `long`) in CV 17 and 18 with
    /// CV 29's bit 5 set, a short one in CV 1 with it cleared (CV 29 read first, so its other bits
    /// stay as they were).
    pub fn write_address(&self, address: i64, long: bool) -> Result<(), String> {
        let config = self.read_cv(29)?;
        if long {
            self.write_cv(17, 0xC0 | (address >> 8))?;
            self.write_cv(18, address & 0xFF)?;
            if config & 0x20 == 0 {
                self.write_cv(29, config | 0x20)?;
            }
        } else {
            self.write_cv(1, address)?;
            if config & 0x20 != 0 {
                self.write_cv(29, config & !0x20)?;
            }
        }
        Ok(())
    }

    /// Reads a decoder's address: CV 29's bit 5 says which one it answers to, CV 17 and 18 holding
    /// the long one, CV 1 the short one. (address, long).
    pub fn read_address(&self) -> Result<(i64, bool), String> {
        let config = self.read_cv(29)?;
        if config & 0x20 != 0 {
            let high = self.read_cv(17)?;
            let low = self.read_cv(18)?;
            Ok((((high & 0x3F) << 8) | low, true))
        } else {
            Ok((self.read_cv(1)? & 0x7F, false))
        }
    }
}

impl Link {
    pub fn open(port: &str, reader_port: Option<&str>) -> Result<Link, OpenError> {
        let base = open(port)?;
        let reader = match reader_port {
            Some(r) if r == port => return Err(OpenError { message: format!("the reader's port is the base station's own ({port}): the reader is a second board") }),
            Some(r) => Some(open(r)?),
            None => None,
        };
        let base_reading = base.try_clone().map_err(|e| words(port, &e))?;
        let base = Arc::new(Mutex::new(base));
        let heard = Arc::new(Mutex::new(Heard::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = (base.clone(), heard.clone(), stop.clone(), port.to_string(), reader_port.map(str::to_string));
        std::thread::Builder::new()
            .name("leah-dcc-link".into())
            .spawn(move || {
                let (base, heard, stop, port, reader_port) = thread;
                run_link(base, base_reading, reader, reader_port, port, heard, stop);
            })
            .map_err(|e| OpenError { message: format!("can't start the link to {port}: {e}") })?;
        Ok(Link { port: port.to_string(), reader_port: reader_port.map(str::to_string), base, heard, stop, busy: Arc::new(AtomicBool::new(false)) })
    }

    /// What reading a decoder's settings needs, to take to a thread of its own.
    pub fn reading(&self) -> Reading {
        Reading { base: self.base.clone(), heard: self.heard.clone(), busy: self.busy.clone() }
    }

    /// Sends the base station a line ("K 3 03 3F A9 95", "+", ...).
    pub fn say(&self, line: &str) -> std::io::Result<()> {
        let mut base = self.base.lock().unwrap_or_else(|p| p.into_inner());
        trace(&format!("-> {line}"));
        write_all(&mut base, format!("{line}\n").as_bytes())
    }

    /// Waits until both boards run Leah's programs and the base station listens: its hello, then its
    /// answer to "?" given after it (an Arduino's program starts again as its port opens, sometimes
    /// twice, and what was sent before it started again is lost). Ok, or an error in words.
    pub fn wait_for_programs(&self, limit: Duration) -> Result<(), String> {
        let start = Instant::now();
        // (The hellos and answers counted when "?" was last asked.)
        let mut asked: Option<(u32, u32)> = None;
        let mut asked_at = Instant::now();
        loop {
            let ask = {
                let heard = self.heard.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(gone) = &heard.gone {
                    return Err(gone.clone());
                }
                let base_said_hello = heard.base_hello.as_deref() == Some(BASE_HELLO);
                let base_ok = match asked {
                    Some((hellos, states)) => heard.base_hellos == hellos && heard.states > states,
                    None => false,
                };
                let reader_ok = self.reader_port.is_none() || heard.reader_hello.as_deref().is_some_and(|h| h == READER_HELLO || h.starts_with("T "));
                if base_ok && reader_ok {
                    return Ok(());
                }
                if start.elapsed() > limit {
                    let which = |said: &Option<String>, port: &str, role: &str, hello: &str, ok: bool| -> Option<String> {
                        let other = if role == "base" { READER_HELLO } else { BASE_HELLO };
                        match said.as_deref() {
                            _ if ok => None,
                            Some(h) if h == other => Some(format!(
                                "the board on {port} runs Leah's {} program, not the {}'s: are the base station's and the reader's ports the wrong way round?",
                                if role == "base" { "reader" } else { "base station" },
                                if role == "base" { "base station" } else { "reader" }
                            )),
                            Some(h) if h.starts_with("LEAH DCC") && h != hello => Some(format!("the board on {port} runs an older program of Leah's ({h}): load the new one with load_dcc_program({port:?}, {role:?})")),
                            Some(h) if h == hello => Some(format!("the board on {port} started Leah's program but didn't answer: unplug it, plug it in again, and try once more")),
                            _ => Some(format!("the board on {port} isn't running Leah's {} program: load it with load_dcc_program({port:?}, {role:?})", if role == "base" { "base station" } else { "reader" })),
                        }
                    };
                    let mut problems = Vec::new();
                    // (A base station that answered "?" without saying its hello runs Leah's program too.)
                    problems.extend(which(&heard.base_hello, &self.port, "base", BASE_HELLO, base_ok || (heard.states > 0 && heard.base_hello.is_none())));
                    if let Some(r) = &self.reader_port {
                        problems.extend(which(&heard.reader_hello, r, "reader", READER_HELLO, reader_ok));
                    }
                    if problems.is_empty() {
                        return Ok(());
                    }
                    return Err(problems.join("; and "));
                }
                // Asked again after each new hello (it started again), and every half second.
                let fresh = match asked {
                    None => base_said_hello || start.elapsed() > Duration::from_millis(2500),
                    Some((hellos, _)) => heard.base_hellos != hellos || asked_at.elapsed() > Duration::from_millis(500),
                };
                fresh.then_some((heard.base_hellos, heard.states))
            };
            if let Some(counts) = ask {
                // (A moment after a hello, so the program is past its start.)
                std::thread::sleep(Duration::from_millis(100));
                asked = Some(counts);
                asked_at = Instant::now();
                let _ = self.say("?");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn close(&self) {
        if !self.stop.swap(true, Ordering::SeqCst) {
            let _ = self.say("X");
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.close();
    }
}

fn write_all(file: &mut File, mut bytes: &[u8]) -> std::io::Result<()> {
    // (The port is non-blocking: a full buffer is waited out, for at most a second.)
    let start = Instant::now();
    while !bytes.is_empty() {
        match file.write(bytes) {
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && start.elapsed() < Duration::from_secs(1) => std::thread::sleep(Duration::from_millis(2)),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// One port's text so far, cut into lines.
#[derive(Default)]
struct Lines {
    partial: Vec<u8>,
}

impl Lines {
    /// Reads what is there; Err when the port went away.
    fn read(&mut self, file: &mut File) -> Result<Vec<String>, ()> {
        let mut buf = [0u8; 512];
        loop {
            match file.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => self.partial.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(()),
            }
        }
        let mut lines = Vec::new();
        while let Some(at) = self.partial.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.partial.drain(..=at).collect();
            let text = String::from_utf8_lossy(&line).trim().to_string();
            if !text.is_empty() {
                lines.push(text);
            }
        }
        if self.partial.len() > 4096 {
            self.partial.clear();
        }
        Ok(lines)
    }
}

/// A line as a program's hello: from where "LEAH DCC" starts in it (a board starting again while it
/// was talking leaves the start of its last line before it).
fn hello_in(line: &str) -> Option<String> {
    line.rfind("LEAH DCC").map(|at| line[at..].to_string())
}

fn run_link(base: Arc<Mutex<File>>, mut base_reading: File, mut reader: Option<File>, reader_port: Option<String>, port: String, heard: Arc<Mutex<Heard>>, stop: Arc<AtomicBool>) {
    let mut base_lines = Lines::default();
    let mut reader_lines = Lines::default();
    let mut last_word = Instant::now();
    while !stop.load(Ordering::SeqCst) {
        let mut fds = vec![libc::pollfd { fd: base_reading.as_raw_fd(), events: libc::POLLIN, revents: 0 }];
        if let Some(r) = &reader {
            fds.push(libc::pollfd { fd: r.as_raw_fd(), events: libc::POLLIN, revents: 0 });
        }
        // SAFETY: the descriptors are open for the loop's life.
        unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 200) };
        let base_gone = fds[0].revents & (libc::POLLHUP | libc::POLLERR) != 0;
        let reader_gone = fds.get(1).is_some_and(|f| f.revents & (libc::POLLHUP | libc::POLLERR) != 0);
        let said = base_lines.read(&mut base_reading);
        let read = match &mut reader {
            Some(r) => reader_lines.read(r),
            None => Ok(Vec::new()),
        };
        {
            let mut h = heard.lock().unwrap_or_else(|p| p.into_inner());
            if base_gone || said.is_err() {
                h.gone = Some(format!("the base station's port {port} went away (was its cable pulled?)"));
            }
            if reader.is_some() && (reader_gone || read.is_err()) {
                h.reader_gone = Some(format!("the reader's port {} went away (was its cable pulled?): decoder() and timing stop", reader_port.as_deref().unwrap_or("")));
            }
            for line in said.unwrap_or_default() {
                trace(&format!("<- {line}"));
                if let Some(hello) = hello_in(&line) {
                    h.base_hello = Some(hello);
                    h.base_hellos += 1;
                } else if line.starts_with("STATE ") {
                    h.states += 1;
                    let words: Vec<&str> = line.split_whitespace().collect();
                    let after = |key: &str| words.iter().position(|w| *w == key).and_then(|i| words.get(i + 1)).copied();
                    if let Some(yes) = after("shield") {
                        h.shield = Some(yes == "yes");
                    }
                    if let Some(n) = after("limit").and_then(|n| n.parse().ok()) {
                        h.limit = Some(n);
                    }
                } else if ["CV ", "W ", "V ", "U ", "Y "].iter().any(|p| line.starts_with(p)) {
                    h.cv_answers.push_back(line);
                } else if let Some(n) = line.strip_prefix("A ").and_then(|n| n.trim().parse().ok()) {
                    h.current = Some(n);
                } else if line != "OK" {
                    h.base_said.push_back(line);
                }
            }
            for line in read.unwrap_or_default() {
                trace(&format!("   reader: {line}"));
                if let Some(hello) = hello_in(&line) {
                    h.reader_hello = Some(hello);
                } else if let Some(t) = timing_of(&line) {
                    h.reader_hello.get_or_insert(line);
                    h.timing = Some(t);
                } else if let Some(p) = packet_of(&line) {
                    if h.packets.len() == 1000 {
                        h.packets.pop_front();
                    }
                    h.packets.push_back(p);
                }
            }
            if h.gone.is_some() {
                break;
            }
            if h.reader_gone.is_some() && reader.is_some() {
                // (After its last lines were taken in: its timing is no longer known.)
                reader = None;
                h.timing = None;
            }
        }
        if last_word.elapsed() >= Duration::from_secs(1) {
            last_word = Instant::now();
            let mut b = base.lock().unwrap_or_else(|p| p.into_inner());
            let _ = write_all(&mut b, b".\n");
            trace("-> .");
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The ports plugged in

/// A serial port plugged in: its path, what the board says it is, and whether this program may open it.
pub struct PortInfo {
    pub port: String,
    pub name: String,
    pub vendor_id: String,
    pub product_id: String,
    pub allowed: bool,
}

/// What a board is, from its USB ids (and the name it gives, for others).
fn board_name(vendor: &str, product: &str, given: &str) -> String {
    match (vendor, product) {
        ("2341" | "2a03", "0042" | "0010") => "Arduino Mega 2560".into(),
        ("2341" | "2a03", "0043" | "0001" | "0243") => "Arduino Uno".into(),
        ("2341", "0058" | "0037") => "Arduino Nano".into(),
        ("1a86", "7523" | "55d4") => "USB serial (CH340): an Uno, Nano, or Mega clone".into(),
        ("0403", "6001") => "USB serial (FTDI FT232R)".into(),
        ("10c4", "ea60") => "USB serial (CP2102)".into(),
        _ if !given.is_empty() => given.to_string(),
        _ => "a serial device".into(),
    }
}

fn read_trimmed(path: &Path) -> String {
    std::fs::read_to_string(path).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Whether this program may read and write the port.
fn allowed(port: &str) -> bool {
    let c = std::ffi::CString::new(port).unwrap_or_default();
    // SAFETY: a C string for access(2).
    unsafe { libc::access(c.as_ptr(), libc::R_OK | libc::W_OK) == 0 }
}

/// The USB serial ports plugged in (`/dev/ttyACM*`, `/dev/ttyUSB*`), by name. (In the addon's tests,
/// `LEAH_TEST_DCC_PORTS` names a file listing stand-ins instead, a line each: the port, its vendor and
/// product ids, and its name, between tabs; never the real boards.)
pub fn ports() -> Vec<PortInfo> {
    if let Some(list) = std::env::var_os("LEAH_TEST_DCC_PORTS") {
        let text = std::fs::read_to_string(list).unwrap_or_default();
        return text
            .lines()
            .filter_map(|l| {
                let mut f = l.split('\t');
                let (port, vendor, product, given) = (f.next()?, f.next()?, f.next()?, f.next().unwrap_or(""));
                Some(PortInfo { port: port.to_string(), name: board_name(vendor, product, given), vendor_id: vendor.to_string(), product_id: product.to_string(), allowed: allowed(port) })
            })
            .collect();
    }
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/tty") else { return found };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.starts_with("ttyACM") || name.starts_with("ttyUSB")) {
            continue;
        }
        // (The USB device is a folder or two above the port's interface.)
        let mut at = std::fs::canonicalize(entry.path().join("device")).unwrap_or_default();
        let mut usb: Option<PathBuf> = None;
        for _ in 0..4 {
            if at.join("idVendor").is_file() {
                usb = Some(at.clone());
                break;
            }
            if !at.pop() {
                break;
            }
        }
        let (vendor, product, given) = match &usb {
            Some(u) => (read_trimmed(&u.join("idVendor")), read_trimmed(&u.join("idProduct")), read_trimmed(&u.join("product"))),
            None => Default::default(),
        };
        let port = format!("/dev/{name}");
        found.push(PortInfo { name: board_name(&vendor, &product, &given), allowed: allowed(&port), port, vendor_id: vendor, product_id: product });
    }
    found.sort_by_key(|p| natural(&p.port));
    found
}

/// "/dev/ttyACM10" after "/dev/ttyACM9".
fn natural(port: &str) -> (String, u32) {
    let digits = port.trim_end_matches(|c: char| !c.is_ascii_digit());
    let start = digits.rfind(|c: char| !c.is_ascii_digit()).map_or(0, |i| i + 1);
    (port[..start].to_string(), port[start..].parse().unwrap_or(0))
}

// ---------------------------------------------------------------------------------------------------
// Loading a program onto a board

/// Where `arduino-cli` is: the one named, or the first on the PATH.
pub fn find_arduino_cli(named: Option<&str>) -> Result<PathBuf, String> {
    if let Some(n) = named {
        let p = PathBuf::from(n);
        return if p.is_file() { Ok(p) } else { Err(format!("there is no arduino-cli at {n}")) };
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let p = dir.join("arduino-cli");
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err("loading a program onto a board needs arduino-cli, Arduino's command-line tool, which isn't installed (or isn't on the PATH): get it from https://github.com/arduino/arduino-cli/releases, then run `arduino-cli core install arduino:avr` once; or give its place with `arduino_cli:`".to_string())
}

/// How loading a program went.
#[derive(Clone, Default)]
pub struct JobState {
    pub finished: bool,
    pub succeeded: bool,
    pub message: String,
}

/// Compiles the program for `role` ("base" or "reader") for the board `fqbn` in `folder` (a temporary
/// folder of the script's own) and uploads it to `port`: what happened, in words.
pub fn load_program(cli: &Path, folder: &Path, port: &str, role: &str, fqbn: &str) -> Result<String, String> {
    let (name, text) = if role == "base" { ("leah_dcc_base", BASE_PROGRAM) } else { ("leah_dcc_reader", READER_PROGRAM) };
    let sketch = folder.join(name);
    let build = folder.join("build");
    let cache = folder.join("cache");
    std::fs::create_dir_all(&sketch).map_err(|e| format!("can't make {}: {e}", sketch.display()))?;
    std::fs::write(sketch.join(format!("{name}.ino")), text).map_err(|e| format!("can't write the program: {e}"))?;
    let run = |args: &[&std::ffi::OsStr]| -> Result<String, String> {
        let mut command = std::process::Command::new(cli);
        command.args(args).stdin(std::process::Stdio::null());
        // (arduino-cli keeps its build cache in the home folder unless told: here, the temporary folder.)
        if std::env::var_os("ARDUINO_BUILD_CACHE_PATH").is_none() {
            command.env("ARDUINO_BUILD_CACHE_PATH", &cache);
        }
        let out = command.output().map_err(|e| format!("can't run {}: {e}", cli.display()))?;
        let all = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        if out.status.success() { Ok(all) } else { Err(all) }
    };
    let fqbn_arg = std::ffi::OsString::from(fqbn);
    let compiled = run(&["compile".as_ref(), "--fqbn".as_ref(), fqbn_arg.as_os_str(), "--build-path".as_ref(), build.as_os_str(), sketch.as_os_str()]).map_err(|out| {
        if out.contains("platform not installed") || out.contains("Platform '") || out.contains("not found") && out.contains("platform") {
            format!("arduino-cli hasn't the support for {fqbn}: run `arduino-cli core install {}` once", fqbn.split(':').take(2).collect::<Vec<_>>().join(":"))
        } else {
            format!("the program couldn't be compiled for {fqbn}: {}", last_lines(&out, 6))
        }
    })?;
    let port_arg = std::ffi::OsString::from(port);
    run(&["upload".as_ref(), "--fqbn".as_ref(), fqbn_arg.as_os_str(), "--input-dir".as_ref(), build.as_os_str(), "--port".as_ref(), port_arg.as_os_str(), sketch.as_os_str()]).map_err(|out| {
        if out.contains("Permission denied") {
            words(port, &std::io::Error::from(std::io::ErrorKind::PermissionDenied)).message
        } else if out.contains("Device or resource busy") {
            format!("{port} is in use by another program (a dcc_base still holding it? close() it first)")
        } else if out.contains("not in sync") || out.contains("programmer is not responding") {
            format!("the board on {port} didn't answer the uploader: is it the board you named ({fqbn})? A Nano with the old bootloader is \"arduino:avr:nano:cpu=atmega328old\"")
        } else {
            format!("the program couldn't be loaded onto {port}: {}", last_lines(&out, 6))
        }
    })?;
    // ("Sketch uses 4376 bytes (1%) of program storage space. Maximum is 253952 bytes.")
    let size = compiled.lines().find(|l| l.starts_with("Sketch uses")).and_then(|l| {
        let numbers: Vec<&str> = l.split_whitespace().filter(|w| w.chars().all(|c| c.is_ascii_digit())).collect();
        Some(format!(" ({} of the board's {} bytes)", numbers.first()?, numbers.get(1)?))
    });
    Ok(format!("Leah's {} program is on the board on {port}{}", if role == "base" { "base station" } else { "reader" }, size.unwrap_or_default()))
}

fn last_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_readers_lines_are_read() {
        let t = timing_of("T one 57.5-58.0 zero 99.5-100.0 packets 172 bad 2 errors 0").unwrap();
        assert_eq!(t, Timing { one: Some((57.5, 58.0)), zero: Some((99.5, 100.0)), packets: 172, outside: 2, errors: 0 });
        let quiet = timing_of("T one none zero none packets 0 bad 0 errors 0").unwrap();
        assert_eq!((quiet.one, quiet.zero, quiet.packets), (None, None, 0));
        assert_eq!(packet_of("P 03 3F A9 95"), Some(vec![0x03, 0x3F, 0xA9, 0x95]));
        assert_eq!(packet_of("P 03 3G"), None);
        assert_eq!(hello_in("LEAH DCC BLEAH DCC BASE 2").as_deref(), Some("LEAH DCC BASE 2"));
        assert_eq!(hello_in("x\u{fffd}LEAH DCC READER 1").as_deref(), Some("LEAH DCC READER 1"));
    }

    #[test]
    fn boards_are_named_by_their_ids() {
        assert_eq!(board_name("2341", "0042", ""), "Arduino Mega 2560");
        assert!(board_name("1a86", "7523", "USB Serial").contains("CH340"));
        assert_eq!(board_name("1234", "5678", "Gadget"), "Gadget");
        assert_eq!(natural("/dev/ttyACM10").1, 10);
    }
}
