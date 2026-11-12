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
            "range_error: a locomotive's `speed` is 0 to 126 (its 128 speed steps), not 127\n",
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
    assert!(stdout.contains("[\"address\": 3, \"long\": false, \"speed\": 60, \"speed_steps\": 128, \"direction\": \"forward\", \"functions\": [true,"), "{stdout}\n{stderr}");
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
        ("old", "runs an older program of Leah's (LEAH DCC BASE 5): load the new one with load_dcc_program(\"{port}\", \"base\")"),
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
            "programming_error: dcc_base.read_cv(): no decoder answered on the programming track (the shield's B screws) (baseline 0 mA, highest 0 mA): is a locomotive on it, with clean wheels? A decoder answers by pulsing its motor, so it needs one\n",
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
    assert_eq!(lines[1], "dcc_base.read_cv(): a reading or writing is going on already: one at a time (wait for its on_read or on_write)");
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

#[test]
fn a_decoders_settings_and_address_are_written_on_the_programming_track() {
    // Writing a setting, read back; the address changed to a long one and back to a short one, CV 29's
    // bit 5 following and its other bits kept; CV 7 and 8 refused; with no decoder, unconfirmed; in
    // the background through on_write.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=3,29=6,3=0"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
$base.write_cv(3, 12)
print($base.read_cv(3))
$base.write_address(730)
print([$base.read_address(), $base.read_cv(29), $base.read_cv(17), $base.read_cv(18)])
$base.write_address(5)
print([$base.read_address(), $base.read_cv(29)])
foreach [function() {{ $base.write_cv(8, 8) }}, function() {{ $base.write_cv(7, 1) }}, function() {{ $base.write_cv(3, 256) }}, function() {{ $base.write_address(200, long: false) }}] as $f {{
    try {{
        $f()
    }} catch $e {{
        print($e.type + ": " + $e.message)
    }}
}}
$base.on_write = function($what, $value, $error) {{
    print(["written", $what, $value, $error])
}}
$base.write_cv(4, 20, in_background: true)
"#,
        boards.base
    );
    let written = run("write-cv", &script);
    drop(boards);
    assert!(written.success, "{}", written.stderr);
    assert_eq!(
        written.stdout,
        concat!(
            "12\n",
            "[[\"address\": 730, \"long\": true], 38, 194, 218]\n",
            "[[\"address\": 5, \"long\": false], 6]\n",
            "range_error: dcc_base.write_cv(): CV 8 isn't written here: writing it is how many decoders are reset to their factory settings, every setting lost\n",
            "range_error: dcc_base.write_cv(): CV 7 is the decoder's version, which can't be written\n",
            "range_error: dcc_base.write_cv(): a setting's value is 0 to 255, not 256\n",
            "range_error: dcc_base.write_address(): 200 is a long address: a short one is 1 to 127\n",
            "[\"written\", 4, 20, null]\n",
        )
    );

    let Some(boards) = fake_boards(&["--shield"]) else { return };
    let none = run("write-none", &format!("$base = dcc_base(\"{}\")\ntry {{\n    $base.write_cv(3, 12)\n}} catch $e {{\n    print($e.message)\n}}\n", boards.base));
    drop(boards);
    assert!(none.success, "{}", none.stderr);
    assert_eq!(none.stdout, "dcc_base.write_cv(): the decoder didn't confirm the new value (baseline 0 mA): it may not have taken it (is the locomotive on the programming track, with clean wheels?)\n");
}

