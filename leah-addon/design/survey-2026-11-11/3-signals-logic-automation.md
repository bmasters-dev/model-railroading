# Signals, logic, and automation (helper 3)

Area: signals and signaling systems, blocks and detection, routes and interlocking, CTC, the two logic-table tools, scripting, automatic running (warrants and dispatching), the fast clock and timetables, and operations (car forwarding, switch lists). Written from the reference software's saved manual; facts only, in my own words.

Format of each feature: **name** (how widely used; Leah today) followed by what it does for a modeller, then *Leah:* how Leah could offer it. Usage: core = nearly everyone who goes past running one train; common; niche. Proposed names follow the addon's design, which already sketches `turnout(12).throw()` and `$block = sensor(5)`.

## 1. Overview

The reference software turns a computer into the layout's brain. It works on layout objects (turnouts, sensors, lights, signal heads and masts, blocks, memories) and builds several layers on them. **Signaling** goes from a beginner's "simple signal logic" (ABS: red if the next block is occupied or a turnout is against you, yellow if the next signal is red) to full aspect signaling. There, signal masts show named aspects from 20+ prototype rulebooks, and source-to-destination "mast logic" is worked out on its own from a drawn track diagram. **Routes** set many turnouts with one button. **Entry/exit (NX)** routing finds a path between two buttons and can lock it as an interlocking. A **CTC** tool models a US&S dispatcher's machine (code button, levers, traffic, route, and time locking). **Two logic-table tools** (an older if-then form and a newer tree of actions and expressions with modules, tables, variables, formulas, and a debugger) let people automate without code. **Scripting** in Python (Jython) is the escape hatch: it has listeners, "automaton" threads that wait for sensors, and dozens of sample scripts. **Automatic running** comes in two separate systems. *Warrants* record or compute a train's throttle commands over a route of occupancy blocks, then follow signals and slow down ahead of them. The *dispatcher* allocates sections of track to "active trains" along transits, runs auto trains that obey signals, and fires per-section actions (station stops, horn patterns, hand-over to a human). A scripted layer on top of it sets up stations, schedules, and departure boards. A **fast clock** (internal or synced to LocoNet/CBUS clocks) drives timed actions. A **timetable** tool draws train graphs. **Operations** generates car movements, manifests, and switch lists from a car roster, industries, and train routes.

## 2. Features

### A. Layout objects underneath everything

- **Turnout with commanded vs. known state** (core; Leah: no, only sketched). A turnout has a commanded state, and it can also have a known state taken from feedback (one or two sensors). Logic treats "commanded but not yet confirmed" as unsafe: a signal stays red while the points move. *Leah:* `turnout(12)` with `.throw()`, `.close()`, `.toggle()`, `.state` (commanded), `.known_state` (`"closed"`, `"thrown"`, `"moving"`, `"unknown"`), `feedback: [sensor(40), sensor(41)]`, and `on_change`/`on_confirmed` events.
- **Turnout locking** (common; no). Refuses changes from panel buttons and/or handheld throttles while locked (cab lockout). Routes, CTC, and interlockings use it. *Leah:* `.locked = true`, `lock_from: ["panel", "throttles"]`. The base station already sees accessory packets from throttles, so it can refuse them.
- **Sensor** (core; no). An occupancy detector, a fascia button, or a feedback contact: active/inactive/unknown. *Leah:* `sensor(5)` with `.active`, an `on_change` event, and `debounce: 200` (the reference software ships a debounce script, which shows the need), plus `invert:`.
- **Internal (virtual) sensors and turnouts** (core; no). Objects with no hardware, used as panel buttons, flags, and stand-ins in simulation. *Leah:* `sensor("NX Down", virtual: true)`, or plain Leah variables. Virtual objects matter mostly so panels and the simulator can treat them like real ones.
- **Request state from hardware at start** (common; no). At start-up the program asks every sensor and turnout for its state. Sample scripts set unknown sensors to inactive and all turnouts to closed, and save turnout states to CSV between sessions. *Leah:* `layout_refresh()` (ask the hardware), `save_layout_states($path)` / `restore_layout_states($path)`. The rest is plain Leah code.
- **Sensor group (radio-button behaviour)** (niche; no). At most one sensor of a group may be active; this simulates a rotary switch on a panel. *Leah:* plain Leah code covers it (an `on_change` that clears the others). A `sensor_group([...])` helper is optional.
- **Followers / light controllers** (common; no). An output follows an input: a turnout follows a fascia toggle (with invert and a veto sensor), a panel lamp follows a turnout or block, a light follows a fast-clock time span. *Leah:* plain Leah code (`sensor(3).on_change = function($s) { turnout(7).thrown = $s.active }`). Perhaps one helper, `follow($input, $output, invert:, unless:)`, for the many fascia-panel cases.

