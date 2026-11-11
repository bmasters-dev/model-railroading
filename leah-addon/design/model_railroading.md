# Model railroading — Leah's own DCC system, and dead rail

**A 3rd-party addon since 2026-11-09 (the owner's decision: the test of Leah's 3rd-party addon system; Maker stays one of Leah's own):** this repository, `bmasters-dev/model-railroading` (`leah-addon/`), built with Leah's addon kit and installed by `leah --install-addon https://github.com/bmasters-dev/model-railroading`, its releases tagged `leah-addon-v<version>`. So it reaches Leah only through the kit: it opens the base station board's serial port itself (its own crate, not Leah's serial port) and sends its own board program (it can't use the Maker addon's). Its issues are in this repository; the links below to Leah's specs and issues are from when it was one of Leah's own addons.

**Status: stage 1a built (2026-11-09): the simulated base station; stage 1b (2026-11-10): Leah's base station board with no track power, checked by a reader board.** Was planned, not started. Rewritten 2026-10-13 after the user's decision on #9: **Leah is the command station.** It doesn't drive other makers' command stations (NCE, DCC-EX, Digitrax, Z21); it competes with them, by building DCC in. The first plan (2026-10-06: drivers for DCC-EX, Z21, and LocoNet) is superseded. Each stage is designed with the user before it is built, as the controls are. Tracked on GitHub: [#32](https://github.com/bmasters-dev/leah-language/issues/32), milestone "Model railroading".

## Goal

A Leah app runs model trains **by itself**, with no other command station and no other model railroad program: throttles, panels, decoder settings, automation, phones as throttles. Two ways to reach a locomotive, behind one Leah object:

