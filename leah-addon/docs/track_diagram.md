# track_diagram

## Description

A window's control drawing the layout as a panel does: track colored by its block, each turnout's points showing which way they lie, sensors' and lights' lamps, and labels. A click throws a turnout or switches a light. With a [track_plan](ref:track_plan) given, clicking one block then another lines and holds the path between them (entry and exit). It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

The script places each piece on a grid, [grid](#grid) pixels a step (20 unless set): `[1, 5]` is one step from the left and five down. The diagram follows the objects it shows, and draws again as they change.

| Track colored | When its block is |
|---|---|
| Light gray | Free |
| Red | Occupied |
| Green | Held by a route for a train |
| Dark gray | Not known yet (its detector hasn't said) |

A turnout's points show bright on the leg it lies on, and dim on the other; both are dim until it is moved. A white dot on the points says it is locked.

```leah check
$win = window(title: "Panel", width: 520, height: 200)
$panel = track_diagram(left: 10, top: 10, width: 500, height: 180, plan: $plan)
$win.add($panel)
$panel.add_track($west, [[1, 4], [6, 4]])
$panel.add_turnout($points, [6, 4], closed_to: [8, 4], thrown_to: [8, 2], block: $west)
$panel.add_track($main, [[8, 4], [16, 4]])
$panel.add_track($siding, [[8, 2], [16, 2]])
$panel.on_route_set = function($route) {
    print("lined: " + $route.name)
}
$win.show()
```

**Entry and exit:** with [plan](#plan) set, the first block clicked becomes the entry (outlined in white), and the second one the exit. The path between them is found ([find_path](ref:track_plan#find_path)), held ([reserve](ref:route#reserve)), and set ([set](ref:route#set)). [on_route_set](#on_route_set) gives the route, or [on_route_refused](#on_route_refused) says there was no clear path. A block clicked twice, or [cancel_entry](#cancel_entry), lets go of the entry.

The example app `layout_panel.leah` is a passing siding drawn this way, its turnouts on accessory decoder outputs 1 and 2 and its detectors on the board's pins 22 to 25, with a train run along the path lined on the simulated base station.

## Making one

**track_diagram**(left:, top:, width:, height:, ...) · track_diagram

Placed in a window as any control is, with its properties as settings.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [background_color](#background_color) | text | |
| [entry](#entry) | block | ✓ |
| [grid](#grid) | number | |
| [plan](#plan) | track_plan | |
| [show_names](#show_names) | boolean | |
| [track_width](#track_width) | number | |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [add_label](#add_label) | text, at | |
| [add_light](#add_light) | light, at | |
| [add_sensor](#add_sensor) | sensor, at | |
| [add_track](#add_track) | block, points | |
| [add_turnout](#add_turnout) | turnout, at, closed_to:, thrown_to:, block: | |
| [cancel_entry](#cancel_entry) | | |
| [clear](#clear) | | |
| [pick_block](#pick_block) | block | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_block_clicked](#on_block_clicked) | block | |
| [on_light_clicked](#on_light_clicked) | light | |
| [on_route_refused](#on_route_refused) | from, to | |
| [on_route_set](#on_route_set) | route | |
| [on_sensor_clicked](#on_sensor_clicked) | sensor | |
| [on_turnout_clicked](#on_turnout_clicked) | turnout | |

## Property descriptions

### background_color

**background_color** · text

The panel's color, as hex text: `"#2B2F36"` (dark, as layout panels usually are) unless set; `null` for none (the window's own).

---

### entry

**entry** · block · read-only

The entry block picked, waiting for its exit; `null` when none is.

---

### grid

**grid** · number

The pixels of one grid step: 4 to 200.

---

### plan

**plan** · track_plan

The [track_plan](ref:track_plan) entry and exit finds paths in; `null` (the default) for none, when a block clicked only fires [on_block_clicked](#on_block_clicked).

---

### show_names

**show_names** · boolean

`true` (the default) to write each block's name above its track.

---

### track_width

**track_width** · number

The track's width in pixels: 1 to 40 (5 unless set).

## Function descriptions

### add_label

**add_label**(text, at)

Writes words on the panel, centered on a place: `[12, 8]`.

---

### add_light

**add_light**(light, at)

A [light](ref:light)'s lamp at a place: lit while it is on. A click switches it.

---

### add_sensor

**add_sensor**(sensor, at)

A [sensor](ref:sensor)'s lamp at a place: green while it is active. A click fires [on_sensor_clicked](#on_sensor_clicked).

---

### add_track

**add_track**(block, points)

Track through two places or more, `[[1, 5], [7, 5]]`, colored by its [block](ref:block)'s state; `null` for track in no block. A click picks the block for entry and exit, and fires [on_block_clicked](#on_block_clicked).

---

### add_turnout

**add_turnout**(turnout, at, closed_to:, thrown_to:, block: null)

A [turnout](ref:turnout)'s points at a place, its legs leading to two others: `closed_to:` the straight route's, `thrown_to:` the diverging one's. `block:` is the block its points are in, which colors the leg it lies on. A click throws it the other way.

---

### cancel_entry

**cancel_entry**()

Lets go of the entry block picked.

---

### clear

**clear**()

Takes every piece off the panel.

---

### pick_block

**pick_block**(block)

Picks a block for entry and exit, as a click on it does: from the keyboard, or another control.

## Event descriptions

### on_block_clicked

**on_block_clicked**(block)

A block's track was clicked.

---

### on_light_clicked

**on_light_clicked**(light)

A light was clicked, and switched.

---

### on_route_refused

**on_route_refused**(from, to)

Entry and exit found no clear path between the two blocks: they aren't joined, or the way is occupied, held, or locked.

---

### on_route_set

**on_route_set**(route)

Entry and exit lined and held a path: the [route](ref:route).

---

### on_sensor_clicked

**on_sensor_clicked**(sensor)

A sensor's lamp was clicked.

---

### on_turnout_clicked

**on_turnout_clicked**(turnout)

A turnout's points were clicked, and it was thrown the other way (unless it is locked, which a warning says).

## See also

[track_plan](ref:track_plan), [route](ref:route), [block](ref:block), [turnout](ref:turnout)
