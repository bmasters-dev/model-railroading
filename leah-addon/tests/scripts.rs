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
    let dir = addons_folder(test);
    let file = dir.join("script.leah");
    std::fs::write(&file, script).unwrap();
    let out = Command::new(leah())
        .arg(&file)
        .env("LEAH_3RD_PARTY_ADDONS", &dir)
        // (None of the person's own addons: only this one, and Leah's own built in.)
        .env("LEAH_USER_ADDONS", "")
        .output()
        .expect("Leah runs");
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
    let script = r#"foreach [function() { return locomotive(3) }, function() { return dcc_base() }, function() { return dcc_base("/dev/ttyACM0") }] as $f {
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
            "hardware_error: locomotive(): there is no base station: make one first, dcc_base(simulate: true)\n",
            "hardware_error: dcc_base(): give the board's port, or simulate: true for a simulated base station (the board isn't supported yet)\n",
            "hardware_error: dcc_base(): Leah's base station board isn't supported yet (\"/dev/ttyACM0\"): dcc_base(simulate: true) runs a simulated one\n",
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
