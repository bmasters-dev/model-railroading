# block

## Description

A stretch of track whose occupancy comes from a [sensor](ref:sensor), a block detector, and the train in it. Panels color track by it, lights follow it, and signals and routes will read it. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

The train in a block is the script's to say: a [locomotive](ref:locomotive), a [consist](ref:consist), a name, or a number. When the block clears, its train is given to [on_cleared](#on_cleared), and the block holds none. A block made without a sensor is occupied and freed by the script.

```leah
$base = dcc_base(simulate: true)
$detector = sensor(22, inactive_delay: 1500)
$siding = block("East siding", sensor: $detector)
$detector.simulate("active")
$siding.train = locomotive(3)
print([$siding.state, $siding.train.address])
```

Output:

```
["occupied", 3]
```

## Making one

**block**(name, sensor: null) · block

| Part | Description |
|---|---|
| name | Required. Its name: `"East siding"`. |
| sensor | Optional. The [sensor](ref:sensor) its occupancy comes from. Without it, the block starts free, and the script sets [occupied](#occupied). |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [name](#name) | text | ✓ |
| [occupied](#occupied) | boolean | |
| [sensor](#sensor) | sensor | ✓ |
| [state](#state) | text | ✓ |
| [train](#train) | any | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_changed](#on_changed) | state | |
| [on_cleared](#on_cleared) | train | |
| [on_occupied](#on_occupied) | | |

## Property descriptions

### name

**name** · text · read-only

Its name.

---

### occupied

**occupied** · boolean

`true` while a train is in it. Set by the script only for a block without a sensor; for one with a sensor, setting it is a `type_error`.

---

### sensor

**sensor** · sensor · read-only

The sensor its occupancy comes from; `null` for none.

---

### state

**state** · text · read-only

`"occupied"` or `"free"`; `"unknown"` until its sensor has said.

---

### train

**train** · any

The train in it: a [locomotive](ref:locomotive), a [consist](ref:consist), a name, or a number; `null` for none. Anything else is a `type_error`. It becomes `null` when the block clears.

## Event descriptions

### on_changed

**on_changed**(state)

It changed: `state` is `"occupied"` or `"free"`.

---

### on_cleared

**on_cleared**(train)

The block became free: `train` is the train it held (`null` for none), which it no longer holds.

---

### on_occupied

**on_occupied**()

A train came into it.

## See also

[sensor](ref:sensor), [light](ref:light), [layout](ref:layout)
