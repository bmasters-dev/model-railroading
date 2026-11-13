# layout_button

## Description

A window's control showing one turnout, light, route, or sensor as a button: its name and state, colored by it, as a [switchboard](ref:switchboard)'s buttons are. A click throws the turnout, switches the light, or sets the route. For a panel laid out by hand, or in a window designer, one control at a time: a turnout's lever, a sensor's lamp, a route's button. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

```leah check
$lever = layout_button(left: 10, top: 10, width: 110, height: 48, object: $yard_lead)
$win.add($lever)
$lever.on_clicked = function($turnout) {
    print($turnout.state)
}
```

## Making one

**layout_button**(left:, top:, width:, height:, object: ...) · layout_button

Placed in a window as any control is, with its properties as settings.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [object](#object) | object | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_clicked](#on_clicked) | thing | |

## Property descriptions

### object

**object** · object

The [turnout](ref:turnout), [light](ref:light), [route](ref:route), or [sensor](ref:sensor) it shows; `null` for none. Anything else is a `type_error`.

## Event descriptions

### on_clicked

**on_clicked**(thing)

It was clicked: the object it shows (a turnout thrown, a light switched, a route set already).

## See also

[switchboard](ref:switchboard), [track_diagram](ref:track_diagram)