#[test]
fn the_dcc_throttle_example_reads_and_writes_a_setting() {
    let gui = std::env::var_os("LEAH_GUI").map(PathBuf::from).unwrap_or_else(|| home().join("Leah_Dev/target/all_addons/debug/leah"));
    let screen = home().join("Leah_Dev/tools/wayland_test_screen.sh");
    if !gui.is_file() || !screen.is_file() {
        eprintln!("skipped: no Leah with windows ({}) or no hidden test screen", gui.display());
        return;
    }
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=1,29=6,3=5"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let dir = addons_folder("throttle-settings");
    let list = dir.join("ports.txt");
    std::fs::write(&list, format!("{}\t2341\t0042\t\n", boards.base)).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dcc_throttle.leah");
    let file = dir.join("script.leah");
    std::fs::write(
        &file,
        format!(
            "include \"{}\"\ncall_later(300, function() {{\n    global $connect\n    $connect.click()\n    call_later(3500, function() {{\n        global $cv_read\n        $cv_read.click()\n        call_later(1500, function() {{\n            global $status, $cv_value, $cv_write, $cv_name\n            print($cv_name.text + \": \" + $cv_value.text)\n            $cv_value.text = \"12\"\n            $cv_write.click()\n            call_later(1500, function() {{\n                global $status\n                print($status.text)\n                quit()\n            }})\n        }})\n    }})\n}})\n",
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
    assert!(printed.contains("Acceleration: 5\nCV 3 is now 12 (the decoder confirmed it)."), "{printed}");
}

#[test]
fn settings_are_written_on_the_main() {
    // On the simulated track: the decoder takes a write on its second identical packet (the base
    // sends it twice), a bit write keeps the other bits, writes to one locomotive are spaced 500 ms
    // apart unless `pause:` says otherwise, and CV 7 and 8 are refused.
    let script = r#"$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$t = milliseconds()
$loco.write_cv(3, 20)
$loco.write_cv_bit(29, 5, true)
$loco.write_cv_bit(29, 1, 1)
$spaced = milliseconds() - $t
$loco.write_cv(4, 12, pause: 0)
print($base.decoder(3)["cvs"])
print([$spaced >= 1000, $spaced < 2500])
print(last($base.packets)["meaning"])
print(last($base.packets)["bytes"])
foreach [function() { $loco.write_cv(8, 8) }, function() { $loco.write_cv(7, 1) }, function() { $loco.write_cv(3, 300) }, function() { $loco.write_cv_bit(29, 8, true) }, function() { $loco.write_cv(3, 1, pause: -1) }] as $f {
    try {
        $f()
    } catch $e {
        print($e.type + ": " + $e.message)
    }
}
"#;
    let sim = run("ops-sim", script);
    assert!(sim.success, "{}", sim.stderr);
    assert_eq!(
        sim.stdout,
        concat!(
            "[3: 20, 4: 12, 29: 34]\n",
            "[true, true]\n",
            "locomotive 3: CV 4 = 12 (on the main)\n",
            "03 EC 03 0C E0\n",
            "range_error: locomotive.write_cv(): CV 8 isn't written here: writing it is how many decoders are reset to their factory settings, every setting lost\n",
            "range_error: locomotive.write_cv(): CV 7 is the decoder's version, which can't be written\n",
            "range_error: locomotive.write_cv(): a setting's value is 0 to 255, not 300\n",
            "range_error: locomotive.write_cv_bit(): a bit is 0 to 7, not 8\n",
            "range_error: locomotive.write_cv()'s `pause:` is 0 to 10000 milliseconds, not -1\n",
        )
    );

    // On a board: each write goes out as a packet sent now (three times, idle packets between).
    let log = std::env::temp_dir().join(format!("model-railroading-ops-log-{}", std::process::id()));
    let _ = std::fs::remove_file(&log);
    let Some(boards) = fake_boards(&["--log", log.to_str().unwrap()]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let board = run(
        "ops-board",
        &format!("$base = dcc_base(\"{}\")\n$base.power = true\n$loco = locomotive(1234, long: true)\n$loco.write_cv(3, 20, pause: 0)\n$loco.write_cv_bit(29, 5, true, pause: 0)\n", boards.base),
    );
    drop(boards);
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    assert!(board.success, "{}", board.stderr);
    let sent: Vec<&str> = said.lines().filter(|l| l.starts_with("S ")).collect();
    assert_eq!(sent, ["S C4 D2 EC 02 14 EC", "S C4 D2 E8 1C FD 1F"], "{said}");
}

#[test]
fn the_dcc_throttle_example_writes_a_setting_on_the_main() {
    let gui = std::env::var_os("LEAH_GUI").map(PathBuf::from).unwrap_or_else(|| home().join("Leah_Dev/target/all_addons/debug/leah"));
    let screen = home().join("Leah_Dev/tools/wayland_test_screen.sh");
    if !gui.is_file() || !screen.is_file() {
        eprintln!("skipped: no Leah with windows ({}) or no hidden test screen", gui.display());
        return;
    }
    let log = std::env::temp_dir().join(format!("model-railroading-main-log-{}", std::process::id()));
    let _ = std::fs::remove_file(&log);
    let Some(boards) = fake_boards(&["--shield", "--log", log.to_str().unwrap()]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let dir = addons_folder("throttle-main");
    let list = dir.join("ports.txt");
    std::fs::write(&list, format!("{}\t2341\t0042\t\n", boards.base)).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dcc_throttle.leah");
    let file = dir.join("script.leah");
    std::fs::write(
        &file,
        format!(
            "include \"{}\"\ncall_later(300, function() {{\n    global $connect\n    $connect.click()\n    call_later(3500, function() {{\n        global $power, $on_main, $cv_value\n        $power.value = true\n        $on_main.value = true\n        $cv_value.text = \"9\"\n        call_later(400, function() {{\n            global $cv_write, $cv_read, $status\n            print([$cv_write.enabled, $cv_read.enabled])\n            $cv_write.click()\n            print($status.text)\n            quit()\n        }})\n    }})\n}})\n",
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
    assert!(printed.contains("[true, false]\nCV 3 = 9 sent to locomotive 1 on the main"), "{printed}");
    assert!(said.lines().any(|l| l == "S 01 EC 02 09 E6"), "{said}");
}

#[test]
fn settings_are_verified_and_bits_read_and_written_on_the_programming_track() {
    // verify_cv (one question), read_cv_bit, write_cv_bit (confirmed), an indexed setting (CV 31
    // and 32 written first), `tries:`, and programming_error for the decoder's side.
    let log = std::env::temp_dir().join(format!("model-railroading-extras-log-{}", std::process::id()));
    let _ = std::fs::remove_file(&log);
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=3,29=6,3=5", "--log", log.to_str().unwrap()]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
print([$base.verify_cv(3, 5), $base.verify_cv(3, 6)])
print([$base.read_cv_bit(29, 1), $base.read_cv_bit(29, 0)])
$base.write_cv_bit(29, 5, true)
print($base.read_cv(29))
$base.write_cv(300, 77, index: [16, 3])
print([$base.read_cv(31), $base.read_cv(32), $base.read_cv(300, index: [16, 3])])
foreach [function() {{ return $base.read_cv(99, tries: 1) }}, function() {{ return $base.read_cv_bit(99, 2) }}, function() {{ $base.write_cv(300, 1, index: [16]) }}, function() {{ return $base.read_cv(1, tries: 0) }}, function() {{ $base.write_cv_bit(8, 3, true) }}] as $f {{
    try {{
        $f()
    }} catch $e {{
        print($e.type + ": " + $e.message)
    }}
}}
"#,
        boards.base
    );
    let extras = run("prog-extras", &script);
    drop(boards);
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    assert!(extras.success, "{}", extras.stderr);
    let no_answer = "no decoder answered on the programming track (the shield's B screws)";
    let lines: Vec<&str> = extras.stdout.lines().collect();
    assert_eq!(lines[..4], ["[true, false]", "[1, 0]", "38", "[16, 3, 77]"], "{}", extras.stdout);
    assert!(lines[4].starts_with(&format!("programming_error: dcc_base.read_cv(): {no_answer}")), "{}", lines[4]);
    assert!(lines[5].starts_with(&format!("programming_error: dcc_base.read_cv_bit(): {no_answer}")), "{}", lines[5]);
    assert_eq!(lines[6], "type_error: dcc_base.write_cv()'s `index:` is the two index settings' values, CV 31's and CV 32's, like [16, 0], not an array");
    assert_eq!(lines[7], "range_error: dcc_base.read_cv()'s `tries:` is 1 to 10, not 0");
    assert!(lines[8].starts_with("range_error: dcc_base.write_cv_bit(): CV 8 isn't written here"), "{}", lines[8]);
    // (One try only for CV 99 with `tries: 1`; three for the bit.)
    assert_eq!(said.lines().filter(|l| *l == "R 99").count(), 1, "{said}");
    assert_eq!(said.lines().filter(|l| *l == "U 99 2").count(), 3, "{said}");
}

#[test]
fn a_decoder_is_identified_and_reset_only_when_asked() {
    // identify_decoder (the maker by the NMRA's list, CV 29 by name, a forgotten consist warned of);
    // reset_decoder only with confirm: true, by the maker's own reset; an unknown maker's refused
    // unless the script names the reset; decoder_maker_name, cv29_parts, make_cv29.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=7,29=38,8=141,7=71,19=5,3=9,17=194,18=218,253=1,254=2,255=17,256=36,105=1,106=3"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
$id = $base.identify_decoder()
print([$id["maker"], $id["maker_id"], $id["version"], $id["consist"], $id["cv29"]["long_address"], $id["cv29"]["speed_steps"]])
print([$id["family"], $id["firmware"], $id["product"]])
try {{
    $base.reset_decoder()
}} catch $e {{
    print($e.type + ": " + $e.message)
}}
print($base.reset_decoder(confirm: true))
print([$base.read_cv(3), $base.read_cv(19)])
print([decoder_maker_name(153), decoder_maker_name(250), cv29_parts(38)["long_address"], make_cv29(long_address: true), make_cv29(speed_steps: 14, analog: false)])
"#,
        boards.base
    );
    let id = run("identify", &script);
    drop(boards);
    assert!(id.success, "{}", id.stderr);
    assert_eq!(
        id.stdout,
        concat!(
            "[\"Throttle-Up (Soundtraxx)\", 141, 71, 5, true, 28]\n",
            "[\"Tsunami2\", \"1.3\", [\"customer\": 1, \"sound\": \"diesel\", \"revision\": 2, \"id\": 36]]\n",
            "programming_error: dcc_base.reset_decoder() sets every one of the decoder's settings back to the maker's: give confirm: true to do it\n",
            "[\"maker\": \"Throttle-Up (Soundtraxx)\", \"cv\": 8, \"value\": 8, \"address\": 3, \"long\": false]\n",
            "[0, 0]\n",
            "[\"Train Control Systems\", null, true, 38, 0]\n",
        )
    );
    assert!(id.stderr.contains("the decoder's consist address (CV 19) is 5"), "{}", id.stderr);

    // An older SoundTraxx family: CV 105 the firmware, CV 106 the set of factory settings.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=1,29=6,8=141,7=82,19=0,105=12,106=40"]) else { return };
    let older = run("identify-older", &format!("$base = dcc_base(\"{}\")\n$id = $base.identify_decoder()\nprint([$id[\"family\"], $id[\"firmware\"], $id[\"product\"]])\n", boards.base));
    drop(boards);
    assert!(older.success, "{}", older.stderr);
    assert_eq!(older.stdout, "[\"Sound Value, RTR, Mainline\", \"12\", [\"settings_set\": 40]]\n");
    // (0 in both, as the owner's Sound Value decoder gave: unknown, not firmware "0".)
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=1,29=6,8=141,7=82,19=0,105=0,106=0"]) else { return };
    let zero = run("identify-zero", &format!("$base = dcc_base(\"{}\")\n$id = $base.identify_decoder()\nprint([$id[\"firmware\"], $id[\"product\"]])\n", boards.base));
    drop(boards);
    assert!(zero.success, "{}", zero.stderr);
    assert_eq!(zero.stdout, "[null, [\"settings_set\": null]]\n");

    // A maker whose reset isn't known: refused, unless the script gives it.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=7,29=6,8=99"]) else { return };
    let unknown = run(
        "reset-unknown",
        &format!("$base = dcc_base(\"{}\")\ntry {{\n    $base.reset_decoder(confirm: true)\n}} catch $e {{\n    print($e.message)\n}}\nprint($base.reset_decoder(confirm: true, cv: 8, value: 8)[\"address\"])\n", boards.base),
    );
    drop(boards);
    assert!(unknown.success, "{}", unknown.stderr);
    assert_eq!(unknown.stdout, "dcc_base.reset_decoder(): no reset is known for Lenz Elektronik GmbH (CV 8 = 99): give the maker's own reset from its manual, as cv: and value:\n3\n");
}

#[test]
fn a_locked_decoder_is_found_unlocked_and_locked() {
    // NMRA decoder lock (CV 15 the key, CV 16 the lock): found by asking, without writing; a locked
    // decoder doesn't take a write; unlocked, it does; lock_decoder(0) takes the lock away.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=3,29=6,3=5,15=0,16=2"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
print($base.find_decoder_locks())
try {{
    $base.write_cv(3, 9)
}} catch $e {{
    print($e.type)
}}
$base.unlock_decoder(2)
$base.write_cv(3, 9)
print($base.read_cv(3))
$base.lock_decoder(0)
print([$base.find_decoder_locks(most: 3), $base.read_cv(15), $base.read_cv(16)])
"#,
        boards.base
    );
    let locks = run("decoder-locks", &script);
    drop(boards);
    assert!(locks.success, "{}", locks.stderr);
    assert_eq!(locks.stdout, "[2]\nprogramming_error\n9\n[[], 0, 0]\n");
}

#[test]
fn decoder_definitions_name_the_settings_and_do_the_bit_work() {
    // The NMRA's standard set built in; a definition of a script's own over it, or from a JSON file;
    // values to and from the CVs' bytes; the mistakes refused with what to do.
    let dir = std::env::temp_dir().join(format!("model-railroading-definition-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("mine.json");
    std::fs::write(&file, r#"{"name": "file one", "maker": 141, "versions": [82], "settings": [{"name": "Master volume", "cv": 128, "group": "Sound", "max": 192}]}"#).unwrap();
    let script = format!(
        r#"$d = decoder_definition("nmra")
print([$d.name, count($d.settings), $d.groups[0], $d.cvs[0]])
print($d.setting("Speed steps"))
print([$d.value_of("Address used", [29: 38]), $d.value_of("Long address", [17: 194, 18: 218]), $d.value_of("Short address", [29: 6])])
print($d.cvs_of("Address used", "short", [29: 38]))
print($d.cvs_of("Long address", 730))
print([$d.is_shown("Speed step 3", ["Speed curve": "three points"]), $d.is_shown("Speed step 3", [])])
$mine = decoder_definition(name: "my sound decoder", maker: 141, versions: [82], settings: [_
    ["name": "Master volume", "cv": 128, "group": "Sound"], _
    ["name": "Bell type", "cv": 115, "bits": [0, 2], "choices": [0: "brass", 1: "steel", 2: "electronic"], "group": "Sound"], _
    ["name": "Acceleration", "cv": 3, "max": 100, "group": "Motor"]])
print([count($mine.settings), $mine.maker, $mine.versions, $mine.setting("Acceleration")["max"], $mine.value_of("Bell type", [115: 0x0A])])
$f = read_decoder_definition_file("{file}")
print([$f.name, $f.setting("Master volume")["max"], $f.groups[count($f.groups) - 1]])
foreach [_
    function() {{ decoder_definition("esu") }}, _
    function() {{ $d.cvs_of("Short address", 200) }}, _
    function() {{ $d.cvs_of("Direction reversed", true) }}, _
    function() {{ $d.setting("Volume") }}, _
    function() {{ decoder_definition(name: "x", settings: [["name": "Mute", "cv": 29, "bit": 5]]) }}, _
    function() {{ decoder_definition(name: "x", settings: [["name": "Mute", "cv": 129, "bit": 1, "colour": "red"]]) }}, _
    function() {{ decoder_definition(name: "x", settings: [["name": "Mute", "cv": 129, "shown_when": ["Volume", 0]]]) }}] as $f {{
    try {{
        $f()
    }} catch $e {{
        print($e.type + ": " + $e.message)
    }}
}}
"#,
        file = file.display()
    );
    let out = run("definitions", &script);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.success, "{}", out.stderr);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(
        lines,
        [
            "[\"nmra\", 57, \"Address\", 1]",
            "[\"name\": \"Speed steps\", \"cv\": 29, \"bit\": 1, \"kind\": \"choice\", \"choices\": [\"14\", \"28/128\"], \"group\": \"Configuration\", \"help\": \"How many speed steps the decoder takes from the throttle.\"]",
            "[\"long\", 730, null]",
            "[29: 6]",
            "[17: 194, 18: 218]",
            "[false, true]",
            "[59, 141, [82], 100, \"electronic\"]",
            "[\"file one\", 192, \"Sound\"]",
            "range_error: decoder_definition(): there is no built-in definition named \"esu\" (there are \"nmra\", \"soundtraxx_tsunami2_diesel\")",
            "range_error: decoder_definition.cvs_of(): the setting \"Short address\" takes a whole number from 1 to 127, not 200",
            "range_error: decoder_definition.cvs_of(): CV 29's other bits aren't known: read it first (the setting \"Direction reversed\" is only part of it)",
            "range_error: decoder_definition.setting(): the definition \"nmra\" has no setting named \"Volume\"",
            "type_error: decoder_definition(): the settings \"Address used\" and \"Mute\" both use bits of CV 29",
            "type_error: decoder_definition(): the setting \"Mute\" has the key \"colour\", which a setting doesn't have (it takes name, cv, bit, bits, kind, min, max, choices, group, help, default, read_only, write_all, shown_when, index)",
            "type_error: decoder_definition(): the setting \"Mute\" depends on \"Volume\", but there is no setting named that",
        ]
    );
}

#[test]
fn a_programmer_reads_edits_and_writes_a_decoder_on_the_programming_track() {
    // decoder_programmer on the programming track: read_all in the background (a setting the decoder
    // doesn't have left unknown, in problems), each setting's state, edits written by
    // write_changes, write_all refused while a CV isn't known, load_cvs never taking CV 8.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=3,3=5,29=6,8=141"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
$d = decoder_definition(name: "small", standard: false, settings: [_
    ["name": "Short address", "cv": 1, "min": 1, "max": 127], _
    ["name": "Acceleration", "cv": 3], _
    ["name": "Speed steps", "cv": 29, "bit": 1, "choices": [0: "14", 1: "28/128"]], _
    ["name": "Analog running", "cv": 29, "bit": 2], _
    ["name": "Volume", "cv": 128], _
    ["name": "Maker", "cv": 8, "read_only": true]])
$p = decoder_programmer($d, $base)
$steps = 0
$p.on_progress = function($done, $total, $cv) {{
    global $steps
    $steps++
}}
$p.on_done = function($what, $error) {{
    global $p, $steps
    print([$what, $error, $steps])
    $steps = 0
    if $what == "read_all" {{
        print($p.values)
        print($p.states)
        print($p.problems)
        $p.set_value("Acceleration", 9)
        $p.set_value("Analog running", false)
        print([$p.state("Acceleration"), $p.state("Speed steps"), $p.value("Analog running"), $p.cvs[29]])
        $p.write_changes()
    }} else if $what == "write_changes" {{
        print([$p.state("Acceleration"), $p.state("Analog running"), $p.state("Short address")])
        try {{
            $p.write_all()
        }} catch $e {{
            print($e.type + ": " + $e.message)
        }}
        $p.load_cvs([8: 3, 128: 7])
        print([$p.state("Volume"), $p.value("Maker")])
        $p.write_all()
    }} else {{
        print($p.states)
        print([$base.read_cv(128), $base.read_cv(3), $base.read_cv(8)])
    }}
}}
$p.read_all()
try {{
    $p.read_all()
}} catch $e {{
    print($e.message)
}}
"#,
        boards.base
    );
    let out = run("programmer", &script);
    drop(boards);
    assert!(out.success, "{}", out.stderr);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(
        lines,
        [
            "decoder_programmer.read_all(): the programmer is busy reading or writing: one at a time (wait for its on_done, or cancel())",
            "[\"read_all\", \"1 CV couldn't be read (the decoder may not have it): see problems\", 5]",
            "[\"Short address\": 3, \"Acceleration\": 5, \"Speed steps\": \"28/128\", \"Analog running\": true, \"Volume\": null, \"Maker\": 141]",
            "[\"Short address\": \"read\", \"Acceleration\": \"read\", \"Speed steps\": \"read\", \"Analog running\": \"read\", \"Volume\": \"unknown\", \"Maker\": \"read\"]",
            "[128: \"the decoder didn't answer for this CV (it may not have it)\"]",
            "[\"edited\", \"read\", false, 2]",
            "[\"write_changes\", null, 2]",
            "[\"written\", \"written\", \"read\"]",
            "programming_error: decoder_programmer.write_all(): CV 128 isn't known: read everything first, or load the CVs (load_cvs)",
            "[\"loaded\", 141]",
            "[\"write_all\", null, 4]",
            "[\"Short address\": \"written\", \"Acceleration\": \"written\", \"Speed steps\": \"written\", \"Analog running\": \"written\", \"Volume\": \"written\", \"Maker\": \"read\"]",
            "[7, 9, 141]",
        ]
    );
}

#[test]
fn a_programmer_writes_changes_on_the_main() {
    // Given a locomotive, the programmer writes on the main (the simulated decoder keeps them), and
    // can't read; a setting that is part of a CV needs the CV known first.
    let script = r#"$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$p = decoder_programmer(decoder_definition("nmra"), $loco)
$p.set_value("Acceleration", 12)
$p.set_value("Long address", 730)
try {
    $p.set_value("Analog running", true)
} catch $e {
    print($e.message)
}
try {
    $p.read_all()
} catch $e {
    print($e.type + ": " + $e.message)
}
$p.on_done = function($what, $error) {
    global $base, $p
    print([$what, $error, $p.main_track])
    print($base.decoder(3)["cvs"])
    print([$p.state("Acceleration"), $p.state("Long address"), $p.state("Deceleration")])
}
$p.write_changes()
"#;
    let out = run("programmer-main", script);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        concat!(
            "decoder_programmer.set_value(): CV 29's other bits aren't known: read it first (the setting \"Analog running\" is only part of it)\n",
            "programming_error: decoder_programmer.read_all(): on the main, a decoder's settings can only be written: read them on the programming track\n",
            "[\"write_changes\", null, true]\n",
            "[3: 12, 17: 194, 18: 218]\n",
            "[\"written\", \"written\", \"unknown\"]\n",
        )
    );
}

#[test]
fn a_programmer_with_no_locomotive_stops_at_once() {
    // Nothing answering the first setting: no use asking about the other 51 CVs.
    let Some(boards) = fake_boards(&["--shield"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
$p = decoder_programmer(decoder_definition("nmra"), $base)
$p.on_progress = function($done, $total, $cv) {{
    print([$done, $total, $cv])
}}
$p.on_done = function($what, $error) {{
    global $p
    print([$what, $error, $p.busy])
}}
$p.read_all(tries: 1)
"#,
        boards.base
    );
    let out = run("programmer-nobody", &script);
    drop(boards);
    assert!(out.success, "{}", out.stderr);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(lines[0], "[1, 52, 1]");
    assert!(lines[1].starts_with("[\"read_all\", \"CV 1: no decoder answered on the programming track"), "{}", out.stdout);
    assert!(lines[1].ends_with(", false]"), "{}", out.stdout);
    assert_eq!(lines.len(), 2, "{}", out.stdout);
}

#[test]
fn a_programmer_keeps_edits_and_loads_where_it_should() {
    // A reading of some settings keeps the changes to the others that share their CV; a decoder
    // without the first CV asked isn't taken for no decoder (CV 8 tells); a loaded CV stays loaded
    // when one of its settings is changed.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=3,29=6,8=141"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
$d = decoder_definition(name: "small", standard: false, settings: [_
    ["name": "Start voltage", "cv": 2], _
    ["name": "Short address", "cv": 1, "min": 1, "max": 127], _
    ["name": "Speed steps", "cv": 29, "bit": 1, "choices": [0: "14", 1: "28/128"]], _
    ["name": "Analog running", "cv": 29, "bit": 2]])
$p = decoder_programmer($d, $base)
$p.on_done = function($what, $error) {{
    global $p
    print([$what, $error])
    if $what == "read_all" {{
        print([$p.state("Start voltage"), $p.problems[2], $p.value("Short address")])
        $p.set_value("Analog running", false)
        $p.read("Speed steps")
    }} else {{
        print([$p.value("Analog running"), $p.state("Analog running"), $p.state("Speed steps"), $p.cvs[29]])
        $q = decoder_programmer($d, $base)
        $q.load_cvs([29: 34, 8: 99])
        $q.set_value("Analog running", true)
        print([$q.state("Speed steps"), $q.state("Analog running"), $q.cvs[29], $q.cvs[8]])
    }}
}}
$p.read_all(tries: 1)
"#,
        boards.base
    );
    let out = run("programmer-keeps", &script);
    drop(boards);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        concat!(
            "[\"read_all\", \"1 CV couldn't be read (the decoder may not have it): see problems\"]\n",
            "[\"unknown\", \"the decoder didn't answer for this CV (it may not have it)\", 3]\n",
            "[\"read\", null]\n",
            "[false, \"edited\", \"read\", 2]\n",
            "[\"loaded\", \"edited\", 38, null]\n",
        )
    );
}

#[test]
fn a_programmer_on_the_main_refuses_to_change_the_address() {
    // On the main, a change of the address the decoder answers to would leave the writes after it
    // unheard: refused before anything is sent. Writing the same address is fine.
    let script = r#"$base = dcc_base(simulate: true)
$base.power = true
$p = decoder_programmer(decoder_definition("nmra"), locomotive(3))
$p.set_value("Short address", 5)
$p.set_value("Acceleration", 9)
try {
    $p.write_changes()
} catch $e {
    print($e.message)
}
$q = decoder_programmer(decoder_definition("nmra"), locomotive(3))
$q.load_cvs([29: 38])
try {
    $q.write_all()
} catch $e {
    print($e.type)
}
$r = decoder_programmer(decoder_definition("nmra"), locomotive(3))
$r.set_value("Short address", 3)
$r.on_done = function($what, $error) {
    global $base
    print([$what, $error, $base.decoder(3)["cvs"]])
}
$r.write_changes()
"#;
    let out = run("programmer-main-address", script);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        concat!(
            "decoder_programmer.write_changes(): on the main, the locomotive's address can't change (the writes after it would go unheard): change it on the programming track\n",
            "programming_error\n",
            "[\"write_changes\", null, [1: 3]]\n",
        )
    );
}

/// Runs an example on the hidden test screen with stand-in boards, after `driver` (Leah lines run as
/// the example starts); what it printed, or None when there is no Leah with windows or no screen.
fn run_example_on_screen(test: &str, example: &str, boards: &FakeBoards, driver: &str, quit_after_ms: u32) -> Option<String> {
    run_example_on_screen_with(test, example, Some(boards), driver, quit_after_ms, &[])
}

/// As `run_example_on_screen`, with no boards or with more of the environment set.
fn run_example_on_screen_with(test: &str, example: &str, boards: Option<&FakeBoards>, driver: &str, quit_after_ms: u32, env: &[(&str, &str)]) -> Option<String> {
    let gui = std::env::var_os("LEAH_GUI").map(PathBuf::from).unwrap_or_else(|| home().join("Leah_Dev/target/all_addons/debug/leah"));
    let screen = home().join("Leah_Dev/tools/wayland_test_screen.sh");
    if !gui.is_file() || !screen.is_file() {
        eprintln!("skipped: no Leah with windows ({}) or no hidden test screen", gui.display());
        return None;
    }
    let dir = addons_folder(test);
    let list = dir.join("ports.txt");
    std::fs::write(&list, boards.map_or(String::new(), |b| format!("{}\t2341\t0042\t\n", b.base))).unwrap();
    let file = dir.join("script.leah");
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join(example);
    std::fs::write(&file, format!("include \"{}\"\n{driver}", example.display())).unwrap();
    let out = Command::new("sh")
        .arg(&screen)
        .arg("--gui")
        .arg(&gui)
        .arg(&file)
        .env("LEAH_3RD_PARTY_ADDONS", &dir)
        .env("LEAH_USER_ADDONS", "")
        .env("LEAH_TEST_DCC_PORTS", &list)
        .env("LEAH_TEST_INTERACT", "")
        .env("LEAH_QUIT_AFTER_MS", quit_after_ms.to_string())
        .envs(env.iter().copied())
        .output()
        .expect("the hidden screen runs");
    let _ = std::fs::remove_dir_all(&dir);
    Some(format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

#[test]
fn the_decoder_workshop_identifies_a_decoder_and_reads_it() {
    // Identify picks the decoder's definition (a Tsunami2 diesel); Read all fills the rows by name,
    // and the speed table's rows hide while the decoder uses three points.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=3,29=6,8=141,7=71,19=0,105=1,106=4,253=1,254=2,255=8,256=36"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let driver = "call_later(300, function() {\n    global $connect\n    $connect.click()\n    call_later(3500, function() {\n        global $identify\n        $identify.click()\n        call_later(4500, function() {\n            global $status, $definitions\n            print($definitions.selected_row_tag)\n            print($status.text)\n            quit()\n        })\n    })\n})\n";
    let Some(printed) = run_example_on_screen("workshop-identify", "decoder_workshop.leah", &boards, driver, 15000) else { return };
    drop(boards);
    assert!(printed.contains("soundtraxx_tsunami2_diesel\nThe decoder: Throttle-Up (Soundtraxx), Tsunami2. Its definition: soundtraxx_tsunami2_diesel. Now Read all."), "{printed}");

    // Every one of the NMRA set's CVs there, so Read all takes about ten seconds.
    let mut cvs: Vec<String> = vec!["1=5".into(), "29=6".into(), "8=141".into(), "7=82".into(), "3=12".into()];
    for cv in [2, 4, 5, 6, 11, 15, 16, 17, 18, 19, 21, 22, 23, 24, 65, 66, 95, 105, 106].into_iter().chain(67..=94) {
        cvs.push(format!("{cv}=0"));
    }
    let Some(boards) = fake_boards(&["--shield", "--cvs", &cvs.join(",")]) else { return };
    let driver = "call_later(300, function() {\n    global $connect\n    $connect.click()\n    call_later(3500, function() {\n        global $read_all\n        $read_all.click()\n        call_later(14000, function() {\n            global $status, $settings\n            print($status.text)\n            print([$settings.find_row(\"Short address\").value, $settings.find_row(\"Acceleration\").value, $settings.find_row(\"Address used\").value, $settings.find_row(\"Analog running\").value, $settings.find_row(\"Speed step 1\").visible])\n            quit()\n        })\n    })\n})\n";
    let Some(printed) = run_example_on_screen("workshop-read", "decoder_workshop.leah", &boards, driver, 25000) else { return };
    drop(boards);
    assert!(printed.contains("Read.\n[5, 12, \"short\", true, false]"), "{printed}");

    // Saved to the roster, then loaded back: the loaded settings yellow, waiting for Write all.
    let Some(boards) = fake_boards(&["--shield", "--cvs", &cvs.join(",")]) else { return };
    let driver = "call_later(300, function() {\n    global $connect\n    $connect.click()\n    call_later(3500, function() {\n        global $read_all\n        $read_all.click()\n        call_later(14000, function() {\n            global $entry_name, $save, $status, $roster, $load, $programmer\n            $entry_name.text = \"Test switcher\"\n            $save.click()\n            print($status.text)\n            $e = $roster.entry(\"Test switcher\")\n            print([$e.address, $e.long, $e.decoder, $e.cvs[3]])\n            $load.click()\n            print([$programmer.state(\"Acceleration\"), $programmer.value(\"Acceleration\")])\n            quit()\n        })\n    })\n})\n";
    let Some(printed) = run_example_on_screen("workshop-roster", "decoder_workshop.leah", &boards, driver, 25000) else { return };
    drop(boards);
    assert!(printed.contains("Test switcher was saved in the roster, with 52 settings.\n[5, false, \"nmra\", 12]\n[\"loaded\", 12]"), "{printed}");
}

#[test]
fn a_programmer_with_nothing_to_write_leaves_the_track_alone() {
    // write_changes() with no changes: done at once, nothing sent, the main track's power as it was.
    let Some(boards) = fake_boards(&["--shield", "--cvs", "1=3,29=6"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let script = format!(
        r#"$base = dcc_base("{}")
$base.power = true
$p = decoder_programmer(decoder_definition("nmra"), $base)
$p.on_done = function($what, $error) {{
    global $base
    print([$what, $error, $base.power])
}}
$p.write_changes()
"#,
        boards.base
    );
    let out = run("programmer-empty", &script);
    drop(boards);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "[\"write_changes\", null, true]\n");
}

#[test]
fn a_roster_keeps_each_locomotive_and_drives_one() {
    // The roster: entries made, changed, saved to their JSON file and read back; function labels and
    // the owner's own fields; a programmer's CVs kept in an entry and loaded back; groups and
    // addresses found; exported and imported (a name taken gets a number); locomotive($entry).
    let dir = std::env::temp_dir().join(format!("model-railroading-roster-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("roster.json");
    let export = dir.join("export.json");
    let script = format!(
        r#"$r = roster("{file}")
print([$r.count, $r.changed])
$up = $r.add("UP 792", road_name: "Union Pacific", road_number: "792", address: 792, decoder: "soundtraxx_tsunami2_diesel", groups: ["Diesel", "Union Pacific"])
$up.set_function(0, "Headlight")
$up.set_function(2, "Horn", momentary: true)
$up.set_attribute("Era", "1950s")
$up.speed_limit = 100
$sw = $r.add("Switcher", address: 3, groups: ["Diesel"])
$base = dcc_base(simulate: true)
$base.power = true
$p = decoder_programmer(decoder_definition("nmra"), locomotive($sw))
$p.set_value("Acceleration", 9)
$p.set_value("Deceleration", 4)
$sw.cvs = $p.cvs
print([$up.long, $up.functions[2], $up.attribute("era"), $sw.cvs, $r.changed])
$r.save()
$again = roster("{file}")
$u = $again.entry("up 792")
print([$again.names, $u.road_name, $u.address, $u.long, $u.functions[0]["label"], $u.speed_limit, $u.attributes, $u.groups])
print([count($again.entries(group: "diesel")), $again.entries(address: 3)[0].name, $again.groups, $again.entry("Big Boy")])
$q = decoder_programmer(decoder_definition("nmra"), $base)
$q.load_cvs($again.entry("Switcher").cvs)
print([$q.value("Acceleration"), $q.state("Acceleration")])
print($again.export_entries("{export}", names: ["Switcher"]))
print([$again.import_entries("{export}"), $again.count])
$again.remove("Switcher (2)")
$loco = locomotive($u)
$loco.speed = 40
print([$base.decoder(792, long: true)["speed"], $loco.long])
foreach [_
    function() {{ $again.add("UP 792") }}, _
    function() {{ $u.address = 0 }}, _
    function() {{ $u.long = false }}, _
    function() {{ $u.speed_limit = 127 }}, _
    function() {{ $u.set_function(70, "x") }}, _
    function() {{ $u.name = "switcher" }}, _
    function() {{ $again.remove("Big Boy") }}, _
    function() {{ locomotive($u, long: false) }}] as $f {{
    try {{
        $f()
    }} catch $e {{
        print($e.type + ": " + $e.message)
    }}
}}
"#,
        file = file.display(),
        export = export.display()
    );
    let out = run("roster", &script);
    let saved = std::fs::read_to_string(&file).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.success, "{}", out.stderr);
    assert!(saved.contains("\"road_name\": \"Union Pacific\""), "{saved}");
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(
        lines,
        [
            "[0, false]",
            "[true, [\"label\": \"Horn\", \"momentary\": true], \"1950s\", [3: 9, 4: 4], true]",
            "[[\"UP 792\", \"Switcher\"], \"Union Pacific\", 792, true, \"Headlight\", 100, [\"Era\": \"1950s\"], [\"Diesel\", \"Union Pacific\"]]",
            "[2, \"Switcher\", [\"Diesel\", \"Union Pacific\"], null]",
            "[9, \"loaded\"]",
            "1",
            "[[\"Switcher (2)\"], 3]",
            "[40, true]",
            "range_error: roster.add(): the roster has an entry named \"UP 792\" already",
            "range_error: a roster_entry's `address` is 1 to 10239, not 0",
            "range_error: a roster_entry: 792 is a long address, so `long` is true",
            "range_error: a roster_entry's `speed_limit` is a speed step, 1 to 126, not 127",
            "range_error: roster_entry.set_function(): a function is 0 to 68 (F0 the lights), not 70",
            "range_error: the roster has an entry named \"switcher\" already",
            "range_error: roster.remove(): the roster has no entry named \"Big Boy\"",
            "type_error: locomotive(): a roster entry says whether its address is long: no `long:`",
        ]
    );
}

#[test]
fn a_roster_without_a_path_is_shared_by_every_app() {
    // Beside the scripts' own data folders (in the app's folder, as Leah keeps every app's data):
    // data/model_railroading/roster.json, its folder made on saving.
    let out = run("roster-default", "$r = roster()\n$r.add(\"Switcher\")\n$r.save()\nprint([file_name($r.path), file_name(file_path($r.path)), file_name(file_path($r.path, levels: 2)), file_exists($r.path)])\nprint(file_name(file_path(data_path())) == file_name(file_path($r.path, levels: 2)))\n");
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "[\"roster.json\", \"model_railroading\", \"data\", true]\ntrue\n");
}

#[test]
fn a_throttle_has_speed_steps_a_limit_and_momentary_keys() {
    // Speed in a locomotive's own steps (28 and 14 in NMRA's basic packets, read back by the simulated
    // decoders as their CV 29 says), speed_percent, a roster entry's speed limit, labels, and
    // momentary horn; press and release; every locomotive, stop_all(), on_locomotive_changed.
    let script = r#"$base = dcc_base(simulate: true)
$base.power = true
$changes = []
$base.on_locomotive_changed = function($loco) {
    global $changes
    $changes[] = $loco.address
}
$a = locomotive(5, speed_steps: 28)
$a.speed = 14
print([$base.decoder(5)["speed"], $base.decoder(5)["speed_steps"], $a.speed_percent, last($base.packets)["bytes"]])
$a.speed_percent = 100
print([$a.speed, $base.decoder(5)["speed"]])
$b = locomotive(6)
$b.write_cv(29, 4)
$c = locomotive(6, speed_steps: 14)
$c.set_function(0, true)
$c.speed = 7
$d = $base.decoder(6)
print([$d["speed"], $d["speed_steps"], $d["functions"][0]])
$r = roster(join_path(make_temp_path(), "roster.json"))
$e = $r.add("Switcher", address: 7, speed_limit: 63)
$e.set_function(0, "Headlight")
$e.set_function(2, "Horn", momentary: true)
$sw = locomotive($e)
$sw.speed = 100
print([$sw.speed, $sw.speed_limit, $sw.labels, $sw.momentary, $sw.entry.name])
$sw.press_function(2)
$on = $sw.function(2)
$sw.release_function(2)
$sw.press_function(1)
$sw.release_function(1)
print([$on, $sw.function(2), $sw.function(1)])
$sw.press_function(1)
print([$sw.function(1), count($base.locomotives)])
$base.stop_all()
print([$a.speed, $c.speed, $sw.speed, $base.decoder(7)["speed"]])
call_later(100, function() {
    global $changes
    print(count($changes))
})
foreach [function() { $a.speed = 29 }, function() { $a.speed_percent = 101 }, function() { locomotive(8, speed_steps: 64) }, function() { $sw.speed_limit = 127 }] as $f {
    try {
        $f()
    } catch $e {
        print($e.type + ": " + $e.message)
    }
}
"#;
    let out = run("throttle-extras", script);
    assert!(out.success, "{}", out.stderr);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(
        lines,
        [
            "[14, 28, 50, \"05 78 7D\"]",
            "[28, 28]",
            "[7, 14, true]",
            "[63, 63, [0: \"Headlight\", 2: \"Horn\"], [2], \"Switcher\"]",
            "[true, false, true]",
            "[false, 4]",
            "[0, 0, 0, 0]",
            "range_error: a locomotive's `speed` is 0 to 28 (its 28 speed steps), not 29",
            "range_error: a locomotive's `speed_percent` is 0 to 100, not 101",
            "range_error: locomotive()'s `speed_steps:` is 14, 28, or 128, as its decoder is set (CV 29), not 64",
            "range_error: a locomotive's `speed_limit` is a speed step, 1 to 126, not 127",
            "13",
        ]
    );
}

#[test]
fn the_dcc_throttle_example_drives_a_roster_locomotive() {
    // A locomotive picked from the roster: its keys' labels, its momentary horn as a plain button, and
    // its speed limit holding the slider back.
    let Some(boards) = fake_boards(&["--shield"]) else {
        eprintln!("skipped: no python3 for the stand-in boards");
        return;
    };
    let driver = "call_later(300, function() {\n    global $connect\n    $connect.click()\n    call_later(3500, function() {\n        global $roster, $picked, $loco, $function_buttons, $throttle, $speed_label, $address\n        $e = $roster.add(\"Switcher\", address: 7, speed_limit: 63)\n        $e.set_function(2, \"Horn\", momentary: true)\n        $picked = \"Switcher\"\n        $address.text = \"7\"\n        take_locomotive()\n        print([$loco.address, $function_buttons[2].text, $function_buttons[2].button_type, $function_buttons[1].button_type, $throttle.maximum_value])\n        $throttle.value = 100\n        speed_from_slider()\n        print([$loco.speed, $throttle.value, $speed_label.text])\n        $loco.press_function(2)\n        $on = $loco.function(2)\n        $loco.release_function(2)\n        print([$on, $loco.function(2)])\n        quit()\n    })\n})\n";
    let Some(printed) = run_example_on_screen("throttle-roster", "dcc_throttle.leah", &boards, driver, 12000) else { return };
    drop(boards);
    assert!(printed.contains("[7, \"F2 Horn\", \"normal\", \"toggle\", 126]\n[63, 63, \"Speed 63 of 126 (its limit: 63)\"]\n[true, false]"), "{printed}");
}

#[test]
fn the_gamepad_throttle_example_drives_with_a_pad() {
    // A recorded pad (LEAH_TEST_GAMEPAD): the stick up for half a second speeds the locomotive up and,
    // let go, the speed stays; A sounds the horn while held; Start stops every locomotive at once.
    // On the simulated base station.
    let dir = std::env::temp_dir().join(format!("model-railroading-pad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pad = dir.join("pad.json");
    std::fs::write(
        &pad,
        // (Each event's time from the start, in milliseconds.)
        r#"[{"after": 0, "event": "connected", "name": "Test pad"},
{"after": 1500, "event": "axis", "axis": "left_y", "value": -1},
{"after": 2000, "event": "axis", "axis": "left_y", "value": 0},
{"after": 2500, "event": "pressed", "button": "south"},
{"after": 2800, "event": "released", "button": "south"},
{"after": 3100, "event": "pressed", "button": "start"},
{"after": 3200, "event": "released", "button": "start"}]"#,
    )
    .unwrap();
    let driver = "call_later(300, function() {\n    global $connect, $base\n    $connect.click()\n    $base.power = true\n    $horn = []\n    call_later(2100, function() {\n        global $loco, $speed\n        print([$loco.speed > 10, $loco.speed < 60, $speed.text])\n    })\n    call_later(2350, function() {\n        global $loco\n        print($loco.function(2))\n    })\n    call_later(3400, function() {\n        global $loco, $base, $pad\n        print([$loco.function(2), $loco.speed, $base.decoder(3)[\"speed\"], $pad.name])\n        quit()\n    })\n})\n";
    let Some(printed) = run_example_on_screen_with("gamepad", "gamepad_throttle.leah", None, driver, 9000, &[("LEAH_TEST_GAMEPAD", pad.to_str().unwrap())]) else { return };
    let _ = std::fs::remove_dir_all(&dir);
    assert!(printed.contains("[true, true, \"Speed "), "{printed}");
    assert!(printed.contains("\ntrue\n[false, 0, 0, \"Test pad\"]"), "{printed}");
}

#[test]
fn consists_run_several_locomotives_as_one() {
    // A software consist: each member its own packets, a reversed one the other way, in its own steps.
    // An advanced one: consist control into CV 19 (reversed in bit 7), the speed to the consist
    // address, which the members obey and their own addresses no longer; functions to the lead, to
    // all, or to the consist address (as CV 21 and 22 let them); break_up() takes them out, stopped.
    let script = r#"$base = dcc_base(simulate: true)
$base.power = true
$a = locomotive(3)
$b = locomotive(5, speed_steps: 28)
$soft = consist(name: "Doubleheader")
$soft.add($a)
$soft.add($b, reversed: true)
$soft.speed = 63
print([$a.speed, $a.direction, $b.speed, $b.direction, $soft.kind, count($soft.members)])
$soft.set_function(0, true)
print([$a.function(0), $b.function(0)])
$soft.set_function(1, true, to: "all")
print([$a.function(1), $b.function(1)])
$soft.stop()
$soft.remove($b)
print([count($soft.members), $b.speed])

$c = locomotive(7)
$d = locomotive(8)
$d.write_cv(21, 2)
$d.write_cv(22, 3)
$adv = consist(address: 74)
$adv.add($c)
$adv.add($d, reversed: true)
print([$base.decoder(7)["cvs"][19], $base.decoder(8)["cvs"][19], $adv.kind, $adv.address])
$adv.speed = 40
print([$base.decoder(7)["speed"], $base.decoder(7)["direction"], $base.decoder(8)["speed"], $base.decoder(8)["direction"]])
$c.speed = 100
print($base.decoder(7)["speed"])
$adv.set_function(2, true, to: "consist")
print([$base.decoder(7)["functions"][2], $base.decoder(8)["functions"][2]])
$adv.break_up()
print([$base.decoder(7)["cvs"][19], $base.decoder(8)["cvs"][19], count($adv.members), $base.decoder(7)["speed"]])
$r = roster(join_path(make_temp_path(), "roster.json"))
$r.add("Forgotten", address: 9, cvs: [19: 74, 29: 6])
$r.add("Fine", address: 10, cvs: [19: 0])
print(map($r.entries(in_consist: true), fn($e) => $e.name))
foreach [function() { $adv.add($adv) }, function() { consist(address: 128) }, function() { $soft.remove($c) }, function() { $soft.set_function(0, true, to: "consist") }] as $f {
    try {
        $f()
    } catch $e {
        print($e.type + ": " + $e.message)
    }
}
"#;
    let out = run("consists", script);
    assert!(out.success, "{}", out.stderr);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(
        lines,
        [
            "[63, \"forward\", 14, \"reverse\", \"software\", 2]",
            "[true, false]",
            "[true, true]",
            "[1, 0]",
            "[74, 202, \"advanced\", 74]",
            "[40, \"forward\", 40, \"reverse\"]",
            "40",
            "[false, true]",
            "[0, 0, 0, 0]",
            "[\"Forgotten\"]",
            "type_error: consist.add(): give a locomotive, not a consist",
            "range_error: consist()'s `address:` is a consist address, 1 to 127 (a decoder keeps it in CV 19), not 128",
            "range_error: consist.remove(): locomotive 7 isn't in this consist",
            "type_error: consist.set_function(): `to: \"consist\"` is for an advanced consist (one with an address)",
        ]
    );
}

// ---- the phone throttle server (model-railroading#12) ----

/// A script started with this addon, its output read a line at a time while it runs.
struct Running {
    child: std::process::Child,
    lines: std::sync::mpsc::Receiver<String>,
    dir: PathBuf,
}

impl Running {
    fn start(test: &str, script: &str) -> Running {
        let dir = addons_folder(test);
        let file = dir.join("script.leah");
        std::fs::write(&file, script).unwrap();
        let mut child = Command::new(leah())
            .arg(&file)
            .env("LEAH_3RD_PARTY_ADDONS", &dir)
            .env("LEAH_USER_ADDONS", "")
            .env("LEAH_TEST_DCC_PORTS", "/nonexistent")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Leah runs");
        let out = child.stdout.take().unwrap();
        let (tx, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for l in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                let _ = tx.send(l);
            }
        });
        Running { child, lines, dir }
    }

    /// The script's next printed line (within 10 seconds).
    fn next(&self) -> String {
        self.lines.recv_timeout(std::time::Duration::from_secs(10)).unwrap_or_else(|_| "(nothing printed)".to_string())
    }

    /// Waits for the script's end (within 10 seconds): whether it ended well, and its error output.
    fn end(mut self) -> (bool, String) {
        let start = std::time::Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                let mut err = String::new();
                use std::io::Read;
                let _ = self.child.stderr.take().unwrap().read_to_string(&mut err);
                let _ = std::fs::remove_dir_all(&self.dir);
                return (status.success(), err);
            }
            if start.elapsed() > std::time::Duration::from_secs(10) {
                let _ = self.child.kill();
                let _ = std::fs::remove_dir_all(&self.dir);
                return (false, "the script didn't end".to_string());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// A phone throttle app, as far as the protocol goes: lines out, lines in.
struct Phone {
    stream: std::net::TcpStream,
    reader: std::io::BufReader<std::net::TcpStream>,
    heard: Vec<String>,
}

impl Phone {
    fn connect(port: u16) -> Phone {
        let stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("the server listens");
        stream.set_read_timeout(Some(std::time::Duration::from_millis(300))).unwrap();
        let reader = std::io::BufReader::new(stream.try_clone().unwrap());
        Phone { stream, reader, heard: Vec::new() }
    }

    fn say(&mut self, line: &str) {
        use std::io::Write;
        self.stream.write_all(format!("{line}\n").as_bytes()).unwrap();
    }

    /// Everything the server sent until it goes quiet (a little while), kept in `heard` too.
    fn listen(&mut self) -> Vec<String> {
        use std::io::BufRead;
        let mut got = Vec::new();
        let start = std::time::Instant::now();
        let mut quiet_since = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(5) {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    got.push(line.trim_end().to_string());
                    quiet_since = std::time::Instant::now();
                }
                Err(_) => {
                    if quiet_since.elapsed() > std::time::Duration::from_millis(400) {
                        break;
                    }
                }
            }
        }
        self.heard.extend(got.clone());
        got
    }
}

#[test]
fn phones_drive_locomotives_through_the_phone_throttle_server() {
    let script = r#"
$base = dcc_base(simulate: true)
$r = roster(join_path(make_temp_path(), "roster.json"))
$e = $r.add("UP 792", address: 792)
$e.set_function(0, "Headlight")
$e.set_function(2, "Horn", momentary: true)
$server = phone_throttle_server(port: 0, name: "Test Railroad", roster: $r, announce: false, heartbeat: 30)
$server.add_turnout("T1", label: "Yard lead")
$server.add_route("R1", label: "Main")
$server.on_client = function($name) {
    print("client " + $name)
}
$server.on_turnout_request = function($name, $state) {
    global $base
    $d = $base.decoder(792, long: true)
    print("turnout " + $name + " " + $state + ", locomotive at " + $d["speed"] + " " + $d["direction"] + ", F0 " + $d["functions"][0])
}
$server.on_client_left = function($name) {
    global $server
    print("left " + $name)
    $server.close()
}
print($server.port)
"#;
    let run = Running::start("phone", script);
    let port: u16 = run.next().trim().parse().expect("the server's port printed");
    let mut phone = Phone::connect(port);
    let hello = phone.listen();
    assert_eq!(hello.first().map(String::as_str), Some("VN2.0"), "the protocol's version first: {hello:?}");
    for want in ["RL1]\\[UP 792}|{792}|{L", "PPA0", "PTL]\\[T1}|{Yard lead}|{2", "PRL]\\[R1}|{Main}|{4", "RCC0", "*30", "HTLeah", "HtTest Railroad"] {
        assert!(hello.iter().any(|l| l == want), "{want} in {hello:?}");
    }
    phone.say("HUphone-1");
    phone.say("NJohn's Phone");
    assert!(phone.listen().contains(&"*30".to_string()), "the heartbeat's seconds after the name");
    assert_eq!(run.next(), "client John's Phone");
    // (Track power on: every phone told.)
    phone.say("PPA1");
    assert!(phone.listen().contains(&"PPA1".to_string()));
    // (A locomotive from the roster: its labels, its functions, speed, direction, speed steps.)
    phone.say("M0+L792<;>EUP 792");
    let took = phone.listen();
    assert_eq!(took.first().map(String::as_str), Some("M0+L792<;>"), "{took:?}");
    assert!(took.iter().any(|l| l.starts_with("M0LL792<;>]\\[Headlight]\\[]\\[Horn]\\[")), "its keys' labels: {took:?}");
    for want in ["M0AL792<;>F00", "M0AL792<;>F028", "M0AL792<;>V0", "M0AL792<;>R1", "M0AL792<;>s1"] {
        assert!(took.iter().any(|l| l == want), "{want} in {took:?}");
    }
    // (Its speed: not echoed to the phone dragging the slider.)
    phone.say("M0A*<;>V63");
    assert!(!phone.listen().iter().any(|l| l.contains("<;>V")), "no speed echoed back");
    // (A latching key, the headlight: on at its press, its state told; nothing at its let-go.)
    phone.say("M0A*<;>F10");
    assert_eq!(phone.listen(), vec!["M0AL792<;>F10".to_string()]);
    phone.say("M0A*<;>F00");
    assert!(phone.listen().is_empty(), "a latching key's let-go changes nothing");
    // (A momentary key, the horn: on while pressed.)
    phone.say("M0AL792<;>F12");
    assert_eq!(phone.listen(), vec!["M0AL792<;>F12".to_string()]);
    phone.say("M0AL792<;>F02");
    assert_eq!(phone.listen(), vec!["M0AL792<;>F02".to_string()]);
    // (Its direction, then a question about its speed.)
    phone.say("M0AL792<;>R0");
    phone.say("M0AL792<;>qV");
    assert!(phone.listen().contains(&"M0AL792<;>V63".to_string()));
    // (A turnout thrown: every phone told, the script's event with what the decoder understood.)
    phone.say("PTATT1");
    assert!(phone.listen().contains(&"PTA4T1".to_string()));
    assert_eq!(run.next(), "turnout T1 thrown, locomotive at 63 reverse, F0 true");
    // (An address nothing knows: the phone is told.)
    phone.say("M1+S200<;>S200");
    assert!(phone.listen().iter().any(|l| l.starts_with("HMLeah: S200 isn't a locomotive's address")));
    // (The phone goes: its locomotives stopped, the script told.)
    phone.say("Q");
    assert_eq!(run.next(), "left John's Phone");
    let (ok, err) = run.end();
    assert!(ok, "{err}");
}

#[test]
fn a_quiet_phones_locomotives_stop_and_another_phone_hears_of_changes() {
    let script = r#"
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$server = phone_throttle_server(port: 0, announce: false, heartbeat: 1)
$server.on_turnout_request = function($name, $state) {
    global $loco, $server
    print("speed now " + $loco.speed)
    $loco.set_function(1, true)
}
$server.on_client_left = function($name) {
    global $server
    if count($server.clients) == 0 {
        $server.close()
    }
}
$server.add_turnout("T9")
print($server.port)
"#;
    let run = Running::start("phone_quiet", script);
    let port: u16 = run.next().trim().parse().expect("the server's port printed");
    let mut a = Phone::connect(port);
    let mut b = Phone::connect(port);
    a.listen();
    b.listen();
    a.say("NA");
    b.say("NB");
    a.say("M0+S3<;>S3");
    b.say("MT+S3<;>S3");
    a.listen();
    b.listen();
    // (A's speed reaches B, which holds the same locomotive: the script's own, shared.)
    a.say("M0A*<;>V40");
    assert!(b.listen().contains(&"MTAS3<;>V40".to_string()));
    a.listen();
    // (A change by the script reaches both.)
    a.say("PTATT9");
    assert_eq!(run.next(), "speed now 40");
    assert!(a.listen().contains(&"M0AS3<;>F11".to_string()));
    assert!(b.listen().contains(&"MTAS3<;>F11".to_string()));
    // (A asks for the heartbeat, then goes quiet past it (and its grace, two seconds more): the
    // locomotive stopped at once, A told why.)
    a.say("*+");
    std::thread::sleep(std::time::Duration::from_millis(3800));
    let told = a.listen();
    assert!(told.iter().any(|l| l.starts_with("HMLeah: nothing from this phone for 1 seconds")), "{told:?}");
    assert!(b.listen().contains(&"MTAS3<;>V0".to_string()), "B hears the stop");
    a.say("Q");
    b.say("Q");
    let (ok, err) = run.end();
    assert!(ok, "{err}");
}

#[test]
fn the_phone_server_sample_serves_and_throws_a_turnout() {
    // Start on the simulated base station (no boards), then a turnout thrown from the window: its
    // button's words, the server's state, then quit.
    let driver = "call_later(300, function() {\n    global $start\n    $start.click()\n    call_later(800, function() {\n        global $t1, $state, $server\n        $t1.click()\n        print($t1.text)\n        print(starts_with($state.text, \"Serving as \\\"My Railroad\\\" on port \"))\n        print($server.running)\n        quit()\n    })\n})\n";
    let Some(printed) = run_example_on_screen_with("phone-sample", "phone_server.leah", None, driver, 10000, &[("LEAH_TEST_PHONE_SERVER", "1")]) else { return };
    assert!(printed.contains("T1 Yard lead: thrown\ntrue\ntrue\n"), "{printed}");
}

#[test]
fn the_phone_server_keeps_phones_told_the_truth() {
    let script = r#"
$base = dcc_base(simulate: true)
$r = roster(join_path(make_temp_path(), "roster.json"))
$e = $r.add("Slow }|{ switcher", address: 7, speed_limit: 63)
$server = phone_throttle_server(port: 0, roster: $r, announce: false, heartbeat: 30)
$server.add_turnout("T1")
$server.on_turnout_request = function($name, $state) {
    global $base, $server
    if $state == "thrown" {
        $base.power = true
        $server.heartbeat = 12
    } else {
        print($server.clients[0]["locomotives"])
    }
}
$server.on_client_left = function($name) {
    global $server
    $server.close()
}
print($server.port)
"#;
    let run = Running::start("phone_truth", script);
    let port: u16 = run.next().trim().parse().expect("the server's port printed");
    let mut phone = Phone::connect(port);
    let hello = phone.listen();
    // (A roster name holding the protocol's separator: sent without it, so the list keeps its shape.)
    assert!(hello.contains(&"RL1]\\[Slow   switcher}|{7}|{S".to_string()), "{hello:?}");
    phone.say("NP");
    phone.listen();
    // (Full speed asked of a locomotive limited to half: the phone told the speed it runs at.)
    phone.say("M0+S7<;>ESlow }|{ switcher");
    phone.listen();
    phone.say("M0AS7<;>V126");
    assert!(phone.listen().contains(&"M0AS7<;>V63".to_string()), "the capped speed told");
    // (The script switching the power on and the heartbeat to 12 seconds: the phone told both.)
    phone.say("PTATT1");
    let told = phone.listen();
    assert!(told.contains(&"PPA1".to_string()), "{told:?}");
    assert!(told.contains(&"*12".to_string()), "{told:?}");
    // (Let go of and taken again: the same locomotive, still running as it was.)
    phone.say("M0-S7<;>r");
    assert!(phone.listen().contains(&"M0-S7<;>".to_string()));
    phone.say("M1+S7<;>S7");
    assert!(phone.listen().contains(&"M1AS7<;>V63".to_string()), "still at its speed");
    phone.say("PTACT1");
    assert_eq!(run.next(), "1");
    phone.say("Q");
    let (ok, err) = run.end();
    assert!(ok, "{err}");
}

#[test]
fn a_phone_makes_drives_and_breaks_up_an_advanced_consist() {
    let script = r#"
$base = dcc_base(simulate: true)
$base.power = true
$server = phone_throttle_server(port: 0, announce: false)
$server.add_turnout("T1")
$server.on_turnout_request = function($name, $state) {
    global $base
    $a = $base.decoder(3)
    $b = $base.decoder(5)
    print([$a["cvs"][19] ?? 0, $b["cvs"][19] ?? 0, $a["speed"], $a["direction"], $b["direction"]])
}
$server.on_client_left = function($name) {
    global $server
    $server.close()
}
print($server.port)
"#;
    let run = Running::start("phone_consist", script);
    let port: u16 = run.next().trim().parse().expect("the server's port printed");
    let mut phone = Phone::connect(port);
    assert!(phone.listen().contains(&"RCC0".to_string()), "no consists at first");
    phone.say("NP");
    phone.listen();
    // (Two locomotives into consist 74, the second back to back with the first: every phone told.)
    phone.say("RC+<;>S74<;>Double<:>S3<;>true");
    phone.listen();
    phone.say("RC+<;>S74<;>Double<:>S5<;>false");
    let told = phone.listen();
    assert!(told.contains(&"RCC1".to_string()), "{told:?}");
    assert!(told.contains(&"RCD}|{74(S)}|{Double]\\[3(S)}|{true]\\[5(S)}|{false".to_string()), "{told:?}");
    // (Driven at the consist address: both decoders follow it, the second the other way.)
    phone.say("M0+S74<;>S74");
    phone.listen();
    phone.say("M0AS74<;>V40");
    phone.say("PTATT1");
    phone.listen();
    assert_eq!(run.next(), "[74, 202, 40, \"forward\", \"reverse\"]");
    // (Broken up: each decoder told it is in no consist, the list empty again.)
    phone.say("RCR<;>S74");
    assert!(phone.listen().contains(&"RCC0".to_string()));
    phone.say("PTACT1");
    phone.listen();
    assert!(run.next().starts_with("[0, 0, 0,"), "out of the consist, stopped");
    // (A long consist address: refused, said.)
    phone.say("RC+<;>L1000<;>Big<:>S3<;>true");
    assert!(phone.listen().iter().any(|l| l.starts_with("HMLeah: L1000 isn't a consist address")));
    phone.say("Q");
    let (ok, err) = run.end();
    assert!(ok, "{err}");
}

#[test]
fn the_browser_throttle_serves_its_page_and_drives_from_a_web_socket() {
    // Start on the simulated base, fetch the page as a browser would, then speak to it as the page's
    // code does: take locomotive 3, set its speed and a function, and read what the decoder understood.
    let driver = r#"call_later(300, function() {
    global $start
    $start.click()
    call_later(500, function() {
        run_in_background(work: function() {
            return http_get("http://127.0.0.1:8080/")
        }, done: function($answer) {
            global $c
            global $web
            print([$answer["status"], contains($answer["body"], "EMERGENCY STOP"), $web.local_only])
            $c = web_socket(address: "ws://127.0.0.1:8080/throttle")
            $c.on_json = function($m) {
                global $base
                if has_key($m, "hello") {
                    print(["hello", $m["power"]])
                    $me.send(["power": true])
                    $me.send(["take": 3])
                } else if has_key($m, "loco") and $m["loco"]["speed"] == 0 and not $m["loco"]["functions"][0] {
                    $me.send(["speed": 63])
                    $me.send(["press": 0])
                    $me.send(["release": 0])
                } else if has_key($m, "loco") and $m["loco"]["functions"][0] {
                    $d = $base.decoder(3)
                    print([$m["loco"]["speed"], $d["speed"], $d["functions"][0], $base.power])
                    quit()
                }
            }
            $c.connect()
        })
    })
})
"#;
    let Some(printed) = run_example_on_screen_with("browser-sample", "browser_throttle.leah", None, driver, 15000, &[]) else { return };
    assert!(printed.contains("[200, true, false]\n[\"hello\", false]\n[63, 63, true, true]\n"), "{printed}");
}
