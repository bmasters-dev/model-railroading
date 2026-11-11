# Panels and the layout's objects (helper 2 of 4)

Area: the panel tool, its tables of layout objects (turnouts, sensors, lights, blocks, reporters, memories, routes, sections, transits, ID tags, audio, ...), and its four panel editors. Signals are covered only briefly (another helper has them in depth); dispatching (sections, transits, automatic trains) only as far as the tables go.

## 1. Overview

The reference software models a layout as a set of **named objects**, each kept in a table of its own: outputs ("turnouts", which really means any output bit), inputs ("sensors"), lights, blocks, reporters, memories, routes, signal heads and masts, sections, transits, ID tags, and more. Every object has a fixed system name (connection letter + type letter + hardware address, such as "LT23" for output 23 on the first connection, or "IS999" for an internal, software-only input), an optional free-text user name that scripts and panels prefer, a state, a comment, and custom properties; anything can listen for its changes. Most objects can be created a range at a time, and internal (hardware-less) objects are used as glue for logic. Everything (all tables and all panels) is stored together in one configuration file, loaded at start-up. On top of the objects sit four panel editors: a free icon-based panel (pictures placed over a background), a menu-driven variant of it with shapes and a palette, a ready-made switchboard grid, and a "layout editor" that draws a connected track diagram and from it derives blocks, paths, signal logic, entry/exit routes and train tracking. Panels can also be shown and operated in a web browser.

## 2. Features

Legend: **Use** = how widely modellers use it (core / common / niche). **Leah** = does Leah have it today (yes / partly / no).

### 2.1 The named-object model (the foundation)

**Named layout objects with a state and change events**
- What: every turnout, sensor, light, block, etc. is an object with a name, a state, and listeners; panels, routes, logic and scripts all refer to the same object, so a click on one panel shows up everywhere.
- Use: core. Leah: no (only `dcc_base` and `locomotive`).
- Leah way: plain objects made by calling their kind, with properties and `on_*` events like any Leah control or class: `$t = turnout(12, name: "West Yard Lead")`, `$t.state`, `$t.on_change = function($t) { }`. A shared registry object, `layout()`, holds them all: `$layout.turnout("West Yard Lead")`, `$layout.sensors`, `$layout.find("Lake Siding")`. Every object carries `name`, `address`, `connection`, `state`, `comment`, `properties` (free key/value array), and `on_change`.

**System name vs user name**
- What: a fixed technical identifier derived from the connection and address, plus a renamable human name; references through the user name let a modeller rewire (move "East OS Occupancy" from input 12 to input 24) without touching routes or panels.
- Use: core. Leah: no.
- Leah way: keep the two ideas but in plain words: `address` (what the hardware needs, e.g. `"dcc:12"`, `"base:pin 22"`, `"lcc:05.01.01.01.22.00.00.FF"`) and `name` (what everything else uses). Renaming an address keeps every reference working because references hold the name. A clear error ("no turnout named `Wset Yard Lead`; did you mean ...?") instead of silently loading a broken route.

**Connection prefixes / several systems at once**
- What: a layout may use one system for trains and another for layout control (for example DCC for locomotives and C/MRI or LocoNet for inputs and outputs); each connection has its own letter, and objects pick their connection.
- Use: common. Leah: partly (one `dcc_base`).
- Leah way: each connection is an object (`dcc_base`, later `lcc_bus`, `cmri_bus`, `loconet`), and layout objects take it: `turnout(12, on: $base)`, `sensor("2:3", on: $cmri)`. A default connection set once (`$layout.default_connection = $base`).

**Internal objects (no hardware)**
- What: turnouts, sensors, memories that exist only in software, used as panel buttons, logic flags, "reset" triggers and phantom addresses that throttles can command to fire a route.
- Use: common. Leah: no.
- Leah way: an object with no address is internal: `sensor(name: "Reset")`. Leah variables already cover most uses, so internal objects matter mainly for panels and for things other programs (phones, the web) must see.

**Unknown and inconsistent states**
- What: besides on/off, objects start "unknown" (nothing heard from the layout yet) and can be "inconsistent" (contradictory feedback, points moving); panels show these with distinct icons.
- Use: core. Leah: no.
- Leah way: `state` is text: `"closed"`, `"thrown"`, `"unknown"`, `"inconsistent"` (turnouts); `"active"`, `"inactive"`, `"unknown"`, `"inconsistent"` (sensors). Constants or an `enum` so typos are caught.

**Add a sequential range**
- What: create many objects at once from consecutive addresses, numbering the user names.
- Use: common. Leah: no.
- Leah way: a `for` loop already does it; a helper `make_turnouts(first: 1, count: 16, name: "Yard %")` is a nicety.

