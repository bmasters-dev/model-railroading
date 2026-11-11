//! The addon's tests: Leah scripts run by Leah itself, with this addon in a 3rd-party addons folder of
//! the test's own (`LEAH_3RD_PARTY_ADDONS`), as a person's Leah loads it once installed.
//!
//! Which Leah runs them: `LEAH` (a program), or Leah's development build beside this repository
//! (~/Leah_Dev/target/debug/leah), or the `leah` on the PATH. The example's test needs a Leah with
//! windows (the gui addon): `LEAH_GUI`, or ~/Leah_Dev/target/all_addons/debug/leah; it runs on Leah's
//! hidden test screen (~/Leah_Dev/tools/wayland_test_screen.sh), never on the real one, and is
//! skipped without them.

use std::path::{Path, PathBuf};
use std::process::Command;

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME"))
}

/// The addon's library, built by `cargo test` beside the tests.
fn library() -> PathBuf {
    let deps = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/libleah_addon_model_railroading.so");
    assert!(deps.is_file(), "{} isn't built: `cargo build` first", deps.display());
    deps
}

/// A 3rd-party addons folder of the test's own, holding this addon as `model_railroading.so`.
fn addons_folder(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("model-railroading-test-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(library(), dir.join("model_railroading.so")).unwrap();
    dir
}

fn leah() -> PathBuf {
    if let Some(l) = std::env::var_os("LEAH") {
        return l.into();
    }
    let dev = home().join("Leah_Dev/target/debug/leah");
    if dev.is_file() { dev } else { "leah".into() }
}

struct Run {
    stdout: String,
    stderr: String,
    success: bool,
}

/// Runs a script with this addon, gives back what it printed.
fn run(test: &str, script: &str) -> Run {
    run_with(test, script, &[])
}

/// As `run`, with more of the environment set.
fn run_with(test: &str, script: &str, env: &[(&str, &str)]) -> Run {
    let dir = addons_folder(test);
    let file = dir.join("script.leah");
    std::fs::write(&file, script).unwrap();
    let mut command = Command::new(leah());
    command
        .arg(&file)
        .env("LEAH_3RD_PARTY_ADDONS", &dir)
        // (None of the person's own addons: only this one, and Leah's own built in.)
        .env("LEAH_USER_ADDONS", "")
        // (Never the real boards' list.)
        .env("LEAH_TEST_DCC_PORTS", "/nonexistent");
    for (k, v) in env {
        command.env(k, v);
    }
    let out = command.output().expect("Leah runs");
    let _ = std::fs::remove_dir_all(&dir);
    Run { stdout: String::from_utf8_lossy(&out.stdout).into(), stderr: String::from_utf8_lossy(&out.stderr).into(), success: out.status.success() }
}

#[test]
fn a_simulated_base_station_sends_nmra_packets_its_decoders_read_back() {
    // design/model_railroading.md, "Stage 1a": the packets as NMRA S-9.2 gives them, and the
    // simulated decoders reading them back from their signal (S-9.1's timing), not from the bytes.
    let script = r#"$base = dcc_base(simulate: true)
$loco = locomotive(3)
$loco.speed = 40
print([$base.power, $base.decoder(3), $base.packets[0]["heard"]])
$base.power = true
$loco.speed = 40
$loco.set_function(0, true)
$loco.set_function(13, true)
$loco.direction = "reverse"
$big = locomotive(1234, long: true)
$big.speed = 126
$short_as_long = locomotive(3, long: true)
$short_as_long.speed = 1
$base.emergency_stop()
foreach $base.packets as $p {
    print($p["bytes"] + " | " + $p["meaning"])
}
print($base.packets[1]["bits"])
$d = $base.decoder(3)
print([$d["speed"], $d["direction"], $d["functions"][0], $d["functions"][13], $d["functions"][1], $d["packets"]])
print([$base.decoder(1234, long: true)["speed"], $base.decoder(3, long: true)["packets"], $loco.speed, $big.speed, $loco.function(0)])
$loco.speed = 2
$loco.stop()
print([$base.decoder(3)["speed"], last($base.packets)["meaning"]])
$base.clear_packets()
print(count($base.packets))
"#;
    let run = run("packets", script);
    assert!(run.success, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        concat!(
            "[false, null, false]\n",
            "03 3F A9 95 | locomotive 3: speed 40, forward\n",
            "03 3F A9 95 | locomotive 3: speed 40, forward\n",
            "03 90 93 | locomotive 3: functions F0 to F4: F0 on, the rest off\n",
            "03 DE 01 DC | locomotive 3: functions F13 to F20: F13 on, the rest off\n",
            "03 3F 29 15 | locomotive 3: speed 40, reverse\n",
            "C4 D2 3F FF D6 | locomotive 1234: speed 126, forward\n",
            "C0 03 3F 82 7E | locomotive 3 (long): speed 1, forward\n",
            "00 41 41 | every locomotive: emergency stop\n",
            "11111111111111 0 00000011 0 00111111 0 10101001 0 10010101 1\n",
            "[0, \"reverse\", true, true, false, 5]\n",
            "[0, 2, 0, 0, true]\n",
            "[0, \"locomotive 3: stop, reverse\"]\n",
            "0\n",
        )
    );
}

#[test]
fn a_base_station_and_a_locomotive_refuse_what_they_cant_do() {
    let script = r#"foreach [function() { return locomotive(3) }, function() { return dcc_base() }, function() { return dcc_base("/dev/leah-no-such-port") }, function() { return dcc_base("ttyACM0") }, function() { return dcc_base(simulate: true, reader: "/dev/x") }, function() { return dcc_base("/etc/hostname") }] as $f {
    try {
        $f()
    } catch $e {
        print($e.type + ": " + $e.message)
    }
}
$base = dcc_base(simulate: true)
$loco = locomotive(3)
foreach [function() { return locomotive(0) }, function() { return locomotive(200) }, function() { return locomotive(10240, long: true) }, function() { $loco.speed = 127 }, function() { $loco.direction = "up" }, function() { $loco.set_function(29, true) }, function() { $loco.address = 4 }, function() { return locomotive(7, base: "x") }] as $f {
    try {
        $f()
    } catch $e {
        print($e.type + ": " + $e.message)
    }
}
"#;
    let run = run("refusals", script);
    assert!(run.success, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        concat!(
            "hardware_error: locomotive(): there is no base station: make one first, dcc_base(\"/dev/ttyACM0\") or dcc_base(simulate: true)\n",
            "hardware_error: dcc_base(): give the base station board's port, like dcc_base(\"/dev/ttyACM0\") (dcc_ports() lists them), or simulate: true for a simulated base station\n",
            "hardware_error: dcc_base(): there is no board on /dev/leah-no-such-port: is it plugged in? (dcc_ports() lists the ports there are)\n",
            "hardware_error: dcc_base(): \"ttyACM0\" isn't a port: give its whole path, like \"/dev/ttyACM0\" (dcc_ports() lists them)\n",
            "type_error: dcc_base(simulate: true) takes no port and no reader: the simulated base station has no boards\n",
            "hardware_error: dcc_base(): you may not open /etc/hostname yet: on Linux, apps may use a board's USB port only once your account is in the `dialout` group (`sudo usermod -aG dialout $USER`, then log out and back in once)\n",
            "range_error: locomotive(): a locomotive's address is 1 to 10239, not 0\n",
            "range_error: locomotive(): 200 is a long address: give long: true (a short address is 1 to 127)\n",
            "range_error: locomotive(): a locomotive's address is 1 to 10239, not 10240\n",
            "range_error: a locomotive's `speed` is 0 to 126 (the 128 speed steps), not 127\n",
            "type_error: a locomotive's `direction` is \"forward\" or \"reverse\", not \"up\"\n",
            "range_error: locomotive.set_function(): a function's number is 0 (the headlight) to 28, not 29\n",
            "type_error: a locomotive's `address` can be read but not set\n",
            "type_error: locomotive()'s `base:` is a dcc_base, not \"x\"\n",
        )
    );
}

