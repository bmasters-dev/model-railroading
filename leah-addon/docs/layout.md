# layout

## Description

The turnouts, sensors, lights, and blocks of one railroad, kept by name: panels, routes, and other scripts find them by the names people use. A layout is saved to a file and read back with [write_layout_file](ref:Layout file functions#write_layout_file) and [read_layout_file](ref:Layout file functions#read_layout_file). It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

Each name is used once in a layout, whatever its kind. A name not found gives `null`, with a warning naming the nearest one.

```leah
$base = dcc_base(simulate: true)
$layout = layout()
$points = sensor(23, name: "Yard points")
$layout.add($points, turnout(12, name: "Yard lead", sensor: $points), block("East siding"))
print($layout.names)
print($layout.find("Yard lead").feedback)
```

Output:

```
["Yard points", "Yard lead", "East siding"]
one_sensor
```

## Making one

**layout**() · layout

An empty layout.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [blocks](#blocks) | array | ✓ |
| [count](#count) | number | ✓ |
| [lights](#lights) | array | ✓ |
| [names](#names) | array | ✓ |
| [sensors](#sensors) | array | ✓ |
| [turnouts](#turnouts) | array | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [add](#add) | objects... | |
| [find](#find) | name | object |
| [remove](#remove) | name | boolean |

## Property descriptions

### blocks

**blocks** · array · read-only

Its blocks, in the order they were added.

---

### count

**count** · number · read-only

How many objects it holds.

---

### lights

**lights** · array · read-only

Its lights, in the order they were added.

---

### names

**names** · array · read-only

Every object's name, in the order they were added.

---

### sensors

**sensors** · array · read-only

Its sensors, in the order they were added.

---

### turnouts

**turnouts** · array · read-only

Its turnouts, in the order they were added.

## Function descriptions

### add

**add**(objects...)

Adds turnouts, sensors, lights, and blocks. One already in it is passed over; another of the same name is a `range_error`.

| Part | Description |
|---|---|
| objects | Required. One or more turnouts, sensors, lights, and blocks. |

---

### find

**find**(name) · object

The object of that name, or `null` with a warning when there is none.

| Part | Description |
|---|---|
| name | Required. Its name. |

---

### remove

**remove**(name) · boolean

Takes the object of that name out of the layout (the object itself goes on). Gives `true` when there was one.

| Part | Description |
|---|---|
| name | Required. Its name. |

## See also

[Layout file functions](ref:Layout file functions), [turnout](ref:turnout), [sensor](ref:sensor), [light](ref:light), [block](ref:block)