**Query and forget**
- What: ask the hardware for an object's current state, or reset the program's idea of it to unknown.
- Use: common. Leah: no.
- Leah way: `$s.query()` and `$s.forget()`; `$layout.query_all()` after connecting.

**One file for everything, loaded at start**
- What: all tables and panels are stored together in one configuration file, reloaded automatically at start; the occupied/value state is not stored (except block values, below).
- Use: core. Leah: partly (Leah has JSON, settings files, the database addon).
- Leah way: `write_layout_file($layout, $path)` and `read_layout_file($path)` (Leah's `_file` naming), as JSON or as Leah code, readable and diffable in git; or simply define the layout in a `.leah` include file (`include "my_layout.leah"`), which is the most Leah-like answer: the layout *is* code.

**Start-up initialisation**
- What: an "initializer" runs once after loading to put the layout in a known state (all turnouts closed, occupancy sensors reset).
- Use: common. Leah: yes in spirit (a script's top level runs at start).
- Leah way: nothing new needed; a sample shows `$layout.set_all_turnouts("closed", delay: 250)`.

**Where used**
- What: lists every route, panel icon, logic rule and signal pair that refers to an object, before you rename or delete it.
- Use: common. Leah: partly (the API's references, the language server).
- Leah way: the IDE's "find references" already works on code; if the layout is code, it covers this. For a layout file, `$layout.uses("West Yard Lead")`.

**Object tables (spreadsheet-like editors)**
- What: each kind has a table window with sortable, hideable columns, a clickable state column, inline editing of names and comments, and edit dialogs with tabs.
- Use: core (it is how users set everything up). Leah: partly (list box, property list).
- Leah way: a ready-made `layout_table` control (a list box bound to `$layout`, one tab per kind, state cell clickable) and a sample "layout manager" app. The IDE's designer could also show layout objects in its tray of unseen objects.

### 2.2 Turnouts (outputs)

**Throw and close**
- What: command an accessory output to its two states; "turnout" is used for any output bit, so it also drives uncouplers, signal lamps and relays.
- Use: core. Leah: no (accessory packets not yet).
- Leah way: `$t = turnout(12)`, `$t.throw()`, `$t.close()`, `$t.toggle()`, `$t.state = "thrown"`. Sent as an NMRA basic accessory packet through `dcc_base`, or later to a servo through the Maker addon's PCA9685 driver (`turnout(servo: $pca.channel(3), closed_angle: 80, thrown_angle: 100)`).

**Commanded state vs known state, and feedback modes**
- What: a turnout tracks what was last asked for and what the layout reports; feedback can be none ("assume it moved"), one sensor (active = thrown), two sensors (one per end of throw, mid-throw = inconsistent), a timed delay (inconsistent for a few seconds, then the new state, good for slow motors and servos), or monitoring the command bus.
- Use: common (core for anyone with slow-motion machines or hidden track). Leah: no.
- Leah way: `$t.commanded` and `$t.state` (known); `$t.feedback = "none" | "one_sensor" | "two_sensors" | "delay"`, `$t.feedback_sensors = [$s1, $s2]`, `$t.move_time = 4000`; events `on_change` (known state) and `on_mismatch` (commanded and known disagree after the move time). A panel shows "moving" while inconsistent.

**Inverted**
- What: swap closed and thrown for a turnout wired backwards.
- Use: common. Leah: no. Leah way: `$t.inverted = true`.

**Retry**
- What: resend the command a number of times (fixed count, or until the feedback sensor confirms), for machines that sometimes fail to move.
- Use: niche. Leah: no. Leah way: `$t.retries = 3`, `$t.retry_interval = 500` (stops early when feedback agrees).

**Lock**
- What: refuse changes from handheld cabs (by sending the opposite command back at once) and/or from fascia pushbuttons (by a decoder-specific setting), so a route can't be broken under a train.
- Use: niche (needs specific decoders). Leah: no.
- Leah way: `$t.locked = true` blocks Leah's own commands and, since Leah is the command station, can simply ignore throttle accessory commands for it; pushbutton lockout only if a decoder documents it.

**Speeds through the turnout**
- What: a closed-route and a thrown-route speed (named or numeric), used by automatic running and by signal aspect choice.
- Use: niche. Leah: no. Leah way: `$t.closed_speed`, `$t.thrown_speed`, when signals and automation come.

**Linked turnouts (crossovers, three-ways)**
- What: one drawn crossover may need two or four machines on different addresses moving together, optionally one inverted; a three-way is two turnouts tied together.
- Use: common. Leah: no.
- Leah way: `turnout(12, also: turnout(13, inverted: true))`, or a `turnout_group` whose state sets every member.

**Turnout control tool**
- What: a small window to type an address and throw or close it, without setting anything up.
- Use: common (bench and testing). Leah: no.
- Leah way: an example app, `accessory_tester.leah`, beside the bench tester; or a one-liner in the console: `turnout(12).throw()`.

### 2.3 Sensors (inputs)

**Active / inactive inputs with events**
- What: an input from the layout (block detector, fascia pushbutton, track contact); the program normally doesn't set it, the hardware does, but it can be clicked for testing.
- Use: core. Leah: no.
- Leah way: `$s = sensor(5, on: $base)`, `$s.active` (true/false), `$s.state`, events `on_active`, `on_inactive`, `on_change`. Inputs come from the base station's pins first (the design already plans BD20-style detectors there), later from LCC/C/MRI. `$s.simulate("active")` for testing without track.

**Debounce delays**
- What: a separate delay before accepting "active" and before accepting "inactive", per sensor or global, to ignore dirty track and electrical noise.
- Use: common (core for current detection). Leah: no.
- Leah way: `$s.active_delay = 0`, `$s.inactive_delay = 1500` (a train's lost wheels shouldn't drop occupancy); `$layout.default_inactive_delay`.

**Inverted, pull-up/pull-down**
- What: swap the meaning; tell hardware that supports it to pull the input up or down.
- Use: common / niche. Leah: no. Leah way: `$s.inverted`, `$s.pull = "up"` (for the base station's own pins this is the Arduino's internal pull-up).

**Initial state of internal sensors**
- What: choose what software-only sensors read after start.
- Use: niche. Leah: no. Leah way: a constructor setting: `sensor(name: "Reset", state: "inactive")`.

**What the detector sees**
- What: a descriptive setting (wheels, vehicle body, a tag) used only as documentation.
- Use: niche. Leah: no. Leah way: skip, or a free `kind` property.

**Sensor groups (one of many)**
- What: a set of sensors of which at most one is active; activating one clears the others, to imitate a rotary switch.
- Use: niche. Leah: no.
- Leah way: `sensor_group([$a, $b, $c])`, or simply Leah's radio buttons on a panel; a few lines of code otherwise.

**Hardware address formats**
- What: each system has its own way to write an input's address (board:channel, node:pin, cab:pin, event pairs), with conversion formulas.
- Use: core for those systems. Leah: no.
- Leah way: each connection object parses its own addresses; accept the familiar "board:channel" form where a system uses it.

### 2.4 Lights (simple outputs with automatic control)

**On/off outputs for scenery and fascia lamps**
- What: an output meant for lights, uncouplers, animations and sounds; shares output bits with turnouts but without feedback.
- Use: common. Leah: no.
- Leah way: `light(30)` with `on`, `off()`, `toggle()`; or just a turnout used as an output. Leah can avoid the split by calling both `output` and letting `turnout` add the feedback (see Skip).

**Light controllers**
- What: a light can follow a sensor, either of two sensors, a turnout's state, a fast-clock time window ("on 21:00 to 07:00"), or switch on for a set time when a sensor triggers; several controllers per light, and an "enabled" switch to suspend them.
- Use: common. Leah: no.
- Leah way: these are one-line events in Leah (`$s.on_active = function() { $lamp.on = true }`), so offer them as conveniences: `$lamp.follow($sensor)`, `$lamp.follow($turnout, on_when: "thrown")`, `$lamp.on_between("21:00", "07:00", clock: $fast_clock)`, `$lamp.pulse(5000)`, `$lamp.enabled`.

**Variable intensity and fades**
- What: brightness 0 to 1 with minimum and maximum limits and a transition time, typically for day/night scenery lighting following the fast clock.
- Use: niche to common (scenery lighting fans). Leah: no.
- Leah way: `$lamp.brightness = 0.4`, `$lamp.fade_to(1.0, seconds: 30)`, with limits `min_brightness`, `max_brightness`; on PWM pins or the PCA9685 through the Maker addon.

### 2.5 Blocks (occupancy and train tracking)

**A block = a stretch of track with an occupancy sensor**
- What: a named piece of track whose occupied state comes from one sensor; the basis of signals, routing, dispatching and coloured track on panels. Not all track has to be in a block.
- Use: core. Leah: no.
- Leah way: `$b = block("Lake Siding", sensor: $s)`, `$b.occupied`, events `on_occupied`, `on_cleared`; occupied sense invertible.

**Block value (train tracking)**
- What: each block can carry a value, usually a train name or a locomotive; when a train moves into the next block, the value passes along with it, and is cleared when the block empties. Values can be saved at shutdown and restored at start (always, only for still-occupied blocks, or only if all are still occupied).
- Use: common. Leah: no.
- Leah way: `$b.train` (any Leah value, e.g. a `locomotive` object) passed along automatically between neighbouring blocks (needs the connections, from the track diagram); `on_train_entered($train)`. Restoring at start with `$layout.restore_trains(rule: "all_occupied")`. Leah can do better than text: the value is the real `locomotive` object, so a panel label can show its speed too.

**Reporter attached to a block**
- What: a block takes its value from an identity reader (RFID, transponder, RailCom) instead of the user typing it; with a choice of the current report (cleared when the reader stops seeing) or the last report (kept).
- Use: niche (growing with RailCom). Leah: no.
- Leah way: `block(..., reader: $rfid_reader)`; `$b.keep_last_report = true` by default (the manual itself recommends that for tracking).

**Physical details: length, curvature, speed, permissive, braking section, route lengths**
- What: optional facts used by automatic running and signal logic: block length (in or cm), how tight the curves are (slows automatic trains), a speed limit, whether a train may enter while occupied, which neighbour has a brake section, and different lengths for different routes through a turnout block.
- Use: niche (automation users). Leah: no.
- Leah way: plain properties on `block` (`length_cm`, `curvature`, `speed_limit`, `permissive`), added when automation is designed.

**Ghost blocks**
- What: a block without a real detector, simulated by an internal sensor, so very short or turnout blocks can still form signal boundaries for automatic running.
- Use: niche. Leah: no. Leah way: `block(..., sensor: null, simulated: true)` later, with dispatching.

**Paths and direction**
- What: each block knows its entry points and which turnout settings each path needs; derived automatically from the drawn track diagram, not entered by hand; a block also keeps a direction.
- Use: core for anything automatic (but hidden from users). Leah: no.
- Leah way: computed by the track diagram's model (below); `$b.neighbours`, `$b.paths` read-only.

**A second, finer block model (occupancy blocks with portals and paths)**
- What: a separate model of track circuits joined by portals, with explicit paths through each, built by pointing and clicking on an icon panel; used by the "warrant" style of automatic running.
- Use: niche. Leah: no. Leah way: skip as a separate model (see Skip): one block model with portals as its connections.

### 2.6 Reporters, ID tags, RailCom

**Reporters**
- What: an input that carries text, not just on/off: which locomotive a transponder or RFID reader sees ("1234 enter"), with a current report (cleared when nothing is seen) and a last report (kept).
- Use: niche to common. Leah: no.
- Leah way: `reader` objects with `current`, `last`, `on_report($tag)`; for Leah's own base station, RailCom detection later (stage 8 of the addon's plan).

**ID tags table**
- What: every tag ever seen (rolling-stock RFID tags, decoder addresses) with where and when it was last seen, optionally on the fast clock; kept in a file of its own.
- Use: niche. Leah: no.
- Leah way: `$layout.tags` (a list of `id_tag` records: `id`, `name`, `last_reader`, `last_seen`), saved with the database addon; a sample "where is my car" app.

**RailCom details**
- What: per address: where and when seen, reported speed, orientation on the track, direction of travel, moving or stopped, signal quality.
- Use: niche today. Leah: no.
- Leah way: properties on the reader's report record; ties into `locomotive` (a `$loco.last_seen` block).

### 2.7 Memories, string I/O, global variables

**Memories (named values shown on panels)**
- What: a named slot holding any value, set by logic or scripts and shown on panels as text, an icon chosen by value, or an editable input or spinner.
- Use: common (because the reference software has no real variables in its logic). Leah: yes in spirit (variables).
- Leah way: Leah variables plus a panel label bound to an expression; for values other programs (web panels, phones) must see, `$layout.value("next_train")`. A label's icon-by-value is a small lookup array, no file editing.

**String I/O**
- What: a named text channel to hardware that accepts and returns strings (newer feature).
- Use: niche. Leah: partly (serial ports, sockets). Leah way: not needed; the connection objects already send and receive text.

**Global variables for the rule engine; data tables for the rule engine**
- What: named typed variables and CSV-backed tables for the reference software's if-then rule system.
- Use: common among rule users. Leah: yes (it is a programming language).
- Leah way: nothing to add.

### 2.8 Routes

**Routes: set many turnouts (and sensors) at once**
- What: a named set of target states (closed, thrown, toggle) applied together, with a pause between commands (250 ms by default, adjustable) so the power supply and decoders cope; used for yard ladders, mainline clearing, and putting the layout in its start position. A route can also play a sound or run a script, and can be copied to make similar routes.
- Use: core. Leah: no.
- Leah way: `$r = route("Track 5", [$t1: "thrown", $t2: "closed", $t3: "toggle"], delay: 250)`, `$r.set()`, `$r.busy`, `on_done`. The sound and the script are just a Leah function in `on_set`.

**Route triggers and vetoes**
- What: up to three sensors and one turnout can trigger a route (on active, on inactive, on change; on closed, on thrown), and any of them can be a veto (no triggering while a block is occupied); a route can be enabled and disabled. Commanding a "phantom" accessory address from a handheld throttle can fire a route.
- Use: common. Leah: no.
- Leah way: `$r.trigger($button, when: "active")`, `$r.veto($block.sensor, when: "active")`, `$r.enabled`; plain Leah logic does any combination. Phantom addresses: `$base.on_accessory = function($address, $state) { }`, since Leah is the command station and sees every accessory command from a throttle.

**Alignment sensor and route lock**
- What: an internal sensor goes active when every turnout in the route is actually in place (by feedback), and a route can lock all its turnouts.
- Use: common (alignment), niche (lock). Leah: no.
- Leah way: `$r.aligned` (true when every member's known state matches) with `on_aligned`, `$r.locked`.

**Extended routes (lights, signal heads as outputs and triggers)**
- What: a more general route type whose triggers and actions can be any mix of sensors, turnouts, lights and signal heads, implemented underneath as rules.
- Use: common. Leah: yes in spirit.
- Leah way: not a separate kind: Leah's `route` takes any layout object with a state, and anything more is code.

### 2.9 Entry/exit routing (push one button, then another)

**Entry/exit (NX) routes**
- What: the operator clicks an entry point then an exit point on the track diagram; the program finds a clear path between them through the connected track (fewest blocks, avoiding occupied ones), sets every turnout, and optionally creates temporary signal logic and reserves the blocks (shown in another colour) until the train has passed. Routes can span several panels joined at their edges, be set "both ways", be built from several segments, be stacked to wait for a conflicting route to clear, or be cleared down by hand when there is no detection. The entry button flashes until an exit is chosen and gives up after a minute.
- Use: common (a favourite on interlocking-style panels). Leah: no.
- Leah way: a method on the track diagram's model, `$layout.find_path($from, $to, avoid_occupied: true)` returning the turnout settings and blocks, and `$layout.set_route($from, $to, reserve: true)`; on the `track_diagram` control, `entry_exit: true` turns the boundary markers into buttons. Path finding is a natural thing for a programming language to expose directly (the reference software keeps it inside its editor).

### 2.10 Sections and transits (for dispatching; brief)

**Sections**
- What: one or more blocks in a row, allocated to one train at a time and travelled "forward" or "reverse", with optional direction sensors (to force opposing signals to stop) and stopping sensors at each end; can be generated from signal pairs.
- Use: niche (dispatching users). Leah: no.
- Leah way: later, with automation: `section([$b1, $b2], ...)`, `$sec.allocate($train, "forward")`, `$sec.state`.

**Transits and their actions**
- What: an ordered list of sections making a train's itinerary (with alternatives for passing sidings, back-and-forth running); actions fire on section or block entry and exit, train stop and start, or a sensor, after an optional delay, and do things like pause, set or ramp speed, horn, bell, lights, set a decoder function, set a sensor, hold or release a signal.
- Use: niche. Leah: no.
- Leah way: in Leah, an itinerary is a script: `on_entered("Station") { $loco.ramp_to(0, seconds: 5); sleep(30000); ... }`; a light `train_run` helper later. (The automation helper covers this.)

### 2.11 Signals (brief)

**Signal heads, masts, groups, repeaters, and logic derived from the track diagram**
- What: heads are single lamps with appearances (red, yellow, green, flashing, dark, lunar), held and lit flags; masts show prototype aspects from a chosen signalling system and drive heads or decoders; groups add junction indicators; repeaters copy one mast to another. With a drawn track diagram, the program discovers every pair of masts and builds the logic (which blocks must be clear, which turnout settings, which speed) automatically.
- Use: common. Leah: no.
- Leah way: see the signals helper. From this area: signal objects follow the same named-object pattern (`state`/`aspect`, `held`, `lit`, `on_change`), and automatic pair discovery is a method of the track diagram model (`$layout.signal_pairs()`).

### 2.12 Panels: the four editors

**Icon panels (free placement over a background)**
- What: a window where pictures stand for layout objects: each object kind has a set of pictures, one per state (closed, thrown, unknown, inconsistent), and clicking a turnout picture throws it; backgrounds can be one drawing or several slices side by side (for classic CTC machine panels); icons can be rotated, scaled, layered (levels 1 to 10), grouped, aligned, locked in place, hidden when not editing, and given tooltips; labels have fonts, colours, borders and fixed sizes; a panel can be set not to control the layout (display only).
- Use: core. Leah: partly (positioned controls, image viewer, canvas, shapes, the IDE's window designer).
- Leah way: no new editor: the IDE's designer plus a few **layout-bound controls**: `turnout_icon` (pictures per state, `turnout:` property, click toggles), `sensor_indicator` (lamp or jewel, click toggles for testing), `light_switch`, `block_label` (shows the block's train), `signal_icon`. Each takes a `pictures` array keyed by state, using Leah's picture sets for sharp icons. `controls_layout = false` on a window makes it display-only. Leah ships a default icon set (drawn by Leah, so no third-party icon licence).

**The icon palette and icon sets**
- What: a tabbed palette of every icon kind; each kind has several named icon sets (large and small jewels in various colours, levers) that users can edit, add, rename and delete; a catalogue browses folders of images; drag an icon onto the panel.
- Use: common. Leah: partly (the designer has a palette).
- Leah way: the designer's palette gains a "Layout" group with the controls above; icon sets are just picture folders named by state (`closed.svg`, `thrown.svg`, `unknown.svg`).

**Lever and multi-position controls**
- What: one icon tied to several sensors, each position of a lever activating one of them (a CTC lever with left, centre, right).
- Use: niche to common (CTC panels). Leah: partly (segmented button, radio buttons).
- Leah way: a `lever` control (positions, pictures, `on_change`) or a segmented button bound to a `sensor_group`.

**Menu-driven editor with shapes, zoom and copy/paste**
- What: the same panels edited through menus on the panel itself, with cut/copy/paste between panels, selecting by type or level, zoom and zoom-to-fit, and drawn rectangles, ellipses and polygons whose visibility or level a sensor can control (to show help text or highlight things).
- Use: common. Leah: yes mostly (the designer; shapes; canvas).
- Leah way: `visible_when: $sensor` (or an expression) on any control; zoom-to-fit as a window property (`scale_to_fit = true`) for touch screens.

**Indicator track (coloured track for occupancy and reservation)**
- What: track pictures whose colour shows the state of the track circuit: clear, occupied, reserved for a train, the reserved train's position, out of use, power fault; can show the train's name.
- Use: common. Leah: no.
- Leah way: built into the `track_diagram` control (below) with a `colors` array per state; for icon panels, a `track_piece` control bound to a block.

**Switchboard (a ready-made grid)**
- What: a panel that fills itself with one switch per address in a range (turnouts, sensors or lights on one connection), drawn as plain buttons, sliders, keyboard-style keys or schematic symbols; arrows page to the next range; right-click names an item; hide addresses with nothing connected.
- Use: common (the quickest way to a working panel). Leah: no.
- Leah way: a `switchboard` control (`kind: "turnout"`, `first: 1`, `count: 24`, `style: "slider"`, `on: $base`), and a sample app `accessory_board.leah`. Leah's switch, toggle buttons and segmented buttons already exist, so it is mostly a grid that creates them.

**Layout editor: a drawn, connected track diagram**
- What: the user draws the track (right- and left-hand turnouts, wyes, single and double crossovers, single and double slips, diamonds, turntables, traversers, track segments, anchor points, end bumpers, and edge connectors linking to other panels). Connection points turn from red to green as they are joined, so an unfinished diagram is visible. Track segments can be straight, arcs, ellipses or Bezier curves; mainline and siding have different widths; segments can be dashed (tunnels) or hidden; decorations add arrows, bridges and tunnels. Each piece is assigned a block, and the program then knows the full connectivity: it builds block paths, colours track by occupancy, finds signal pairs, entry/exit routes and sections, and passes train names along. Grid with snapping, rotation, translate and scale tools, one-level undo of a move, and checks for unconnected track, track without a block, blocks in two separate places, needless anchors, and curves that are really straight. Turnouts animate to show their state and are clicked to throw (or left-click closes, right-click throws). Drawing styles (rails, ballast, ties, block lines, presets) and turnout circles sized for touch.
- Use: core (the recommended starting point in the manual's own getting-started guide). Leah: no.
- Leah way: the biggest piece of new work, in two halves:
  - a **track model** (no window): `$layout.add_turnout(...)`, `add_track($from, $to)`, `connect(...)`, with connection points named a, b, c, d as in the reference software's convention; queries `neighbours`, `find_path`, `signal_pairs`, `check()` (unconnected ends, blocks in two places). Usable by scripts with no display at all, and by tests.
  - a **`track_diagram` control** that draws the model with Leah's canvas engine (cairo), animates turnouts, colours blocks, and handles clicks (`on_turnout_click`, `on_block_click`), plus an edit mode in the IDE's designer for drawing it. Saved as Leah code or JSON, not as a coordinate blob.

**Turntables and traversers**
- What: a turntable with any number of rays, or a traverser with slot pairs; with DCC control, each ray or slot maps to an accessory address and state, so clicking a ray turns the bridge (for common turntable controllers); optional signals guarding the bridge.
- Use: niche to common. Leah: no.
- Leah way: `turntable` object (`positions` array of accessory commands, `position`, `turn_to(5)`, `on_arrived`) and its drawing in the track diagram.

**Markers (manual train tracking)**
- What: small coloured labels with a train or locomotive number that the operator drags along the diagram, for layouts without detection; one made from the roster can open a throttle.
- Use: niche. Leah: no.
- Leah way: `track_diagram.add_marker($loco)`, draggable, with a right-click to open Leah's throttle control.

**Moving icons from a positioning system**
- What: icons that move to follow a radio or ultrasonic positioning system, after a two-point calibration.
- Use: niche (rare hardware). Leah: no. Leah way: skip (or `place_marker($x, $y)` on the diagram).

**Fast clock, reporter, memory and audio icons**
- What: a clock face on the panel, labels showing reporters' and memories' values, input fields and spinners that edit a value from the panel (also from a web browser), and a sound button that plays on the computer or in the browser.
- Use: common. Leah: yes mostly (labels, text fields, up/down arrows, a sound addon).
- Leah way: bind ordinary controls to layout values; a `fast_clock` object belongs to the automation/operations helper's area.

**Tooltips**
- What: by default an icon's tooltip shows its object's names (always current), or a custom text, per icon or for the whole panel.
- Use: common. Leah: yes (controls have tooltips). Leah way: layout-bound controls default their tooltip to the object's name and address.

**Several panels of one layout**
- What: many panels can show and control the same objects (a schematic, a CTC machine, a switchboard); a panel keeps working when hidden; layout-editor panels join at edge connectors.
- Use: common. Leah: yes in spirit (several windows over one `$layout`).
- Leah way: nothing special; all controls bind to the same objects.

**Web panels**
- What: every loaded panel can be opened, operated or shown read-only in a browser (tablets at the fascia), kept up to date over a WebSocket.
- Use: common. Leah: partly (the web addon's `http_server`, web sockets).
- Leah way: a `layout_server` (or an option on the web addon's server) that publishes `$layout` as JSON and events over a web socket, and a ready-made page that draws the switchboard and the track diagram; phones as throttles are already planned (WiThrottle).

**Panel file as a readable report**
- What: the configuration file can be shown as a formatted, printable web page with a table of contents.
- Use: niche. Leah: partly (reports addon). Leah way: if the layout is Leah code, the IDE shows it; a sample report prints the object tables.

**CTC tools**
- What: a tool that configures centralised-traffic-control logic in a dialog and builds a skeleton CTC panel (levers, code buttons, indicators).
- Use: niche. Leah: no. Leah way: a sample app, `ctc_panel.leah`, built from levers and routes, rather than a built-in.

### 2.13 Other objects in the tables

**Audio objects (positioned sounds)**
- What: sound buffers loaded from WAV files and sources placed in 3D around a listener, with gain, pitch, distance roll-off, fades and movement, played by rules or scripts.
- Use: niche. Leah: partly (the sound addon plays files and tones).
- Leah way: use the sound addon; panning by position is a small addition if ever wanted.

**Analog values, power districts**
- What: named numeric inputs (a current meter) and power managers per district.
- Use: niche / common. Leah: partly (`dcc_base` reports current and power).
- Leah way: `$base.current` already; later `power_district` objects if Leah's base station gets boosters.

## 3. Leah's own advantages

- **The layout is code.** The reference software keeps everything in tables plus an if-then rule editor plus scripts in another language. In Leah one language does all three: objects, panels, and logic, with names checked when the script is read and "did you mean" on typos.
- **One object model, events everywhere.** Every layout object is a Leah object with `on_*` events, the same as a button; no separate "listener" API, no separate routes-versus-extended-routes kinds.
- **The track model works without a window.** Path finding, signal pairs and checks as functions mean tests, command-line tools and phone servers can use them; the reference software keeps that knowledge inside its editor.
- **The command station is Leah.** Since Leah drives the track itself, "phantom" throttle addresses, cab lockout and accessory monitoring are just events on `dcc_base`, with no command-bus sniffing or decoder-specific tricks.
- **Simulation built in.** The simulated base station already exists; sensors and turnouts can be simulated the same way (`simulate()`, feedback delays), so panels and routes are tested on the bench before any wiring.
- **The IDE's window designer** places layout-bound controls like any control, with the layout objects in the designer's tray, instead of four editors with different mouse rules.
- **Readable, diffable files** in git and the daily backup, rather than one large machine-written file that users are told to hand-edit for some features.
- **Single-file apps.** A club's panel can be built into one program or AppImage for the layout computer.
- **Real values in blocks.** A block's train is the `locomotive` object itself, so a label can show speed and functions, not just a name.
- **Undo, find references, rename** come from the IDE and language server rather than per-table warnings.

## 4. Skip list

- **Per-system address formats for dozens of command stations and buses**: Leah talks to its own base station first; add a bus's format only when Leah supports that bus.
- **The connection-letter + type-letter system-name scheme** ("LT23", "IS999"): replace with a `name` and an `address`; the letters exist to fit everything into one text field.
- **Two block models** (plain blocks and the finer occupancy blocks with portals): one model, with the connections (portals) derived from the track diagram.
- **Four separate panel editors**: one designer plus a `track_diagram` control and a `switchboard` control.
- **Table-driven rule engines and extended routes**: Leah's code replaces them; keep `route` only for its everyday convenience (delays, alignment, triggers).
- **Memories and global variables as objects**: Leah has variables; only values that outside clients must see need publishing.
- **Hand-edited XML for value-dependent icons**: a lookup array in code.
- **Multi-slice background images for CTC panels**: the canvas or one background picture is enough.
- **Decoder-specific pushbutton lockout and feedback modes tied to one maker's stationary decoder**: niche, undocumented standards.
- **Positioning-system moving icons and the 3D audio listener**: rare hardware and niche use; Leah's sound addon is enough.
- **"Panel has no menu" and similar legacy switches**, and the old simple signal logic beside the newer mast logic (keep one signal model).
- **The separate light/turnout split for output bits**: keep `turnout` (with feedback) and `light` (with brightness) as objects, but let both sit on the same output layer, so nothing is lost by choosing one.

## 5. Pages read

All paths are relative to help/en; "…" stands for the reference software's own package folders.

html/apps/(the panel tool's folder)/: index, Panels (same text as index), GettingStarted, ShowPanel, ToolsMenu, DebugMenu, HelpMenu, FAQ, AdvIcons (9 pages).

package/…/beantable/: TurnoutTable, TurnoutAddEdit, SensorTable, SensorAddEdit, LightTable, LightAddEdit (first half; the controller sections), BlockTable, BlockAddEdit, BlockEdit, OBlockTable, RouteTable, RouteAddEdit, LRouteTable (glance), LRouteAddEdit (glance), MemoryTable, MemoryAddEdit, ReporterTable, ReporterAddEdit, IdTagTable, IdTagAddEdit, RailComTable, StringIOTable (glance), StringIOAddEdit, AudioTable (glance), AudioAddEdit, SectionTable (headings), SectionAddEdit (headings), TransitTable (headings), TransitAddEdit (actions part), AddEditAction, ViewSpecialActions (start), SignalHeadTable (start), SignalAddEdit (start), SignalMastTable (start), SignalMastAddEdit (start), SignalGroupTable (start), SignalGroupAddEdit (title only), SignalMastLogicTable (start), AddEditSignalingLogic (title only), SignalMastRepeater (start), the rule engine's tables: its rule table (start), rule add/edit (title only), ConditionalAddEdit (start), and the newer rule engine's table, data-table, module and global-variable pages (start). Signals and the rule engine were read briefly on purpose (other helpers' areas).

package/…/display/: PanelEditor, ControlPanelEditor, ItemPalette, IconAdder, SwitchboardEditor, LayoutEditor (all sections, the long signal-placing tool descriptions skimmed), EditTurntable, EditTraverser, CircuitBuilder (start and headings), CoordinateEdit, PanelTarget, PanelMenuHelp, Tooltips, TranslateSelection, RpsIcon, AudioIcon, MultiSensorIconFrame (an empty placeholder page).

html/tools/: index (listing), Turnouts, Sensors, Lights, Blocks, Routes, LRoutes (first half), Memories, Reporters, IdTags, StringIO, Audio (first half), Sections, Transits, EntryExit, CTC.

Also: html/doc/Technical/Names (names and naming), html/web/PanelServlet (web panels), package/…/sensorgroup/SensorGroupFrame, package/…/whereused/WhereUsed, package/…/entryexit/EntryExitFrame (glance), html/apps/LoadStoreWork (glance).

About 95 pages in all, most read whole.

**Missing or thin pages that seemed important:** the multi-sensor icon page is an empty placeholder; the layout editor's per-dialog help pages (edit turnout, edit track segment, set signals at ..., the scale/translate dialog) and the track-drawing-options page were linked but not in this area's folders; the light-control and turnout-control tool pages live under other folders (simplelightctrl, simpleturnoutctrl) and were not read; the "turnout feedback" technical page (html/doc/Technical/TurnoutFeedback) was found but not read (the turnout table page already explains the modes). The block-tracking and dispatcher pages are outside this area.
