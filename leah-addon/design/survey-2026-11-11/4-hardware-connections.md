# 4. Hardware connections: command stations, layout buses, devices

Helper 4 of the scanning session. Source: the saved manual of the reference software (its hardware pages, its per-system tool pages, and a skim of its technical pages). Everything below is summarised in my own words; the facts (ports, speeds, command numbers, limits) are from the pages unless marked *(general knowledge)*.

## 1. Overview

The reference software talks to about 45 kinds of hardware through one idea: a **connection**. Each connection is one link (a serial port, a USB serial device, a TCP or UDP socket, a CAN adapter, or a simulator) to one system, gets a short prefix, and fills the program's shared object types (locomotive throttles, a programmer, power, turnouts, sensors, lights, reporters, signal masts) with that system's objects. Several connections run at once. A "defaults" setting picks which connection does throttles, programming, power, and the command station's jobs. The systems split into: **command stations** (NCE, Digitrax, Lenz and the XpressNet family, Roco/Fleischmann Z21, ESU ECoS, Märklin CS2/CS3, DCC-EX, SPROG, MRC, CVP EasyDCC, TAMS, ZIMO, Lionel TMCC, TCS CS-105, and others); **layout buses**, mostly for inputs and outputs (LocoNet without a command station, OpenLCB/LCC, MERG CBUS, C/MRI, BiDiB, MQTT, CTI Acela, Grapevine, Oak Tree, XBee, Raspberry Pi GPIO, X10/Insteon); and **devices** (RFID readers, RPS positioning, RailCom detectors, speedometers, the RailDriver desk, the NCE packet analyzer, programmers). Nearly every system has a **simulator** connection, a **traffic monitor**, and a **send raw command** tool. Several can share one hardware link with other computers over TCP. For Leah, the useful lesson is the split: a small **connection object** per system that offers Leah's own `dcc_base`/`locomotive`/`turnout`/`sensor` interface, plus a **capability list**, because the same box can do very different things depending on the cable used (see NCE USB below).

## 2. Every system and bus covered

Legend for features: **T** throttle, **OP** operations-mode (main track) programming, **SP** service-mode (programming track) programming, **Tu** turnouts/accessories, **Se** sensors, **Li** lights, **Re** reporters (loco ID or RFID tag at a place), **Sig** signals or masts, **Pw** track power. "Open?" says what the pages say about the protocol being published; *(gk)* marks my general knowledge where the pages are silent. "Users" is my own guess at how many modellers use it: many / some / few.

### Command stations

