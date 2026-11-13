# track_plan

## Description

Which block joins which, and through which turnout lying which way, written in code with no window. It finds paths: the cheapest way from one block to another, around what it must, given back as a [route](ref:route) ready to set and hold for a train. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

**How track is written:** a block joined plainly to two others lies between them. Where the track splits, a turnout joins the blocks: the points' block joined to one track through the turnout lying closed, and to the other through it lying thrown. A path never goes through one turnout twice, since a train can't reverse at the points, in at one leg and out at the other.

```leah
$base = dcc_base(simulate: true)
$west = block("West main")
$points = block("Points")
$main = block("Main")
$siding = block("Siding")
$t1 = turnout(1)
$plan = track_plan()
$plan.connect($west, $points)
$plan.connect($points, $main, turnout: $t1, position: "closed")
$plan.connect($points, $siding, turnout: $t1, position: "thrown")
$path = $plan.find_path($west, $siding)
print($path.name)
print([count($path.blocks), $path.settings[0][1]])
print($plan.find_path($siding, $main))
```

Output:

```
West main to Siding
[3, "thrown"]

```

Each block costs 1 to pass, unless [set_cost](#set_cost) says otherwise: a siding a path should keep out of costs more. A path goes around blocks that are occupied (but the first and the last), and those another route holds.

## Making one

**track_plan**() · track_plan

An empty plan.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [blocks](#blocks) | array | ✓ |
| [connections](#connections) | array | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [connect](#connect) | from, to, turnout:, position:, one_way: | |
| [cost](#cost) | block | number |
| [find_path](#find_path) | from, to, via:, avoid:, avoid_occupied:, max_blocks:, delay: | route |
| [neighbours](#neighbours) | block | array |
| [set_cost](#set_cost) | block, cost | |

## Property descriptions

### blocks

**blocks** · array · read-only

Its blocks, in the order they were first joined.

---

### connections

**connections** · array · read-only

Its connections, each a map: `from`, `to`, and `one_way`, and for one through a turnout, `turnout` and `position`.

## Function descriptions

### connect

**connect**(from, to, turnout: null, position: null, one_way: false)

Joins two blocks.

| Part | Description |
|---|---|
| from | Required. A [block](ref:block). |
| to | Required. Another block. |
| turnout | Optional. The [turnout](ref:turnout) between them. |
| position | Required with `turnout:`. The way it lies for this connection: `"closed"` or `"thrown"`. |
| one_way | Optional. `true` when trains pass only from `from` to `to`. |

---

### cost

**cost**(block) · number

What passing the block costs a path: 1 unless set.

---

### find_path

**find_path**(from, to, via: null, avoid: null, avoid_occupied: true, max_blocks: 64, delay: 250) · route

The cheapest path from one block to another, as a [route](ref:route): its blocks in order, and its turnouts' ways. `null` when there is none.

| Part | Description |
|---|---|
| from | Required. The block the train starts in. |
| to | Required. The block it goes to. |
| via | Optional. A block, or a list of them, the path passes, in that order. |
| avoid | Optional. A block, or a list of them, the path keeps out of. |
| avoid_occupied | Optional. `false` to pass occupied blocks too. |
| max_blocks | Optional. The most blocks the path may have: 1 to 1000. |
| delay | Optional. The route's [delay](ref:route#delay) between two turnouts' moves. |

---

### neighbours

**neighbours**(block) · array

The blocks joined to it, each once.

---

### set_cost

**set_cost**(block, cost)

What passing the block costs a path: 1 to 1000000.

## See also

[route](ref:route), [block](ref:block), [turnout](ref:turnout)
