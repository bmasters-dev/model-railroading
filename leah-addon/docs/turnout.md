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

## Making one

**turnout**(address, name: "T" and the address, inverted: false, base: ...) · turnout

| Part | Description |
|---|---|
| address | Required. Its accessory decoder's output, 1 to 2044. Anything else is a `range_error`. |
| name | Optional. Its name, as phones list it: `"Yard lead"` (`"T12"` for address 12 when not given). |
| inverted | Optional. `true` for a turnout wired the other way round: closed sent as the decoder's thrown output. |
| base | Optional. The [dcc_base](ref:dcc_base) it is moved through; by default the one made last. With none made, it is a `hardware_error`. |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [address](#address) | number | ✓ |
| [base](#base) | dcc_base | ✓ |
| [inverted](#inverted) | boolean | ✓ |
| [name](#name) | text | ✓ |
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

## Property descriptions

### address

**address** · number · read-only

Its accessory decoder's output, 1 to 2044.

---

### base

**base** · dcc_base · read-only

The base station it is moved through.

---

### inverted

**inverted** · boolean · read-only

`true` when it is wired the other way round.

---

### name

**name** · text · read-only

Its name.

---

### state

**state** · text · read-only

`"closed"` or `"thrown"`, as it was last moved; `"unknown"` until it is (a turnout can't be asked which way it lies).

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

It moved, by the script or a phone: `state` is `"closed"` or `"thrown"`.

## See also

[dcc_base](ref:dcc_base), [phone_throttle_server](ref:phone_throttle_server), [Model railroading](ref:Model railroading)
