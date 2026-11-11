# Findings 1: decoder programming, the roster, and throttles

Helper 1 of 4. Source: the saved manual of the reference software (local copy only). Names used here: "the reference software", "the decoder tool" (its decoder-programming program), "the panel tool" (its panel program). Everything below is summarised in my own words; facts only.

## 1. Overview

In this area the reference software does three connected jobs. **Decoder programming:** the decoder tool reads and writes a decoder's CVs on the programming track (service mode, in direct, paged, register, or address mode) or on the main (ops mode, blind writes). It shows them as named settings ("Acceleration Rate", "F2 controls output 4") rather than raw numbers. To do that it uses a large library of XML **decoder definitions**: each one maps named settings to CVs, bit masks, and choices, and they are grouped into panes (Basic, Motor, Speed Table, Function Map, Lights, Sound, Consist, CV list). It identifies a decoder from CV8/CV7/product-ID CVs, tracks whether each value was read, written, edited, or loaded from a file, and has helpers for speed tables, function mapping, decoder locks (CV15/16), indexed CVs, and printing or exporting. **The roster:** one file per locomotive, holding its identity, all named values, every CV, function labels (with latching or momentary, icons, and visibility), pictures, custom attributes, a measured speed profile, and physics data. Entries can be put in groups, copied, imported and exported, printed, and served over the web. **Throttles:** on-screen throttles (address or roster pick, a speed slider in several styles, stop and idle, F0-F28 buttons labelled from the roster, keyboard control, several throttles per window, a list of all running locos, saved layouts, gamepad and RailDriver input). There is also a WiThrottle-protocol server for phone apps (with mDNS discovery, a heartbeat that stops the loco, multi-throttles, consists, turnouts and routes, and the fast clock), a browser-based web throttle, a consist tool (basic, command-station, and decoder-assisted CV19 consists), speedometers and automated speed profiling and matching, and a virtual sound decoder that plays loco sounds from the PC following throttle commands.

## 2. Feature list

Legend: **Use** = core (nearly everyone) / common / niche. **Leah** = yes / partly / no (today).

### A. Talking to the decoder (programming modes and low-level access)

1. **Service-mode programming modes (direct byte, direct bit, paged, register, address)**
   - What it does: reads and writes CVs on the programming track. The mode is picked automatically from what the command station and the decoder allow, and can be changed when a decoder won't answer (the advice is to try paged, then direct, then register, then address).
   - Use: core. Leah: partly (direct byte only).
   - Leah way: a `mode:` option on `read_cv`/`write_cv` (`"direct"`, `"bit"`, `"paged"`, `"register"`). Add `read_cv_bit($cv, $bit)` and `write_cv_bit($cv, $bit, $value)`. Have `read_cv` use bit-verify reads internally (8 asks instead of up to 256), which makes reading much faster.

2. **Verify ("confirm") instead of read**
   - What it does: checks that a CV holds an expected value, which is faster than a full read. It is used after writes and to check roster values against the decoder.
   - Use: common. Leah: no.
   - Leah way: `verify_cv($cv, $value)` gives true or false. Also add `write_cv(..., verify: true)`.

3. **Ops mode (programming on the main), blind writes**
   - What it does: writes CVs to one address while the loco runs on the layout. There is no acknowledgement; the change you see in how the loco runs is the check. The manual warns about address 0 and about command stations that broadcast the write to every loco. Some decoders need a pause between writes (500 ms by default) or they miss packets. Ops mode is the usual way to tune momentum, speed, and sound while watching the loco.
   - Use: core. Leah: no (planned).
   - Leah way: `$loco.write_cv($cv, $value)` and `$loco.write_cv_bit(...)` on the existing `locomotive` object, with a `pause:` option in milliseconds (default 500 ms). Refuse address 0 unless `broadcast: true` is given. Show a `last_write` time.

4. **Ops-mode accessory decoder programming**
   - What it does: writes CVs to stationary decoders on the main. Their addresses come in several forms: a 9-bit decoder address, an 11-bit output address, an 11-bit signal address, or a legacy packet.
   - Use: niche. Leah: no.
   - Leah way: `accessory_decoder($address, kind: "output")` with `.write_cv()`. The same object can later throw turnouts.

5. **Indexed CVs (CV31/CV32 pages)**
   - What it does: reaches CVs above 256 on modern sound decoders by first writing index CVs. The manual's notation is `PI.SI.CV` (one maker) or `CV.PI.SI` (another). Repeated index writes can be skipped, and the index can be reset after the access.
   - Use: common (ESU, sound decoders, the RailCom product data). Leah: no.
   - Leah way: `read_cv(257, index: [16, 0])` and `write_cv(...)`. The base keeps track of the current index so it doesn't write it twice.

6. **High-CV workarounds for limited command stations**
   - What it does: decoder-specific tricks (a double index, or a partial index with a reset) for command stations that can only reach CV 256 or 1024.
   - Use: niche. Leah: no.
   - Leah way: not needed on Leah's own base. Add it to the NCE support only if a decoder needs it.