#[test]
fn a_locomotive_gives_back_its_own_base_station() {
    // `base:` names the station; `$loco.base` is the very object the script holds, and the station
    // comes back from the locomotive even after the script let go of its own variable.
    let script = r#"$a = dcc_base(simulate: true)
$b = dcc_base(simulate: true)
$b.power = true
$loco = locomotive(5, base: $a)
print([$loco.base == $a, $loco.base == $b])
$other = locomotive(6)
$other.speed = 10
print([$other.base == $b, $b.decoder(6)["speed"]])
$b = null
$back = $other.base
print([class_of($back), $back.power, $back.decoder(6)["speed"], $other.base == $back])
"#;
    let run = run("base", script);
    assert!(run.success, "{}", run.stderr);
    assert_eq!(run.stdout, "[true, false]\n[true, 10]\n[\"dcc_base\", true, 10, true]\n");
}

#[test]
fn the_dcc_simulator_example_runs_its_train() {
    // The example: track power on, the throttle open, the headlight on; the train is drawn where the
    // simulated decoder has taken it. On Leah's hidden test screen only.
    let gui = std::env::var_os("LEAH_GUI").map(PathBuf::from).unwrap_or_else(|| home().join("Leah_Dev/target/all_addons/debug/leah"));
    let screen = home().join("Leah_Dev/tools/wayland_test_screen.sh");
    if !gui.is_file() || !screen.is_file() {
        eprintln!("skipped: no Leah with windows ({}) or no hidden test screen", gui.display());
        return;
    }
    let dir = addons_folder("example");
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dcc_simulator.leah");
    let script = format!(
        "include \"{}\"\ncall_later(400, function() {{\n    global $power, $throttle, $headlight\n    $power.value = true\n    $throttle.value = 60\n    $headlight.value = true\n    call_later(400, function() {{\n        global $base\n        print($base.decoder(3))\n        quit()\n    }})\n}})\n",
        example.display()
    );
    let file = dir.join("script.leah");
    std::fs::write(&file, script).unwrap();
    let out = Command::new("sh")
        .arg(&screen)
        .arg("--gui")
        .arg(&gui)
        .arg(&file)
        .env("LEAH_3RD_PARTY_ADDONS", &dir)
        .env("LEAH_USER_ADDONS", "")
        .env("LEAH_TEST_INTERACT", "")
        .env("LEAH_QUIT_AFTER_MS", "5000")
        .output()
        .expect("the hidden screen runs");
    let _ = std::fs::remove_dir_all(&dir);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stdout.contains("[\"address\": 3, \"long\": false, \"speed\": 60, \"direction\": \"forward\", \"functions\": [true,"), "{stdout}\n{stderr}");
}

