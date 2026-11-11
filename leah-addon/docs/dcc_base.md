# dcc_base

## Description

A DCC base station, the command station's heart: it sends the packets the decoders in the locomotives obey. [locomotive](ref:locomotive) objects are made on it, and each change to one sends its packet. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

A base station is one of two kinds:

- **Simulated** (`simulate: true`): no board, no power on any rails. Each packet is turned into the signal a base station would put on the rails, and a simulated decoder for each address reads it back from that signal, as a decoder does.
- **Leah's base station board** (a port): an Arduino Mega running Leah's base station program makes the real DCC signal, its timing from the board's own hardware timer. With an **EX-MotorShield8874** on it, the signal goes onto the rails, and the board watches the track's current and turns the power off by itself on a short; without the shield, the signal is 5 V logic on its pin 12, for checking on the bench. A second board, the **reader** (an Arduino Uno or Nano running Leah's reader program), can read the signal back as a decoder would and tell Leah what it heard and how long each half-bit lasted. [Model railroading](ref:Model railroading#leahs-base-station-board) says how to join them.

What each decoder understood (the simulated ones, or the one the reader stands for) is given by [decoder](#decoder). A base station isn't shown and needs no window.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$loco.speed = 64
$loco.direction = "reverse"
print($base.decoder(3)["speed"])
print($base.decoder(3)["direction"])
```

Output:

```
64
reverse
```

## Making one

**dcc_base**(simulate: true) · dcc_base

**dcc_base**(port, reader: null) · dcc_base

| Part | Description |
|---|---|
| simulate | `true`: a simulated base station. It takes no port and no reader (a `type_error`). |
| port | The base station board's port, its whole path: `"/dev/ttyACM0"`. [dcc_ports](ref:DCC board functions#dcc_ports) lists the boards plugged in. |
| reader | Optional. The reader board's port, to read the signal back. Without it, [decoder](#decoder) gives `null` and [timing](#timing) is `null`. |

Opening a board's port starts its program again, as an Arduino's does, so making a base station on a board takes a second or two: it waits until both boards say they run Leah's programs and the base station answers, then starts clean (nothing kept from before, the track power off). It is a `hardware_error`, in words that say what to do, when there is no such port (`dcc_base(): there is no board on /dev/ttyACM1: is it plugged in? (dcc_ports() lists the ports there are)`), when this account may not use it yet (`... on Linux, apps may use a board's USB port only once your account is in the `dialout` group ...`), when another program has it open, or when a board isn't running Leah's program (`... load it with load_dcc_program("/dev/ttyACM0", "base")`). Without a port and without `simulate: true`, it is a `hardware_error` saying to give one.

Either kind starts with the track power off, as a command station does: until [power](#power) is `true`, packets are sent but no decoder hears them.

```leah check
$base = dcc_base("/dev/ttyACM0", reader: "/dev/ttyUSB0")
$loco = locomotive(3)
$loco.speed = 40
$base.power = true
```

**On a board**, it starts with a [current_limit](#current_limit) of 1500 mA. A locomotive's speed and its lights (F0 to F4) are kept in the board's refresh table and sent again and again, as command stations do, so a decoder that missed one packet, or a locomotive put on the track later, still gets them; every other packet is sent at once, three times. The table holds 16 locomotives. **Safety**: while the track power is on, Leah tells the board every second that it is still there; after 3 seconds without a word (the program ended or crashed, or the cable came out) the board turns the track power off by itself, and Leah says so with a warning when it next hears from the board. With the motor shield, it also turns the power off when the track's current is over the [current_limit](#current_limit) for about 5 ms all told (a short: a coin on the rails, a derailed wheel; the first 20 ms after the power comes on are left out, for the capacitors in the track and the decoders to charge), and at once when the shield's driver reports a fault (too hot, a short it caught itself, or no supply in the shield's jack), each with a warning saying why. After such a cut-off it won't turn the power on again for 2 seconds: setting [power](#power) to `true` sooner leaves it `false`, with a warning. A board that starts again by itself (its power dipped) has its refresh table and limit sent again, with its track power off and a warning; turn it on again when the cause is found. The board checks the current itself, many thousands of times a second, so nothing the script does can delay it. A warning reads: `the base station turned the track power off by itself: the current was over the limit (2210 mA, the limit 1500 mA): is something shorting the track?`.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [current](#current) | number | ✓ |
| [current_limit](#current_limit) | number | |
| [packets](#packets) | array | ✓ |
| [port](#port) | text | ✓ |
| [power](#power) | boolean | |
| [reader](#reader) | text | ✓ |
| [shield](#shield) | boolean | ✓ |
| [simulate](#simulate) | boolean | ✓ |
| [timing](#timing) | array | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [clear_packets](#clear_packets) | | |
| [close](#close) | | |
| [read_address](#read_address) | in_background: boolean | array |
| [read_cv](#read_cv) | cv as number, in_background: boolean | number |
| [decoder](#decoder) | address as number, long: boolean | array |
| [emergency_stop](#emergency_stop) | | |

## Events

| Name | Parameters |
|---|---|
| [on_read](#on_read) | what, value, error |

## Property descriptions

### current

**current** · number · read-only

The track's current in milliamps, as the motor shield measured it (smoothed, said by the board every half second): 0 with the power off. `null` without a shield, and on a simulated base station. Its steps are about 5 mA.

```leah check
$base = dcc_base("/dev/ttyACM0")
$base.power = true
sleep(1000)
print("the track draws " + $base.current + " mA")
```

---

### current_limit

**current_limit** · number

The current, in milliamps, over which the board turns the track power off by itself (held for 5 ms): 10 to 5000, 1500 at the start. Outside that is a `range_error`, and anything but a whole number a `type_error`. A low limit is a safe way to try the cut-off: a 470 Ω resistor across the track at 12 V draws about 25 mA, so with the limit at 15 the power goes off. On a simulated base station it is kept but does nothing.

---

### packets

**packets** · array · read-only

The last 200 packets sent, the oldest first. Each is an array of four:

| Key | Holds |
|---|---|
| `bytes` | the packet's bytes in hex, the error byte last: `"03 3F A9 95"` |
| `bits` | its bits as they go onto the rails, grouped: the preamble, then each byte after its 0, then the closing 1 |
| `meaning` | what it says, in words: `"locomotive 3: speed 40, forward"` |
| `heard` | Simulated: `true` when a decoder heard it, `false` when the track power was off. On a board: `true` once the reader heard it go out, `false` until then (and for good when a newer packet took its place before it went out, or with no reader) |

```leah
$base = dcc_base(simulate: true)
$base.power = true
locomotive(3).speed = 40
$p = $base.packets[0]
print($p["bytes"])
print($p["bits"])
print($p["meaning"])
```

Output:

```
03 3F A9 95
11111111111111 0 00000011 0 00111111 0 10101001 0 10010101 1
locomotive 3: speed 40, forward
```

---

### port

**port** · text · read-only

The base station board's port, or `null` when it is simulated.

---

### power

**power** · boolean

The track power: `true` puts the signal on the rails (the simulated ones, or the board's pin), so the decoders hear the packets; `false` (the default) takes it off. With the power off, packets are still sent and listed in [packets](#packets), and the decoders keep what they last understood. On a board, it turns `false` by itself when the board turned the power off for safety (Leah had gone quiet for 3 seconds), with a warning. Anything but `true` or `false` is a `type_error`; on a board that was closed, or whose cable came out, setting it is a `hardware_error`.

---

### reader

**reader** · text · read-only

The reader board's port, or `null` when there is none.

---

### shield

**shield** · boolean · read-only

`true` when the base station board has its motor shield on it (the board finds it as its program starts), so its signal goes onto the rails and its [current](#current) is measured; `false` without one, and on a simulated base station.

---

### simulate

**simulate** · boolean · read-only

`true` when the base station is simulated, `false` on a board.

---

### timing

**timing** · array · read-only

What the reader measured of the signal over its last second, or `null` before it has said anything (and always, simulated or without a reader):

| Key | Holds |
|---|---|
| `one` | the shortest and longest half of a 1 bit, in microseconds, as a list: `[57.5, 58.0]`; `null` when there was no signal |
| `zero` | the same for a 0 bit: `[99.5, 100.0]` |
| `packets` | the packets it heard |
| `outside` | the half-bits outside what a decoder takes (a 1's half 52 to 64 µs, a 0's 90 to 10,000 µs) |
| `errors` | the packets whose error byte was wrong |

The NMRA's limits for a command station are a 1's half of 55 to 61 µs and a 0's of at least 95 µs.

```leah check
$base = dcc_base("/dev/ttyACM0", reader: "/dev/ttyUSB0")
$base.power = true
sleep(1500)
$t = $base.timing
print("a 1's halves: " + $t["one"][0] + " to " + $t["one"][1] + " µs")
```

## Function descriptions

### clear_packets

**clear_packets**()

Empties [packets](#packets).

---

### close

**close**()

On a board: turns the track power off, forgets the refresh table, and lets go of both ports, so another program (or [load_dcc_program](ref:DCC board functions#load_dcc_program)) can use them. A base station the script lets go of closes by itself. Afterwards, changing it is a `hardware_error`. A simulated one only has its power turned off.

---

### decoder

**decoder**(address, long: false) · array

What the decoder at an address understood from the signal: the simulated decoder's, or on a board, what the reader heard for that address, as an array: `address`, `long`, `speed` (0 to 126), `direction` (`"forward"` or `"reverse"`), `functions` (F0 to F28, each `true` or `false`), and `packets` (how many it heard). `null` when no packet has reached that decoder (and always on a board with no reader). On a board the reader tells Leah each packet when it first hears it and then at most every 2 seconds, so `packets` counts those, not every repeat. `long: true` asks for the decoder at a long address, as for [locomotive](ref:locomotive).

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$loco.set_function(0, true)
$d = $base.decoder(3)
print([$d["functions"][0], $d["functions"][1], $d["packets"]])
print($base.decoder(4))
```

Output:

```
[true, false, 1]

```

---

### read_address

**read_address**(in_background: false) · array

Reads the address of the locomotive on the **programming track** (the motor shield's **B** screws), as an array: `address`, and `long` (`true` when the decoder answers to its long address). It reads the decoder's settings CV 29 (whose bit 5 says which address it answers to), then CV 1 (the short address) or CV 17 and 18 (the long one), as [read_cv](#read_cv) does, so it takes a few seconds, longer when a setting needs another try.

```leah check
$base = dcc_base("/dev/ttyACM0")
$a = $base.read_address()
print("the locomotive answers to " + $a["address"] + (if $a["long"] then " (long)" else ""))
```

With `in_background: true` it gives back `null` at once and reads in the background: [on_read](#on_read) gets the answer (its `what` is `"address"`). In a window's code, use that: waiting would freeze the window.

---

### read_cv

**read_cv**(cv, in_background: false) · number

Reads a decoder setting (a CV, 1 to 1024) of the locomotive on the **programming track**, the motor shield's **B** screws, and gives back its value, 0 to 255. Only a base station board with its motor shield can read: on a simulated one, or without the shield, it is a `hardware_error`. A number outside 1 to 1024 is a `range_error`.

Reading uses the NMRA's service mode: the base station turns the **main track's power off** (A), powers the programming track, gives the decoder a moment to start (up to 1 A for its first second and a half, as a sound decoder's capacitors need), then asks it eight yes-or-no questions, one per bit, and checks the whole byte at the end. The decoder says "yes" by pulsing its motor for a few milliseconds, which the shield sees as a rise in current: so a decoder needs its motor connected, and clean wheels. A setting no decoder answered is asked again, three times in all, then it is a `hardware_error` saying what the base station saw: `no decoder answered on the programming track (the shield's B screws) (baseline 0 mA, highest 0 mA): is a locomotive on it, with clean wheels?`. After it, the main track's [power](#power) is off: turn it on again to drive. One reading at a time: a second one while one is going on is a `hardware_error`.

Some useful settings: CV 1 the short address, CV 7 the decoder's version, CV 8 its maker (141 is SoundTraxx, 151 ESU, 153 TCS, 129 Digitrax, 11 NCE), CV 29 its configuration, CV 17 and 18 its long address.

```leah check
$base = dcc_base("/dev/ttyACM0")
print("the decoder's maker: " + $base.read_cv(8))
```

With `in_background: true` it gives back `null` at once, and [on_read](#on_read) gets the answer (its `what` is the CV's number).

---

### emergency_stop

**emergency_stop**()

Stops every locomotive at once: the emergency stop to the broadcast address, which every decoder obeys. Each [locomotive](ref:locomotive) made on this base station has its [speed](ref:locomotive#speed) set to 0; on a board, each one's kept speed becomes its own emergency stop too, so the refresh table doesn't start it again.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$a = locomotive(3)
$b = locomotive(7)
$a.speed = 80
$b.speed = 50
$base.emergency_stop()
print([$a.speed, $b.speed, $base.decoder(3)["speed"], $base.decoder(7)["speed"]])
print(last($base.packets)["bytes"])
```

Output:

```
[0, 0, 0, 0]
00 41 41
```

## Event descriptions

### on_read

**on_read**(what, value, error)

A reading started with `in_background: true` is done. `what` is the CV's number ([read_cv](#read_cv)) or `"address"` ([read_address](#read_address)); `value` is what was read (a number, or the address's array), or `null` when it failed, and then `error` says why in words (`null` otherwise). The main track's power is off after a reading. A script without windows waits for it.

```leah check
$base = dcc_base("/dev/ttyACM0")
$base.on_read = function($what, $value, $error) {
    if $error == null {
        print("address " + $value["address"])
    } else {
        print("couldn't read: " + $error)
    }
}
$base.read_address(in_background: true)
```

## See also

[locomotive](ref:locomotive), [DCC board functions](ref:DCC board functions), [dcc_program_job](ref:dcc_program_job), [Model railroading](ref:Model railroading)