### B. Blocks, detection, and the track graph

- **Block with occupancy** (core; no). A block is a stretch of track with one or more detectors. It also carries a length, a speed limit, curvature, a "permissive" flag, and a value (usually the train in it). Signaling, NX, warrants, and the dispatcher all build on blocks. *Leah:* `block("Main 3", sensors: [sensor(5)], length: 120, speed_limit: 40)` with `.occupied`, `.train`, `.reserved_by`, and `on_occupied` / `on_cleared` events.
- **Block tracking (train names move with trains)** (common; no). When a block next to an occupied one becomes occupied, the train's name moves into it. A separate "tracker" shows each train's block and time in the block, and asks the user when two trains could have made the same move. *Leah:* `train_tracker` (or `block.train` filled in by the track graph), with `on_train_moved($train, $from, $to)` and an `on_ambiguous` event the app answers. A sample panel shows the names on the diagram.
- **Track graph (paths, portals, turnout settings)** (core for anything automatic; no). The program needs to know which block connects to which, through which turnout in which position. It learns this from a drawn diagram, from a "circuit" tool, or from scripts. *Leah:* a `track_plan` object. Connections can be written in code (`$plan.connect("A", "B", turnout: turnout(3), position: "thrown")`) or later read from a track-diagram control. Signals, NX, auto trains, and the simulator all sit on this one object.
- **Route finding with costs** (common; no). It finds the cheapest path between two places. Each block has a cost (main line cheap, sidings dear). The search avoids occupied or reserved blocks, can force or avoid blocks on the way, and respects one-way block links. *Leah:* `$plan.find_path(from:, to:, via:, avoid:, max_blocks:)`. It returns the blocks and the turnout positions; `$path.set()` lines them.
- **Detection modes and train length** (common; no). Whole-train detection, head and tail, or head only, together with the train's length, decide when a block counts as free behind a train. *Leah:* `detection: "whole" | "head_and_tail" | "head"` and `length:` on a train object.
- **Transponding / RailCom reporters** (common with Digitrax and RailCom users; no). A block reports *which* locomotive is in it; scripts format these reports for display. *Leah:* a `reporter` event on blocks (`on_reported($block, $address)`), arriving when the base station adds RailCom (the design already lists RailCom for later).

### C. Signals

- **Signal head** (common; no). One lamp unit, set to an appearance: red, yellow, green, lunar, their flashing forms, or dark. It can also be held or unlit. Drivers include 1 output (two looks plus flashing), 2 outputs (yellow = both on), 3 outputs (one per lamp), 3-output RGB, 4 outputs (with lunar), the NMRA extended-accessory "signal decoder" packet, and vendor boards. *Leah:* `signal_head(outputs: [...], kind: "triple")` or `signal_head(dcc_address: 201)`, with `.appearance = "flashing_yellow"`, `.held`, `.lit`. Flashing is timed by Leah itself.
- **Signal mast with named aspects** (core for anyone signaling to a rulebook; no). A whole signal shows an aspect ("Approach Medium") taken from a chosen signaling system and mast type, and the program works out which head shows what. Aspects can be disabled, the mast can be unlit, and it can be held. Drivers:
  - heads
  - one turnout command per aspect
  - an output truth table (1–6 outputs per aspect)
  - the NMRA extended accessory packet (aspect numbers 0–31, with an offset-address variant)
  - virtual (no hardware)

  *Leah:* `signal_mast(system: "basic", type: "three_aspect", driver: ...)` with `.aspect`, `.aspects`, `.held`, `.lit`, an `on_aspect_changed` event, and `$mast.info("Clear").speed`. Leah's base station can send the extended accessory packet itself.
