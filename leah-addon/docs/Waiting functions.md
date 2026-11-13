# Waiting functions

## Description

Nearly every train script is "wait, then act": wait until the train reaches the siding's detector, then slow it; wait until the turnout has moved, then go. These functions wait until a [sensor](ref:sensor), a [turnout](ref:turnout), a [block](ref:block), or a [light](ref:light) is in a state. They come with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

They wait as `sleep()` does, with Leah's events running while they wait: the board's pins are heard, a sensor's delay passes, a slow turnout finishes its move. So they are for a script without windows. In a window's app, waiting would freeze the window, and they are a `gui_error` there: use the objects' events instead (a sensor's [on_active](ref:sensor#on_active), a block's [on_occupied](ref:block#on_occupied)).

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$siding = sensor(22)
$loco.speed = 40
$siding.simulate("active")
if wait_for($siding, "active", timeout: 30000) {
    $loco.speed = 10
}
print($loco.speed)
```

Output:

```
10
```

The states each waits for:

| Thing | States |
|---|---|
| sensor | `"active"`, `"inactive"` (or `true`, `false`) |
| turnout | `"closed"`, `"thrown"`, `"moving"`, `"inconsistent"` |
| block | `"occupied"`, `"free"` (or `true`, `false`) |
| light | `"on"`, `"off"` (or `true`, `false`) |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [wait_for](#wait_for) | thing, state, timeout: | boolean |
| [wait_for_any](#wait_for_any) | pairs, timeout: | number |

## Function descriptions

### wait_for

**wait_for**(thing, state, timeout: null) · boolean

Waits until the thing is in that state: `true` then (at once, when it is already), or `false` when the timeout passed first.

| Part | Description |
|---|---|
| thing | Required. A [sensor](ref:sensor), a [turnout](ref:turnout), a [block](ref:block), or a [light](ref:light). |
| state | Required. The state waited for, from the table above. |
| timeout | Optional. The longest wait, in milliseconds: 0 to 86400000. `null` (the default) waits for as long as it takes. |

---

### wait_for_any

**wait_for_any**(pairs, timeout: null) · number

Waits until the first of several things is in its state: its place in the list (from 0), or `null` when the timeout passed first.

```leah check
$which = wait_for_any([[$east_detector, "active"], [$west_detector, "active"]], timeout: 60000)
```

| Part | Description |
|---|---|
| pairs | Required. A list of pairs, each a thing and the state waited for. |
| timeout | Optional. The longest wait, in milliseconds: 0 to 86400000. `null` (the default) waits for as long as it takes. |

## See also

[sensor](ref:sensor), [turnout](ref:turnout), [block](ref:block), [light](ref:light)
