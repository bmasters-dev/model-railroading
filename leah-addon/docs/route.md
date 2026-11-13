# route

## Description

Many turnouts set at once: a yard ladder lined to one track, the main line cleared, the layout's start position. A route found by a [track_plan](ref:track_plan) also holds its blocks, and can keep them for one train. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

[set](#set) moves the turnouts one after another, [delay](#delay) apart, so the power supply and the decoders cope: the first at once, the rest while the script goes on. The route is [aligned](#aligned) once every turnout lies its way, by its feedback when it has some ([turnout](ref:turnout)).

```leah
$base = dcc_base(simulate: true)
$base.power = true
$ladder = route("Track 5", [[turnout(1), "thrown"], [turnout(2), "closed"]], delay: 0)
$ladder.set()
print($ladder.aligned)
```

Output:

```
true
```

**Holding a path:** [reserve](#reserve) keeps a found path's blocks for one train, and locks its turnouts, so no other route or phone moves them under it. It is refused while one of the blocks is occupied, held by another route, or one of its turnouts is locked the other way. Each block is let go as the train clears it (occupied, then free); with the last one, the turnouts are unlocked, and [on_released](#on_released) says so. [release](#release) lets go of everything at once.

A route given to a [phone_throttle_server](ref:phone_throttle_server) with [add_route](ref:phone_throttle_server#add_route) is set from the phones too, and shows there as active while it is aligned.

## Making one

**route**(name, settings, delay: 250) · route

| Part | Description |
|---|---|
| name | Required. Its name: `"Track 5"`. |
| settings | Required. A list of pairs, each a [turnout](ref:turnout) and the way it lies in the route: `"closed"`, `"thrown"`, or `"toggle"` (the other way from where it lies). A turnout twice is a `type_error`. |
| delay | Optional. Milliseconds between two turnouts' moves: 0 to 10000. |

A route found by [find_path](ref:track_plan#find_path) is named for its ends: `"West main to Yard"`.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [aligned](#aligned) | boolean | ✓ |
| [blocks](#blocks) | array | ✓ |
| [busy](#busy) | boolean | ✓ |
| [delay](#delay) | number | |
| [name](#name) | text | ✓ |
| [reserved](#reserved) | boolean | ✓ |
| [settings](#settings) | array | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [release](#release) | | |
| [reserve](#reserve) | | boolean |
| [set](#set) | | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_aligned](#on_aligned) | | |
| [on_released](#on_released) | | |
| [on_set](#on_set) | | |

## Property descriptions

### aligned

**aligned** · boolean · read-only

`true` while every turnout lies the route's way.

---

### blocks

**blocks** · array · read-only

A found path's blocks, in the order a train passes them; an empty list for a route made by name.

---

### busy

**busy** · boolean · read-only

`true` while [set](#set) is still moving its turnouts.

---

### delay

**delay** · number

Milliseconds between two turnouts' moves: 0 to 10000.

---

### name

**name** · text · read-only

Its name.

---

### reserved

**reserved** · boolean · read-only

`true` while it holds any of its blocks.

---

### settings

**settings** · array · read-only

Its turnouts and their ways, as pairs: `[[$t1, "closed"], [$t2, "thrown"]]`.

## Function descriptions

### release

**release**()

Lets go of every block it still holds, and unlocks its turnouts.

---

### reserve

**reserve**() · boolean

Holds its blocks for one train and locks its turnouts: `true`, or `false` with a warning saying why (a block occupied, or held by another route; a turnout locked the other way; a route with no blocks).

---

### set

**set**()

Moves its turnouts its way, one after another, [delay](#delay) apart: the first at once, the rest while the script goes on. One already told its way isn't sent again. A turnout locked by another route, or by the script, is a `hardware_error`, and none moves; a route moves its own turnouts while it holds its blocks.

## Event descriptions

### on_aligned

**on_aligned**()

Every turnout lies the route's way now.

---

### on_released

**on_released**()

It let go of its last block (the train cleared it, or [release](#release)), and its turnouts are unlocked.

---

### on_set

**on_set**()

[set](#set) has sent its last turnout's move. With feedback, the turnouts may still be moving: [on_aligned](#on_aligned) says when they lie its way.

## See also

[track_plan](ref:track_plan), [turnout](ref:turnout), [block](ref:block), [phone_throttle_server](ref:phone_throttle_server)