- **Signaling system definitions** (common; no). Each system has an aspect list with indication, speed, "speed at next signal", route (normal/diverging), and default DCC aspect number. Each mast type maps aspects to head appearances and says which aspect to show for each aspect of the signal ahead. Four special looks (danger, permissive/call-on, held, dark) round it out. Users copy and tweak these definitions. *Leah:* data files in the addon (TOML or JSON, Leah's own), loaded with `signal_system("basic")`. Ship a few (a basic 3-aspect, a US speed-signaling set, a UK 4-aspect set), and document the format so users can add their railroad.
- **Mast logic (source → destination pairs)** (core for aspect signaling; no). Each mast lists the next masts it can lead to, and for each the blocks, turnout positions, sensors, and other masts that must be right. The first rule that holds releases the mast from Stop. The aspect follows from the next mast's aspect, from diverging versus normal, and from the lowest block speed on the way. All of this can be discovered automatically from the track diagram, and virtual masts mark buffer stops and dark territory. *Leah:* `$plan.signal_rules()` builds the pairs from the `track_plan`, and `$mast.leads_to($next, blocks:, turnouts:)` sets one by hand. A built-in engine re-evaluates when any watched object changes. This is the single most valuable signaling built-in, because writing it by hand for a whole layout is error-prone.
- **Simple ABS logic** (core for beginners; no). For single-head signals there are four cases: plain block, main or diverging leg of a trailing turnout, and facing point. The signal is red if the protected block is occupied, the turnout is against it, or the turnout is moving; yellow before a red; optionally flashing yellow before a yellow. It also has "distant" (copies the next signal), limited speed, restricting, and approach lighting. *Leah:* a helper `abs_signal($head, protects: [block(...)], next: $other, turnout:, leg: "main")`. Since the reference software publishes the rule as about forty lines of code, a Leah sample app that shows the same rule in plain Leah is also a good teaching piece.
- **Approach lighting** (common; no). A signal stays dark until a train approaches; the panel still shows the aspect. *Leah:* `lit_when: block("East approach")` on heads and masts.
- **Hold and release** (common; no). A dispatcher, CTC machine, interlocking, or auto train forces a signal to its most restrictive aspect. *Leah:* the `.held` property, plus an `on_held` event.
- **Signal groups (route, junction, and speed indicators)** (niche, UK and Europe; no). Extra indicators beside a mast (junction feathers for up to 6 routes, platform letters, speed digits) light according to the mast's aspect and turnout positions. *Leah:* plain Leah code in the mast's `on_aspect_changed`. A small `indicator` property on masts could cover the common feather case.
- **Mast repeater** (niche; no). One mast copies another (one way or both ways), for fascia repeaters or as a trick when automatic discovery misses a second route. *Leah:* plain Leah code (`$m1.on_aspect_changed = function($m) { $m2.aspect = $m.aspect }`). With Leah's own logic engine the "missed route" workaround is unnecessary.
- **Cab signals** (niche; no). The next signal's aspect is sent to a throttle's display (Digitrax semaphore icons, CBUS). *Leah:* an `on_next_signal($loco, $aspect)` event from the auto-train or tracker layer. Leah's own throttle sample app can show it on screen, which is better than a few blades on a handheld.
- **Vendor signal boards** (niche; no). Some boards have fixed turnout-command tables per aspect (European LS-DEC style, the SE8C, MERG SD2, and others). *Leah:* covered by the generic "output table" mast driver. Ship ready-made tables only if users ask.

### D. Routes and interlocking

- **Routes (several turnouts at once)** (core; no). One button lines a whole yard ladder or main line: turnouts and sensors set in order, 250 ms apart by default (adjustable), with optional sound or script. A route can be triggered by up to three sensors or a control turnout, with on-active, on-inactive, on-change, and veto rules. It can lock its turnouts, and it can drive an "aligned" sensor that shows when the layout matches. *Leah:* `route("Yard 3", [turnout(1): "thrown", turnout(4): "closed"], spacing: 250)` with `.set()`, `.is_aligned`, `on_aligned`, and `lock: true`. Triggers are plain Leah events, and `copy_route` is not needed because a route is just a Leah array.
- **Extended routes** (common; no). The same, but any number of triggers, including signal appearances and lights, with vetoes. *Leah:* plain Leah code covers it (events plus `if`).
- **Entry/exit (NX) routing** (common, especially UK-style panels; no). Press an entry button, then an exit button within 10 s, and the cheapest free path is found and its turnouts set. Three levels:
  1. turnouts only
  2. turnouts plus mast logic
  3. full interlock: the path's blocks are reserved and shown in another colour, conflicting paths are refused, and the path is released as the train clears it

  The source signal is held until a route exists. Options include "both ways", cancel versus clear-down (pretend a train went through), and pairs across linked panels. *Leah:* `nx_panel($plan)` with `$nx.press($button)`, or simply `$plan.set_route(from:, to:, interlock: true)`. The latter returns a `route_lock` with `.cancel()`, `.clear_down()`, `on_released`, and automatic release from block occupancy. A sample "NX panel" app would draw the diagram with the canvas.
- **Interlocking rules** (common among CTC and interlocking fans; no). Five kinds of locking:
  - route (signal) locking: no changes while a cleared signal admits a train
  - indication locking: the turnouts in a cleared route can't move
  - traffic locking: no opposing signals onto one track
  - time locking / running time: a signal taken back to stop keeps the route locked for N seconds
  - occupancy locking: no turnout changes under a train

  Call-on lets a train enter an occupied block. *Leah:* an `interlocking` engine with `.lock_route($route)` and `.can_set($turnout)` that returns a reason in plain words. CTC and NX both use it.
- **CTC machine** (common among CTC builders; no). An editor defines each OS section's switch lever, signal lever (L/N/R), code button, indicator lamps, and call-on. A run-time then applies code delays, fleeting, traffic, route, indication, and time locking, and a maintainer call. It also generates a skeleton panel. The older tool built the same from scripts: a code line, stations, locks, a track-circuit lamp with a bell, and a log of which lock refused. *Leah:* a `ctc_machine` helper (`ctc_section(lever:, switch_lever:, code_button:, signals_left:, signals_right:)`) on the interlocking engine. On-screen levers are gui controls, and a physical panel is just sensors and outputs. A showcase sample app, "CTC panel", should come with the relay-click sounds the reference software mentions.

### E. Logic without code (the two logic-table tools)

- **If-then conditionals** (core in the reference software; Leah: yes, the language itself). "If sensor A is active and turnout B is closed then set signal C". Triggers fire on any watched change. There are on-change-only versus always modes, AND/OR/mixed "antecedent" formulas, delayed set actions, fast-clock ranges, memory comparisons, warrant and NX states, run script, and play sound. *Leah:* plain Leah code covers it. Two small helpers would remove boilerplate:
  - `watch([sensor(1), turnout(2), block("A")], $function)`: one function called when any of them changes (the "siglet" pattern)
  - `after(5, $function)`: already `call_later`

  Leah's `--trace` and the IDE debugger replace the table tool's step debugger.
- **Next-generation logic trees** (common among power users; Leah: yes, the language). Trees of actions and expressions build in the following:
  - if-then-else, many, for, for-each, break, continue, return
  - modules with inputs and outputs
  - local and global variables
  - Java-like formulas
  - indirect references (a memory naming a turnout)
  - CSV lookup tables with named rows and columns
  - timers, sequences, "execute delayed", listen-on-objects
  - error-handling modes (dialog, log, log once, throw, abort)
  - a clipboard, row locking, a where-used search
  - an IDE-style debugger with breakpoints and a statement log
  - web requests, JSON decoding, MQTT, running programs, a file-as-flag check, a "power line lost" check

  This is a programming language rebuilt as forms. *Leah:* plain Leah code covers all of it: functions, arrays and maps, `read_csv_file`, try/catch, `on_error`, the web and network addons, `run_command`, and the language server's find-references for where-used. Nothing to build. Instead, a guide page could translate "logic table" thinking into Leah.
- **Turnout-feedback simulation** (common when testing; no). An action makes turnouts report "confirmed" after a delay, so logic can be tested without hardware. *Leah:* part of the simulated base station: `simulate_feedback: 0.5` (seconds).
- **Start-up logic** (core; partly: a script runs from the top). Logic runs once at load: set initial states, start the clock, and so on. *Leah:* plain Leah code; the top of the script is the start-up.

### F. Scripting and automation programs

- **Scripting language with layout access** (core for advanced users; Leah: yes, it *is* the language). Python 2.7 (Jython) has managers for turnouts, sensors, and the rest, plus property-change listeners, start-up scripts, buttons added to the main window, and widgets in throttle windows. Python 3 is experimental and not on all systems. *Leah:* this is Leah's whole reason to exist. The addon's objects should feel native (`turnout(3).thrown = true`, events as properties), with no manager lookups.
- **Automaton threads that wait** (common; partly: tasks exist). The script runs in its own thread with "wait for sensor active", "wait for any of these to change", "wait N ms", and "get a throttle and wait for it". A thread table shows the running ones and can stop them. *Leah:* `wait_for(sensor(4), "active", timeout: 30)`, `wait_for_any([...])`, and `wait_for_block(block("B"), "occupied")`, usable inside `run_in_background` tasks and the event loop. Leah's task list and cancel already stand in for the thread table. This is the most useful scripting helper to add, because almost every sample script is "wait, then act".
- **The sample-script catalogue** (common as starting points; no). These make good Leah sample apps:
  - back-and-forth shuttle between two sensors, or timed
  - a "robot throttle" that runs a loco around following signals and blocks
  - layout power off after N minutes without block activity
  - crossing-gate or bell sound when a block is entered
  - an alarm clock off the fast clock
  - debounce a sensor
  - turnout and memory state saved across sessions
  - assign RFID tags to cars
  - an N-way selector
  - number entry from 10 sensors
  - spoken announcements when a power district trips
  - a loco test sequence (lights and functions in turn)
  - two trains that slow down when one gains on the other (position system)
  - sensor logging
  - USB game-pad or RailDriver as a throttle
  - a power-status lamp

  *Leah:* a "Model railroad samples" folder with about 8 of these written in Leah, each a screen long. Start with the shuttle, power-off-when-idle, the crossing bell, the loco test, and sensor logging. They show Leah's advantage at once.
- **English-like train scripts** (common; Leah: yes). A popular add-on script reads one text file per train ("Assign short address 3", "Wait for sensor X active", "Set speed 0.4", "Sound horn", "Repeat if…") so users avoid Python. *Leah:* Leah's syntax is already that readable; a sample with three trains as three small `run_in_background` functions shows it. Nothing to build.

### G. Running trains automatically

- **Speed profiles** (core for automatic running; no). Each loco is timed over a measured block, forward and reverse, at chosen speed steps, using three sensors in a row (start, timed, finish) and a sensor-delay adjustment. The result is a step → scale-speed table, interpolated and stored with the roster entry. It is needed to stop at a point and to slow down in time. *Leah:* `$loco.speed_profile` (a table) and `measure_speed_profile($loco, start:, timed:, finish:, length_mm:, steps:)` as a background job with progress. A sample app, "speed profiler", draws the curve with the chart control. It belongs with the roster (helper 1 or 2 may cover it too).
- **Software momentum / ramping** (common; no). Speed changes come in steps at a chosen rate (none, fast, medium, slow). A newer option is physics-based ramping from the loco's weight, power, and tractive effort. *Leah:* `$loco.ramp_to(0.6, seconds: 4)` and `$loco.momentum = ...` (Leah's own, so it works the same with any decoder).
- **Stopping at a point** (common; no). A train can stop at a stop sensor, at a stop distance into a block (worked out from the speed profile and block lengths, and corrected at each block boundary), or by train-fits-in-section logic. It has a safety stop if a stop sensor fails (stop on entering an unexpected block). *Leah:* `$train.stop_at(block("Station 1"), distance_mm: 300)` with a `stop_sensor:` option. Built into the auto-train object.
- **Warrants (record or compute a run over a route)** (common; no). A route of occupancy blocks (paths and portals) is set and allocated. A recorded throttle session (commands with elapsed times) is played back, or the ramps are computed (the "NX warrant": origin, destination, via, avoid, start and end distance, max speed). While running, it looks ahead the stopping distance and ramps down for restrictive signals, block speed limits, or a "rogue" train ahead, then ramps back up. It has halt, resume, and e-stop, plus a fast-reacting variant for small layouts with short blocks. Dark blocks run on time alone. A session log is kept. *Leah:* one `auto_train` object with these options, not three warrant kinds:
  - `auto_train($loco, path: $plan.find_path(...), max_speed: 0.7, follow_signals: true)`
  - `.start()`, `.halt()`, `.resume()`, `.emergency_stop()`
  - events: `on_block_entered`, `on_signal`, `on_arrived`, `on_blocked`

  Separately, a `throttle_recorder` records a hand-driven run and plays it back on the same path.
