# layout_simulator

## Description

Trains that move through a [track_plan](ref:track_plan)'s blocks with no layout, so panels, routes, and train scripts are tried on the simulated base station before any track is wired. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

Each train is a [locomotive](ref:locomotive) the script (or a phone) drives as usual. Ten times a second, the simulator moves it by its speed, [top_speed](#top_speed) centimeters a second at full speed, and its direction:

- Into the block its turnouts lead to, never back through the points it came through (a train can't reverse at the points).
- Stopping at the end of the track, or at points set against it ([on_train_stopped](#on_train_stopped)). It goes on by itself once they are moved its way.
- The other way when its direction changes: the train turns round, its last car leading.

Every block a train covers is occupied: through the block's sensor, as [simulate](ref:sensor#simulate) changes it (its delays too), or, for a block without one, as its [occupied](ref:block#occupied) would say. So a route held for the train lets go of each block as the train clears it.

```leah
$base = dcc_base(simulate: true)
$west = block("West main")
$east = block("East main")
$plan = track_plan()
$plan.connect($west, $east)
$sim = layout_simulator($plan, top_speed: 400)
$loco = locomotive(3)
$sim.add_train($loco, $west, toward: $east)
$sim.on_train_stopped = function($l, $block) {
    global $sim
    print("stopped in " + $block.name)
    $sim.stop()
}
$sim.start()
$loco.speed_percent = 100
```

Output:

```
stopped in East main
```

The example app `layout_panel.leah` has a simulated train in its passing siding on the simulated base station.

## Making one

**layout_simulator**(plan, top_speed: 40, block_length: 100) · layout_simulator

It starts stopped: [start](#start) sets its trains moving.

| Part | Description |
|---|---|
| plan | Required. The [track_plan](ref:track_plan) its trains run on. |
| top_speed | Optional. Centimeters a second at full speed: more than 0, up to 1000 (40 is a brisk HO train). |
| block_length | Optional. Every block's length in centimeters, unless [set_length](#set_length) says otherwise: more than 0, up to 100000. |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [block_length](#block_length) | number | |
| [plan](#plan) | track_plan | ✓ |
| [running](#running) | boolean | ✓ |
| [top_speed](#top_speed) | number | |
| [trains](#trains) | array | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [add_train](#add_train) | locomotive, block, toward:, length: | |
| [length](#length) | block | number |
| [remove_train](#remove_train) | locomotive | boolean |
| [set_length](#set_length) | block, centimeters | |
| [start](#start) | | |
| [stop](#stop) | | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_train_entered](#on_train_entered) | locomotive, block | |
| [on_train_stopped](#on_train_stopped) | locomotive, block | |

## Property descriptions

### block_length

**block_length** · number

Every block's length in centimeters, but those [set_length](#set_length) gave.

---

### plan

**plan** · track_plan · read-only

The track plan its trains run on.

---

### running

**running** · boolean · read-only

`true` while its trains move ([start](#start) to [stop](#stop)). While it runs, a script without windows goes on running.

---

### top_speed

**top_speed** · number

Centimeters a second at full speed.

---

### trains

**trains** · array · read-only

Its trains, each a map: `locomotive`, `block` (where its front is), `blocks` (every block it covers), `position` (its front's centimeters into its block), and `stopped`.

## Function descriptions

### add_train

**add_train**(locomotive, block, toward: null, length: 30)

A train in a block: its locomotive, which faces the block `toward:` names (one joined to it), and its length in centimeters. Without `toward:`, it runs forward into the first way open. A locomotive that is a train already is a `range_error`.

---

### length

**length**(block) · number

A block's length in centimeters.

---

### remove_train

**remove_train**(locomotive) · boolean

Takes a train off the track: its blocks are free again. Gives `true` when there was one.

---

### set_length

**set_length**(block, centimeters)

A block's length: more than 0, up to 100000.

---

### start

**start**()

Sets its trains moving.

---

### stop

**stop**()

Stops moving its trains (their locomotives keep their speeds).

## Event descriptions

### on_train_entered

**on_train_entered**(locomotive, block)

A train's front came into a block.

---

### on_train_stopped

**on_train_stopped**(locomotive, block)

A train stopped by itself in a block: the track ends there, or the points ahead are set against it.

## See also

[track_plan](ref:track_plan), [block](ref:block), [locomotive](ref:locomotive), [track_diagram](ref:track_diagram)