#[test]
fn the_examples_page_gives_its_documented_output() {
    // docs/examples.md: each ```leah block is run, and what it prints compared with the Output block
    // under it (as Leah's own guide examples are).
    let page = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/examples.md")).unwrap();
    let mut lines = page.lines().peekable();
    let mut checked = 0;
    while let Some(line) = lines.next() {
        if line.trim() != "```leah" {
            continue;
        }
        let code: Vec<&str> = lines.by_ref().take_while(|l| l.trim() != "```").collect();
        while lines.peek().is_some_and(|l| l.trim().is_empty()) {
            lines.next();
        }
        assert_eq!(lines.next().map(str::trim), Some("Output:"), "an example has no Output block:\n{}", code.join("\n"));
        assert_eq!(lines.next().map(str::trim), Some("```"));
        let wanted: Vec<&str> = lines.by_ref().take_while(|l| l.trim() != "```").collect();
        let run = run(&format!("example{checked}"), &(code.join("\n") + "\n"));
        assert!(run.success, "{}\n{}", code.join("\n"), run.stderr);
        assert_eq!(run.stdout.trim_end(), wanted.join("\n").trim_end(), "{}", code.join("\n"));
        checked += 1;
    }
    assert!(checked > 0, "no examples found");
}

/// The stand-in boards (tests/fake_boards.py): their ports, and the program playing them, which ends
/// when this is dropped.
struct FakeBoards {
    base: String,
    reader: String,
    _child: std::process::Child,
}

fn fake_boards(args: &[&str]) -> Option<FakeBoards> {
    use std::io::BufRead;
    let mut child = Command::new("python3")
        .arg("-I")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake_boards.py"))
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    let mut out = std::io::BufReader::new(child.stdout.take()?);
    let mut line = String::new();
    out.read_line(&mut line).ok()?;
    let base = line.trim().strip_prefix("BASE ")?.to_string();
    line.clear();
    out.read_line(&mut line).ok()?;
    let reader = line.trim().strip_prefix("READER ")?.to_string();
    Some(FakeBoards { base, reader, _child: child })
}

#[test]
fn a_base_station_board_sends_what_the_reader_hears() {
    // Leah's base station on its (stand-in) board: the locomotive's speed and lights kept in the
    // refresh table, the emergency stop sent now, the reader's packets and timing read back, and the
    // track power kept on by the PC's word every second (the board turns it off after 3 seconds of
    // quiet).
    let log = std::env::temp_dir().join(format!("model-railroading-board-log-{}", std::process::id()));
    let _ = std::fs::remove_file(&log);
    let Some(boards) = fake_boards(&["--log", log.to_str().unwrap()]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{base}", reader: "{reader}")
print([$base.simulate, $base.port == "{base}", $base.reader == "{reader}", $base.power, $base.timing == null or $base.timing["one"] == null])
$loco = locomotive(3)
$loco.speed = 40
$loco.set_function(0, true)
$base.power = true
sleep(1600)
$d = $base.decoder(3)
print([$d["speed"], $d["direction"], $d["functions"][0]])
print([$base.timing["one"], $base.timing["zero"], $base.timing["errors"]])
print(map($base.packets, function($p) {{ return $p["heard"] }}))
$base.emergency_stop()
sleep(800)
print($base.decoder(3)["speed"])
sleep(3000)
print($base.power)
$base.close()
print($base.power)
try {{
    $base.power = true
}} catch $e {{
    print($e.message)
}}
"#,
        base = boards.base,
        reader = boards.reader
    );
    let run = run("board", &script);
    drop(boards);
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    assert!(run.success, "{}\n{}", run.stdout, run.stderr);
    assert_eq!(
        run.stdout,
        "[false, true, true, false, true]\n[40, \"forward\", true]\n[[57.5, 58], [99.5, 100], 0]\n[true, true]\n0\ntrue\nfalse\na dcc_base's `power`: the base station is closed\n"
    );
    let lines: Vec<&str> = said.lines().filter(|l| *l != ".").collect();
    assert_eq!(lines, ["?", "X", "L 1500", "K 0 03 3F A9 95", "K 1 03 90 93", "+", "S 00 41 41", "K 0 03 3F 81 BD", "X"], "{said}");
    assert!(said.lines().filter(|l| *l == ".").count() >= 4, "the PC said it was there every second:\n{said}");
}