- **Dispatcher: sections, transits, and active trains** (common at clubs and among automation users; no).
  - *Sections* are runs of blocks with a forward and reverse direction, one train at a time. They can have direction "sensors" that force opposing signals to red, stop sensors, and stop points.
  - *Transits* are ordered sections, with alternates for passing sidings and an option to reverse back to the start.
  - An *active train* links a transit to a train (from the roster, from operations, or typed in) with a start and end block, priority, and type. It can start at a fast-clock time and wait before restarting.
  - Allocation of sections can be manual or automatic, with automatic release, a request queue by priority, "extra" sections for switching moves, and automatic turnout setting.
  - Auto trains follow signals (green = max, yellow ≈ 35%, red = stop in this section) and have a max speed and a per-loco speed compensation (50–150%).
  - Per-section actions combine a "when" (section or block entry or exit, train stop or start, sensor change, after a delay) with a "what": pause N fast-clock minutes, set or ramp speed, set max speed, hand over to a human and wait for a "done" sensor, lights, bell, horn for a time or a pattern like "long-short-long", any function, set a sensor, hold or release a signal, e-stop, end the train, or chain the next saved train.
  - A side pane can stop all trains, take one to manual, or resume.
  - Ad-hoc transits can be made between two blocks, and train setups can be saved to files.

  *Leah:* two built-ins:
  - a `block_allocator` (reserve and release a block chain for a train, a priority queue of requests, `on_granted`)
  - the `auto_train` above, which asks it for blocks ahead

  The "actions" are simply Leah code in the train's events (`$t.on_block_entered = function($b) { if $b.name == "Station" { $t.pause(fast_minutes: 3) } }`). Sections and transits as stored objects are optional; a Leah list of blocks is enough. The dispatcher's window becomes a sample app ("Dispatcher board") on Leah's list box and canvas.