- **DCC on the rails**: Leah's own **base station**, a small board running Leah's board program, makes the DCC signal (the NMRA standard, open to anyone) and powers the track. Any DCC decoder answers to it, the user's NCE and Digitrax decoders included.
- **Dead rail**: battery-powered locomotives, each with an ESP32 and a motor driver doing **PWM** speed control, reached by radio over **ESP-NOW** (the ESP32's own link, no Wi-Fi network needed). No power on the rails; old DC locomotives come back to life.

## The user's situation (2026-10-13)

- The layout is being rebuilt: no track down yet. Testing starts on a short **bench track**.
- One DCC locomotive and one old DC locomotive to test with.
- A large amount of NCE equipment (the controller among it) and a handful of Digitrax, **to test with**: a known-good system and decoders to compare against, the Digitrax PRO4 programmer to check decoder settings. Not to be driven by Leah.
- Parts on hand (docs/maker_inventory.md, sections 01, 07, and 14): an Arduino Mega, the **EX-MotorShield8874** (DRV8874, 5 A, current sensing) and an ESP8266 Wi-Fi shield, an ESP32-S3 camera board, and a **Seeed XIAO ESP32-C3** (tiny, with a battery charger: a natural first dead-rail receiver).

## Why a board makes the signal

DCC is a square wave switched every 58 µs (a 1) or 100 µs (a 0), refreshed continuously; a PC can't hold that timing. So the signal comes from a microcontroller, and this builds on the microcontroller work ([#26](https://github.com/bmasters-dev/leah-language/issues/26), [#27](https://github.com/bmasters-dev/leah-language/issues/27); microcontrollers.md): Leah ships the base station's board program (C++, written for Leah, using a hardware timer or the ESP32's RMT peripheral for exact pulses), and Leah apps talk to it.

- **The board, to choose at stage 1:** an **ESP32** (exact pulses from its RMT peripheral, Wi-Fi to the PC, and the same chip sends dead rail's ESP-NOW, so one base station serves both), or the user's **Mega** with the EX-MotorShield8874 plugged straight on (no wiring). Either drives the track through the motor shield's H-bridge, whose current sensing gives short-circuit cut-off and the programming track's acknowledgments.
- **The standards:** NMRA S-9.1 (the electrical signal), S-9.2 and S-9.2.1 (packets: speed in 14/28/128 steps, functions F0–F68, accessory decoders, operations-mode programming), S-9.2.3 (service mode: the programming track). RailCom (S-9.3.2, decoders talking back) later.

## What Leah would build (the whole of the usual model railroad software, Leah's way)

| Part | Leah |
|---|---|
| The command station | Leah's base station: its board program, and a `dcc_base()` object in Leah apps (USB or Wi-Fi) |
| Throttles | `locomotive()` objects, the same for DCC and dead rail (speed, direction, functions, consists), and a ready-made throttle control |
| Turnouts and sensors | DCC accessory decoders through the base station; or straight from pins and boards (servos through the PCA9685, maker.md's Direct mode); sensors (block detectors such as the NCE BD20) read on the base station's pins, as events |
| Decoder settings (CVs) | the programming track (service mode) and writes on the main (operations mode); the NMRA standard CVs by name |
| Panels | turnout, sensor, and block objects with events, drawn with the canvas; a track diagram control later |
| Roster | locomotives and their settings, saved with the `database` object |
| Automation | timers and events for the basics; blocks, routes, and signal logic as built-ins |
| Phones as throttles | a built-in WiThrottle server, so phone throttle apps (Engine Driver, WiThrottle) drive trains through a Leah app (its protocol is published) |

A sketch of the language (to be designed properly):

```
$base = dcc_base("/dev/ttyUSB0")       ` Leah's base station, over USB (or Wi-Fi)
$base.power = true
$loco = locomotive(3)                  ` DCC address 3
$loco.speed = 40
$loco.direction = "forward"
$loco.set_function(0, true)            ` F0: headlight
$old_switcher = locomotive("deadrail:yard-2")   ` the same object, by radio
$old_switcher.speed = 20
turnout(12).throw()
$block = sensor(5)
$block.on_occupied = function() { ... }
```

## To do, in stages

- [x] **Serial ports, sockets (TCP, UDP), and events from background work**: built (2026-10-07: `serial_port`, `tcp_socket`, `udp_socket`, tasks).
- [x] **1a. The simulated base station, first** (the owner, 2026-11-09: "maybe some sort of emulator at first would be better", to keep an expensive locomotive safe; was stage 2): see "Stage 1a" below.
- [ ] **0. The microcontroller foundation** (#26, #27): flashing boards with `arduino-cli`, Leah's board program, talking to it from a Leah app.
- [ ] **1. DCC on the bench**: the base station's board program makes the signal (idle packets, speed in 128 steps, F0–F28, power on and off, emergency stop, short-circuit cut-off); the `dcc_base()` and `locomotive()` objects over USB; the user's DCC locomotive runs on a bench track. The board (ESP32 or Mega) is chosen here.
- [ ] **2. A simulated base station**, for automated tests and for trying scripts without power on the track (and a check of the real signal with the oscilloscope sample's DCC view, segments.md "Maker sample apps", against the user's NCE system).
- [ ] **3. The programming track**: reading and writing CVs (service mode, the decoder's current pulse as its answer); writes on the main; the standard CVs by name.
- [ ] **4. Controls**: a throttle (speed, direction, function buttons) and a turnout and sensor panel; accessory decoders; sensors on the base station's pins.
- [ ] **5. Wi-Fi to the base station**, and the **WiThrottle server**.
- [ ] **6. Dead rail**: the receiver's board program (ESP32, motor driver, PWM, lights and sound outputs as functions, a battery reading), ESP-NOW from the base station; the old DC locomotive converted first.
- [ ] **7. The rest of the usual model railroad software's whole**: functions up to F68, consists, the roster, blocks, routes and signals, a track diagram, automation.
- [ ] **8. Later**: RailCom, boosters for larger layouts, LCC/OpenLCB for layout control (its CAN layer is there since 2026-10-15: the `can_bus` object, hardware.md).

## Stage 1a: the simulated base station (built 2026-11-09)

The owner's safe ladder (2026-11-09): (1) software only, a simulated base station and decoder;
(2) two boards and no track power, the Mega making the real timing on a 5 V pin and a second board
reading it back; (3) the motor shield and a supply with no locomotive, checking voltage, that the
output is never stuck DC, and the short cut-off; (4) reading the locomotive's address with known-good
gear (the Digitrax PRO4 or the NCE system); (5) a cheap locomotive or a decoder on a test harness
before the expensive one. This is step 1.

Decided by Claude (the standing policy; the owner can overrule each):

- **`dcc_base(simulate: true)`** makes the base station. With a port (`dcc_base("/dev/ttyACM0")`)
  it will open Leah's board; until then that is a `hardware_error` saying only the simulated one is
  built. Properties: `simulate` (read-only), `power` (track power, `false` at first, as a real
  command station starts), `packets` (read-only: the last 200 packets sent, oldest first, each
  `["bytes": "03 3F A9 95", "bits": "11111111111111 0 00000011 ...", "meaning": "locomotive 3:
  speed 40, forward", "heard": true]`). Functions: `emergency_stop()` (the broadcast emergency stop
  every decoder obeys, and every locomotive object's speed set to 0), `decoder($address, long:
  false)` (what the simulated decoder at that address understood, read back from the signal:
  `["address", "long", "speed", "direction", "functions" (F0 to F28, true or false), "packets"
  (how many it heard)]`, or `null` when no packet reached it), `clear_packets()`.
- **`locomotive($address, long: false, base: ...)`**: a locomotive on a base station (by default the
  last one made; none made is a `hardware_error`). An address is 1 to 127 (short, one byte) or 128 to
  10239 (long, two bytes); `long: true` uses the long form for 1 to 127 too (a decoder answers to
  one or the other, so 3 and long 3 are two locomotives). Properties: `address`, `long`, `base`
  (read-only), `speed` (0 to 126, the 128-step speed, NMRA S-9.2.1), `direction` (`"forward"` or
  `"reverse"`). Functions: `set_function($number, $on)` (F0, the headlight, to F28), `function($n)`,
  `stop()` (speed 0, easing as the decoder is set to), `emergency_stop()` (this locomotive at once).
  Each change sends its packet at once.
- **The packets** (NMRA S-9.2 and S-9.2.1): the address (one byte, or two as `11AAAAAA AAAAAAAA`),
  the 128-step speed instruction (`0x3F`, then `DSSSSSSS`: 0 stop, 1 emergency stop, 2 to 127 the
  steps 1 to 126), the function groups (F0 to F4 `100xxxxx` with F0 in bit 4, F5 to F8 `1011xxxx`,
  F9 to F12 `1010xxxx`, F13 to F20 `0xDE` and a byte, F21 to F28 `0xDF` and a byte), the broadcast
  emergency stop (`00 41`), and the error byte (all bytes XOR-ed).
- **The signal** (NMRA S-9.1): a 14-bit preamble of ones, then each byte after a 0 bit, then a 1;
  each bit two halves, 58 µs for a 1 and 100 µs for a 0.
- **The simulated decoder reads the timing, not the bytes:** each packet is turned into its bits and
  half-bit timings, and the decoder reads those back as a real one does (a half of 52 to 64 µs is a
  1, of 90 to 10,000 µs a 0; a preamble of at least 10 ones; the error byte checked), so stage 2's
  board program and its reader can be checked against the same rules. With the track power off, no
  decoder hears anything (`heard: false`).
- No events yet: a window watching the decoder reads `decoder()` on a timer (the example).

## Stage 1b: two boards, no track power (wiring decided 2026-11-10)

Step 2 of the safe ladder. Decided by Claude (the owner can overrule):

- **The Mega makes the signal** on **pin 12** (Timer1's output OC1B, so the pulses come from the
  hardware timer, not from code), with **no shield** on it. The motor shield stays in its bag until step 3.
- **A classic Uno or a Nano reads it back** on **pin D8** (Timer1's input capture, ICP1: each edge's
  time taken in hardware, to 0.5 us; pin D2's interrupt was tried first and was off by up to 6 us
  whenever the Uno was busy sending), measures each half-bit, decodes the packets, and sends them to Leah over USB. Not the Uno R4 WiFi: its chip is
  different, and the classic ATmega328P board is the one the reader is written for.
- **Two wires**: Mega pin 12 → a **1 kΩ resistor** → the reader's D8 (the resistor keeps either board safe
  if a pin is set wrong), and Mega GND → the reader's GND. Both boards 5 V, so no level shifting.
- **Both boards on USB** to the PC, which powers them; no other supply connected.
- The owner's account needs the `dialout` group to open the ports (`sudo usermod -aG dialout $USER`, then
  log out and in).
- **The board programs** are `boards/dcc_base/dcc_base.ino` (the Mega) and
  `boards/dcc_reader/dcc_reader.ino` (the Uno: `P ...` for each packet heard, at most every 2 s, and a
  `T` line each second with the halves' shortest and longest, the packets, the halves outside S-9.1's
  limits, and the error-byte failures; starts with `LEAH DCC READER 1`).
- **The base station's program, version 2** (2026-11-10, `LEAH DCC BASE 2`): a **refresh table** of 32
  places (`K <place> <packet>` keeps a packet repeating, `D <place>` drops it), packets to **send now**
  (`S <packet>`, each three times, ahead of the table), track **power** (`+`, `-`: off holds pin 12 low),
  `X` (everything dropped, power off), `?` (`STATE power on places 2`), and `.` (only that the PC is
  there). Packets come with their error byte, which the board checks. An idle packet goes between every
  two others, so no decoder gets two in a row (S-9.2's 5 ms between packets to one address). It answers
  `OK` or `ERR ...` to every line but `.`. The timer's interrupt works each bit out from the packet's
  bytes, so the table holds bytes, not bits.
- **Safety (decided 2026-11-10):** with the power on, **3 seconds without a line from the PC turn the
  power off**, and the board says `OFF no word from the PC for 3 seconds`. Leah's link says `.` every
  second, from a thread of its own, so a script busy for a while is fine, but a program that ended,
  crashed, or lost its cable leaves no signal on the track. Leah turns `power` false and warns when it
  hears the `OFF`. **A known limit** (the code review, 2026-11-10): the `.` comes from the link's own
  thread, so a script stuck in a loop of its own (or a frozen window) keeps the power on; ending the
  program, or closing it from the desktop, cuts it within 3 seconds. Tying the `.` to the script's
  progress would turn the power off under a script that is merely waiting (a `sleep()` while a train
  runs), so it is left so, as every command station program leaves it. After a cable is pulled,
  turning the power off and the emergency stop still work (they stop the locomotives' objects and send
  nothing); a lost reader stops only the reading back, with a warning, never the base station. Two
  locomotive objects at one address share its places in the table, so the board never sends two
  speeds for one decoder in turn. `load_dcc_program()` refuses a board its program doesn't run on (the
  base station: the Mega 2560, OC1B being pin 12 there only; the reader: the ATmega328P boards, ICP1
  being pin 8). A simulated base station needs nothing from a script runner's rules (the addon gives
  its needs by its options); one on a port needs "device".
- **What the script sees (decided 2026-11-10):** `dcc_base($port, reader: $port)`; `simulate` false;
  new read-only `port`, `reader`, and `timing` (the reader's last `T` line: `one` and `zero` as
  `[shortest, longest]` in µs or null, `packets`, `outside`, `errors`; null without a reader); `close()`.
  `packets`' `heard` on a board is the reader having heard it go out; `decoder()` on a board is the
  reader's packets fed to the same decoder logic as the simulation (null with no reader). Each
  locomotive takes two places in the table (its speed, and F0 to F4): 16 locomotives, then its packets
  are sent once with a warning; letting go of a locomotive drops its places. The base's
  `emergency_stop()` on a board also turns each locomotive's kept speed into its emergency stop, or the
  table would start it again. `dcc_ports()` lists the USB serial ports (`port`, `name` from the USB ids,
  `vendor_id`, `product_id`, `allowed` by `access(2)`); `load_dcc_program($port, "base" or "reader",
  board:, arduino_cli:)` compiles and uploads through arduino-cli in a thread, giving back a
  `dcc_program_job` (`port`, `role`, `board`, `finished`, `succeeded`, `message`; `wait()`;
  `on_finished($succeeded, $message)`), its sketch, build, and arduino-cli's build cache
  (`ARDUINO_BUILD_CACHE_PATH`, unless set) in a `make_temp_path()` folder. A missing arduino-cli is a
  `shell_error`. `dcc_base` and `dcc_ports` need "device", `load_dcc_program` "programs" and "device".
- **Connecting:** opening a port starts an Arduino's program again (sometimes twice: lines sent in
  between are lost), so `dcc_base()` waits for the base station's hello, asks `?`, and goes on only
  once a `STATE` came after the last hello (and the reader's hello or a `T` line), within 5 s; then `X`.
  A board that doesn't start again on opening answers the `?` asked after 2.5 s. A silent board, or one
  with an older program, is a `hardware_error` naming the `load_dcc_program()` call to make.
- **Tests without hardware:** `tests/fake_boards.py` plays both boards on pseudo-terminals (the
  protocol, the safety, the reader's lines; `--base silent|old`, `--cut-after`); `LEAH_TEST_DCC_PORTS`
  names a file listing stand-in ports for `dcc_ports()`, and every test sets it, so no test ever sees
  the real boards. `LEAH_DCC_TRACE` prints every line to and from the boards, for working on them.
- **Measured on the owner's bench, 2026-11-10** (a genuine Mega 2560, a CH340 Uno clone): halves of a 1
  57.5 to 58.0 us, of a 0 99.5 to 100.0 us (S-9.1 allows a command station 55 to 61 and at least 95);
  about 172 packets a second; no error-byte failures; short (3) and long (231) addresses, speed and stop
  packets read back exactly. The signal half of step 2 passes. Leah's side the same evening: from a
  Leah script and from `examples/dcc_bench.leah`, the throttle drove the real Mega, and the reader's
  decoder 3 followed it (speed, direction, headlight, the emergency stop), the timing unchanged, every
  packet heard. **Step 2 passes.**
- **`arduino-cli` is a prerequisite** (installed 2026-11-10 for development in `~/Leah_Tools`, version
  1.5.1, with the `arduino:avr` core; a wrapper sets its folders so nothing goes into `~/.arduino15`).
  It isn't a Debian package, so it can't go in addon.toml's `needs` (Debian packages only): the
  README's "What you need" names it, and `dcc_base()` with a port says plainly when it is missing and
  where to get it. A field for outside programs in addon.toml is asked of Leah (leah-language issue).
- **The app must make that clear to its user** (the owner, 2026-11-10: end users won't know about
  `dialout`). `dcc_base("/dev/ttyACM0")` refused by the system gives a `hardware_error` of kind
  `"access_denied"` (as `serial_port` does) in plain words. The example app catches it and shows a
  window explaining it without jargon ("Linux needs your permission before apps can talk to the
  board"), with a **Give access** button that runs `fix_device_access("serial")` (it asks for the
  password) when the devices addon is there, the command to type otherwise, and then **log out and back
  in**, which the window says plainly. The board list shows a board that is plugged in but not
  allowed as such, not as missing.

## Stage 1c: the motor shield, no locomotive (decided 2026-11-10)

Step 3 of the safe ladder: the EX-MotorShield8874 on the Mega, a 12 V supply (the owner has some) in
its barrel jack, and no locomotive. Decided by Claude (the owner can overrule):

- **The pins**, as DCC-EX's own definition of the shield gives them
  (`MotorDriver(3, 12, UNUSED_PIN, 9, A0, 5.08, 5000, A4)` on a 5 V, 10-bit board): channel A's
  direction is **pin 12** (the signal Leah's program already makes there), its output on and off
  **pin 3** (the track power), its brake **pin 9** (held low), its current **A0** at **5.08 mA a
  step**, its fault line **A4** (low on a fault). The shield's regulator gives the Mega 7.2 V on VIN by
  default; USB on as well is fine (DCC-EX's page), but never a supply in the Mega's own barrel jack too.
- **The base station's program, version 3** (`LEAH DCC BASE 3`): finds the shield as it starts (the
  internal pull-up on A0 reads high with nothing there, low against the shield's sense resistor), so
  the same program serves the bench without a shield (no current check, no false cut-offs from a
  floating A0). With it: the power on and off on pin 3 (the signal joined first, the output cut
  first); the current sampled every millisecond, said every half second (`A 25`), smoothed; **the
  power off when it stays over the limit for 5 ms**, or the fault line stays low for 5 ms, saying why
  (`OFF the current was over the limit (...)`). The limit (`L <mA>`, 10 to 5000) is 1500 mA at the
  start, and Leah sets it again on connecting. `STATE` adds `shield yes`, `current`, `limit`.
- **After the code review (2026-11-10):** pin 3 is never turned on without the shield found (a wrong
  guess means no power, not unwatched power); the current is watched by the analog reader's own
  interrupt, free-running on A0 at about 9,600 samples a second, which cuts the power itself (no
  serial traffic or busy loop delays it); the over-limit count rises by 2 a sample over the limit and
  falls by 1 under it, tripping at 96 (5 ms straight, or a short the driver chops on and off), the
  first 20 ms after power-on left out for inrush; a fault on A4 cuts at once; after a cut-off for the
  current or a fault, `+` is refused for 2 seconds (with an `OFF ...` line, so Leah keeps `power`
  false); the `S` packet's store into the ring is done with interrupts off. No watchdog timer: some
  Megas' bootloaders loop for ever after its reset, and only a programmer brings them back. On the
  Leah side: the limit is Leah's (never taken from a `STATE`), `current` goes to 0 when the power
  changes, and a board that says its hello again after connecting (a brown-out) gets its limit and
  refresh table sent again, its `power` false, with a warning.
- **In Leah:** `dcc_base`'s `shield` (read-only), `current` (mA; 0 with the power off, null without a
  shield or simulated), `current_limit` (settable). The bench app shows the current and lets the
  limit be chosen, a 15 mA one among them for trying the cut-off, and shows the board's reason when it
  turns the power off.
- **The bench test, with the owner's parts:** the meter on the shield's channel A outputs (DC volts
  near 0: never stuck at DC; AC volts near the supply's: the square wave); the owner's **470 ohm,
  0.5 W resistor** across them (12 V / 470 = 25 mA, 0.3 W: warm at most) read by Leah as about 25 mA;
  then the limit set to 15 mA, and the power must go off by itself. No dead short (no power resistor
  on hand, and the cut-off is proved without one). The USB isolator is on order: until it comes, the
  Mega's USB goes to the PC as DCC-EX's own users do.

- **Measured on the owner's bench, 2026-11-10/11** (a 12 V 5 A centre-positive supply, HTY-1200500,
  in the shield's jack; the owner's DCC Concepts DCC meter and a multimeter on channel A's two
  screws): the shield found (`shield yes`); with no supply in the jack, `+` is cut at once by the
  driver's fault line (the safety working before any 12 V); **12.22 V of DCC** on the DCC meter; DC
  volts across A's two screws between -1.5 and -0.5 (never stuck at DC; one A screw to a B screw read
  5.38 V, half the supply, as it swings 0 to 12 V); the multimeter's AC volts read only 1.79 (it
  can't follow DCC's frequency: use a DCC meter). With the 470 ohm resistor across A: Leah read 23 to
  30 mA; with the limit at 15 mA **the board cut the power by itself** and Leah gave its reason; `+`
  again at once was refused for 2 seconds; after it, at 1500 mA, the power came on again. The Uno on
  the shield's pin 12 socket heard every packet, 57.5 to 58.0 and 99.5 to 100.0 us. **Step 3 passes.**
  (No dead short was tried: no power resistor on hand, and the cut-off is proved without one.)

## Stage 1d: the first locomotive (2026-11-11, the owner's bench)

Step 5 of the safe ladder, done with the owner's dropped HO sound locomotive (step 4 dropped: the
owner's NCE and Digitrax gear isn't set up on the bench, so Leah is the first thing to talk to it;
steps 2 and 3 had proved the signal and the protection). On a treadmill wired to channel A, 12 V, the
limit at 1 A:

- **The address was found by its horn:** the broadcast address 0 with F0 and F2 (`00 92 92`) sounded
  the horn, so the decoder heard Leah; then F2 at one address at a time: **address 1** answered, 730
  (long) and 3 didn't. The headlight lit only once a direction had been sent (`speed 0, forward`):
  some decoders keep F0's light off until they have been told a direction.
- **It moved:** speed 5 forward for 4 s (the decoder's own acceleration first, then 130 to 160 mA),
  stopped at speed 0; the same in reverse (about 210 mA); speed 20 (about 165 mA), then the
  **emergency stop**: the wheels stopped at once (the current back to 15 mA within the next
  half-second reading). No cut-off fired; the current never came near the limit.
- **Found on the way:** the driver flags a fault for a moment whenever it catches a surge, and a DCC
  meter (or a sound decoder's capacitors) makes one as the power comes on: the program's first cut at
  the first flag cut the power whenever a DCC meter was on the track, so the fault flag now has the
  same 20 ms start-up grace and an adding-up count as the current (2 ms all told). Dirty wheels gave a
  patchy connection (0 mA, sounds coming and going) until cleaned. A missing 12 V supply looks exactly
  like a fault, and the board says so in those words. One DCC meter, on its IN side only, still trips
  the driver on a treadmill (not on flex track): left for later; the locomotive runs without it.
- **Not yet:** reading or writing a decoder's settings (the programming track, a later stage), so
  Leah can't yet read an address; the expensive locomotive goes the same way when the owner chooses.

## Stage 2: the programming track, reading (2026-11-11, decided and built on the owner's bench)

The stage list's "3. The programming track", its reading half. Decided by Claude (the owner can
overrule): the shield's **channel B** is the programming track, as on DCC-EX (pin 11 its output,
pin 13 its direction, which is Timer1's OC1C: the same timer gives B the same bit timing, its middle
set with OCR1C beside OCR1B; pin 8 its brake, held low; A1 its current; A5 its fault line). Reading
turns the main track off (both channels share the one timer's bit timing, so B gets its own packets
only while A is off): simpler and safe. NMRA S-9.2.3's direct mode, verify bit then verify byte, as
`R <cv>` on the board, which answers `CV <cv> <value>`, `CV <cv> none (baseline .. mA, highest .. mA)`,
or `CV <cv> off <why>`; B's filler is the reset packet, never the idle one. Learned on the bench:

- **A sound decoder draws about 800 mA as it starts** (SoundTraxx, a stay-alive): 1 A is allowed for
  B's first 1.5 s, and the questions start after 260 resets (about 1.6 s).
- **Its "yes" is its motor at stall for about 6 ms, near 800 mA:** the 250 mA limit must be over for
  about 50 ms before it cuts (a short is still cut in about 2 ms by B's fault line); with 5 ms every
  reading was cut at the first "yes".
- **An answer is 2 ms all told over the baseline + 60 mA**, not one unbroken run (a flickering wheel
  breaks a run); the byte verified at the end guards against a false "yes"; Leah tries a setting 3
  times. The owner's locomotive read reliably once its wheels touched the rollers firmly: address 1,
  CV 29 = 6, CV 8 = 141 (SoundTraxx).
- `B <seconds>` powers B with resets and says its current four times a second: for checking on the
  bench (it found the decoder drawing nothing in service mode, which pointed at the answer pulses).
- **In Leah:** `read_cv($cv)` and `read_address()` on a `dcc_base`, each `in_background: true` with the
  answer through `on_read($what, $value, $error)` (a window can't wait seconds); one reading at a
  time; the main track's `power` false after. The throttle's **Read address** button uses it.
- **Writing (decided and built the same morning):** `W <cv> <value>` on the board (`LEAH DCC BASE 5`):
  the decoder started as for reading, the write-byte packet 5 times with 6 resets after, 30 resets
  for it to store the value, then verify-byte up to twice; `W 3 12 ok`, `W 3 12 unconfirmed (...)`,
  or `W 3 off <why>`. Leah writes once more when unconfirmed. `write_cv($cv, $value)` and
  `write_address($address, long:)` (CV 1, or CV 17 and 18, with CV 29's bit 5, CV 29 read first so
  its other bits stay), each `in_background: true` with `on_write($what, $value, $error)`. CV 7 is
  refused (read only) and so is CV 8 (writing it resets many decoders to their factory settings: a
  deliberate reset can come later, its own function). The throttle's **Decoder settings** row (CV,
  its name for the usual ones, the value, Read, Write), with the power off only. Operations-mode
  programming on the main is next. **On the owner's bench** (the same morning, from the throttle in
  the IDE, addon 0.5.0): CV 3 read as 6 (the acceleration), the same 6 written back, and the decoder
  confirmed it: writing proven on a real decoder (SoundTraxx), nothing changed in it.

## Phase A, item 1: programming on the main (built 2026-11-11, model-railroading#5)

`$loco.write_cv($cv, $value, pause: 500)` and `write_cv_bit($cv, $bit, $on)`: S-9.2.1 2.3.7.3's long
form (`1110 11VV`, `VVVVVVVV`, the value; bit: `1110 10VV`, `VVVVVVVV`, `1111 DBBB`), checked against
the saved standard (2025 edition): a decoder writes on the second identical packet with no other
packet to it between them (a broadcast counts), and never acts on one sent to its consist address. On
the board it goes out as a packet sent now (three copies, idle packets between); the simulated track
delivers it twice, and the simulated decoders keep the two-packet rule (`decoder()`'s `cvs`). Writes
to one locomotive are 500 ms apart by default (`pause:`), as some decoders miss one sent sooner (the
scanning session's finding). CV 7 and 8 are refused, as on the programming track. The throttle's
settings row has **On the main**: Write goes to the locomotive being driven, with the power on, Read
greyed out (nothing can answer on the main without RailCom).

## Phase A, item 2: programming extras (built 2026-11-11, model-railroading#6)

The base station's program v6 (`LEAH DCC BASE 6`) adds `V cv value` (verify byte: "yes"/"no"),
`U cv bit` (asks "is it 1?" then "is it 0?": 1, 0, or "none", so no decoder isn't read as 0), and
`Y cv bit 0|1` (write bit, then verify bit, twice at most). In Leah: `verify_cv`, `read_cv_bit`,
`write_cv_bit` on the programming track; `index: [cv31, cv32]` on every read and write (both tracks;
each index write a write of its own); `tries:` (1 to 10, 3) on readings; the decoder's side's
failures are a `programming_error` (nothing answered, not confirmed, the programming track cut), the
base station's a `hardware_error`. **Deferred (Claude's decision):** paged and register modes: every
decoder of the last 20+ years has direct mode; they come back if someone's decoder needs them. A
quicker index (keeping the programming track powered across the CV 31/32 writes and the access) is a
later speed-up.

## Phase A, item 3: decoder identification and named bits (built 2026-11-11, model-railroading#7)

No firmware change (v6's R, W, and V do it all). `identify_decoder()` reads CV 8, 7, 19, and 29 and
gives the maker by the NMRA's list (`src/makers.rs`, the 168 IDs of Appendix A to S-9.2.2, revised
2026-08-17), the version, the consist address (a warning when it isn't 0: a forgotten consist is the
usual "it won't respond"), and CV 29 by name. **Product IDs:** only SoundTraxx's own guide (rev. F,
2025-09) says what its CV 7 values mean (the family) and where Tsunami2, Econami, and Blunami keep the
product (CV 253 customer, 254 sound type, 255 bits 3 to 7 the revision, 256 the product number);
the product number is given as a number, its name left to the decoder definitions (item 4). Every
SoundTraxx decoder's firmware is in CV 105/106 (major.minor in those three families; in the others
CV 105 the version and CV 106 the factory settings set, given as `settings_set`). The owner's
locomotive (2026-11-11): CV 7 = 82, "Sound Value, RTR, Mainline", a factory-fitted sound decoder, with 0 in CV 105 and 106 (the NMRA user IDs, left unset): 0 there is given as unknown (null). ESU's
LokSound 5 manual documents no product ID; other makers' come with their definitions. Global
functions needing no base: `decoder_maker_name`, `cv29_parts`, `make_cv29`.

`reset_decoder(confirm: true)`: without `confirm:` a `programming_error`; each maker's documented
reset (SoundTraxx, ESU, Digitrax, ZIMO: CV 8 = 8; TCS: CV 8 = 2; NCE: CV 30 = 2; MRC: CV 125 = 1),
an unknown maker's refused unless the script gives `cv:` and `value:`; the write not being confirmed
is taken as the reset (a resetting decoder can't acknowledge); it reads the address afterwards.

**Decoder locks** (S-9.2.2, CV 15 the key, CV 16 the lock; writes to 15 and 16 always taken; reads
never locked): `find_decoder_locks(most: 7)` asks "is CV 16 = n?" for each n and writes nothing;
`unlock_decoder(n)` writes CV 15; `lock_decoder(n)` writes CV 16 then CV 15 (every decoder on the
track takes it, so with only that decoder there). **For item 4:** the decoder programmer's "write
all" never writes CV 15 or 16, nor CV 7 and 8.

## Phase A, item 4: decoder definitions and the programmer (built 2026-11-11, model-railroading#8)

`decoder_definition`: Leah's own format, a list of settings, each with `name`, `cv` (or a list of CVs,
most significant first), `bit`/`bits`, `kind` (number, choice, flag, signed: S-9.2.2's sign-and-
amount, long_address: CV 17/18), `min`/`max`, `choices` (value → words; the words are the value),
`group`, `help`, `default`, `read_only`, `write_all`, `shown_when` ([setting, op, value]), `index`.
Built from Leah arrays, a JSON file (`read_decoder_definition_file`), or built in. Every definition
starts from the NMRA standard set (57 settings over 52 CVs, S-9.2.2; CV 33-46 function mapping left
for the function-map item) unless `standard: false`; a setting of the same name replaces a standard
one. Checked when made: unknown parts, two settings on one bit, a condition naming no setting, a
choice outside its bits, one CV number on two index pages (refused for now: the programmer knows a
CV by its number). Built in: `nmra`; `soundtraxx_tsunami2_diesel` (58 of SoundTraxx's settings from
its Tsunami2 Diesel Technical Reference: master volume and the 29 mixer channels, the sound choices
as numbers (SoundTraxx's sound lists name them), sound options, brakes (signed), lights, back-EMF).
`decoder_definition_for(identity)` picks one from `identify_decoder()`'s answer (Tsunami2 diesel by
CV 7 = 71 and CV 254 = diesel), else `nmra`. Next definitions (as people ask, from the manuals in
the private reference): Tsunami2 steam and electric, Econami, Blunami, ESU LokSound 5, TCS WOW.

`decoder_programmer(definition, base | locomotive)`: on the programming track every run is on a
thread of its own (`on_progress(done, total, cv)`, `on_done(what, error)`); on the main it writes
only, synchronously, through the locomotive's writes. Bookkeeping: the decoder's known bytes (read
or written), pending bytes (edits and loads), edited settings with their values, loaded CVs, and
per-setting states unknown/read/written/edited/loaded. A reading replaces pending changes to what it
reads but puts back edits to other settings sharing the CV. `read_all` asks each CV `tries` times
(2), a CV a decoder lacks is left unknown in `problems`, and with nothing answering (CV 8 probed to
tell) it stops at once. `write_all` never writes CV 7, 8, 15, 16 or read-only settings; `load_cvs`
never takes them. On the main, a run changing the address the decoder answers to is refused before
sending. A run's end is ordered (keep_running(false), the reading turn let go, not busy, on_done),
also on a panic; `DccBase::in_background` got the same order. Code-reviewed twice (2026-11-11).

The **decoder workshop** sample (`examples/decoder_workshop.leah`): a `property_list` with a group per
definition group, rows by kind, rows painted by state, speed-table rows hidden by `is_shown`, Identify
picking the definition, Read all, Write changes, Write all, Cancel, a progress bar, and On the main.

## Phase A, item 5: the roster (built 2026-11-11, model-railroading#9)

`roster(path)`: entries kept in one JSON file (`{"format": 1, "entries": [...]}`), read when made,
written only by `save()` (Leah's safe write, the folder made). Without a path, the roster every app
shares: `model_railroading/roster.json` beside the apps' own data folders (the parent of
`data_path()`, so in the app's folder or the portable folder, as Leah keeps every app's data).
`roster_entry`: name (unique, case-blind), road_name, road_number, address (128+ makes long), long,
decoder (a definition's name), model, notes, picture (a path), groups, cvs, functions (F0 to F68:
label and momentary, set with `set_function`), speed_limit (1 to 126, used by item 6), attributes
(the owner's fields). `entries(group:, address:)`, `export_entries(path, names:)`,
`import_entries(path)` (a taken name gets " (2)"), `remove`. `locomotive($entry)`. The programmer's
`cvs` go into an entry and back through `load_cvs`. Not built (later, as people ask): a speed profile
and physics data per entry, the roster on the web, exporting to a command station's format, CSV of
the CVs. The decoder workshop got a Roster row: Save to roster (the decoder's CVs, definition, and
address under a name) and Load (a saved locomotive's settings back, loaded, to Write all).

## Phase A, item 6: throttle extras (built 2026-11-11, model-railroading#10)

`locomotive(..., speed_steps: 128 | 28 | 14)` (from a roster entry: 14 when its kept CV 29 says so):
`speed` counts in the locomotive's own steps; 28 and 14 go in S-9.2's basic packet `01DCSSSS` (28:
step n sent as n + 3 over SSSS and C; 14: SSSS = n + 1 and C the headlight, so F0 changes resend the
speed packet). The simulated decoder reads basic packets by its own CV 29 bit 1, as a decoder does,
and `decoder()` says `speed_steps`; a basic stop or emergency stop leaves the direction as it was.
`speed_percent` (0 to 100), `speed_limit` (in the locomotive's steps; a roster entry's limit is in 126
steps, scaled up), `momentary`, `labels`, `entry`, `press_function`/`release_function` (momentary:
on while held; latching: toggles). The base: `locomotives`, `stop_all()`, `on_locomotive_changed`
(every change of speed, direction, or function). The throttle example: a Roster list (address,
labels, momentary keys as plain buttons held, the limit), keyboard driving (Up/Down, Page Up/Down,
Space, Left/Right, 0 to 9 as F0 to F9, each digit taken once until let go), and Stop now tells the
locomotive (setting a slider from the script doesn't run its event: Stop had only moved the slider).
`examples/gamepad_throttle.leah` on the gui addon's `game_controller` (the stick changes speed at a
rate; a pad that goes stops the locomotive; the horn momentary for a locomotive without an entry),
on the simulated base too; tested with a recorded pad (`LEAH_TEST_GAMEPAD`, each event's time from the
start). Later, as people ask: several throttles in a window, saved throttle layouts, a centre-off
shunting slider.

## Phase A, item 7: consists (built 2026-11-11, model-railroading#11)

`consist(name:, address:, base:)`. Software (no address): each member its own packets, the consist's
speed (0 to 126) made to fit each member's steps and limit (`Locomotive::drive`), a reversed member
the other way. Advanced (`address:` 1 to 127): `add` sends S-9.2.1 2.3.1.4's consist control
(`0001001D 0AAAAAAA`, twice; D reversed), speed and direction go to the consist address through a
locomotive the consist keeps (128 steps, in the refresh table on a board); `break_up()`/`remove()`
send consist address 0. A member's own speed is set to 0 as it joins and leaves (a stale speed
repeating at its own address would start it on leaving). `set_function(n, on, to: "lead" | "all" |
"consist" | $loco)`; `stop`, `emergency_stop` (the consist address and every member at once). The
simulated decoders follow S-9.2.1: consist control sets CV 19; a consisted decoder ignores speed and
direction at its own address; packets to a short address also reach every decoder with that consist
address (speed flipped for a reversed unit; functions only as CV 21 and 22 allow), through
`dcc::deliver`, which both delivery paths use. The forgotten-CV-19 check: `roster.entries(in_consist:
true)` (and identify_decoder's warning, item 3). Not built (as people ask): reordering members, saved
consists in the roster, NCE's consists (Phase E), the long consist address (CV 20).

## Decided: no BiDiB (the owner, 2026-11-11)

BiDiB's specification is free but supervised: the working group must be told before anything is
published or implemented, no part of its text may be published without written permission, and
implementers agree to its compatibility testing and to fix what it finds within two months. The
owner doesn't agree to those terms, so Leah won't support BiDiB, and its specification isn't kept
in the private reference either. (A mainly European bus; nothing in the plan depends on it.)

## Decided: how Leah names its phone throttle server (Claude, 2026-11-11)

"WiThrottle" is a trademark of the protocol's author, who publishes no specification. Leah's server
(plan item 8) is described as working **with WiThrottle apps** (Engine Driver, WiThrottle, and others),
never as "Leah's WiThrottle"; its object gets a name of Leah's own (`phone_throttle_server`, to be
settled when it is designed). The protocol is implemented from facts in the descriptions that exist,
never from another program's code.

## Decided: the addon's own documentation (Claude, 2026-11-11, at the owner's word)

A 3rd-party addon keeps its documentation apart from Leah's reference (the owner: "We have to keep a
separate documentation for Model Railroading since it's considered a 3rd party addon"). Its pages
stay in `docs/`, in the reference's style (Leah's STYLE.md), written with each item as it is built
rather than saved for a session; `tools/docs.py check` runs every sample with Leah and the addon as
built, and checks every link and anchor; `tools/docs.py wiki` publishes the committed pages to this
repository's wiki, with a Home page (the overview, the function pages, the objects), a header, and
a sidebar. A `ref:` link to a title that isn't one of these pages goes to Leah's own wiki, so the
pages can point at Leah's reference without copying it. Publishing goes one way, as Leah's does: an
edit made on the website stops it until brought into `docs/`. A documentation run is: `cargo
build`, `tools/docs.py check`, commit, `tools/docs.py wiki`, at each release that changed a page.
Leah's kit has no such tool yet, nor help in the IDE for a 3rd-party addon's names: filed in Leah.

## Rules to keep (agreed in principle)

- **Emergency stop** always works, from code and from every throttle control.
- **A lost connection stops the trains**: the base station stops them if it stops hearing from the app, and a dead-rail locomotive stops if it stops hearing from the base station.
- **A short circuit cuts track power** at once, and says so.
- Built on the cross-cutting rules: hardware problems are hard errors (error_handling.md); events run on the GUI thread; scripts never deal with threads.

## Open questions

- **The base station's board** (stage 1): ESP32 or the Mega with the motor shield.
- **Dead rail's radio messages** (stage 6): Leah's own, over ESP-NOW; whether a handheld throttle can talk to a locomotive directly.
- **Decoder definitions (decided by the owner, 2026-11-11):** Leah's **own** decoder definitions, in a simple format of its own, under Leah's licence, written from the NMRA's standards (S-9.2.2's CVs, the maker IDs) and the decoder makers' own published manuals, never copied or adapted from another program's files (the usual open-source model railroad software's library is GPL). Leah doesn't install, read, download from, or talk to that software: it competes with it in its own way, as a language whose addon's sample apps can grow to do what that software does. Its manuals are read as a reference for what to build (a scanning session, as the language's own was built from the usual app builder's manual), and the product's names are among the forbidden names (2026-11-11).

**The user's model railroad electronics** (NCE boards and USB interface, DCC-EX motor and Wi-Fi shields, a Digitrax programmer, signals, lighting, powered trucks): [docs/maker_inventory.md](../maker_inventory.md), section 14 (2026-10-10).
