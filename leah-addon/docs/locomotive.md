# locomotive

## Description

A train on a [dcc_base](ref:dcc_base), by its decoder's DCC address: its speed, its direction, and its functions (the headlight and whatever else its decoder does). Each change sends its packet at once. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$loco.speed = 30
$loco.set_function(0, true)
$loco.stop()
print($loco.speed)
print(last($base.packets)["meaning"])
```

Output:

```
0
locomotive 3: stop, forward
```

## Making one

**locomotive**(address, long: false, base: null) · locomotive

| Part | Description |
|---|---|
| address | Required. The decoder's address: 1 to 127 for a short address; 128 to 10239 with `long: true` for a long one. 0 or anything past 10239 is a `range_error`, and so is a long address without `long: true`: `locomotive(): 200 is a long address: give long: true (a short address is 1 to 127)`. |
| long | Optional. `true` sends the address in its long, two-byte form, for 1 to 127 too: a decoder answers to one or the other, so `locomotive(3)` and `locomotive(3, long: true)` are two different locomotives. |
| base | Optional. The [dcc_base](ref:dcc_base) it runs on; by default the one made last. With none made, it is a `hardware_error`: `locomotive(): there is no base station: make one first, dcc_base("/dev/ttyACM0") or dcc_base(simulate: true)`. |

It starts stopped, going forward, with every function off; nothing is sent until something changes. On a base station board, its speed and its lights (F0 to F4) are kept in the board's refresh table and sent again and again while the script holds the locomotive; once the script lets go of it, the board stops sending them. The table holds 16 locomotives: past that, a locomotive's packets are sent once each time they change, with a warning.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [address](#address) | number | ✓ |
| [base](#base) | dcc_base | ✓ |
| [direction](#direction) | text | |
| [long](#long) | boolean | ✓ |
| [speed](#speed) | number | |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [emergency_stop](#emergency_stop) | | |
| [function](#function) | number | boolean |
| [set_function](#set_function) | number, on as boolean | |
| [stop](#stop) | | |
| [write_cv](#write_cv) | cv as number, value as number, pause: number, index: array | |
| [write_cv_bit](#write_cv_bit) | cv as number, bit as number, on as boolean, pause: number, index: array | |

## Property descriptions

### address

**address** · number · read-only

The decoder's address.

---

### base

**base** · dcc_base · read-only

The base station it runs on.

---

### direction

**direction** · text

`"forward"` (the default) or `"reverse"`. Setting it sends the speed again with the new direction. Anything else is a `type_error`.

---

### long

**long** · boolean · read-only

Whether its address is sent in the long form.

---

### speed

**speed** · number

The speed, in 128 steps: 0 is stopped and 126 is full speed. Setting it sends the speed and the direction. A number outside 0 to 126 is a `range_error`, and anything but a whole number a `type_error`.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$loco.speed = 126
print(last($base.packets)["bytes"])
```

Output:

```
03 3F FF C3
```

## Function descriptions

### emergency_stop

**emergency_stop**()

Stops this locomotive at once, without the slowing down its decoder may be set to, and sets [speed](#speed) to 0. To stop every locomotive, use the base station's [emergency_stop](ref:dcc_base#emergency_stop).

---

### function

**function**(number) · boolean

Whether a function is on: 0 (the headlight) to 28.

---

### set_function

**set_function**(number, on)

Turns a function on (`true`) or off (`false`): 0, the headlight, to 28. What F1 to F28 do depends on the decoder. The functions go in groups (F0 to F4, F5 to F8, F9 to F12, F13 to F20, F21 to F28), and the packet sent holds the whole group. A number outside 0 to 28 is a `range_error`.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$loco.set_function(0, true)
$loco.set_function(2, true)
print(last($base.packets)["meaning"])
print($loco.function(2))
```

Output:

```
locomotive 3: functions F0 to F4: F0, F2 on, the rest off
true
```

---

### stop

**stop**()

Sets [speed](#speed) to 0: the locomotive slows to a stop as its decoder is set to.

---

### write_cv

**write_cv**(cv, value, pause: 500)

Writes a value (0 to 255) into one of the locomotive's decoder settings (a CV, 1 to 1024) **on the main track**, while it runs: the usual way to tune its momentum, speed, or sound while watching it. A decoder takes the change only after two identical packets with nothing else for it in between, which the base station sees to. A decoder can't answer on the main, so nothing confirms the change: watch or listen for it (on the programming track, [dcc_base.write_cv](ref:dcc_base#write_cv) confirms it). Writes to one locomotive are at least `pause:` milliseconds apart (500 by default, 0 to 10000), as some decoders miss a write sent sooner; a write sooner than that waits. `index: [16, 3]` writes CV 31 and CV 32 first (each a write of its own, with the pause between), for an indexed setting above 256 (see [dcc_base.read_cv](ref:dcc_base#read_cv)).

CV 7 (the decoder's version) can't be written, and CV 8 isn't written here, since writing it is how many decoders are reset to their factory settings: both are a `range_error`, as is a number or value out of range. Writing the address settings (CV 1, 17, 18, 29) on the main changes which address the decoder answers to, while this object goes on sending to its old one.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$loco.write_cv(3, 20)
print($base.decoder(3)["cvs"])
```

Output:

```
[3: 20]
```

---

### write_cv_bit

**write_cv_bit**(cv, bit, on, pause: 500)

As [write_cv](#write_cv), for one bit (0 to 7) of a setting, leaving its other bits as they are: `true` or `1` sets it, `false` or `0` clears it.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$loco.write_cv_bit(29, 1, true)
print($base.decoder(3)["cvs"][29])
```

Output:

```
2
```

## See also

[dcc_base](ref:dcc_base), [Model railroading](ref:Model railroading)