- **Station-to-station system with schedules and departure boards** (niche but a strong showcase; no). Stop blocks are marked. Stations, routes, and saved trains are generated, and the program finds the best route from where a train is to where it should go, express or stopping. Departure times can be typed or worked out from timed runs, with station dwell times and a running period on the fast clock. Station announcements are spoken, and timetables are shown on screen or on tablets through MQTT. *Leah:* a showcase sample app, "Station dispatcher", using `find_path`, `auto_train`, the fast clock, the sound addon's speech, and the web or network addons for a departure board in a browser. It shows off what a programming language makes short.
- **Auto-train simulator** (common while setting up; partly: Leah has a DCC simulator). It moves occupancy along a running train's allocated blocks so the whole system can be tested with no trains. *Leah:* extend the existing simulator into a `layout_simulator($plan)`. It moves virtual trains through the `track_plan` at their profile speeds and turns sensors on and off, so every signaling and automation sample runs without a layout. This is a real advantage for Leah, since its tests and samples already rely on simulation.

### H. Time

- **Fast clock** (core for operating sessions; no). An internal timebase has a rate (for example 4:1), start and pause, set time, and start-up options (time, paused or running). It can sync with LocoNet or CBUS hardware clocks (send or receive, and correct a drifting command-station clock), runs at 12 or 24 hours, and exposes a "clock running" sensor and a current-time memory. Displays include analog, LCD, nixie-tube, and flip clocks, in a window or on a panel. *Leah:* `fast_clock(rate: 4, start: "06:00")` with `.time`, `.run()`, `.pause()`, `.rate`, `on_minute`, `$clock.call_at("07:15", $f)`, and `$clock.between("18:00", "06:30")`. A clock control (analog and digital looks) belongs in the gui addon or as a canvas sample. Sharing the clock over the network (MQTT or a small web page for phones) is better than LocoNet slots.
- **Fast-clock-driven actions** (common; no). Street and house lights follow the clock, an alarm pulses an output, and dispatcher pauses count in fast minutes. *Leah:* plain Leah code with the clock's events and `between`.
- **Timetable and train graph** (niche, timetable-and-train-order fans; no). It holds layouts (scale, clock rate), train types with colours, segments with stations (distance, double track, sidings, staging), schedules (start hour, duration), and trains with stops (dwell, speed). It works out times, draws the time-distance graph (printable), and imports or exports CSV or the operations data. *Leah:* a sample app, "Train graph", on the canvas and chart, with CSV in and out, plus a helper `scale_speed($scale, $clock_rate)`.

