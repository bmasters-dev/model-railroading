# Model railroading — Leah's own DCC system, and dead rail

**A 3rd-party addon since 2026-11-09 (the owner's decision: the test of Leah's 3rd-party addon system; Maker stays one of Leah's own):** this repository, `bmasters-dev/model-railroading` (`leah-addon/`), built with Leah's addon kit and installed by `leah --install-addon https://github.com/bmasters-dev/model-railroading`, its releases tagged `leah-addon-v<version>`. So it reaches Leah only through the kit: it opens the base station board's serial port itself (its own crate, not Leah's serial port) and sends its own board program (it can't use the Maker addon's). Its issues are in this repository; the links below to Leah's specs and issues are from when it was one of Leah's own addons.

**Status: stage 1a built (2026-11-09): the simulated base station.** Was planned, not started. Rewritten 2026-10-13 after the user's decision on #9: **Leah is the command station.** It doesn't drive other makers' command stations (NCE, DCC-EX, Digitrax, Z21); it competes with them, by building DCC in. The first plan (2026-10-06: drivers for DCC-EX, Z21, and LocoNet) is superseded. Each stage is designed with the user before it is built, as the controls are. Tracked on GitHub: [#32](https://github.com/bmasters-dev/leah-language/issues/32), milestone "Model railroading".

## Goal

A Leah app runs model trains **by itself**, with no other command station and no JMRI: throttles, panels, decoder settings, automation, phones as throttles. Two ways to reach a locomotive, behind one Leah object:

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

## What Leah would build (the JMRI-like whole, Leah's way)

| Part | Leah |
|---|---|
| The command station | Leah's base station: its board program, and a `dcc_base()` object in Leah apps (USB or Wi-Fi) |
| Throttles | `locomotive()` objects, the same for DCC and dead rail (speed, direction, functions, consists), and a ready-made throttle control |
| Turnouts and sensors | DCC accessory decoders through the base station; or straight from pins and boards (servos through the PCA9685, maker.md's Direct mode); sensors (block detectors such as the NCE BD20) read on the base station's pins, as events |
| Decoder settings (CVs) | the programming track (service mode) and writes on the main (operations mode); the NMRA standard CVs by name |
| Panels | turnout, sensor, and block objects with events, drawn with the canvas; a track diagram control later |
| Roster | locomotives and their settings, saved with the `database` object |
| Automation | timers and events for the basics; blocks, routes, and signal logic as built-ins |
| Phones as throttles | a built-in WiThrottle server, so phone throttle apps (Engine Driver, WiThrottle) drive trains through a Leah app ([the protocol](https://www.jmri.org/help/en/package/jmri/jmrit/withrottle/Protocol.shtml)) |

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
- [ ] **7. The rest of the JMRI-like whole**: functions up to F68, consists, the roster, blocks, routes and signals, a track diagram, automation.
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
- **A classic Uno or a Nano reads it back** on **pin D2** (INT0, an interrupt on every edge), measures
  each half-bit, decodes the packets, and sends them to Leah over USB. Not the Uno R4 WiFi: its chip is
  different, and the classic ATmega328P board is the one the reader is written for.
- **Two wires**: Mega pin 12 → a **1 kΩ resistor** → the reader's D2 (the resistor keeps either board safe
  if a pin is set wrong), and Mega GND → the reader's GND. Both boards 5 V, so no level shifting.
- **Both boards on USB** to the PC, which powers them; no other supply connected.
- The owner's account needs the `dialout` group to open the ports (`sudo usermod -aG dialout $USER`, then
  log out and in).
- **The app must make that clear to its user** (the owner, 2026-11-10: end users won't know about
  `dialout`). `dcc_base("/dev/ttyACM0")` refused by the system gives a `hardware_error` of kind
  `"access_denied"` (as `serial_port` does) in plain words. The example app catches it and shows a
  window explaining it without jargon ("Linux needs your permission before apps can talk to the
  board"), with a **Give access** button that runs `fix_device_access("serial")` (it asks for the
  password) when the devices addon is there, the command to type otherwise, and then **log out and back
  in**, which the window says plainly. The board list shows a board that is plugged in but not
  allowed as such, not as missing.

## Rules to keep (agreed in principle)

- **Emergency stop** always works, from code and from every throttle control.
- **A lost connection stops the trains**: the base station stops them if it stops hearing from the app, and a dead-rail locomotive stops if it stops hearing from the base station.
- **A short circuit cuts track power** at once, and says so.
- Built on the cross-cutting rules: hardware problems are hard errors (error_handling.md); events run on the GUI thread; scripts never deal with threads.

## Open questions

- **The base station's board** (stage 1): ESP32 or the Mega with the motor shield.
- **Dead rail's radio messages** (stage 6): Leah's own, over ESP-NOW; whether a handheld throttle can talk to a locomotive directly.
- **Decoder definitions:** JMRI's large library of decoder definitions (thousands of models with their CVs) is GPL-licensed. Reading them from the user's own JMRI files may be fine; building them into Leah raises a licensing question. Starting with the NMRA standard CVs avoids it. To look into properly at stage 3.

**The user's model railroad electronics** (NCE boards and USB interface, DCC-EX motor and Wi-Fi shields, a Digitrax programmer, signals, lighting, powered trucks): [docs/maker_inventory.md](../maker_inventory.md), section 14 (2026-10-10).
