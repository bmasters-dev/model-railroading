# turnout

## Description

A turnout (a track switch, a set of points) moved by its accessory decoder: Leah's base station sends the NMRA's accessory packet to the decoder's output, which drives the turnout's motor, a twin-coil machine or a slow-motion one. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

A turnout is known by its **address**, the decoder output it is wired to, 1 to 2044, numbered as throttles and command stations number them: a decoder with four turnouts, such as the first one set to address 1, has the turnouts 1 to 4. **Closed** is the straight route and **thrown** the diverging one; a turnout wired the other way round is made with `inverted: true`.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$yard = turnout(12, name: "Yard lead")
$yard.throw()
print([$yard.state, $base.accessory(12)])
$yard.close()
print([$yard.state, $base.accessory(12)])
```

Output:

```
["thrown", "thrown"]
["closed", "closed"]
```

Each move is sent with the output switched on, twice, then switched off: a decoder for twin-coil machines times its own pulse, and the switching off lets go of one that doesn't. Only the track power carries the packets: with it off, a turnout doesn't move.

A turnout given to a [phone_throttle_server](ref:phone_throttle_server) with [add_turnout](ref:phone_throttle_server#add_turnout) is moved by the phones too.

**Feedback:** without it, a turnout lies where it was last told ([commanded](#commanded)). With feedback, [state](#state) says where it is known to lie, which a slow machine reaches later, and one that sticks never does:

- **One sensor** (`sensor:`), active when the turnout is thrown: a contact on the machine, or a detector on the diverging route.
- **Two sensors** (`sensors:`), one at each end of the throw: `"moving"` while neither is active, `"inconsistent"` if both are.
- **Only a move time** (`move_time:`), for a slow-motion machine or a servo without contacts: `"moving"` until it has passed.

A move its sensors don't confirm within its [move_time](#move_time) (3 seconds unless given) is said by [on_mismatch](#on_mismatch): a machine that stuck, or a broken wire. A turnout moved by hand is followed too: its sensors change its state.

```leah
$base = dcc_base(simulate: true)
$points = sensor(24)
$yard = turnout(12, name: "Yard lead", sensor: $points)
$yard.throw()
print([$yard.commanded, $yard.state])
$points.simulate("active")
print($yard.state)
$yard.locked = true
print($yard.feedback)
```

Output:

```
["thrown", "closed"]
thrown
one_sensor
```

A **locked** turnout ([locked](#locked)) refuses to move, from the script and from the phones: a route holding it under a train.

## Making one

**turnout**(address, name: "T" and the address, inverted: false, sensor: null, sensors: null, move_time: 0, base: ...) · turnout

| Part | Description |
|---|---|
| address | Required. Its accessory decoder's output, 1 to 2044. Anything else is a `range_error`. |
| name | Optional. Its name, as phones list it: `"Yard lead"` (`"T12"` for address 12 when not given). |
| inverted | Optional. `true` for a turnout wired the other way round: closed sent as the decoder's thrown output. |
| sensor | Optional. A [sensor](ref:sensor) active when it is thrown. |
| sensors | Optional. Two sensors: the one active when it is closed, then the one active when it is thrown. Not with `sensor:`. |
| move_time | Optional. Milliseconds, 0 to 60000: how long it takes to move. Without sensors, it is `"moving"` for that long after each move; with them, a move they don't confirm in that time is said by [on_mismatch](#on_mismatch) (3000 when not given). |
| base | Optional. The [dcc_base](ref:dcc_base) it is moved through; by default the one made last. With none made, it is a `hardware_error`. |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [address](#address) | number | ✓ |
| [base](#base) | dcc_base | ✓ |
| [commanded](#commanded) | text | ✓ |
| [feedback](#feedback) | text | ✓ |
| [inverted](#inverted) | boolean | ✓ |
| [locked](#locked) | boolean | |
| [move_time](#move_time) | number | ✓ |
| [name](#name) | text | ✓ |
| [sensors](#sensors) | array | ✓ |
| [state](#state) | text | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [close](#close) | | |
| [throw](#throw) | | |
| [toggle](#toggle) | | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_changed](#on_changed) | state | |
| [on_mismatch](#on_mismatch) | commanded, state | |

## Property descriptions

### address

**address** · number · read-only

Its accessory decoder's output, 1 to 2044.

---

### base

**base** · dcc_base · read-only

The base station it is moved through.

---

### commanded

**commanded** · text · read-only

What it was last told: `"closed"` or `"thrown"`; `"unknown"` until it is.

---

### feedback

**feedback** · text · read-only

How it knows where it lies: `"none"`, `"one_sensor"`, `"two_sensors"`, or `"delay"` (only a move time).

---

### inverted

**inverted** · boolean · read-only

`true` when it is wired the other way round.

---

### locked

**locked** · boolean

`true` to keep it where it is: [close](#close), [throw](#throw), and [toggle](#toggle) are then a `hardware_error`, and a phone's request is refused with a message on the phone. It reads `true` also while a [route](ref:route) holds it for a train (each route holding it keeps it locked until the last lets go); setting it is the script's own lock, apart from the routes'.

---

### move_time

**move_time** · number · read-only

How long it takes to move, in milliseconds (0: at once).

---

### name

**name** · text · read-only

Its name.

---

### sensors

**sensors** · array · read-only

Its feedback sensors: none, one (active when thrown), or two (closed's, then thrown's).

---

### state

**state** · text · read-only

Where it lies: `"closed"` or `"thrown"`; `"moving"` while a move takes its move time, or while neither of two sensors is active; `"inconsistent"` while both are; `"unknown"` until it is moved, or its sensors say. Without feedback, where it was last told.

## Function descriptions

### close

**close**()

Moves it to the straight route.

---

### throw

**throw**()

Moves it to the diverging route.

---

### toggle

**toggle**()

Moves it the other way: thrown when it is closed, else closed.

## Event descriptions

### on_changed

**on_changed**(state)

Its [state](#state) changed: moved by the script or a phone, or, with sensors, by hand. `state` is `"closed"`, `"thrown"`, `"moving"`, or `"inconsistent"`.

---

### on_mismatch

**on_mismatch**(commanded, state)

Its sensors didn't agree with a move within its [move_time](#move_time): `commanded` is where it was told to go, and `state` where they say it lies.

## See also

[sensor](ref:sensor), [dcc_base](ref:dcc_base), [phone_throttle_server](ref:phone_throttle_server), [Model railroading](ref:Model railroading)