7. **Decoder identification**
   - What it does: reads CV8 (the maker, mapped to a name through the NMRA maker list), CV7 (the version), and a product ID from maker-specific CVs: CV47-50 for DIY decoders, and indexed RailCom CVs for ESU. It then narrows down the matching models. The roster can also be matched by reading the address back ("Ident").
   - Use: core. Leah: no.
   - Leah way: `identify_decoder()` gives `["maker": "ESU", "maker_id": 151, "version": 255, "product_id": ..., "models": [...]]`. `nmra_maker_name(151)` gives the maker's name. `$roster.find(address: read_address())` finds the entry.

8. **Factory reset**
   - What it does: writes the maker's reset value, usually CV8 = 8 (some makers have a list of partial resets).
   - Use: common. Leah: no.
   - Leah way: `reset_decoder(kind: "full")`. It asks for confirmation in the sample apps, and the definition gives the maker's own reset choices.

9. **Decoder lock (CV15/CV16), for more than one decoder in a loco**
   - What it does: writes the key to unlock decoder 0-7 (1 = motor, 2 = sound, 3 = functions), searches for the locks present, and recovers a decoder that was locked by accident. CV15 is deliberately left out of "write all".
   - Use: common (locos with a sound decoder and a motor decoder). Leah: no.
   - Leah way: `unlock_decoder($id)`, `lock_decoder()`, and `find_decoder_locks()` (gives a list). `write_all` never writes CV15.

10. **Programming errors in plain words**
    - What it does: reports the usual failures: no loco on the track, no acknowledgement (dirty wheels, a sound decoder whose acknowledgement current is too weak), a short on the programming track, a timeout talking to the command station, a confirm that failed, a stop by the user, and a communication error. There is also a status line and automatic retries.
    - Use: core. Leah: partly.
    - Leah way: a `programming_error` error type with a `kind` (`"no_loco"`, `"no_ack"`, `"short"`, `"timeout"`, `"verify_failed"`, `"cancelled"`) and a hint in Leah's own words. Add an `on_progress` event giving the CV being read, and a `retries:` option.

11. **Accessory and long addresses split across CVs (CV1/CV9, CV17/18, CV29 bit 5)**
    - What it does: reads and writes addresses whose bits are spread across several CVs.
    - Use: core. Leah: yes for loco addresses (`read_address`, `write_address`); no for accessory addresses.
    - Leah way: `read_accessory_address()` and `write_accessory_address($a)`.

12. **CV29 as named bits**
    - What it does: shows CV29's bits by name: direction reversed, 14 or 28/128 speed steps, analog (DC) running, RailCom, speed table on, and long address.
    - Use: core. Leah: no.
    - Leah way: `cv29_parts($value)` gives a keyed array, and `make_cv29(long_address: true, speed_table: false, analog: false, reversed: false)` builds the value. `$prog.config` reads and writes it by name.

13. **Driving on the programming track ("drive-away")**
    - What it does: the programming track works as part of the main while nothing is being programmed. (This is a DCC-EX feature, listed on its hardware page.)
    - Use: niche. Leah: partly (`set_programming_enabled`).
    - Leah way: a `$base.programming_track` property with the values `"main"` and `"programming"`.

### B. Named settings (decoder definitions)

