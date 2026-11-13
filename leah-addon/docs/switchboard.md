# switchboard

## Description

A window's control with a button for each turnout, light, route, and sensor, in rows: each says its name and state, and is colored by it. A click throws the turnout the other way, switches the light, or sets the route; a sensor's button only says it was clicked. It's the bench and club tool when no [track_diagram](ref:track_diagram) is drawn. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

| Color | When |
|---|---|
| Green | Thrown, on, set (aligned), or active |
| Slate | Closed, off, not set, or inactive |
| Light gray | Not known yet (a turnout never moved, a sensor not yet heard) |

```leah check
$win = window(title: "Switchboard", width: 460, height: 160)
$board = switchboard(left: 10, top: 10, width: 440, height: 140, columns: 4)
$win.add($board)
$board.add_layout($layout)
$board.on_clicked = function($thing) {
    print($thing.name)
}
$win.show()
```

The example app `switchboard.leah` is a switchboard for an accessory decoder's eight outputs.

## Making one

**switchboard**(left:, top:, width:, height:, ...) · switchboard

Placed in a window as any control is, with its properties as settings.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [columns](#columns) | number | |
| [count](#count) | number | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [add](#add) | things... | |
| [add_layout](#add_layout) | layout | |
| [clear](#clear) | | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_clicked](#on_clicked) | thing | |

## Property descriptions

### columns

**columns** · number

How many buttons a row has: 1 to 50 (4 unless set).

---

### count

**count** · number · read-only

How many buttons it has.

## Function descriptions

### add

**add**(things...)

A button for each [turnout](ref:turnout), [light](ref:light), [route](ref:route), or [sensor](ref:sensor), after those it has.

---

### add_layout

**add_layout**(layout)

A button for each of a [layout](ref:layout)'s turnouts, then its lights, then its sensors (blocks have none).

---

### clear

**clear**()

Takes every button away.

## Event descriptions

### on_clicked

**on_clicked**(thing)

A button was clicked: the turnout, light, route, or sensor it is for (the turnout thrown, the light switched, the route set already). A locked turnout isn't moved, and a warning says so.

## See also

[track_diagram](ref:track_diagram), [layout](ref:layout)
