# consist

## Description

Several locomotives run as one train, such as a double-header or a lash-up of diesels: one speed and one direction for them all, each member facing either way. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

A consist is one of two kinds:

- **Software** (made without an address): Leah sends each member its own packets: the consist's speed, made to fit each member's own [speed steps](ref:locomotive#speed_steps) and [speed limit](ref:locomotive#speed_limit), and its direction, the other way for a member that runs reversed. Any locomotives can be in one, and on Leah's base station there is no limit to how many.
- **Advanced** (made with `address:`, 1 to 127): each member's decoder is told the consist address (its CV 19, by the NMRA's consist control), and from then on takes its speed and direction from that address only, ignoring those sent to its own address. Its functions still answer at its own address, and, at the consist address, those its CV 21 and CV 22 let through. The decoders keep the consist even after the power goes off, so the train runs from any throttle at the consist address, until [break_up](#break_up) takes them out.

A member's own speed is set to 0 as it joins and as it leaves: what is still repeating at its own address can't start it when it leaves an advanced consist.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$lead = locomotive(3)
$second = locomotive(5)
$train = consist(name: "Doubleheader")
$train.add($lead)
$train.add($second, reversed: true)
$train.speed = 40
print([$lead.speed, $lead.direction, $second.speed, $second.direction])
```

Output:

```
[40, "forward", 40, "reverse"]
```

## Making one

**consist**(name: "", address: ..., base: ...) · consist

| Part | Description |
|---|---|
| name | Optional. Its name, for the script's own use: `"Doubleheader"`. |
| address | Optional. The consist address of an advanced consist, 1 to 127 (a decoder keeps it in seven bits of CV 19): anything else is a `range_error`. Without it, a software consist. |
| base | Optional. The [dcc_base](ref:dcc_base) it runs on; by default the one made last. With none made, it is a `hardware_error`. |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [address](#address) | number | ✓ |
| [base](#base) | dcc_base | ✓ |
| [direction](#direction) | text | |
| [kind](#kind) | text | ✓ |
| [members](#members) | array | ✓ |
| [name](#name) | text | ✓ |
| [speed](#speed) | number | |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [add](#add) | locomotive, reversed: boolean | |
| [break_up](#break_up) | | |
| [emergency_stop](#emergency_stop) | | |
| [is_reversed](#is_reversed) | locomotive | boolean |
| [remove](#remove) | locomotive | |
| [set_function](#set_function) | number, on as boolean, to: value | |
| [stop](#stop) | | |

## Property descriptions

### address

**address** · number · read-only

An advanced consist's address, or `null` for a software one.

---

### base

**base** · dcc_base · read-only

The base station it runs on.

---

### direction

**direction** · text

`"forward"` or `"reverse"`: the train's direction. A member added with `reversed: true` runs the other way. Setting it sends it, with the speed.

---

### kind

**kind** · text · read-only

`"software"` or `"advanced"`.

---

### members

**members** · array · read-only

Its locomotives, the lead first, in the order added.

---

### name

**name** · text · read-only

The name it was given.

---

### speed

**speed** · number

The train's speed, 0 to 126: each member of a software consist gets it made to fit its own speed steps (a 28-step member at 63 runs at 14) and kept within its own speed limit. An advanced consist sends it to the consist address in 128 steps. Outside 0 to 126 is a `range_error`.

## Function descriptions

### add

**add**(locomotive, reversed: false)

Adds a [locomotive](ref:locomotive) on the same base station, at the end (the first added is the lead). `reversed: true` for one that runs the other way, back to back with the one before, say. In an advanced consist, its decoder is told the consist address, sent twice. A locomotive in the consist already, or one on another base station, is an error, and so is one at an advanced consist's own address.

---

### break_up

**break_up**()

Stops the train and takes every member out: in an advanced consist, each decoder is told it is in no consist (its CV 19 back to 0), so it answers to its own address again. Do this before putting the locomotives away: a decoder left in a consist "won't respond" at its own address later (a roster's [entries](ref:roster#entries) with `in_consist: true` finds such ones, from their kept settings, and [identify_decoder](ref:dcc_base#identify_decoder) warns of one on the programming track).

---

### emergency_stop

**emergency_stop**()

Stops the train at once, ignoring the decoders' deceleration: the consist address and every member's own.

---

### is_reversed

**is_reversed**(locomotive) · boolean

Whether a member runs reversed. A locomotive that isn't a member is a `range_error`.

---

### remove

**remove**(locomotive)

Takes one locomotive out, stopped: in an advanced consist, its decoder is told it is in no consist. One that isn't a member is a `range_error`.

---

### set_function

**set_function**(number, on, to: "lead")

Turns a function on or off: on the lead locomotive (by default: its headlight, its horn), on every member (`to: "all"`), on one member (`to: $loco`), or, in an advanced consist, at the consist address (`to: "consist"`), where each member answers as its CV 21 (F1 to F8) and CV 22 (the headlight, F9 to F12) let it.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$a = locomotive(3)
$b = locomotive(5)
$train = consist()
$train.add($a)
$train.add($b, reversed: true)
$train.set_function(0, true)
$train.set_function(1, true, to: "all")
print([$a.function(0), $b.function(0), $a.function(1), $b.function(1)])
```

Output:

```
[true, false, true, true]
```

---

### stop

**stop**()

Stops the train, eased by each decoder's deceleration.

## See also

[locomotive](ref:locomotive), [dcc_base](ref:dcc_base), [roster](ref:roster), [Model railroading](ref:Model railroading)
