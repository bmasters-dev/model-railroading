# signal_head

## Description

A signal head of colored lamps, each a [light](ref:light) (an accessory decoder's output), showing one aspect: `"red"`, `"yellow"`, `"green"`, or `"dark"`. It can be set by the script, or be automatic, as automatic block signals (ABS) work. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

**Automatic:** [protect](#protect) gives it the blocks ahead of it, in order, and the turnouts ahead with the way each must lie. It shows:

| Aspect | When |
|---|---|
| Red | The first block ahead is occupied (or its detector hasn't said yet), or a turnout ahead isn't lying its way |
| Yellow | The next block is occupied |
| Green | Otherwise |

[held](#held) keeps it red whatever it protects, as a dispatcher holds a train. A head without a lamp shows the next more restrictive aspect it has: a two-lamp head (red and green) shows red for yellow.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$main = block("Main", sensor: sensor(22))
$east = block("East main", sensor: sensor(23))
$head = signal_head(name: "West home", red: light(30), yellow: light(31), green: light(32))
$head.protect([$main, $east])
print($head.aspect)
$east.sensor.simulate("active")
print($head.aspect)
```

Output:

```
green
yellow
```

Signal decoders that take aspect numbers, and masts showing a railroad's named aspects, come later.

## Making one

**signal_head**(name: "Signal", red:, yellow: null, green: null) · signal_head

It starts dark.

| Part | Description |
|---|---|
| name | Optional. Its name: `"West home"`. |
| red | Required. The [light](ref:light) of its red lamp. |
| yellow | Optional. Its yellow lamp's light. |
| green | Optional. Its green lamp's light. |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [aspect](#aspect) | text | |
| [automatic](#automatic) | boolean | ✓ |
| [held](#held) | boolean | |
| [lamps](#lamps) | array | ✓ |
| [name](#name) | text | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [protect](#protect) | blocks, turnouts: | |
| [stop_protecting](#stop_protecting) | | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_changed](#on_changed) | aspect | |

## Property descriptions

### aspect

**aspect** · text

What it shows: `"red"`, `"yellow"`, `"green"`, or `"dark"`. Setting it lights that lamp and puts the others out; an automatic head's is a `type_error` to set.

---

### automatic

**automatic** · boolean · read-only

`true` while it [protects](#protect) blocks.

---

### held

**held** · boolean

`true` to keep it red; `false` lets it show its aspect again.

---

### lamps

**lamps** · array · read-only

Its red, yellow, and green lamps' lights (`null` for one it hasn't).

---

### name

**name** · text · read-only

Its name.

## Function descriptions

### protect

**protect**(blocks, turnouts: null)

Makes it automatic: its aspect comes from the blocks ahead of it and the turnouts' ways, at once and at every change.

| Part | Description |
|---|---|
| blocks | Required. The blocks ahead, in order: the first the one it guards, the second the one after. |
| turnouts | Optional. Pairs of a turnout and the way it must lie for the signal to clear: `[[$points, "closed"]]`. |

---

### stop_protecting

**stop_protecting**()

Makes it set by the script again, showing what it showed.

## Event descriptions

### on_changed

**on_changed**(aspect)

It shows another aspect: `"red"`, `"yellow"`, `"green"`, or `"dark"`.

## See also

[light](ref:light), [block](ref:block), [track_diagram](ref:track_diagram)