### I. Operations (car forwarding)

- **Car forwarding, manifests, and switch lists** (core for the operations crowd, a large separate world; no). The model:
  - a car roster: road, number, type, length, weight, load, owner, and the RFID or value fields
  - locations with spurs, yards, staging, and classification/interchange tracks, each with a length and accepted types, roads, loads, and trains
  - train routes with direction, moves, max length, departure and travel times, caboose or FRED rules, and loco changes

  Train *builds* pick cars by least-moved counters ("random") or by routing towards final destinations, possibly across several trains through interchanges. Spur *schedules* (sequential or match) and custom loads, return-when-empty or loaded, divisions, quick service, and an "on time" mode let cars ride several trains in one session using a dwell time. The program prints manifests, per-location switch lists, and detailed build reports, and moves train icons on the panel. *Leah:* not part of the signals and automation core. It is best as its own later addon or library ("operations") on the database addon, with a sample app that makes a switch list from a small CSV roster. Start with the simple form: random least-moved cars plus spur schedules, printed through Leah's reports addon.

### J. Tools around the logic

- **Where-used search** (common; partly). Find every rule, route, panel icon, or CTC section that refers to an object before deleting it. *Leah:* the language server's find-references does this for code. For a `track_plan`, add `$plan.uses($object)`.
- **Debugging the logic** (common; yes). The reference software has a step debugger and statement logging for its logic trees, plus a console. *Leah:* `--trace`, the IDE debugger, and `on_error` already cover it.

