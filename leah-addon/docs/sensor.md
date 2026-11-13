# sensor

## Description

An input from the layout: a block detector, a fascia pushbutton, a track contact, wired to one of the base station board's pins. Leah's base station program watches the pin and says each change, so the sensor's events come while the script waits. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

A sensor is **active when its pin is low**: block detectors and pushbuttons pull the pin to ground, against the board's own pull-up, which holds it high (inactive) otherwise. One that is high when active is made with `inverted: true`. A sensor made without a pin, `sensor(name: "Reset")`, is an internal one: nothing but [simulate](#simulate) changes it, for a panel's button or a flag other programs see.

```leah
$base = dcc_base(simulate: true)
$siding = sensor(22, name: "East siding", inactive_delay: 1500)
print([$siding.name, $siding.state])
$siding.simulate("active")
print([$siding.state, $siding.active])
```

Output:

```
["East siding", "inactive"]
["active", true]
```

**Wiring:** the detector's output to the pin, and its ground to one of the board's **GND** pins. The pins free for sensors on an Arduino Mega are **2, 4 to 7, 10, 14 to 53, and A2, A3, A6 to A15**: the others are the base station program's own (the track's signal, power, brakes, current, and faults). A pin carries 5 V at most. Pins 22 to 53, on the Mega's long double row at its end, are the easiest to reach with the motor shield on.

A board running Leah's base station program from before sensors drives the track as before; a sensor on it is a `hardware_error` asking for the new program, loaded with [load_dcc_program](ref:DCC board functions#load_dcc_program). On the simulated base station, a sensor's pin starts inactive and changes only by [simulate](#simulate).

**Delays:** [active_delay](#active_delay) and [inactive_delay](#inactive_delay) hold a change back until it has lasted that long. A train's wheels lose the rails for a moment on dirty track, so a block detector's sensor usually has an `inactive_delay` of a second or two: the block stays occupied through the flicker. A change that goes back within its delay is never said.

## Making one

**sensor**(pin, name: "S" and the pin, inverted: false, pull: true, active_delay: 0, inactive_delay: 0, base: ...) · sensor

| Part | Description |
|---|---|
| pin | Optional. The base station board's pin: 2 to 69, or `"A0"` to `"A15"` (54 to 69). Without it, the sensor is an internal one, and `name:` is needed. A pin the board hasn't, or one its program uses itself, is a `range_error`. |
| name | Optional. Its name: `"East siding"` (`"S22"` for pin 22 when not given). |
| inverted | Optional. `true` for one that is high when active. |
| pull | Optional. `false` to leave the board's internal pull-up off, for a detector that drives the pin both ways itself. Only with a pin. |
| active_delay | Optional. Milliseconds, 0 to 60000: how long a change to active must last before it is said. |
| inactive_delay | Optional. Milliseconds, 0 to 60000: how long a change to inactive must last before it is said. |
| base | Optional. The [dcc_base](ref:dcc_base) whose board its pin is on; by default the one made last. With a pin and no base station made, it is a `hardware_error`. |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [active](#active) | boolean | ✓ |
| [active_delay](#active_delay) | number | |
| [base](#base) | dcc_base | ✓ |
| [inactive_delay](#inactive_delay) | number | |
| [inverted](#inverted) | boolean | ✓ |
| [name](#name) | text | ✓ |
| [pin](#pin) | number | ✓ |
| [pull](#pull) | boolean | ✓ |
| [state](#state) | text | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [simulate](#simulate) | state | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_active](#on_active) | | |
| [on_changed](#on_changed) | state | |
| [on_inactive](#on_inactive) | | |

## Property descriptions

### active

**active** · boolean · read-only

`true` while it is active.

---

### active_delay

**active_delay** · number

How long, in milliseconds, a change to active must last before it is said: 0 to 60000.

---

### base

**base** · dcc_base · read-only

The base station whose board its pin is on; `null` for an internal one.

---

### inactive_delay

**inactive_delay** · number

How long, in milliseconds, a change to inactive must last before it is said: 0 to 60000.

---

### inverted

**inverted** · boolean · read-only

`true` when it is active while its pin is high.

---

### name

**name** · text · read-only

Its name.

---

### pin

**pin** · number · read-only

The board's pin it is wired to (54 to 69 for A0 to A15); `null` for an internal one.

---

### pull

**pull** · boolean · read-only

Whether the board's internal pull-up is on for its pin.

---

### state

**state** · text · read-only

`"active"` or `"inactive"`; `"unknown"` until the board has said its pin's level, a moment after the sensor is made.

## Function descriptions

### simulate

**simulate**(state)

Changes it as its pin would: `"active"` or `"inactive"` (or `true` or `false`), after its delay. For trying a script without the layout, and for an internal sensor. On a board, the pin's next change is taken as usual.

| Part | Description |
|---|---|
| state | Required. `"active"` or `"inactive"`, or `true` or `false`. |

## Event descriptions

### on_active

**on_active**()

It became active (after its `active_delay`).

---

### on_changed

**on_changed**(state)

It changed: `state` is `"active"` or `"inactive"`.

---

### on_inactive

**on_inactive**()

It became inactive (after its `inactive_delay`).

## See also

[turnout](ref:turnout), [dcc_base](ref:dcc_base), [Model railroading](ref:Model railroading)
