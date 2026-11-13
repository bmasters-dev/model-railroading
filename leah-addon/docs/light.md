# light

## Description

An accessory decoder's output switched on and off: a scenery lamp, a fascia indicator, an uncoupler, an animation. It is sent through the base station as a turnout is, and it can follow a [sensor](ref:sensor), a [turnout](ref:turnout), or a [block](ref:block) by itself. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

On is the decoder's output for closed, and off its output for thrown; `inverted: true` swaps them. Unlike a turnout's move, the output is never let go: a lamp's decoder keeps it as last set. Only the track power carries the packets.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$siding = sensor(22)
$lamp = light(30, name: "Siding lamp")
$lamp.follow($siding)
print($lamp.on)
$siding.simulate("active")
print([$lamp.on, $lamp.following.name])
```

Output:

```
false
[true, "S22"]
```

## Making one

**light**(address, name: "L" and the address, inverted: false, base: ...) · light

| Part | Description |
|---|---|
| address | Required. Its accessory decoder's output, 1 to 2044. Anything else is a `range_error`. |
| name | Optional. Its name (`"L30"` for address 30 when not given). |
| inverted | Optional. `true` to send on as the decoder's thrown output. |
| base | Optional. The [dcc_base](ref:dcc_base) it is switched through; by default the one made last. With none made, it is a `hardware_error`. |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [address](#address) | number | ✓ |
| [base](#base) | dcc_base | ✓ |
| [following](#following) | object | ✓ |
| [inverted](#inverted) | boolean | ✓ |
| [name](#name) | text | ✓ |
| [on](#on) | boolean | |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [follow](#follow) | what, when: | |
| [pulse](#pulse) | milliseconds | |
| [toggle](#toggle) | | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_changed](#on_changed) | on | |

## Property descriptions

### address

**address** · number · read-only

Its accessory decoder's output, 1 to 2044.

---

### base

**base** · dcc_base · read-only

The base station it is switched through.

---

### following

**following** · object · read-only

The sensor, turnout, or block it follows; `null` when it follows nothing.

---

### inverted

**inverted** · boolean · read-only

`true` when on is sent as the decoder's thrown output.

---

### name

**name** · text · read-only

Its name.

---

### on

**on** · boolean

`true` while it is on. Setting it switches it, and ends a [pulse](#pulse) under way. A light following something is switched again at that thing's next change.

## Function descriptions

### follow

**follow**(what, when: ...)

Switches it on while a sensor, a turnout, or a block is in one state, and off otherwise: at once, and at each change. `follow(null)` stops following.

| Part | Description |
|---|---|
| what | Required. A [sensor](ref:sensor), a [turnout](ref:turnout), a [block](ref:block), or `null`. |
| when | Optional. The state that turns it on: `"active"` (the default) or `"inactive"` for a sensor, `"thrown"` (the default) or `"closed"` for a turnout, `"occupied"` (the default) or `"free"` for a block. |

---

### pulse

**pulse**(milliseconds)

Switches it on, then off again after that long: an uncoupler, a crossing bell's lamp, a lamp lit while a button is pressed.

| Part | Description |
|---|---|
| milliseconds | Required. How long it stays on: 1 to 600000. |

---

### toggle

**toggle**()

Switches it the other way.

## Event descriptions

### on_changed

**on_changed**(on)

It was switched: `on` is `true` or `false`.

## See also

[turnout](ref:turnout), [sensor](ref:sensor), [block](ref:block), [layout](ref:layout)