## 3. Leah's own advantages

- **One language instead of five layers.** The reference software has a simple signal form, mast logic, an old and a new logic-table tool, extended routes made of hidden logic tables, Python scripts, and Java automatons. Each layer was added because the one before ran out of power. In Leah every layer is the same code, so nothing has to be "converted" or imported between tools.
- **The newer logic tool reinvents a language in dialogs**: variables, formulas, loops, modules with parameters, CSV tables, error handling, a debugger. Leah has all of this natively, readable, versioned in git, and searchable.
- **Waiting is natural.** A train script in Leah reads top to bottom: `wait_for(sensor(4), "active")`, then `$loco.speed = 0`. There are no listener classes, thread subclasses, or "handle" loops.
- **No name juggling.** Objects are values (`turnout(12)`) rather than system-name strings like `LT12`/`IT_1_1`, or memory-based indirection (`{IM1}`). An array or map of turnouts does what the newer tool's "references" and "tables" do.
- **One track model.** The reference software keeps two incompatible block models (diagram blocks for signals and the dispatcher, separate occupancy blocks for warrants) and three warrant types. Leah can have one `track_plan`, one `auto_train`, and one `block_allocator`.
- **Actions are just functions.** The dispatcher's fixed menu of "when" and "what" (horn pattern "lsl", pause N fast minutes, and so on) becomes ordinary event functions that can do anything: speech, a web call, a database write.
- **Simulation built in.** Leah already simulates its base station; extending that to a layout simulator lets every signaling and automation sample, and the tests, run without track power. The reference software needs a special script to fake occupancy.
- **Single-file apps.** A club's dispatcher board, CTC panel, or departure board can be built into one program file and run on any Linux computer, with no Java install and no preferences setup.
- **Shown without a GUI form designer**, yet a GUI when wanted: Leah's gui addon, canvas, chart, and list box make panels and boards in a few lines.

## 4. Skip list

- **Two logic-table tools and their editors** (conditional forms, tree editor, clipboard, sockets, antecedent formulas, row locking, import from the old tool). Leah code replaces them; building a form-based rule editor would duplicate the language. If anything, the IDE could later offer snippets.
- **System-name conventions and memory indirection** (`LT12`, `IS1`, `{IM1}`, table-cell references). Leah uses values and maps.
- **Python, Python 3, AppleScript, Java automatons, GUI plug-ins into throttle windows.** Leah is the scripting language.
- **The legacy simple-signal tool as a separate system.** Offer one `abs_signal` helper on the same engine as masts, not a second engine that must not be mixed with the first.
- **Three warrant types plus a separate dispatcher.** Merge them into one auto-train with options.
- **Section "direction sensors" (internal sensors used to force opposing signals red).** That is a workaround for logic that can't see allocations; Leah's signal engine can read the allocator directly.
- **Vendor-specific signal-head drivers** (the LS-DEC family, SE8C, Grapevine, the Acela SignalMan, the MERG SD2) and the LocoNet "Bushby" forwarder. These are hardware-specific to other systems; the generic output-table driver covers the boards, and Leah's own base station doesn't have the Bushby problem.
- **Cab signals on Digitrax handhelds.** Show the next aspect in Leah's own throttle app instead.
- **Exotic clock faces** (nixie, flip-clock styles) at first. One analog and one digital face is enough.
- **The full operations system now.** It is a large separate product. Keep it for a later addon, after signals and automation, unless the owner wants operations first.
- **Its XML signaling-definition files and sample panel files.** Write Leah's own definitions from the public rulebook facts rather than copying the project's files (they belong to that project).

