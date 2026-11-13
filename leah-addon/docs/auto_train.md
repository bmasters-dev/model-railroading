# auto_train

## Description

A train run automatically along a path: a [locomotive](ref:locomotive) and a [route](ref:route) a [track_plan](ref:track_plan) found. It holds the path, lines its turnouts, departs, slows for the end, and stops there by itself, reading where the train is from the blocks' detectors. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

[start](#start) holds the path for the train ([reserve](ref:route#reserve)), or says [on_blocked](#on_blocked) when it can't. Then it lines the path's turnouts, and, once they lie its way, departs at [max_speed](#max_speed). Entering the block before the last, it slows to [approach_speed](#approach_speed); reaching the last, it stops ([on_arrived](#on_arrived)). Each block is let go as the train clears it, so another train can have it. A script without windows goes on running while the train runs.

```leah
$base = dcc_base(simulate: true)
$west = block("West main")
$middle = block("Middle")
$east = block("East main")
$plan = track_plan()
$plan.connect($west, $middle)
$plan.connect($middle, $east)
$sim = layout_simulator($plan, top_speed: 400)
$loco = locomotive(3)
$sim.add_train($loco, $west, toward: $middle)
$sim.start()
$run = auto_train($loco, $plan.find_path($west, $east), max_speed: 80)
$run.on_arrived = function($block) {
    global $sim
    print("arrived in " + $block.name)
    $sim.stop()
}
$run.start()
```

Output:

```
arrived in East main
```

It stops as the train's front reaches the last block's detector: give that block room for the train to stop in (a decoder's momentum lets it roll on a little).

## Making one

**auto_train**(locomotive, route, max_speed: 60, approach_speed: 20, direction: "forward") · auto_train

| Part | Description |
|---|---|
| locomotive | Required. The [locomotive](ref:locomotive) it runs. |
| route | Required. The path, a [route](ref:route) [find_path](ref:track_plan#find_path) gave: the train starts in its first block. One without blocks is a `type_error`. |
| max_speed | Optional. Its running speed, a percent of full speed: more than 0, up to 100. |
| approach_speed | Optional. Its speed in the block before the last, a percent of full speed. |
| direction | Optional. The way the locomotive runs: `"forward"` or `"reverse"`. |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [approach_speed](#approach_speed) | number | |
| [block](#block) | block | ✓ |
| [direction](#direction) | text | ✓ |
| [locomotive](#locomotive) | locomotive | ✓ |
| [max_speed](#max_speed) | number | |
| [route](#route) | route | ✓ |
| [state](#state) | text | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [emergency_stop](#emergency_stop) | | |
| [halt](#halt) | | |
| [resume](#resume) | | |
| [start](#start) | wait: | boolean |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_arrived](#on_arrived) | block | |
| [on_block_entered](#on_block_entered) | block | |
| [on_blocked](#on_blocked) | | |
| [on_departed](#on_departed) | | |

## Property descriptions

### approach_speed

**approach_speed** · number

Its speed in the block before the last, a percent of full speed.

---

### block

**block** · block · read-only

The furthest block of its path the train has reached.

---

### direction

**direction** · text · read-only

The way the locomotive runs: `"forward"` or `"reverse"`.

---

### locomotive

**locomotive** · locomotive · read-only

The locomotive it runs.

---

### max_speed

**max_speed** · number

Its running speed, a percent of full speed; changed while it runs, the train takes it at once.

---

### route

**route** · route · read-only

Its path.

---

### state

**state** · text · read-only

`"waiting"` (not started), `"lining"` (its turnouts moving), `"running"`, `"approaching"` (in the block before the last), `"halted"`, `"arrived"`, or `"blocked"` (its path couldn't be held).

## Function descriptions

### emergency_stop

**emergency_stop**()

Stops the locomotive at once, as its decoder's emergency stop does; [resume](#resume) goes on.

---

### halt

**halt**()

Stops the locomotive, eased by its decoder's momentum; [resume](#resume) goes on.

---

### resume

**resume**()

Goes on after [halt](#halt) or [emergency_stop](#emergency_stop), at the speed for where the train is.

---

### start

**start**(wait: false) · boolean

Holds the path, lines it, and departs once it is lined: `true`, or `false` (and [on_blocked](#on_blocked)) when the path can't be held. With `wait: true`, a train whose path is held waits for it: it tries again whenever one of the path's blocks or turnouts changes, and departs as soon as the path is free (another train cleared it). Starting it again is done after it arrived or was blocked.

| Part | Description |
|---|---|
| wait | Optional. `true` to wait for a held path instead of giving up. |

## Event descriptions

### on_arrived

**on_arrived**(block)

The train reached its path's last block, and stopped.

---

### on_block_entered

**on_block_entered**(block)

The train came into the next block of its path.

---

### on_blocked

**on_blocked**()

Its path couldn't be held: a block on it is occupied or held by another route, or a turnout is locked the other way. A warning says which (but for a train told to wait, which tries again by itself).

---

### on_departed

**on_departed**()

Its turnouts lie the path's way, and it set off.

## See also

[track_plan](ref:track_plan), [route](ref:route), [layout_simulator](ref:layout_simulator), [locomotive](ref:locomotive)