14. **Decoder definitions: named settings mapped to CVs**
    - What it does: each setting has a name, a CV, a bit mask, a default, a tooltip, and a type. The types are decimal (min and max, negative numbers in two's complement), hex, enum (choices with values), long address, speed table, and "split" values spread over several CVs (numbers, hex, text, dates, enums). There are also factor and offset scaling, read-only constants, and a "radix" mask for settings stored as decimal digits.
    - Use: core (it is the main reason the decoder tool exists). Leah: partly (a short list of CV names).
    - Leah way: a `decoder_definition` object. It can be loaded from a Leah file of arrays (`decoder_definition("esu_loksound5.leah")`) or built in a script. `.settings` gives the list, `.setting("Start Volts")` gives `["cv": 2, "mask": "VVVVVVVV", "kind": "number", "min": 0, "max": 255]`, and `value_to_cvs()` and `cvs_to_value()` do the bit work. Licence note: the reference software's own definition files are GPL. Leah could read a copy the user owns at run time, but must not bundle or convert them without a licence check.

15. **The NMRA standard set**
    - What it does: ready-made definitions for the standard CVs (1, 2, 3, 4, 5, 6, 7, 8, 15/16, 17/18, 19, 21/22, 29, 65-95) that every definition includes.
    - Use: core. Leah: partly.
    - Leah way: build the NMRA set into the addon (`nmra_definition()`), so any decoder gets named settings with no file at all.

16. **Conditional settings**
    - What it does: a setting or pane is included only for certain models or product IDs. "Qualifiers" show or hide a setting depending on another setting's current value (for example, CV 11 means different things for short and long addresses).
    - Use: common (inside definitions). Leah: no.
    - Leah way: a `shown_when:` entry in a setting, such as `["Address Format", "==", 1]`, checked live by the definition object.

17. **The state of each value (read / written / edited / from file / unknown)**
    - What it does: each setting shows whether its value can be trusted. The decoder tool colours them: white for read or written, orange for edited, yellow for from file, red for unknown.
    - Use: core (the user experience depends on it). Leah: no.
    - Leah way: a `state` per setting in the programmer object, with an `on_state_changed` event. The sample app colours rows in a `property_list`.

18. **Read and write: one sheet, all, or changes only**
    - What it does: reads or writes the whole decoder, one pane, or only what was edited. "Changes only" is the fast choice for ops mode.
    - Use: core. Leah: no.
    - Leah way: a `decoder_programmer($definition, base: $base, mode: "programming_track" | "main", address:)` object with `read_all()`, `write_all()`, `write_changes()`, `read_settings($names)`, and `cancel()`, plus `on_progress` and `on_done` events.

19. **Panes and programmer layouts**
    - What it does: settings are grouped into tabs. There is a simple "Basic" layout (address, direction, speed steps, analog), a full layout, layouts per maker, and a club's custom layout. Empty panes are hidden or explained, and there is a find bar.
    - Use: common. Leah: no.
    - Leah way: the definition carries a `group` per setting. The sample "decoder workshop" app builds one `property_list` group per pane, and the `search_field` filters the rows.

20. **Function mapping grid**
    - What it does: a checkbox grid showing which function key (and "forward" or "reverse" variants, plus states like STOP, DRIVE, FWD, REV, sensors) drives which output wire. It follows the NMRA CV33-46 layout, which shifts at F3 and F10, and shows the number of outputs per model. Some makers use their own extended maps (ESU, Tsunami).
    - Use: common. Leah: no.
    - Leah way: `function_map_get($cvs)` and `function_map_set(...)` for the NMRA layout give a keyed array like `["F2": [4], "F0f": [1]]`. A sample canvas or checkbox grid shows it, and `$prog.function_map` holds it.

21. **Speed tables (CV67-94, with trims in CV66/95)**
    - What it does: 28 linked sliders with numbers. There are presets: a straight line, "match ends", a constant-ratio curve, a log curve, and shift left or right. Pinned sliders bend the curve between them, and the values must always rise. The manual also notes how makers differ: one adds CV2 on top, one scales the curve between CV2 and CV5, and one fixes CV67 = 1 and CV94 = 255. Its advice is to match steps in the order 1, 28, 14, 7, 21.
    - Use: common. Leah: no.
    - Leah way: `speed_table_curve(kind: "straight" | "ratio" | "log", start: 10, top: 200)` gives 28 numbers. `$loco.write_speed_table($values)` (ops mode or the programming track) refuses a table that doesn't rise and warns about the maker quirk the definition names. A sample editor uses 28 `slider`s or a `chart`.

22. **The basic three-point speed curve (Vstart/Vmid/Vhigh), kick start, momentum, back-EMF, and trims**
    - What it does: provides these settings and a practical way to tune them in ops mode: find the creep speed at step 1, trim the top speed, then set Vmid. The decoder tool refuses a Vmid outside the range from Vstart to Vhigh.
    - Use: core. Leah: no (the raw CVs can be written).
    - Leah way: these are settings in the NMRA definition, with the range check in `decoder_definition`. A guided "tune on the main" sample app walks through the steps.

23. **Lighting effects, sound effects, and sound levels**
    - What it does: panes that differ by maker for FX lighting (rates, ditch-light hold time, dimming) and for sound choices and volumes. These are written in ops mode so the user hears the change at once.
    - Use: common (sound locos). Leah: no.
    - Leah way: nothing special: definition settings plus `write_changes()` in ops mode.

24. **Print and export decoder data**
    - What it does: prints all settings or selected panes (by name, CV, and value), with a preview. Exports to CSV (CV, decimal, hex) and imports it.
    - Use: common. Leah: no.
    - Leah way: `$entry.export_cvs($path)` and `import_cvs($path)` as CSV, and a `report` sample "decoder sheet" (with the reports addon).

25. **Adding and checking definition files**
    - What it does: imports a definition from a file or web address, validates the XML, rebuilds the index of definitions, and gives the user's copy priority over the bundled one.
    - Use: niche. Leah: no.
    - Leah way: `decoder_definitions(folder:)` lists the definitions in a folder, and `check_decoder_definition($path)` gives the problems with line numbers.

### C. The roster

26. **Roster entries**
    - What it does: a unique ID, road name and number, maker, model, owner, DCC address, decoder family and model (fixed once chosen), programming modes, comments, and a per-loco throttle speed limit. One file per entry plus an index file.
    - Use: core. Leah: no.
    - Leah way: a `roster($path)` object, with its folder inside the app folder or the portable folder. `$roster.add("UP 792", address: 792)` gives a `roster_entry` with properties (`road_name`, `road_number`, `maker`, `model`, `owner`, `comment`, `decoder_model`, `speed_limit`) and `save()`. `$roster.entries` and `$roster.find(address:)`.

27. **Stored values for every CV and setting**
    - What it does: keeps the CVs and the named values together, so the decoder needn't be read again. Reading is slow, and some systems can't read at all. Both are stored in case a definition changes later.
    - Use: core. Leah: no.
    - Leah way: `$entry.cvs` (a keyed array from CV number to value), `$entry.settings`, and `$entry.apply_to($prog)`.

28. **Roster groups**
    - What it does: a loco can be in many groups (steam, Santa Fe, a member's name). An active group filters every list and print, and a new entry joins the active group. Groups can be exported.
    - Use: common. Leah: no.
    - Leah way: `$entry.groups` (a list), `$roster.groups()`, and `$roster.in_group("Steam")`.

29. **Copy as a template, import and export (one entry, a group, or everything), and sharing between computers**
    - What it does: these let a club keep a standard setup and move the roster between machines.
    - Use: common. Leah: no.
    - Leah way: `$entry.copy("New ID")`, `$roster.export($path, group:)`, and `$roster.import($path)`. A single file holds a whole roster.

30. **The roster table**
    - What it does: sort by any column, hide columns, edit cells in place, reorder columns, and show an icon column.
    - Use: common (user interface). Leah: no.
    - Leah way: a sample built on `list_box`. Nothing new is needed in the language.

31. **Pictures and a web link**
    - What it does: a large image (used as the throttle background) and an icon (head to the right, used in lists), plus a web address for the prototype.
    - Use: common. Leah: no.
    - Leah way: `$entry.image`, `$entry.icon`, and `$entry.web_link` (paths copied into the roster folder).

32. **Custom attributes**
    - What it does: free key/value pairs per entry that tools use, for example labels shown on the speed slider (JSON), or a sound profile to auto-load. Each one becomes an optional column in the table.
    - Use: common (for tools). Leah: no.
    - Leah way: `$entry.attributes` as a keyed array, so a script stores anything.

33. **Function labels per entry**
    - What it does: a label for each function, latching or momentary, "on" and "off" icons, visibility, a "shunt" flag (that function switches the slider to centre-off), and a custom display order. They can be reset to the NMRA/NEM defaults, filled from the decoder definition, or copied from another entry. Every throttle uses them, including phone throttles.
    - Use: core (for throttles). Leah: no.
    - Leah way: `$entry.functions`, a list of `["number": 2, "label": "Horn", "momentary": true, "icon_on":, "icon_off":, "visible": true, "shunt": false]`, and `nmra_function_labels()`.

34. **A speed profile per entry**
    - What it does: measured speed (mm/s) per step, forward and reverse, used by automatic running to stop at a set point and to slow down over a known distance.
    - Use: common (among people who automate). Leah: no.
    - Leah way: `$entry.speed_profile` (a keyed array from step to `[forward, reverse]`) and `speed_for_step($entry, 40)` and `step_for_speed(...)` helpers (see G).

35. **Physics data**
    - What it does: traction type, weight, power, tractive effort, top speed, and mechanical transmission, used for realistic acceleration in automatic running.
    - Use: niche. Leah: no.
    - Leah way: keep it in `$entry.attributes` until automation needs it.

36. **The roster on the web**
    - What it does: a list (with filters), each entry, and its image and icon at fixed web addresses, for web and phone throttles.
    - Use: common (it feeds the web throttle). Leah: no.
    - Leah way: `$roster.to_json()`, plus a sample with the web addon's `http_server`.

37. **Exporting the roster to a command station's format**
    - What it does: for example, DCC-EX `ROSTER()` lines with the function labels.
    - Use: niche. Leah: no.
    - Leah way: `$roster.export_text(format: "dcc_ex")`, which is useful when Leah talks to DCC-EX boards.

### D. On-screen throttles

38. **Taking an address**
    - What it does: type an address or pick from the roster, then press "Set". Optionally the roster is looked up automatically from the typed address. "Release" frees the loco and "Dispatch" hands it to another throttle. Taking an address reads back its current direction and functions.
    - Use: core. Leah: partly (an address field; no release, dispatch, or roster).
    - Leah way: `locomotive(roster: "UP 792")` takes the address and labels from the roster, and `$loco.release()`. Dispatch and steal matter only for other command stations (NCE).

39. **Speed control styles**
    - What it does: a 0-100% slider, a number field with step buttons, a centre-off "shunting" slider (-100..+100, where crossing zero changes direction) that can be switched on by a function key, and a large slider for viewing from across the room. The speed-step mode (14/28/128) is taken from the decoder.
    - Use: core. Leah: partly (a 128-step slider).
    - Leah way: `locomotive(..., speed_steps: 28)`, `$loco.speed_percent`, and a `shunt_mode` in the sample throttle. A possible small `throttle_slider` control could come later.

40. **Stop vs idle**
    - What it does: an emergency stop ignores momentum; idle sets speed 0 and lets the decoder's momentum slow the loco.
    - Use: core. Leah: yes (`stop`, `emergency_stop`).
    - Leah way: nothing new is needed.

41. **Function buttons F0-F28 (the WiThrottle protocol goes to F31)**
    - What it does: buttons are added for as many functions as the loco has. Labels and icons come from the roster, undefined buttons can be hidden, momentary buttons send "on" while pressed, and the order can be customised.
    - Use: core. Leah: partly (F0-F12 buttons; F0-F28 in the object).
    - Leah way: `$loco.set_function(n, on)` exists. Add `$loco.press_function(n)` and `release_function(n)` that respect momentary from the roster. The sample builds its buttons from `$entry.functions`.

42. **Keyboard control**
    - What it does: keypad +/- and arrows change the step, Page Up/Down change by a multiplier, Space is idle, Escape is stop, up/down set the direction, and the function keys (with Shift or Ctrl) reach F10-F28. Ctrl + mouse wheel also changes speed. Every key can be reassigned.
    - Use: common. Leah: no.
    - Leah way: the sample throttle uses the window's key events, with a `key_map` keyed array the user can edit.

43. **Several throttles at once, and a list of running locos**
    - What it does: several throttles per window, with next / previous / next-running buttons, "stop all", and a power button. A list window shows every throttle with its icon, speed, and direction, and a roster entry can be dragged in to make a throttle.
    - Use: common. Leah: partly (the refresh table holds many locos).
    - Leah way: `$base.locomotives()` (all active ones, with speed and direction), `$base.stop_all()`, and an `on_locomotive_changed` event, so any app can show a live list.

44. **Saved throttle layouts and default throttles**
    - What it does: saves window positions, which throttles are open, and customised buttons per loco. A default throttle file (an NMRA layout) is used for unknown addresses.
    - Use: niche. Leah: no.
    - Leah way: the app's own job. A sample keeps its layout in a settings file.

45. **Per-loco speed limit and slider labels**
    - What it does: caps the throttle at a percentage for this loco, and marks points such as "SS", "NS", "FS" on the slider.
    - Use: common (the limit) / niche (the labels). Leah: no.
    - Leah way: `$loco.speed_limit` (a percentage), applied in `set_speed`. Labels come from `$entry.attributes`.

46. **Controlling each member of a consist**
    - What it does: in addition to the consist's own buttons, a panel sends functions to each loco in the consist separately (for example, lights on the lead loco only, horn on the B unit).
    - Use: common (with consists). Leah: no.
    - Leah way: `$consist.members[1].set_function(...)` (see F).

47. **Hardware controllers on the PC**
    - What it does: a USB gamepad (stick for speed, a hat to choose the throttle, preset speed buttons, a double tap for an emergency stop), a RailDriver desk (levers, horn, bell; a udev rule on Linux), and the Wiimote. A script maps the device for each model.
    - Use: niche (but much liked). Leah: partly (the core has `game_controller`).
    - Leah way: a sample "gamepad throttle" joining `game_controller` events to a `locomotive`. RailDriver support would be a HID device through the devices addon later.

48. **Following a hardware throttle**
    - What it does: listens for commands that a hand throttle sends to one address and mirrors them on screen. Sound and display tools also use this.
    - Use: niche. Leah: no.
    - Leah way: on Leah's own base every command already goes through Leah, so `$loco.on_speed_changed` and `on_function_changed` events give this for free. With NCE, it needs cab-bus monitoring.

49. **"Game" view**
    - What it does: hides the window frames, uses the roster image as the background, and can show video from a camera on the train.
    - Use: niche. Leah: no.
    - Leah way: a sample app (a picture background, `camera_view`). Nothing new is needed.

### E. Phone and browser throttles

50. **The WiThrottle server**
    - What it does: a plain-text TCP protocol, announced over mDNS as `_withrottle._tcp`. On connect it sends the protocol version, the roster list (name, address, short/long), track power, turnout and route captions and lists, consists, the web port, the fast clock (seconds plus a rate), and alert, info, and server-type messages. The client names itself (`N`, `HU`). A heartbeat with a timeout makes the server stop the loco when the phone goes silent. Multi-throttles (`M0+`, `M0-`, `M0A`) let one phone run several locos with: speed 0-126, direction, functions as press and release pairs, forced function states, momentary or latching overrides, idle, emergency stop, speed-step mode, queries, and steal. It also sends function labels from the roster, turnout and route commands with state updates, and advanced-consist commands (create, add, remove, reorder, delete, set CV21/22).
    - Use: core today (it is how most modellers run trains: Engine Driver, the WiThrottle app, and others). Leah: no (planned).
    - Leah way: a `withrottle_server(port: 12090, name: "My Railroad", base: $base, roster: $roster)` object with the events `on_client`, `on_client_left`, `on_turnout_request`, and `on_route_request`. The script registers turnouts and routes (`$server.add_turnout("LT12", "Rico Station N")`), and `heartbeat:` defaults to on. It is built on the network addon's `socket_server` and local-service announcing.

51. **Heartbeat safety stop**
    - What it does: stops the loco if a phone stops talking (a dead battery, Wi-Fi lost).
    - Use: core (part of item 50). Leah: no.
    - Leah way: always on in Leah's server. The `$base` can also apply the same idea to any app (`$base.stop_if_quiet(seconds)`).

52. **A browser throttle**
    - What it does: an HTML5 and websocket page served by the PC that drives locos, turnouts, routes, and panels from any phone or tablet browser, with nothing to install.
    - Use: common. Leah: no.
    - Leah way: a sample app using the web addon's `http_server` and `web_socket`, serving one page. This shows off single-file apps.

53. **Remote (internet) phone throttles**
    - What it does: drive the layout from another city through port forwarding.
    - Use: niche. Leah: no.
    - Leah way: documentation only, with a security warning.

### F. Consists

54. **Basic consist (the same address)**
    - What it does: program every loco to the same address.
    - Use: common. Leah: yes (`write_address`).
    - Leah way: nothing new is needed.

55. **Command-station (software) consist**
    - What it does: the command station sends each member its own packets. Limits vary by make: NCE's old-style consists hold 8 locos and the Lenz "double header" 2; Digitrax's limit is its number of slots.
    - Use: common. Leah: no.
    - Leah way: `consist(name:)` with `add($loco, reversed: false)`, `remove()`, `reorder()`, `set_speed`, `set_direction`, and `set_function`. On Leah's own base it is just members of the refresh table (no limit). With NCE, the command station's own consists.

56. **Advanced (decoder-assisted) consist**
    - What it does: CV19 holds the consist address (1-127, plus 128 for a reversed loco). It is set by an ops-mode write or by the NMRA consist-control packet. CV21/22 choose which functions answer to the consist address. Such consists always run in 28 steps, and a forgotten CV19 makes a loco "dead" at home.
    - Use: common. Leah: no.
    - Leah way: `consist(address: 74, kind: "advanced")`, whose `add` writes CV19 (choose packet or ops write). `$consist.break_up()` clears CV19 on every member. Add `find_consisted($roster)` to list entries with CV19 ≠ 0, and a warning in `identify_decoder()` when CV19 ≠ 0.

57. **Saved consists and syncing CV19 with the roster**
    - What it does: scans the roster for advanced consists and optionally writes CV19 changes back to each roster entry. For NCE: back up and restore the command station's consists to a file, get, add, replace, or clear them, check that a loco isn't already a lead or rear loco elsewhere, and keep a roster of more than 127 consists.
    - Use: common (NCE users) / niche (syncing). Leah: no.
    - Leah way: `$roster.consists()`. In the planned NCE support, `nce.consists()`, `nce.save_consists($path)`, and `nce.restore_consists($path)`.

### G. Speed: measuring, profiling, matching

58. **Speedometer from track sensors**
    - What it does: two or three occupancy or spot sensors with the distances between them (in scale feet or centimetres, scale = real length × scale factor, e.g. 87.1 for HO), timing on entry or exit, slow and fast timers, and single-block or loop variants. NCE's AIU sensor numbering is given.
    - Use: common (for speed matching). Leah: no (sensors aren't in the addon yet).
    - Leah way: a `speedometer(sensors: [$s1, $s2, $s3], distances: [300, 900], scale: "HO")` object with an `on_speed` event (`["scale_mph": .., "scale_kmh": .., "mm_per_s": ..]`). A cheap path that is Leah's own: an Arduino with two IR gates through the Maker addon.

59. **Roller-stand speedometers (USB or serial)**
    - What it does: reads scale speed from a rolling-road reader and draws a dial and a speed-profile graph. Results can be exported as CSV, and a reference profile can be loaded to match against.
    - Use: niche (the main brand is retired). Leah: no.
    - Leah way: `serial_port` plus a sample. Only add a device driver if the owner has one.

60. **Automatic speed profiling**
    - What it does: runs the loco step by step, forward and back, through a timed block; it needs momentum off. Records mm/s, can sample every Nth step, and repeats the lowest steps at the end with mean, minimum, maximum, spread (±%) and a warm-up change %. Tables can be merged, saved, or discarded, and there is a stopping test. Odd (non-rising) cells are flagged and can be edited.
    - Use: common (among people who automate). Leah: no.
    - Leah way: `profile_speed($loco, $speedometer, steps: range(1, 126, 5), repeats: 3)`, run as a task with `on_progress`, gives a table and saves it to `$entry.speed_profile`. A chart shows the result.

61. **Automatic speed matching**
    - What it does: reads and zeroes momentum, warms the loco up, and optionally trims reverse to match forward. "Basic" sets Vstart/Vhigh (and Vmid) for target start and top speeds. "Speed table" puts steps 10 and 19 at 1/3 and 2/3 of the way. "Step = scale mph" makes step N in 128 run at N mph. It then restores momentum. There is also a manual method: lead with a reference loco and adjust the other loco's step 28 with "match ends".
    - Use: common (it is the end goal of the speed tools). Leah: no.
    - Leah way: `speed_match($loco, $speedometer, start: 3, top: 65, method: "speed_table", warm_up: 120)`, which always restores momentum (even after an error). The sample app "match two locos" uses ops mode.

62. **Scale helpers**
    - What it does: conversions between real and scale distances and speeds, for scales from Z to O.
    - Use: common. Leah: no.
    - Leah way: `scale_speed($mm_per_s, scale: "HO", unit: "mph")` and `scale_factor("N")`.

### H. Sound on the PC

63. **Virtual sound decoder**
    - What it does: sound profiles come in a zip (a config file plus WAVs). They follow the throttle of an address: engine start and stop (from a key, or automatically), steam chuffs tied to speed and driver diameter, diesel notches up and down, coasting, a brake key, accel and decel rates, and gain. There is per-loco volume and master volume and mute, and the profile is saved in the roster. A headless mode exists. 3D sound follows the loco's reported position (RailCom/transponding reporters, occupancy blocks, or a track geometry with straights and curves) relative to a listener position and heading.
    - Use: niche. Leah: no (the sound addon has playing and mixing).
    - Leah way: `virtual_sound($loco, profile: "class64.zip")` with `start_engine()`, `volume`, and `position`. It reacts to `$loco` events and starts quiet. A later step would be positional sound.

### I. Related

64. **RailCom reports**
    - What it does: address, where and when it was seen, speed reported by the decoder, orientation (east or west), direction, motion, and quality of service. It is also the only way to read CVs on the main.
    - Use: niche (for now). Leah: no.
    - Leah way: later, if Leah's base adds the cutout. That would give `$loco.read_cv()` on the main and an `on_railcom` event.

65. **Turnout control from a throttle**
    - What it does: thrown or closed by accessory number, with feedback modes (none, delayed, one sensor, two sensors, monitoring).
    - Use: common (another helper's area). Leah: partly (DCC accessory packets planned).
    - Leah way: noted only because the WiThrottle server must expose turnouts.

66. **Fast clock to throttles**
    - What it does: a model time and rate pushed to phones (the WiThrottle `PFT` message) and to hand throttles. A stopped clock is sent as rate 0.
    - Use: niche. Leah: no.
    - Leah way: a `$server.clock = [time, rate]` property on the WiThrottle server, fed by whatever fast-clock object the operations helper proposes.

## 3. Leah's own advantages

- **Programming as a script.** "Set momentum CV3 = 20 on every steam loco in the roster and log what changed" is a 5-line `foreach`. "Audit the roster for locos with CV19 ≠ 0" is a 3-line `foreach`. The reference software needs a GUI pass per loco, or its separate scripting layer.
- **Definitions as Leah data, not XML plus XSLT.** Repeated blocks (signal heads, LED groups, function maps) are generated with loops in a few lines. The manual spends a whole page on XSLT tail recursion to get the same result.
- **Its own command station.** Leah's base has no slot limits, no steal or dispatch conflicts, and knows every loco's state (no "slider out of sync after taking a moving loco"). It can follow any throttle's commands for free, and software consists of any size are trivial. Bit-verify reads and verify-after-write can be built in. The simulated base lets every sample and test run with no hardware.
- **Single-file apps.** A club's "program my loco" kiosk, a WiThrottle server, or a gamepad throttle can each ship as one double-click file or AppImage, with the roster in the app's own folder (portable).
- **Controls already exist for the hard user-interface parts.** `property_list` (rows of 16 types, groups) fits named settings, `chart` fits speed profiles and tables, `canvas` fits the function-map grid, `game_controller` is in the core, `camera_view` covers the cab-ride view, and the database addon can hold big rosters.
- **Safety by default.** Leah can refuse ops writes to address 0, keep CV15 out of "write all", always restore momentum after matching even on an error, use a heartbeat stop on network throttles, and start virtual sound quietly.
- **Network pieces are already there.** `socket_server`, the local-service announcing, `http_server`, and `web_socket` make the WiThrottle server and a browser throttle small addon objects rather than separate products.

## 4. Skip list

- **XSLT-generated definitions and definition migration** (replacement models, the "show = maybe" option, an interactive update chooser): Leah loops make generation trivial. Migration only matters with a decade of old roster files.
- **The Java grid-layout options for panes** (insets, weights, the CURRENT and NEXT positions): Leah apps lay out their own panes.
- **The throttle window's mini-windows, saved window positions, the merge-or-replace layout dialog, and default throttle XML files:** desktop-framework details. Leah apps save their own layout.
- **Drop-in GUI scripts for throttles:** Leah is already the scripting language.
- **The Wiimote driver:** an old device. Gamepads cover it.
- **The PR1DOS and PR1WIN export formats:** the programs that used them are long gone. CSV is enough.
- **Emulating the "legacy" dual decoder and initialising one particular old decoder pair:** described as unreliable, for a handful of old decoders.
- **High-CV access workarounds and TCS WOW-specific CV access:** not needed on Leah's own base. Revisit only for NCE if a real decoder needs it.
- **Address-mode (physical register) programming:** outdated. Paged, direct, and register cover real decoders. Only add it if a user has such a decoder.
- **Digitrax dispatch, steal, and share semantics:** only meaningful when Leah drives a Digitrax station. The WiThrottle steal reply can just say "taken by another throttle".
- **Remote internet throttles through port forwarding:** a security risk for little gain. Document only.
- **Physics data in the roster:** keep it in `attributes` until automatic running exists.
- **Bundling the reference software's decoder definition library:** it is GPL. Leah should ship its own NMRA set and let users point to their own copy, after a licence check.

## 5. Pages read

Paths are relative to `help/en`. `<DT>` = the decoder tool's folder name, `<ref>/<tools>` = the reference software's package path.

**Decoder tool guide** (`html/apps/<DT>/`): index, Tour, Roster, RosterFile, LocoFile, Files, FileUpdate, NetworkFiles, FAQ, Install, IntroXML, MoreInfo, Programmer, FnMapping, CreateDecoder, CreateDecoderAdvanced, TransformDecoder (17).

**Packages**:
- `package/<ref>/<tools>/roster/swing/RosterGroupTable`
- `package/<ref>/<tools>/roster/swing/speedprofile/SpeedProfileFrame`
- `package/<ref>/<tools>/throttle/ThrottleFrame`
- `package/<ref>/<tools>/withrottle/Protocol`
- `package/<ref>/<tools>/withrottle/UserInterface`
- `package/<ref>/<tools>/simpleprog/SimpleProgFrame`
- `package/<ref>/<tools>/dualdecoder/DualDecoderSelectFrame`
- `package/<ref>/<tools>/webThrottle/webThrottle`
- `package/<ref>/<tools>/beantable/RailComTable`
- `package/<ref>/<hardware>/nce/consist/NceConsistEditFrame`
- `package/<ref>/<hardware>/bachrus/SpeedoConsoleFrame`

**Throttle tool** (`html/tools/throttle/`): AdvancedThrottleControl, BasicThrottleWindow, HowTo…OrientedThrottle, RostersMediaPane, ThrottleChapter1-4, ThrottleMain, ThrottlesListWindow, ThrottlesPreferencesPane, ThrottleTOC, ThrottleToolBar, ThrottleWindowMenus, ToolsMenuThrottleEntry (15).

**Other tools**:
- `html/tools/speedometer/`: index, NCE-Speedometer, SpeedMatch
- `html/tools/vsd/`: index, VSDMain, VSDManager, VSDLocationsMgr, VSDFileAndConfig, VSDHeadless
- `html/tools/fastclock/`: index, LocoNetClock (read; only the fast-clock-to-throttle link is relevant)
- `html/tools/consisttool/ConsistTool`
- `html/tools/scripting/<its throttle-scripting page>` (the throttle customisation scripts)

**User manual, current edition** (`manual/<DT>3/`): Programmer_OpsMode, Programmer_ServiceMode, Programmer_Setup, Programmer_SingleCV, Basic_BasicPane, Basic_Mode, Basic_Start, Comp_Advanced, Comp_Analog, Comp_Basic, Comp_Consist, Comp_FMap, Comp_GlobalCVs, Comp_LightFX, Comp_ManfSpecific, Comp_Motor, Comp_PrintData (+ _a/_b/_c, empty), Comp_Setup_Roster, Comp_SoundFX, Comp_SoundLevels, Comp_Speed, Comp_Speed_Talble, Adv_FunctionLabel, RosterMedia, Error (error codes), dp3_decoderInfo, dp3_Main_ConsistControl, dp3_Main_Main, dp3_Main_Speedometer, dp3_Main_Throttle (+ addressPanel, controlPanel, functionPanel, menubar, toolbar), dp3_Main_TurnoutControl, dp3_menubar, dp3_rosterTable, dp3_settings_rosterGroups, dp3_toobar.

**User manual, older edition** (`manual/<DT>/`): Programmer_Multi_Decoder, Multi_Decoder_Control, Comp_SoundFX, Main_RosterGroup, Dcode_Format, Dcode_Start, Dcode_Submit, Dcode_Testing, Main_Speeometer.

**Hardware and other**:
- `html/hardware/nce/NCE` (the USB interface's programming limits per system)
- `html/hardware/dcc-ex/index`
- `html/hardware/sprog/SPROG`
- `html/hardware/bachrus/index`
- `html/hardware/raildriver/index`
- `html/web/RosterServlet`
- `html/apps/<its sound app>/` main page (just a pointer)
- `manual/…OPS_UsersGuide/Ops_ImportEngines_FromRoster`

About 116 pages in total.

**Missing or thin:**
- The `symbolicprog`, `progsupport`, and `speedometer` package help pages don't exist in the saved copy.
- The "Decoder Information" page (the project's collected data on decoder makers and models), linked from the guide's index, isn't saved.
- The list of standard setting names (names.xml) isn't saved.
- The guide's Comp_PrintData_a/b/c pages are empty (images only).
- The throttle window's "Help → Window Help" pages are referenced but are the same throttle pages listed above.