## 5. Pages read

About 152 pages (paths relative to `help/en`; `P/` stands for the reference software's tools folder, `package/…/…t/`; `OPS/` for its operations user guide folder in `manual/`). Long pages were read in full or by section. The 360 KB operations page was read by its table of contents and key sections, and the European signal-decoder example pages were skimmed.

- html/tools/: Blocks, EntryExit, Logix, LRoutes, Routes, Sections, Signals, TimeTable, Transits
- html/tools/signaling/: index, IntroToSignalingYourMRR, SignalHeads, SignalMasts, AspectSignaling, DefineNewAspects, Logic, SignalGroups, SignalMastLogic, SignalingSetup, SimpleSignalExample
- html/tools/signaling/interlock/: index
- html/tools/signaling/ldt/: output, output1–output6 (skimmed)
- html/tools/logixng/: LogixNG, ErrorHandling, ErrorHandlingModule, ExportLogixNG
- html/tools/logixng/reference/: index, chapter1, chapter1_1, chapter2–chapter13, chapter11_1, original_add, WebRequestExample/index, YardLadderExample/index
- html/tools/scripting/: index, Start, HowTo, Examples, WhatWhere, Python, Python3, AppleScript, Jynstruments, JythonApps, ex_set_turnouts
- html/tools/automation/: index, viaJava
- html/tools/fastclock/: index, LocoNetClock
- html/tools/uss/: index, Conventions
- html/scripthelp/: index; DispatcherSystem/ (index, DispatcherSystem); yaat/ (index, YAAT); LnBushbyForwarder/ (index, LnBushbyForwarder)
- html/hardware/can/cbus/: CbusFastClock
- OPS/: Ops_schedule, Ops_Trains_Timetable
- P/signalling/: AddEditSignalingLogic, SignallingSourceFrame
- P/beantable/: SignalHeadTable, SignalAddEdit, SignalMastTable, SignalMastAddEdit, SignalMastLogicTable, SignalMastRepeater, SignalGroupTable, SignalGroupAddEdit, AddEditSignalingLogic, BlockTable, BlockAddEdit, BlockEdit, OBlockTable, RouteTable, RouteAddEdit, LRouteTable, LRouteAddEdit, SectionTable, SectionAddEdit, TransitTable, TransitAddEdit, ViewSpecialActions, LogixTable, LogixAddEdit, ConditionalAddEdit, LogixNGTable, LogixNGTableTable, LogixNGModuleTable, LogixNGGlobalVariables, LightAddEdit (light controllers only)
- P/logixng/: ConditionalNGEditor, ConditionalNGDebugger, InlineLogixNG, LogixImport, LogixNGBrowser, LogixNGInitializationTable, LogixNGTableEditor, LogixNGTableTableEditor, LogixNGWhereUsed, ModuleEditor
- P/logix/: Warrant, WarrantTable, CreateEditWarrant, NXWarrant, SCWarrant, SpeedChanges, Tracker, OBlockTable, OBlockEdit
- P/conditional/: ConditionalListEditor, ConditionalTreeEditor, ConditionalCopy, StateVariableActionList
- P/dispatcher/: Dispatcher, AutoTrains, NewTrain, Options, AllocateExtra
- P/ussctc/: FollowerFrame, OsIndicatorFrame
- P/ctc/: CTC
- P/entryexit/: EntryExitFrame
- P/blockboss/: BlockBossFrame
- P/cabsignals/: CabSignalPane
- P/sensorgroup/: SensorGroupFrame
- P/simpleclock/: SimpleClockFrame
- P/automat/monitor/: AutomatTableFrame
- P/whereused/: WhereUsed
- P/roster/swing/speedprofile/: SpeedProfileFrame
- P/operations/: Operations (by section), the "On Time" mode page

**Important but missing or outside this copy:**
- The signaling-system definitions themselves (the site's `xml/signals/` listing of the 20+ rulebooks) are not in the saved copy.
- The sample-script folder (the scripting examples page lists the scripts but the code isn't saved).
- The warrant preferences page and the "Locomotive Data" page behind the physics-based ramping were not found.
- The track-diagram editor and circuit-builder pages exist (P/display/) but belong to the panels area. I didn't read them beyond what the pages above say; they matter for how the track graph gets drawn.
- The turnout-lock feature page is referenced but wasn't in my list.