| System | Kind | Connects by | Features supported | Open? | Users |
|---|---|---|---|---|---|
| **NCE** Power Pro / PowerHouse (CS02), PH5 / CS03; PowerCab, SB3/SB5, Twin | command station | Power Pro: 9-pin serial at 9,600 baud (straight cable), or a serial-to-network box (terminal server); NCE USB Interface (Silicon Labs CP210x USB serial) for PowerCab/SB3/SB5/Twin; PH5/CS03 have built-in USB (a new memory map); simulator | T (28/128 steps, F0–F28), OP, SP (paged, register, direct), Tu (1–2044; 1–250 on old firmware), Se (AIU input cards: cab 1–63 × 14 pins), Li (as accessories), Sig (extended-accessory aspects 0–31), Pw, consists and macros kept in the station's memory, fast clock, text on cab screens. **Through the USB interface** many of these disappear (see §3) | The binary command list is in the pages (codes 0x80–0xBF, with replies and error codes) | many |
| **Wangrow System One** (made until 2003, NCE-derived) | command station | serial, fixed 9,600 baud | as early NCE | not stated | few |
| **Digitrax** (Zephyr/DCS50/51/52, Chief DCS100/200, DB150, DCS210/240/240+, Super Empire Builder) | command station on **LocoNet** | a LocoNet adapter: LocoBuffer-NG / LocoBuffer-USB (USB), PR3/PR4 (USB), LocoBuffer-II/LocoBuffer (serial 19,200 or 57,600), MS100 (odd 16,600 baud, not advised), Bluetooth LocoBridge; DCS52/DCS240 built-in USB; LocoNet over TCP through the LbServer program (ASCII, port 1234); simulator | T (through command-station "slots"; newer "expanded" slots only partly), OP, SP (no read-back on DB150), Tu (1–2048), Se (BDL16x, DS54/64/74 inputs), Re (Digitrax transponding zones), Li (mapped onto turnout numbers), Sig (SE8C, SE74), Pw and power districts (PM4x), fast clock, volt/current meters (DCS52/210/240), firmware and sound-file download, device configuration (BDL16x, DS64, PM4x, SE8C, UR9x radio IDs, Duplex group, command-station option switches) | not stated *(gk: Digitrax publishes a "personal use" edition of LocoNet; the expanded protocol is not fully public)* | many |
| **Digitrax PR2 / PR3 / PR4** as stand-alone programmers | programmer | PR2 serial 57,600; PR3/PR4 USB, switched between "programmer" and "LocoNet interface" modes | SP on its own track, run one loco there, sound download | as LocoNet | many |
| **Uhlenbrock Intellibox** I/II, IB-Com; **Fleischmann Twin Center** | command station (LocoNet) | its LocoNet port through any LocoNet adapter; or its own serial port (19,200, LocoNet protocol) or USB | as LocoNet; Uhlenbrock LNCV modules, Power4/8 boosters, IntelliLight, Track-Control | not stated | some |
| **Digikeijs DR5000 / YaMoRC** | command station | as an ECoS over the network (port 15471), as LocoNet over TCP (port 5550 on the original DR5000, 1234 on YaMoRC), or as a LocoBuffer-USB | T, Tu, Se (S88 and LocoNet share addresses: a known trap), Sig | not stated | some |
| **Lenz** LZ100/LZV100/LZV200 and the **XpressNet** family (Roco LocoMaus/MultiMaus, Hornby Elite/Select, Atlas Commander, Lenz Compact/LH200, ZTC 511/521, Viessmann Commander, OpenDCC, Paco NanoX) | command station on XpressNet | Lenz LI100/LI100F/LI101F (serial 9,600–115,200), LI-USB, LI-USB-Ethernet (USB as /dev/ttyACM, or Ethernet), LZV200 built-in, XnTcp network adapter, XPA phone adapter + modem, ZTC640, Hornby Elite USB, GenLi DIY; simulator | depends on the station, which the pages grade as full / partial / minimal: T, OP, SP, Tu (1–1024), Se via the Lenz feedback bus (LR100/LR101, LS100) or S88 through GenLi-S88; exact turnout feedback with LS100 | **yes**: the XpressNet specification is published (the page links it) | many (Europe), some (N. America) |
| **Roco / Fleischmann Z21 and z21** | command station | Ethernet (default address 192.168.0.111) *(gk: UDP port 21105)*; also its XpressNet/LocoNet ports; simulator | XpressNet over the network: T, Tu, programming (the white z21 has no separate programming track), RailCom, connection flags; the black Z21 also has LocoNet and Zimo CAN | **yes**: documented on the Z21 website (the page says) | many (Europe) |
| **ESU ECoS** | command station | network, TCP port 15471 | T (basic; no consists), Tu, Se (S88 boards), Re (RailCom), Pw, SP from firmware 4.1, OP from 4.2.3; the station's own loco database read and kept in step with a roster | not stated *(gk: ESU publishes its network protocol)* | some |
| **Märklin** Central Station 2/3, Mobile Station 2; CdB CC-Schnitte | command station | Märklin CAN ("MCAN") frames over TCP or UDP (UDP replies are broadcasts, so same subnet only); CC-Schnitte over USB serial; a serial-to-TCP bridge program (default port 15731) shares one adapter | T (Märklin-Motorola, DCC, and mfx), Tu, Se, Pw, MS2 loco list import; no programming, no consists | not stated *(gk: Märklin published the CS2 CAN protocol)* | many (Germany, 3-rail) |
| **DCC-EX EX-CommandStation** (and the old DCC++) | DIY command station (Arduino Mega or Uno; ESP32 boards) | USB serial; Ethernet/WiFi TCP port 2560; "DCC-EX over TCP" server so other computers and phone throttles share one station; simulator | T (128 steps, F0–F28, many locos), OP (write byte, set/clear bit), SP (read and write), Tu (direct accessory `a`, the station's own turnout table `T`, output pins `Z`, any virtual pin `z`), Se (any pin, kept in the station's table), Li (accessory or virtual pin), Pw, volt/current meter, its automation scripts (EXRAIL) listed and started, its display mirrored, its messages shown, roster exported as its own macro lines | **yes**, open source | many and growing |
| **SPROG** II / II USB / 3 / Nano / Pi-SPROG One | programmer or small command station | USB (CDC serial, no driver on Linux); old ones 9-pin serial; simulator (either mode) | as programmer: SP; as command station: T, OP, Tu, Pw; a "mode word" setting; firmware update | not stated *(gk: the ASCII commands are in the SPROG manual)* | some (UK) |
| **SPROG Generation 5** (SPROG 3 Plus, Pi-SPROG 3) | command station + programmer | MERG CBUS over USB-CAN or its built-in adapter; CANISB isolated adapter | T, OP, SP, two track outputs (run and program at once; the second can follow the main or act as a reverser), mode switcher | CBUS is open | some (UK) |
| **MERG CANCMD** (DCC over CBUS) | command station | CBUS (see buses) | T (sessions with "steal" and "share" between throttles), SP, Pw, stop-all | open | some (UK) |
| **MRC Prodigy Advanced 2** / Gaugemaster DCC55 | command station | USB computer interface (wired or wireless), with a cab address of its own | T (28/128 steps, F0–F28; no 14 steps, no consists), OP, SP (station picks the mode), Tu, Pw, the station's routes triggered | not stated | some |
| **CVP EasyDCC** | command station | serial 9,600 with a hand-made RJ11 cable; or a serial-to-network box (port 2000 + port number on Lantronix boxes); simulator | T, consists, programming, Tu | not stated *(gk: its manual documents the computer protocol)* | few |
| **TAMS MasterControl** | command station | USB (57,600) and serial (default 2,400) at once; P50X protocol; simulator | T (only 14 functions, and only F1–F8 changes seen from outside), Tu, Se (S88), Pw, programming, loco database | not stated | few |
| **ZIMO** MX-1 (ASCII), MXULF / MX-10 (binary) | command station | serial 1,200–38,400; MXULF over USB | ASCII: programming only; binary: SP, OP, T, Tu (tentative) | not stated | few |
| **TCS CS-105** | command station on LCC | LCC over CAN or network | T (through LCC train nodes), function labels copied to and from the roster | yes (LCC is an NMRA standard) | some |
| **Lionel TMCC / Legacy** (Command Base 1/1L/992, Base 3) | 3-rail command system (not DCC) | serial 9,600 (straight cable); WiFi to the 992 and Base 3; simulator | T (32, 100, or 200 steps; about 30 function keys mapped to Lionel's controls), switches, accessories, setting device IDs | the page refers to Lionel's Legacy command protocol document (rev. 1.21) | some (O gauge) |
| **SRCP servers** (srcpd and others) | server in front of other systems | TCP | Tu, Li, signal heads, early T; no SP | **yes** (der-moba's published protocol) | few |
| **QSI Quantum Programmer** | programmer | USB (needs a vendor driver) | SP only; no sound loading | not stated | few |
| TCH Technology | — | — | support dropped in 2008 | — | — |

### Layout buses (inputs, outputs, signals; some carry DCC too)

| Bus | Kind | Connects by | Features supported | Open? | Users |
|---|---|---|---|---|---|
| **Standalone LocoNet** (LocoNet without a command station) | layout bus | any LocoNet adapter | Se, Tu, Re, Sig, fast clock; used beside a non-Digitrax command station (the page even covers RailSync from an NCE station) | as LocoNet | some |
| **OpenLCB / LCC** | layout bus (CAN) | USB CAN adapters speaking the GridConnect text format (RR-CirKits LCC Buffer-USB, SPROG USB-LCC, CANable via the Lawicell format, MERG CANUSB4), a Pi hat (PI-LCC), or TCP to a network adapter or hub (default port 23; adapters announce themselves over mDNS) | Se and Tu as pairs of 8-byte event IDs, Li, signal masts driven by events, shared strings between computers, DCC turnouts through a bridge, node configuration read from the node itself (no definition files), firmware update, memory read/write, "identify node" by blinking its LEDs, a hub sharing one adapter | **yes**: adopted as an NMRA standard | some, growing |
| **MERG CBUS** | layout bus (CAN) | MERG CANUSB4 / CAN-RS (serial 57,600–460,800), GridConnect, Lawicell; network interfaces (TCP port 5550: CANPi, CANether, CAN-GC1e, a Node.js "CBUS Server"); simulator | Se, Tu, Li as short or node-numbered events, DCC over CBUS (sessions, steal/share), fast clock messages, volt/current meters, node manager, firmware bootloader, event capture and tables, a hub sharing the link | **yes**: the pages call it an open protocol, published on MERG's public site | some (UK) |
| **C/MRI** (CMRInet) | layout bus (polled serial, RS-485) | serial or USB-serial; TCP through a serial-to-network box; simulator | Se, Tu (one or two bits, steady or pulsed), Li, signals built from outputs; node types SMINI (24 in/48 out), USIC/SUSIC (cards of 24 or 32 lines), cpNode/cpMega; Arduinos emulate nodes | **yes**: published by the NMRA as LCS-9.10 and LCS-9.10.1 | some |
| **BiDiB** | layout bus | serial over USB (9,600, 115,200, or 1 Mbaud); "netBiDiB" over TCP (port 62875, paired once, mDNS discovery, keep-alive pings); BiDiB over TCP to another program; simulator from an XML file | Se (feedbacks, input ports), Tu (accessories, confirmed), Li (light, switch, and servo ports), Re (RailCom loco addresses), signal masts (station-side macros), a command-station node for T and programming; hot-plugging | **yes**: an open protocol, not tied to any maker | some (Germany) |
| **MQTT** | message broker (network) | TCP to a broker (port 1883), user name and password, a "last will" message | Tu (CLOSED/THROWN), Se (ACTIVE/INACTIVE), Li (ON/OFF/INTENSITY n), Re ("id text"), signal masts ("Aspect; Lit; Unheld"), Pw (ON/OFF), T; topic = channel + per-type topic + the object's name, with a `{0}` placeholder; publishes with QoS 2 and retain | **yes** (open standard) | some, growing (DIY ESP32/Arduino) |
| **CTI Acela** | layout bus (token ring) | serial, through the Acela module only | Se (infrared, current, push-button, reed, photocell; inverted, debounce, threshold), outputs as Tu/Li; modules found at start | not stated | few |
| **ProTrak Grapevine** | layout bus | serial; simulator (can invent replies) | Se (parallel inputs, occupancy and motion sensors), Tu; nodes 1–127, renumbering tool | not stated | few |
| **Oak Tree Systems RCI** | layout bus | serial (RSC protocol) | Se, Tu on IO-24, IO-48, O-48 cards; the HH-2 hand throttle is not supported | not stated | few |
| **TracTronics SECSI** | layout bus | serial | Se, Tu (maker gone since 2016) | not stated | few |
| **Maple Systems** touch panels | panel | serial ("Simple ASCII" protocol) | 48 inputs as Se, 48 outputs as Tu per panel | not stated | few |
| **Digi XBee** (IEEE 802.15.4) | wireless bus | USB or serial XBee adapter in API mode 2 | Se (created by themselves when a pin reports), Li, Tu (one or two pins); node discovery | not stated | few |
| **Raspberry Pi GPIO** | pins | the Pi's own pins (an old Java pin library) | Se, Tu | — | some |
| **X10 / Insteon** powerline (CM11, CP290, PowerLinc 2412S) | home automation | serial | Li (dimming), Tu (X10), Se entered by hand | not stated | few |
| **Anyma DMX** | lighting | USB | Li | not stated | few |
| **IPOCS / IPOCSMR** | DIY (ESP8266 + Arduino Uno) | the computer listens on TCP port 10000 and announces itself by mDNS | Tu, Li, Se, matched by user name | the protocol is open source | few |
| **xAP** | home automation | network (a script) | Se, Tu from "basic status and control" devices | yes (xAP) | few |
| **Modbus** | industrial | — | planned, never released | yes (gk) | — |
| The reference software's own "simple server" link between two of its copies | software link | TCP port 2048 | Tu, Li, Se, Re, Pw | — | — |

### Devices

| Device | Kind | Connects by | What it gives | Open? | Users |
|---|---|---|---|---|---|
| **RFID readers** (CORE-ID / ID-Innovations, Olimex MOD-RFID125 and MOD-RFID1356MIFARE in CDC mode, Parallax; MERG concentrator for up to 8 readers) | reader | USB serial; or readers posting to MQTT | Se (tag present), Re (which tag), tag-to-car tables | not stated | some |
| **NAC Services RPS** | positioning | serial or USB, 115,200 | x/y/z of locos with transmitters (up to 10 readings a second, needs a DCC connection to poll them), area sensors defined by corner points, a tracking map | not stated | few |
| **DCC4PC** boards | detection + RailCom | own OmniBus protocol | Se (16 blocks per board), Re (RailCom), CV read-back in ops mode via RailCom | not stated | few |
| **Bachrus MTS-DCC, KPF-Zeller** speedometers | input device | serial | scale speed; automated speed profiles and speed matching (CVs 2, 5, 6) | not stated | some |
| **RailDriver** desk (P.I. Engineering) | hardware throttle | USB HID (a udev rule on Linux) | throttle levers, horn, bell, function buttons | not stated | some |
| USB game devices / keyboards | input | USB | Se from buttons (old, unmaintained library) | — | few |
| **NCE DCC packet analyzer** (2002 serial board at 38,400; 2023 USB DCC meter) | track monitor | serial / USB | every DCC packet on the rails, decoded or as hex; filters by packet type | its commands are reproduced in the pages | few |
| **DCC Specialties** Hare, Wabbit, Block Watcher, PSX | accessory/booster devices | over DCC | configured through scripts | — | some |
| Interfaces: terminal servers (Lantronix), USB-serial adapters (Keyspan), old Mac serial ports, Bluetooth | link hardware | — | — | — | — |

## 3. Recommended order for Leah

Order, with the reasons:

1. **NCE** (the owner's own gear and his USB interface; one of the two largest North American systems; a small, published binary protocol; serial only, which Leah already does).
2. **DCC-EX** (open source and documented; very popular with new and DIY modellers; it runs on **the same Arduino Mega + EX-MotorShield8874 the owner already uses** for Leah's base station, so it can be tested by loading a different firmware, with no purchase; simulator easy).
3. **MQTT layout objects** (Leah already has `mqtt_client`; nearly free; gives an NCE owner cheap WiFi sensors and outputs (ESP32/Arduino) where NCE's own inputs are weak; fits the owner's ESP32 dead-rail plans; tested against a local broker).
4. **LocoNet** (Digitrax is the other big North American system, and LocoNet peripherals (detection, signals, stationary decoders) are used beside other systems as a "standalone LocoNet"; Uhlenbrock, Fleischmann, Digikeijs/YaMoRC also speak it; testable without hardware through hex-trace playback and LocoNet over TCP).
5. **OpenLCB / LCC** (an NMRA standard and the growing layout bus; Leah already has a SocketCAN `can_bus`; fully testable on Linux with a virtual CAN interface (`vcan`) and open-source nodes; also reaches the TCS CS-105 command station).
6. **C/MRI** (NMRA-published, polled serial, very simple; the owner's own Arduinos can be C/MRI nodes over USB or RS-485, so it is testable for a few dollars; a long-standing North American favourite for panels and signalling).

Then, by reach and openness: 7 **Roco/Fleischmann Z21** (published UDP protocol, big in Europe, testable with a fake UDP server), 8 **Lenz/XpressNet** (published spec; many European sets), 9 **ESU ECoS**, 10 **SPROG** (cheap, UK), 11 **MERG CBUS** (CAN again, reuses the LCC work), 12 **Märklin CAN**, 13 **BiDiB**, 14 **RFID readers** (simple serial, nice for sample apps). The rest (MRC, EasyDCC, TAMS, ZIMO, TMCC, Acela, Grapevine, Oak Tree, X10, XBee, ...) only on request.

### What each of the top 6 needs

**1. NCE** — a connection object, e.g. `nce_station(port: "/dev/serial/by-id/...", via: "usb" | "serial" | "network", system: "powercab" | "sb3" | "sb5" | "twin" | "powerpro" | "ph5")`, plus `host:`/`port:` for a serial-to-network box.
- **Start:** ask for the firmware version (command 0xAA, three bytes back); while there is no answer, warn and ask again about every 10 seconds. That version also tells which USB interface jumpers are set (7.3.0–7.3.7), and so what this link can do.
- **Capabilities:** keep a list per link, because they vary a lot:
  - USB interface to a PowerCab or Twin (fw 1.28c+): throttle, ops programming, programming track. AIU inputs only with USB firmware 7 and station firmware 1.65+.
  - USB to SB3/SB5: ops programming only.
  - USB to a Power Pro: **no programming at all** and no AIU.
  - Never over USB: turnout monitoring, the clock, signal (extended accessory) programming, or memory access (so no consists or macros read from the station).
  - The script should be able to ask (`$cs.can("service_programming")`) and get a plain error otherwise.
- **`locomotive`:** 0xA2 (address high/low, the long-address bits set; ops 01–04 for 28/128 steps each way, 05/06 emergency stop, 07–09 function groups, 15/16 for F13–F28). Speed mode: 0x8D.
- **`dcc_base` programming:** 0x9E enters the programming track ('3' back = short circuit) and 0x9F leaves it. Direct mode writes with 0xA8 and reads with 0xA9; paged mode is 0xA0/0xA1 and register mode 0xA6/0xA7. A read gives the value then '!', or 0xFF then '3' when it can't read. Ops-mode writes are 0xAE for locos and 0xAF for accessories.
- **Power:** 0x89 turns the main on and the programming track off; 0x8B does the opposite. Both are listed only for the larger station. Soft and hard reset are 0xAB and 0xAC (careful: the hard reset changes the baud rate).
- **Raw DCC packets:** 0x93–0x96 and 0xA3–0xA5 queue raw 3–6 byte packets. This means Leah's own packet builder could drive any NCE station.
- **`turnout`:** 0xAD with op 03 (normal) or 04 (reverse). Address 1–2044, but 1–250 on old PowerCab/SB3 firmware. Op 05 sends a signal aspect (0–31) and op 01 runs a station macro (0x9C does too).
- **`sensor`:** AIU status 0x8A (4 bytes: state and changed bits) or 0x9B (2 bytes), polled. The number is (cab − 1) × 16 + pin − 1, so a script could write `sensor(nce: "50:2")`. AIU broadcasts must be off in the station's setup.
- **Extras (later):** fast-clock read/set/ratio (0x82–0x87); messages on cab screens (0x90–0x92); consists and macros in station memory (0x8E/0x8F/0x9D/0x97–0x9A), with backup/restore in NCE's own file format. Each reply is '!' for success, or an error code 0–4 (not supported / address / cab or op code / CV or data / byte count): turn these into plain Leah errors.
- **Testing:** a Leah-side NCE simulator that answers the command table (with a memory image) for the automated tests; the owner's PowerCab for the real check. Advise /dev/serial/by-id names: the pages spend a lot of space on Windows renumbering ports.

**2. DCC-EX** — `dcc_ex_station(port:)` or `(host:, port: 2560)`, speaking DCC-EX's text commands in `< >`.
- **Mapping:** loco speed and functions → `locomotive`; programming-track and main-track CV read/write/bit → `dcc_base`; power and the meter readings → `dcc_base`'s power and current.
- **`turnout`:** four kinds: a direct accessory address (no feedback), the station's own turnout table by ID 0–32767 (feedback from the station), an output pin by table ID, or a virtual pin with `z` (fire-and-forget, needs fw 4.2.35+).
- **`sensor`:** the station's sensor table (pin, pull-up), read at start.
- **Extras:** list and start its EXRAIL routes; show its messages and its display; export the roster as its `ROSTER()` lines.
- **Two ideas beyond copying:**
  - (a) Leah's own board program could accept the common subset of DCC-EX's commands. Then phone throttles that speak DCC-EX (Engine Driver, EX-Toolbox) work with Leah's own base station.
  - (b) A "share over TCP" switch on the connection, like DCC-EX over TCP, so several Leah apps or phones use one station.
- **Testing:** a simulator in Leah; the owner's Mega with DCC-EX firmware loaded for the real check (it is the same board as Leah's base station, so no purchase).

**3. MQTT layout objects** — `mqtt_layout($client, channel: "trains/", topics: [...])` giving `turnout`, `sensor`, `light`, `signal_mast`, `reporter`, power, and throttle objects whose state goes over topics.
- **Use the same default words** as the reference software (CLOSED/THROWN/INCONSISTENT/UNKNOWN, ACTIVE/INACTIVE, ON/OFF/INTENSITY 0.00–1.00, "id text", "Aspect; Lit; Unheld"). Then the many ESP32/Arduino sketches already written for it work with Leah unchanged.
- **Topics:** a per-type topic template with `{0}` for the object's name, which may contain `/` for the device.
- **Delivery:** retained messages, so states arrive when an app starts; QoS 2 when sending; a last-will message so gadgets see the app go away.
- **Mapping:** turnout feedback modes as in §4. A reporter's ID becomes a tag (RFID readers posting to MQTT come for free).
- **Testing:** a local mosquitto broker, as Leah's own MQTT tests already do.

**4. LocoNet** — `loconet(port:, interface: "locobuffer_ng" | "locobuffer_usb" | "pr3" | "pr4" | "dcs52_usb" | "dcs240_usb" | "locobuffer" | "pr2", command_station: "dcs240" | ... | "standalone" | "pr_programmer")`, or `(host:, port: 1234)` for LocoNet over TCP. One object can be a full command-station link, a standalone bus, or a stand-alone programmer.
- **Mapping:**
  - Throttles take a command-station **slot** for the loco (speed steps, functions, consist) → `locomotive`. The service and ops programmer → `dcc_base`.
  - Switch requests 1–2048 → `turnout`, where a light is just a turnout number. Input reports → `sensor`. Transponding zones → `reporter`.
  - Global power on/off/idle; the LocoNet fast clock; SE8C/SE74 signals.
  - Configuration of devices (LNCV, LNSV1, BDL16x/DS64/PM4x/SE8C option switches) is later work.
- **Copy the hard-won behaviours** listed in §4: retry when the command station rejects a switch command, a minimum gap between turnout commands in a route (250 ms), and updating on send instead of waiting for the echo.
- **Testing:** play back hex traces (one packet per line), as their simulator does; a fake LbServer over TCP; real hardware only when someone with Digitrax asks.

**5. OpenLCB / LCC** — `lcc_bus(can: $can_bus)` on Leah's SocketCAN object, or `(host:, port: 23)` for GridConnect text over TCP, or a GridConnect USB adapter as a serial port. Leah becomes an LCC node and needs a 6-byte node ID.
- **Node ID:** the reference software makes one from an IP byte and the process ID at each start. Leah should keep one in the app's data instead (portable folder rules), so event IDs it makes stay the same between runs.
- **Mapping:** `sensor` = two event IDs (active, inactive), or one that resets itself after a moment; `turnout` and `light` = the two events Leah produces; `signal_mast` = one event per aspect plus lit/held; train nodes → `locomotive` (TCS CS-105).
- **Event IDs:** written as `x0102...` or dotted bytes, and given names in an event table.
- **Node configuration:** read the node's own description (CDI) to configure it with no definition files.
- **"Authoritative" and "always listen" settings:** decide who owns a shared state. Worth copying as options.
- **Testing:** `vcan0` on Linux plus an open-source software node; no purchase. The same CAN code later serves CBUS.

**6. C/MRI** — `cmri_bus(port:, baud:)` or `(host:, port:)`, with `node(address:, type: "smini" | "susic" | "cpnode", cards: ...)` definitions. Each node gets an initialization message, then is polled in a loop.
- **Mapping:** each input bit → `sensor`; each output bit (or a pair, steady or pulsed) → `turnout` or `light`; signals from 2–3 outputs. Number bits from 1 but say plainly that Arduino libraries count from 0 (a known source of confusion in the pages).
- **Extras:** a node on/off switch for testing, and network statistics.
- **Testing:** a simulated node in Leah; then the owner's Arduino running a C/MRI library over USB.

## 4. Connection handling worth copying (in spirit)

- **Many connections at once, each with its own namespace.** A turnout 2 on one link and turnout 2 on another are different objects. A per-job **default** decides which link does throttles, programming, power, and command-station duties. Example from the pages: one PR3 as a stand-alone programmer and another as the layout's LocoNet. Leah: a script can open several station objects, and `locomotive`/`turnout` take an optional station argument, with a default station per app.
- **A simulator for every system**, chosen like a real link, so panels and scripts can be built away from the layout. Notable ones:
  - LocoNet's plays back recorded hex traces.
  - Grapevine's can invent replies ("all inputs", "first 4 per bank").
  - CBUS's simulates command stations, nodes, and event responders, each with a direction and a delay.
  - BiDiB's is described by an XML file.
  - SPROG's simulator can be a programmer or a command station.
- **Simulated trains for testing**, beyond simulated stations. Virtual trains run on the layout's blocks under real throttles, set block sensors as they pass, and keep a "ground truth" (where each train really is, its speed, collisions). Tests compare the program's conclusions with that truth. Their rules: wait for conditions, assert the order of events, never sleep or assert durations. This fits Leah's DCC safe ladder and the dead-rail plans well.
- **A handshake and health check:** ask the station for its version at start (NCE 0xAA; XpressNet reads the station's type and version and adjusts what it offers), and ask again whenever errors appear. Plus a loopback test (NCE: join pins 2 and 3) to prove the cable and port.
- **Capabilities known per link and per firmware**, and features that can't work disabled rather than failing later (NCE USB; XpressNet's full / partial / minimal stations; ECoS programming only from certain firmware).
- **Auto-detection and stable naming:**
  - BiDiB stores the interface's unique ID and **finds the port by scanning** at the next start, so USB renumbering doesn't matter.
  - Network devices are found through **mDNS** (OpenLCB adapters as `openlcbcan.local.`, netBiDiB, the CBUS hub as `_cbus-can._tcp`; IPOCS units find the computer the same way).
  - On Linux, Leah can simply prefer /dev/serial/by-id names.
- **Sharing one link with other computers:** LbServer (LocoNet over TCP in plain text), DCC-EX over TCP (also serves phone throttles), the OpenLCB and CBUS hubs (GridConnect frames over TCP), and the Märklin serial-to-TCP bridge. For Leah: one "share" option on any station or bus object.
- **Commanded vs known state** on turnouts, with feedback modes: direct (assume it moved), one sensor, two sensors, delayed (show "moving" for a few seconds), monitoring (believe the bus), and exact (two microswitches through the decoder). Leah's `turnout` should have this.
- **Throttle sharing rules** when another cab holds the same loco: steal, share, or ask (CBUS), and "loss of control" on ECoS. The reference software greys out a throttle window whose loco was taken; Leah should raise an event instead.
- **The station's own data:** read and write the station's loco database (ECoS, TAMS, MS2 import), consists and macros (NCE backup and restore in NCE's own file format), roster export (DCC-EX), and function labels (TCS). Fast-clock sync with the station's clock is also covered (NCE with a small control loop; LocoNet; CBUS clock messages).
- **Robust sending:**
  - LocoNet's "strict" mode sends one message at a time, waits for its echo, retries 5 times, and backs off when busy.
  - Handling a rejected switch command: retry fast, spread out, or send once only.
  - An optional "update the state on send, ignore our own echo".
  - A minimum interval between outputs in a route.
- **A monitor and a raw-send tool for every system**, with logging to a file and decoding into readable text. A Leah connection object should have an `on_traffic` event and a `send_raw()`, so sample apps can show a monitor.
- **Hot-plug:** BiDiB invalidates the objects of a node that disappears and brings them back when it returns. **Keep-alive** pings (netBiDiB). **Last will** (MQTT).
- **Where the reference software is weak (improve, don't copy):** almost every connection change needs a restart; port names are chosen by hand; and the NCE packet analyzer needs a dummy "NCE simulator" connection just to show its menu. Leah should connect and disconnect at run time and report plainly what a link can't do.
- **A Leah-specific extra:** the **NCE DCC packet analyzer** (USB, 2023 model) reads the actual packets on the rails. It would be an excellent independent check of Leah's own base station output on the owner's safe ladder, and its command set is in the pages.

## 5. Pages read

All paths are relative to `help/en`. The folder holding the per-system tool pages is written `<pkg>/` (its real name repeats the reference software's name).

**`html/hardware/` (115 text pages, all read):** index.shtml; TerminalServer, USBtoSerial, MacintoshSerialPorts, PDA; acela/index.html, index.shtml, Names; anyma/index; arduino/index, ArduinoCMRIByteComm; bachrus/index; bidib/index, index_de, BiDiB-SignalMast, BiDiB-SignalMast_de (German copies skimmed); can/index, scripting; can/cbus/index, Names, CbusFastClock, CbusMeters; cmri/CMRI, Signals; dcc4pc/index; dcc-ex/index, Turnouts, Sensors, Lights; dccpp/index; digirails/index; easydcc/EasyDCC; ecos/index; grapevine/index, Names; ipocsmr/index (.html and .shtml); the reference software's own client-link page (its folder name repeats the name); loconet/Addressing, BTLocoBridge, CommandStationConfig, DCS240, DCS52, DigitraxMeters, DigitraxPower, Digitrax, DS54, EnhancedSlots, Fleischmann, KeyspanUSB, LbServer, LocoBuffer, LocoBufferII, LocoBufferUSB, LocoIO, LocoNetClasses, LocoNetSim, LocoNetTools, LocoNetworking, MS100, PR2, PR3, PR4, SE74, StandaloneLocoNet, Uhlenbrock; maple/index; marklin/index, McanLibrary, McanTcpBridge; merg/index, Details; misc/DccSpecialties; modbus/index; mqtt/index; mrc/index; nce/NCE, Wangrow; oaktree/OakTree; openlcb/index, Connections, Details, Names, OlcbSignalMast, TCS; pi/index; powerline/index, Compatibility, Names; qsi/index; raildriver/index, details; rfid/index, GettingStartedWithRFID; roco/z21/index; rps/index, sensors; secsi/index; sprog/SPROG; sproggen5/index, hardware; SRCP/index; tams/index; tchtech/index; tmcc/index; xap/index; XBee/index, XBeeNodeConfigurationTool; XPressNet/index, XNetAddressing, LI101Configurationtool, LZV100Configurationtool, XNetSystemInformationtool, CreateXNetLogFile; zimo/Zimo.

How deeply: the long ones (Digitrax, C/MRI, Standalone LocoNet, SE74, the RFID guide, MQTT, XPressNet) were read for connections and features, and skimmed past that (wiring examples, signalling recipes).

**`<pkg>/` (99 tool pages, each read at least to its summary; the NCE ones in full):** AbstractMonFrame; acela (monitor, node config, packet gen); bachrus SpeedoConsoleFrame; can/cbus/swing (bootloader, slot monitor, config tool, console, event request monitor, event table, hub, two mode switchers, node manager, simulator); can/swing/send; cmri/serial (assignments, diagnostics, node config); ecos/swing (loco database, packet gen, preferences, status); grapevine (node config, node table, renumber); ieee802154/swing (three placeholder pages); jinput tree; loconet (BDL16, clock monitor, command-station config, firmware downloader, DS64, duplex group, LNSV1 discover, packet gen, LocoNet ID, LocoIO, monitor, stats, PM4, PR3 mode select, SDF editor and trigger conditions, SE8, slot monitor, sound editor and loader, LNCV programmer, LNSV1 programmer, throttle message, DCS240 and DCS52 USB mode select); maple (assignments, node config, monitor); marklin packet gen; mrc (monitor, packet gen); nce (packet analyzer, clock monitor, consist editor, macro editor, command generator, USB interface tool); oaktree node config; openlcb/swing (event table, hub, ID tool, memory tool, network tree, send frame, STL editor); powerline (node config, packet gen, monitor); rfid tag monitor; rps (align table, alignment, debugger, polling, sound speed, tracking); secsi (node config, packet gen, monitor); sprog (console, packet gen, monitor, slot monitor, two update tools); tams (loco data, packet gen); zimo (monitor, packet gen).

**`html/doc/Technical/` (skimmed for support facts only):** Names, simulatedtrains, TurnoutFeedback, Networking, MultiConnectionUpdate, SystemStructure, NewSystem; the other 50 are developer internals (version control, build tools, Java, XML) and were not used.

**Missing or seemingly important:**
- Pages that are linked but not in the copy: `html/hardware/rfid/Names.shtml` (RFID object naming), `html/hardware/anyma/Names.shtml`, `<pkg>/openlcb/swing/downloader/LoaderFrame.shtml` (LCC firmware update), the per-release notes linked from loconet, openlcb, and marklin, and several images.
- Topics with no page of their own: the NCE "Show Cabs" and "Program Booster" tools, the XpressNet and DCC-EX monitors, the Z21 configuration tool, the LocoBuffer-NG (it shares the LocoBuffer-USB page), Hornby, Atlas, and ZTC (covered only inside XPressNet), and a **network-throttle server** page (WiThrottle / phone throttles, which lives outside this area).
- There is **no single feature-by-system matrix** anywhere in the copy; the table in §2 is pieced together from the separate pages.
