# The model railroading addon: the plan from the scanning session (2026-11-11)

From the saved manual of the usual open-source model railroad software (its decoder program and its
panel program, and the pages they link to: 804 pages, read by four helpers). Detailed findings, with
the "how Leah would do it" for every item: 1-decoders-roster-throttles.md, 2-panels-layout-objects.md,
3-signals-logic-automation.md, 4-hardware-connections.md. The product is never named.

**Leah's way, in one line:** one language instead of the reference software's five layers (tables,
two rule-table tools, signal logic, Python scripts, Java automatons); one set of layout objects with
events, like any Leah control; one track model that works without a window; Leah's own command station
at the heart, simulated for testing; single-file apps for clubs.

Ranking: by value to modellers and by what the rest builds on. Size: S (hours), M (a day or two),
L (several days).

## Phase A: programming and running (builds on what exists)

1. **Programming on the main** (ops mode): `$loco.write_cv($cv, $value)`, a pause between writes,
   address 0 refused. Core. M.
2. **Programming extras**: bit read/write, `verify_cv`, `write_cv(verify:)`, paged and register modes,
   indexed CVs (CV31/32), plain-word `programming_error` kinds with `on_progress`. Core/common. M.
3. **Decoder identification and named bits**: `identify_decoder()` (CV8 maker names from the NMRA list,
   CV7, product IDs), `cv29_parts()` / `make_cv29()`, a deliberate `reset_decoder()`, decoder locks
   (CV15/16, never in "write all"). Core. S-M.
4. **Leah's own decoder definitions**: the NMRA standard set built in; a `decoder_definition` format of
   Leah's own (settings with CV, mask, kind, range, choices, groups, conditions) written from the NMRA
   standards and makers' manuals; a `decoder_programmer` (read all, write changes, each setting's state:
   read / written / edited / unknown). The "decoder workshop" sample app on `property_list`. Core. L.
5. **The roster**: `roster()` and entries (road name and number, address, decoder, every CV, function
   labels with momentary/latching, groups, pictures, attributes, speed limit), import/export,
   `locomotive(roster: "UP 792")`. Core. M-L.
6. **Throttle extras**: 14/28/128 speed steps, a per-locomotive speed limit, momentary functions from the
   roster, keyboard control, `$base.locomotives()` (all running, with an event), stop all; a gamepad
   throttle sample (the core's `game_controller`). Core/common. M.
7. **Consists**: software consists (trivial on Leah's base) and advanced ones (CV19, CV21/22), with a
   check for forgotten CV19s. Common. M.

## Phase B: phones and the network

8. **The phone throttle server (WiThrottle protocol)**: roster, multi-throttle, functions with labels,
   a heartbeat that stops a locomotive when a phone goes quiet, turnouts, routes, consists, the clock;
   announced on the local network. Core (how most modellers run trains today). M-L.
9. **A browser throttle** sample on the web addon's `http_server` and `web_socket`. Common. M.

## Phase C: the layout

10. **Layout objects**: `turnout` (accessory packets on Leah's base; commanded and known state;
    feedback none / one sensor / two sensors / timed for servos; inverting, retries, locking), `sensor`
    (debounce on and off, inverting, `simulate()`), `light`, `block` (occupancy, the train in it as the
    real `locomotive`), kept in a `layout()` that saves and loads. Core. L.
11. **Waiting helpers**: `wait_for($sensor, "active", timeout:)`, `wait_for_any(...)` inside tasks:
    nearly every train script is "wait, then act". Core. S.
12. **Routes and the track plan**: `route()` (many turnouts, spacing, aligned check, lock); a
    `track_plan` (which block joins which through which turnout position) with
    `find_path(from:, to:, via:, avoid:)`; entry/exit routing with interlocking (reserve, refuse
    conflicts, release as the train clears). Core/common. L.
13. **Panels**: a `track_diagram` control (track coloured by occupancy, click to throw), a `switchboard`
    control, panel controls bound to layout objects for the IDE's designer (turnout icon, sensor
    indicator, lever, block label), web panels. Core. L.

## Phase D: signals, speed, and automation

14. **Signals**: `signal_mast` with named aspects from signalling-system data files of Leah's own (from
    public rulebook facts), aspects worked out from the `track_plan`; an `abs_signal` helper. Common. L.
15. **A fast clock**: rate, run/pause, `call_at`, shown on throttles and panels. Common. S.
16. **Speed**: a `speedometer` from two or three sensors (or a cheap Arduino IR gate), automatic speed
    profiling, speed matching (always restoring momentum), scale helpers, software ramping. Common. M-L.
17. **Running trains automatically**: one `auto_train` with a block allocator (in place of the reference
    software's three warrant kinds and separate dispatcher); per-section actions as plain event
    functions. Common. L.
18. **A layout simulator** built from the DCC simulator: simulated turnouts, sensors, and trains moving
    through blocks, so every sample and test runs with no layout. Common (and for Leah's own tests). M.

## Phase E: other systems (optional addons, in this order)

19. **NCE** (the owner's gear; published binary protocol; a capability list per link, since the USB
    interface loses programming on a Power Pro; raw DCC packets let Leah's packet builder drive any NCE
    station). M-L.
20. **DCC-EX** (open; tested on the owner's own Mega and shield with DCC-EX's firmware; idea: Leah's
    own board program could also speak DCC-EX's common commands, so phone apps that speak DCC-EX work
    with Leah's base). M.
21. **MQTT layout objects** (Leah already has MQTT; cheap ESP32/Arduino sensors and outputs; the same
    message words as the usual sketches). S-M.
22. **LocoNet** (Digitrax and others; standalone LocoNet boards beside other systems; tested with
    recorded traffic or a fake LocoNet-over-TCP server). L.
23. **LCC / OpenLCB** (NMRA standard, on Leah's `can_bus`, tested on Linux's virtual CAN). L.
24. **C/MRI** (NMRA-published polled serial; the owner's Arduinos as nodes). M.
Then: Z21, XpressNet, ECoS, SPROG, MERG CBUS, Märklin, BiDiB, as people ask.

## Later, or an addon of its own

- **Virtual sound** on the PC following a locomotive (the sound addon; starts quiet). Niche.
- **Operations** (car forwarding, switch lists, manifests): a large product of its own; a later addon.
- **RailCom** (reading CVs on the main, locomotive positions): when Leah's base gets the cutout.
- Positioning systems, RFID readers (beyond MQTT), RailDriver, roller-stand speedometers: on request.

## Skip

Logic-table tools and their editors, memory/global-variable objects, system-name letter schemes,
Python/Java scripting add-ons (Leah is the language); two block models, four panel editors, three
warrant kinds (one of each in Leah); XSLT-built definitions; hand-edited XML features; old export
formats; vendor-specific signal-head drivers for other systems; the legacy simple-signal tool as a
second engine; bundling the reference software's GPL decoder definitions or copying its data files.

## Rules for the work

- The product is never named (forbidden names); its manual is a reference for what to build, never a
  source of text or data. Leah's decoder and signalling definitions come from the NMRA's standards and
  the makers' own published manuals.
- Every feature works on Leah's simulated base first (tests without hardware), then on the bench.
- Each phase ends with a sample app that shows it off.
