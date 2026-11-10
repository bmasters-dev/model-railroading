# dcc_base

## Description

A DCC base station, the command station's heart: it sends the packets the decoders in the locomotives obey. [locomotive](ref:locomotive) objects are made on it, and each change to one sends its packet. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

The base station is simulated: it has no board and puts no power on any rails. Each packet is turned into the signal a base station would put on the rails, and a simulated decoder for each address reads it back from that signal, as a decoder does. What each decoder understood is given by [decoder](#decoder). A base station isn't shown and needs no window.

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

**dcc_base**(simulate: false) · dcc_base

| Part | Description |
|---|---|
| simulate | Required, as `true`: a simulated base station. Without it, or with a port given, it is a `hardware_error`: `dcc_base(): give the board's port, or simulate: true for a simulated base station (the board isn't supported yet)`. |

It starts with the track power off, as a command station does: until [power](#power) is `true`, packets are sent but no decoder hears them.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [packets](#packets) | array | ✓ |
| [power](#power) | boolean | |
| [simulate](#simulate) | boolean | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [clear_packets](#clear_packets) | | |
| [decoder](#decoder) | address as number, long: boolean | array |
| [emergency_stop](#emergency_stop) | | |

## Property descriptions

### packets

**packets** · array · read-only

The last 200 packets sent, the oldest first. Each is an array of four:

| Key | Holds |
|---|---|
| `bytes` | the packet's bytes in hex, the error byte last: `"03 3F A9 95"` |
| `bits` | its bits as they go onto the rails, grouped: the preamble, then each byte after its 0, then the closing 1 |
| `meaning` | what it says, in words: `"locomotive 3: speed 40, forward"` |
| `heard` | `true` when a decoder heard it; `false` when the track power was off |

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

### power

**power** · boolean

The track power: `true` puts the signal on the simulated rails, so the decoders hear the packets; `false` (the default) takes it off. With the power off, packets are still sent and listed in [packets](#packets), with `heard` `false`, and the decoders keep what they last understood. Anything but `true` or `false` is a `type_error`.

---

### simulate

**simulate** · boolean · read-only

`true`: the base station is simulated.

## Function descriptions

### clear_packets

**clear_packets**()

Empties [packets](#packets).

---

### decoder

**decoder**(address, long: false) · array

What the simulated decoder at an address understood from the signal, as an array: `address`, `long`, `speed` (0 to 126), `direction` (`"forward"` or `"reverse"`), `functions` (F0 to F28, each `true` or `false`), and `packets` (how many it heard). `null` when no packet has reached that decoder. `long: true` asks for the decoder at a long address, as for [locomotive](ref:locomotive).

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

### emergency_stop

**emergency_stop**()

Stops every locomotive at once: the emergency stop to the broadcast address, which every decoder obeys. Each [locomotive](ref:locomotive) made on this base station has its [speed](ref:locomotive#speed) set to 0.

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

## See also

[locomotive](ref:locomotive), [Model railroading](ref:Model railroading)