#[test]
fn a_base_station_that_turns_its_power_off_says_so() {
    let Some(boards) = fake_boards(&["--cut-after", "0.5"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!("$base = dcc_base(\"{}\")\n$base.power = true\nsleep(1200)\nprint($base.power)\n", boards.base);
    let run = run("cut", &script);
    assert!(run.success, "{}", run.stderr);
    assert_eq!(run.stdout, "false\n");
    assert!(run.stderr.contains("the base station turned the track power off by itself: no word from the PC for 3 seconds"), "{}", run.stderr);
}

#[test]
fn a_board_without_leahs_program_is_named_with_what_to_do() {
    for (mode, words) in [
        ("silent", "isn't running Leah's base station program: load it with load_dcc_program(\"{port}\", \"base\")"),
        ("old", "runs an older program of Leah's (LEAH DCC BASE 3): load the new one with load_dcc_program(\"{port}\", \"base\")"),
        ("reader", "runs Leah's reader program, not the base station's: are the base station's and the reader's ports the wrong way round?"),
    ] {
        let Some(boards) = fake_boards(&["--base", mode]) else {
            eprintln!("skipped: no python3 for the stand-in boards");
            return;
        };
        let script = format!("try {{\n    $base = dcc_base(\"{}\")\n}} catch $e {{\n    print($e.type + \": \" + $e.message)\n}}\n", boards.base);
        let run = run(&format!("program-{mode}"), &script);
        assert!(run.success, "{}", run.stderr);
        let wanted = format!("hardware_error: dcc_base(): the board on {} {}\n", boards.base, words.replace("{port}", &boards.base));
        assert_eq!(run.stdout, wanted);
    }
}

#[test]
fn dcc_ports_names_the_boards_and_whether_they_may_be_opened() {
    let dir = std::env::temp_dir().join(format!("model-railroading-ports-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let open = dir.join("ttyACM0");
    let shut = dir.join("ttyUSB0");
    std::fs::write(&open, "").unwrap();
    std::fs::write(&shut, "").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();
    let list = dir.join("ports.txt");
    std::fs::write(&list, format!("{}\t2341\t0042\t\n{}\t1a86\t7523\tUSB Serial\n", open.display(), shut.display())).unwrap();
    let run = run_with("ports", "foreach dcc_ports() as $p {\n    print([$p[\"name\"], $p[\"vendor_id\"], $p[\"product_id\"], $p[\"allowed\"]])\n}\n", &[("LEAH_TEST_DCC_PORTS", list.to_str().unwrap())]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.success, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "[\"Arduino Mega 2560\", \"2341\", \"0042\", true]\n[\"USB serial (CH340): an Uno, Nano, or Mega clone\", \"1a86\", \"7523\", false]\n"
    );
}

#[test]
fn load_dcc_program_compiles_and_uploads_through_arduino_cli() {
    // A stand-in arduino-cli writes down what it was asked, checks the program is in its sketch
    // folder, and answers as the real one does (or fails as an upload to the wrong board does).
    let dir = std::env::temp_dir().join(format!("model-railroading-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let asked = dir.join("asked.txt");
    let cli = dir.join("arduino-cli");
    std::fs::write(
        &cli,
        format!(
            "#!/bin/sh\necho \"$@\" >> '{asked}'\nfor last; do :; done\n[ -f \"$last/$(basename \"$last\").ino\" ] && echo 'the sketch is there' >> '{asked}'\n\
             case \"$*\" in *--port*/dev/broken*) echo 'avrdude: stk500_recv(): programmer is not responding' >&2; exit 1;; esac\n\
             echo 'Sketch uses 4376 bytes (1%) of program storage space. Maximum is 253952 bytes.'\n",
            asked = asked.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
    let script = format!(
        r#"$job = load_dcc_program("/dev/fake0", "base", arduino_cli: "{cli}")
$job.on_finished = function($ok, $message) {{
    print("finished: " + $ok)
}}
print([$job.port, $job.role, $job.board])
print($job.wait())
print($job.message)
$other = load_dcc_program("/dev/broken", "reader", board: "arduino:avr:nano", arduino_cli: "{cli}")
print($other.wait())
print($other.message)
foreach [function() {{ return load_dcc_program("ttyACM0", "base") }}, function() {{ return load_dcc_program("/dev/x", "booster") }}, function() {{ return load_dcc_program("/dev/x", "base", board: "-p") }}, function() {{ return load_dcc_program("/dev/x", "base", board: "arduino:avr:uno") }}, function() {{ return load_dcc_program("/dev/x", "reader", board: "arduino:avr:mega") }}, function() {{ return load_dcc_program("/dev/x", "base", arduino_cli: "/nowhere/arduino-cli") }}] as $f {{
    try {{
        $f()
    }} catch $e {{
        print($e.type + ": " + $e.message)
    }}
}}
"#,
        cli = cli.display()
    );
    let run = run("load", &script);
    let said = std::fs::read_to_string(&asked).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.success, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        concat!(
            "[\"/dev/fake0\", \"base\", \"arduino:avr:mega\"]\n",
            "true\n",
            "Leah's base station program is on the board on /dev/fake0 (4376 of the board's 253952 bytes)\n",
            "false\n",
            "the board on /dev/broken didn't answer the uploader: is it the board you named (arduino:avr:nano)? A Nano with the old bootloader is \"arduino:avr:nano:cpu=atmega328old\"\n",
            "type_error: load_dcc_program(): \"ttyACM0\" isn't a port: give its whole path, like \"/dev/ttyACM0\" (dcc_ports() lists them)\n",
            "type_error: load_dcc_program()'s program is \"base\" (the base station) or \"reader\" (the board reading the signal back), not \"booster\"\n",
            "type_error: load_dcc_program()'s `board:` is arduino-cli's name for the board, like \"arduino:avr:mega\", \"arduino:avr:uno\", or \"arduino:avr:nano\", not \"-p\"\n",
            "type_error: load_dcc_program(): Leah's base station program runs on an Arduino Mega 2560 (\"arduino:avr:mega\"), not \"arduino:avr:uno\"\n",
            "type_error: load_dcc_program(): Leah's reader program runs on an Arduino Uno, Nano, or Pro Mini (\"arduino:avr:uno\", \"arduino:avr:nano\", \"arduino:avr:pro\"), not \"arduino:avr:mega\"\n",
            "shell_error: load_dcc_program(): there is no arduino-cli at /nowhere/arduino-cli\n",
            "finished: true\n",
        )
    );
    let calls: Vec<&str> = said.lines().collect();
    // (Each call, then the stand-in's note that the program was in the sketch folder it was given.)
    assert_eq!(calls.len(), 8, "{said}");
    assert!(calls[0].starts_with("compile --fqbn arduino:avr:mega --build-path /") && calls[0].ends_with("/leah_dcc_base"), "{said}");
    assert!(calls[2].starts_with("upload --fqbn arduino:avr:mega --input-dir /") && calls[2].contains(" --port /dev/fake0 /"), "{said}");
    assert!(calls[4].starts_with("compile --fqbn arduino:avr:nano") && calls[4].ends_with("/leah_dcc_reader"), "{said}");
    assert!(calls[6].starts_with("upload --fqbn arduino:avr:nano") && calls[6].contains(" --port /dev/broken /"), "{said}");
    assert!([1, 3, 5, 7].iter().all(|i| calls[*i] == "the sketch is there"), "{said}");
}

#[test]
fn the_dcc_bench_example_drives_the_boards_and_asks_for_permission() {
    // The bench example on Leah's hidden test screen, with the stand-in boards listed as the ports:
    // connected, the track power on, the throttle open, it shows what the reader heard; and with a
    // port this account may not open, it asks for permission instead.
    let gui = std::env::var_os("LEAH_GUI").map(PathBuf::from).unwrap_or_else(|| home().join("Leah_Dev/target/all_addons/debug/leah"));
    let screen = home().join("Leah_Dev/tools/wayland_test_screen.sh");
    if !gui.is_file() || !screen.is_file() {
        eprintln!("skipped: no Leah with windows ({}) or no hidden test screen", gui.display());
        return;
    }
    let Some(boards) = fake_boards(&[]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let dir = addons_folder("bench");
    let shut = dir.join("ttyUSB9");
    std::fs::write(&shut, "").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dcc_bench.leah");
    let on_screen = |ports: String, then: &str| -> String {
        let list = dir.join("ports.txt");
        std::fs::write(&list, ports).unwrap();
        let file = dir.join("script.leah");
        std::fs::write(&file, format!("include \"{}\"\n{then}", example.display())).unwrap();
        let out = Command::new("sh")
            .arg(&screen)
            .arg("--gui")
            .arg(&gui)
            .arg(&file)
            .env("LEAH_3RD_PARTY_ADDONS", &dir)
            .env("LEAH_USER_ADDONS", "")
            .env("LEAH_TEST_DCC_PORTS", &list)
            .env("LEAH_TEST_INTERACT", "")
            .env("LEAH_QUIT_AFTER_MS", "12000")
            .output()
            .expect("the hidden screen runs");
        format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
    };
    let driving = on_screen(
        format!("{}\t2341\t0042\t\n{}\t1a86\t7523\tUSB Serial\n", boards.base, boards.reader),
        "call_later(300, function() {\n    global $connect\n    $connect.click()\n    call_later(3500, function() {\n        global $power, $throttle, $headlight\n        $power.value = true\n        $throttle.value = 64\n        $headlight.value = true\n        call_later(1800, function() {\n            global $decoder_text, $verdict, $current_text\n            print($decoder_text.text)\n            print($verdict.text)\n            print($current_text.text)\n            quit()\n        })\n    })\n})\n",
    );
    let asking = on_screen(
        format!("{}\t1a86\t7523\tUSB Serial\n", shut.display()),
        "call_later(300, function() {\n    global $status, $give_access\n    print($give_access.visible)\n    print($status.text)\n    quit()\n})\n",
    );
    drop(boards);
    let Some(shielded) = fake_boards(&["--shield", "--current", "25"]) else { return };
    let cutting = on_screen(
        format!("{}\t2341\t0042\t\n{}\t1a86\t7523\tUSB Serial\n", shielded.base, shielded.reader),
        "call_later(300, function() {\n    global $connect\n    $connect.click()\n    call_later(3500, function() {\n        global $power\n        $power.value = true\n        call_later(1200, function() {\n            global $current_text, $limit\n            print($current_text.text)\n            $limit.select_row_with_tag(15)\n            call_later(1200, function() {\n                global $status, $power\n                print($power.value)\n                print($status.text)\n                quit()\n            })\n        })\n    })\n})\n",
    );
    drop(shielded);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(driving.contains("Decoder 3 heard: speed 64, forward, headlight on.\nWithin the NMRA's limits for a command station"), "{driving}");
    assert!(driving.contains("No motor shield: the signal is 5 V logic on pin 12."), "{driving}");
    assert!(cutting.contains("Track current: 25 mA\nfalse\nthe base station turned the track power off by itself: the current was over the limit (25 mA, the limit 15 mA)"), "{cutting}");
    assert!(asking.contains("true\nLinux needs your permission before apps can talk to a board over USB."), "{asking}");
}

#[test]
fn a_pulled_cable_stops_what_it_must_and_nothing_more() {
    // The reader's cable out: the base station goes on (its power kept on by the PC's word), only the
    // reading back stops, with a warning. The base station's cable out: it can't be used any more, but
    // turning its power off and the emergency stop still work (there is nothing to send them to).
    let Some(boards) = fake_boards(&["--drop-reader-after", "0.5"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        "$base = dcc_base(\"{}\", reader: \"{}\")\n$base.power = true\nsleep(1500)\nprint([$base.power, $base.timing])\n$loco = locomotive(3)\n$loco.speed = 20\nsleep(3500)\nprint($base.power)\n",
        boards.base, boards.reader
    );
    let reader_gone = run("reader-gone", &script);
    drop(boards);
    assert!(reader_gone.success, "{}", reader_gone.stderr);
    assert_eq!(reader_gone.stdout, "[true, null]\ntrue\n");
    assert!(reader_gone.stderr.contains("went away (was its cable pulled?): decoder() and timing stop"), "{}", reader_gone.stderr);

    let Some(boards) = fake_boards(&["--drop-base-after", "2.2"]) else { return };
    let script = format!(
        "$base = dcc_base(\"{}\")\n$loco = locomotive(3)\n$loco.speed = 50\n$base.power = true\nsleep(2500)\n$base.power = false\n$base.emergency_stop()\nprint([$base.power, $loco.speed])\ntry {{\n    $base.power = true\n}} catch $e {{\n    print($e.message)\n}}\n",
        boards.base
    );
    let gone = run("base-gone", &script);
    let base = boards.base.clone();
    drop(boards);
    assert!(gone.success, "{}", gone.stderr);
    assert_eq!(gone.stdout, format!("[false, 0]\na dcc_base's `power`: the base station's port {base} went away (was its cable pulled?)\n"));
}

#[test]
fn locomotives_at_one_address_share_their_places_in_the_refresh_table() {
    let log = std::env::temp_dir().join(format!("model-railroading-share-log-{}", std::process::id()));
    let _ = std::fs::remove_file(&log);
    let Some(boards) = fake_boards(&["--log", log.to_str().unwrap()]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        "$base = dcc_base(\"{}\")\n$a = locomotive(3)\n$b = locomotive(3)\n$c = locomotive(4)\n$a.speed = 80\n$b.speed = 0\n$c.speed = 10\n$a = null\n$b = null\nsleep(300)\n",
        boards.base
    );
    let run = run("share", &script);
    drop(boards);
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    assert!(run.success, "{}", run.stderr);
    let lines: Vec<&str> = said.lines().filter(|l| *l != "." && *l != "?" && !l.starts_with("L ")).collect();
    // (Locomotive 3's two objects keep their speed in place 0; locomotive 4 has places 2 and 3; once
    // both of 3's are let go, its places are dropped, and 4's as the script ends.)
    assert_eq!(lines, ["X", "K 0 03 3F D1 ED", "K 0 03 3F 80 BC", "K 2 04 3F 8B B0", "D 0", "D 1", "D 2", "D 3", "X"], "{said}");
}

#[test]
fn a_base_station_with_its_motor_shield_reads_its_current_and_cuts_off_over_the_limit() {
    // The shield draws 25 mA (the bench's 470 ohm resistor across the track at 12 V): read, and under
    // the usual limit; with the limit set to 15 mA the board turns the power off and says why.
    let Some(boards) = fake_boards(&["--shield", "--current", "25"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
print([$base.shield, $base.current, $base.current_limit])
$base.power = true
sleep(1200)
print([$base.power, $base.current])
$base.current_limit = 15
sleep(600)
print([$base.power, $base.current, $base.current_limit])
foreach [function() {{ $base.current_limit = 5 }}, function() {{ $base.current_limit = "lots" }}, function() {{ $base.shield = false }}] as $f {{
    try {{
        $f()
    }} catch $e {{
        print($e.type + ": " + $e.message)
    }}
}}
$sim = dcc_base(simulate: true)
print([$sim.shield, $sim.current, $sim.current_limit])
"#,
        boards.base
    );
    let run = run("shield", &script);
    drop(boards);
    assert!(run.success, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        concat!(
            "[true, 0, 1500]\n",
            "[true, 25]\n",
            "[false, 0, 15]\n",
            "range_error: a dcc_base's `current_limit` is 10 to 5000 mA, not 5\n",
            "type_error: a dcc_base's `current_limit` is a whole number of milliamps, not \"lots\"\n",
            "type_error: a dcc_base's `shield` can be read but not set\n",
            "[false, null, 1500]\n",
        )
    );
    assert!(run.stderr.contains("the base station turned the track power off by itself: the current was over the limit (25 mA, the limit 15 mA): is something shorting the track?"), "{}", run.stderr);
}

#[test]
fn after_a_cut_off_the_power_waits_and_a_restarted_board_gets_its_table_again() {
    // Cut off over a 15 mA limit: turning the power on at once is refused for 2 seconds (the power
    // stays off, with the board's words); later it comes on again.
    let Some(boards) = fake_boards(&["--shield", "--current", "25"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        "$base = dcc_base(\"{}\")\n$base.current_limit = 15\n$base.power = true\nsleep(400)\nprint($base.power)\n$base.power = true\nsleep(300)\nprint($base.power)\n$base.current_limit = 1500\nsleep(2000)\n$base.power = true\nsleep(700)\nprint([$base.power, $base.current])\n",
        boards.base
    );
    let backoff = run("backoff", &script);
    drop(boards);
    assert!(backoff.success, "{}", backoff.stderr);
    assert_eq!(backoff.stdout, "false\nfalse\n[true, 25]\n");
    assert!(backoff.stderr.contains("it cut the power off a moment ago: it waits 2 seconds before the power can come on again"), "{}", backoff.stderr);

    // The board starts again by itself: Leah says so, its power is off, and its limit and table are
    // sent again.
    let log = std::env::temp_dir().join(format!("model-railroading-restart-log-{}", std::process::id()));
    let _ = std::fs::remove_file(&log);
    let Some(boards) = fake_boards(&["--shield", "--restart-after", "3.5", "--log", log.to_str().unwrap()]) else { return };
    let script = format!(
        "$base = dcc_base(\"{}\")\n$base.current_limit = 250\n$loco = locomotive(3)\n$loco.speed = 30\n$base.power = true\nsleep(2500)\nprint([$base.power, $base.current_limit])\n",
        boards.base
    );
    let restart = run("restart", &script);
    drop(boards);
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    assert!(restart.success, "{}", restart.stderr);
    assert_eq!(restart.stdout, "[false, 250]\n");
    assert!(restart.stderr.contains("started again by itself (its power dipped, or it was reset): the track power is off; its refresh table and current limit were sent again"), "{}", restart.stderr);
    let lines: Vec<&str> = said.lines().filter(|l| *l != "." && *l != "?").collect();
    assert_eq!(lines, ["X", "L 1500", "L 250", "K 0 03 3F 9F A3", "+", "L 250", "K 0 03 3F 9F A3", "D 0", "D 1", "X"], "{said}");
}

#[test]
fn the_dcc_throttle_example_drives_a_locomotive() {
    // The throttle on Leah's hidden test screen, with the stand-in boards (a shield, 40 mA): connected,
    // the power on, speed 30, the horn, and the emergency stop; what the base station was sent.
    let gui = std::env::var_os("LEAH_GUI").map(PathBuf::from).unwrap_or_else(|| home().join("Leah_Dev/target/all_addons/debug/leah"));
    let screen = home().join("Leah_Dev/tools/wayland_test_screen.sh");
    if !gui.is_file() || !screen.is_file() {
        eprintln!("skipped: no Leah with windows ({}) or no hidden test screen", gui.display());
        return;
    }
    let log = std::env::temp_dir().join(format!("model-railroading-throttle-log-{}", std::process::id()));
    let _ = std::fs::remove_file(&log);
    let Some(boards) = fake_boards(&["--shield", "--current", "40", "--log", log.to_str().unwrap()]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let dir = addons_folder("throttle");
    let list = dir.join("ports.txt");
    std::fs::write(&list, format!("{}\t2341\t0042\t\n", boards.base)).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dcc_throttle.leah");
    let file = dir.join("script.leah");
    std::fs::write(
        &file,
        format!(
            "include \"{}\"\ncall_later(300, function() {{\n    global $connect\n    $connect.click()\n    call_later(3500, function() {{\n        global $power, $throttle, $function_buttons\n        $power.value = true\n        $throttle.value = 30\n        $function_buttons[2].click()\n        call_later(1200, function() {{\n            global $status, $current, $speed_label\n            print($status.text)\n            print($current.text)\n            print($speed_label.text)\n            emergency_stop()\n            call_later(600, function() {{\n                quit()\n            }})\n        }})\n    }})\n}})\n",
            example.display()
        ),
    )
    .unwrap();
    let out = Command::new("sh")
        .arg(&screen)
        .arg("--gui")
        .arg(&gui)
        .arg(&file)
        .env("LEAH_3RD_PARTY_ADDONS", &dir)
        .env("LEAH_USER_ADDONS", "")
        .env("LEAH_TEST_DCC_PORTS", &list)
        .env("LEAH_TEST_INTERACT", "")
        .env("LEAH_QUIT_AFTER_MS", "12000")
        .output()
        .expect("the hidden screen runs");
    drop(boards);
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_dir_all(&dir);
    let printed = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(printed.contains("Driving locomotive 1.\nTrack current: 40 mA\nSpeed 30 of 126\n"), "{printed}");
    let lines: Vec<&str> = said.lines().filter(|l| *l != "." && *l != "?" && !l.starts_with("L ")).collect();
    // (Forward and speed 0 on connecting; then the horn (a button's click() runs at once, the switch's
    // and the slider's change events a moment later), the power, speed 30; the emergency stop.)
    assert_eq!(lines, ["X", "K 0 01 3F 80 BE", "K 0 01 3F 80 BE", "K 1 01 82 83", "+", "K 0 01 3F 9F A1", "S 00 41 41", "K 0 01 3F 81 BF", "D 0", "D 1", "X"], "{said}");
}

#[test]
fn a_decoders_settings_and_address_are_read_on_the_programming_track() {
    // The stand-in decoder on the programming track: a short address (CV 29 without bit 5, CV 1), and
    // one with a long address (bit 5, CV 17 and 18); a setting nobody answers; reading turns the main
    // track off; a simulated base station can't read.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=1,29=6,8=141"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
$base.power = true
print($base.read_address())
print([$base.read_cv(8), $base.power])
foreach [function() {{ return $base.read_cv(7) }}, function() {{ return $base.read_cv(0) }}, function() {{ return dcc_base(simulate: true).read_cv(1) }}] as $f {{
    try {{
        $f()
    }} catch $e {{
        print($e.type + ": " + $e.message)
    }}
}}
"#,
        boards.base
    );
    let read = run("read-cv", &script);
    drop(boards);
    assert!(read.success, "{}", read.stderr);
    // (Reading turns the main track off by design: no warning says the board did it by itself.)
    assert!(!read.stderr.contains("turned the track power off by itself"), "{}", read.stderr);
    assert_eq!(
        read.stdout,
        concat!(
            "[\"address\": 1, \"long\": false]\n",
            "[141, false]\n",
            "hardware_error: dcc_base.read_cv(): no decoder answered on the programming track (the shield's B screws) (baseline 0 mA, highest 0 mA): is a locomotive on it, with clean wheels? A decoder answers by pulsing its motor, so it needs one\n",
            "range_error: dcc_base.read_cv(): a setting (CV) is 1 to 1024, not 0\n",
            "hardware_error: dcc_base.read_cv(): reading a decoder's settings needs Leah's base station board with its motor shield (its channel B is the programming track)\n",
        )
    );

    let Some(boards) = fake_boards(&["--shield", "--cvs", "29=38,17=194,18=218"]) else { return };
    let long = run("read-long", &format!("$base = dcc_base(\"{}\")\nprint($base.read_address())\n", boards.base));
    drop(boards);
    assert!(long.success, "{}", long.stderr);
    // (17 = 194 = 0xC2, 18 = 218 = 0xDA: (0x02 << 8) | 0xDA = 730.)
    assert_eq!(long.stdout, "[\"address\": 730, \"long\": true]\n");
}

#[test]
fn a_reading_in_the_background_answers_through_on_read() {
    // read_address(in_background: true) gives back at once, refuses a second reading meanwhile, and
    // its answer comes as on_read; a script without windows waits for it. A setting nobody answers
    // comes as the error.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=1,29=6"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
$base.on_read = function($what, $value, $error) {{
    global $base
    print([$what, $value, $error])
    if $what == "address" {{
        $base.read_cv(8, in_background: true)
    }}
}}
print($base.read_address(in_background: true))
try {{
    $base.read_cv(1)
}} catch $e {{
    print($e.message)
}}
"#,
        boards.base
    );
    let read = run("read-background", &script);
    drop(boards);
    assert!(read.success, "{}", read.stderr);
    let lines: Vec<&str> = read.stdout.lines().collect();
    assert_eq!(lines[0], "");
    assert_eq!(lines[1], "dcc_base.read_cv(): a reading is going on already: one at a time (wait for its on_read)");
    assert_eq!(lines[2], "[\"address\", [\"address\": 1, \"long\": false], null]");
    assert!(lines[3].starts_with("[8, null, \"no decoder answered on the programming track"), "{}", read.stdout);
}

#[test]
fn the_dcc_throttle_example_reads_the_address_on_the_programming_track() {
    let gui = std::env::var_os("LEAH_GUI").map(PathBuf::from).unwrap_or_else(|| home().join("Leah_Dev/target/all_addons/debug/leah"));
    let screen = home().join("Leah_Dev/tools/wayland_test_screen.sh");
    if !gui.is_file() || !screen.is_file() {
        eprintln!("skipped: no Leah with windows ({}) or no hidden test screen", gui.display());
        return;
    }
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=5,29=6"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let dir = addons_folder("throttle-read");
    let list = dir.join("ports.txt");
    std::fs::write(&list, format!("{}\t2341\t0042\t\n", boards.base)).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dcc_throttle.leah");
    let file = dir.join("script.leah");
    std::fs::write(
        &file,
        format!(
            "include \"{}\"\ncall_later(300, function() {{\n    global $connect\n    $connect.click()\n    call_later(3500, function() {{\n        global $read\n        $read.click()\n        call_later(2500, function() {{\n            global $status, $address, $long\n            print($address.text)\n            print($long.value)\n            print($status.text)\n            quit()\n        }})\n    }})\n}})\n",
            example.display()
        ),
    )
    .unwrap();
    let out = Command::new("sh")
        .arg(&screen)
        .arg("--gui")
        .arg(&gui)
        .arg(&file)
        .env("LEAH_3RD_PARTY_ADDONS", &dir)
        .env("LEAH_USER_ADDONS", "")
        .env("LEAH_TEST_DCC_PORTS", &list)
        .env("LEAH_TEST_INTERACT", "")
        .env("LEAH_QUIT_AFTER_MS", "12000")
        .output()
        .expect("the hidden screen runs");
    drop(boards);
    let _ = std::fs::remove_dir_all(&dir);
    let printed = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(printed.contains("5\nfalse\nThe locomotive on the programming track answers to address 5."), "{printed}");
}
