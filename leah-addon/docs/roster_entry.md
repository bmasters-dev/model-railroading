# roster_entry

## Description

One locomotive's record in a [roster](ref:roster): its names, its address, its decoder, every one of its decoder's settings, what each function key does, its groups, a picture, notes, fields of your own, and the fastest it may run. It is made by the roster's [add](ref:roster#add), and found with its [entry](ref:roster#entry) and [entries](ref:roster#entries). Changing one changes the roster, which keeps it at its next [save](ref:roster#save). It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

A decoder's settings go into an entry from a [decoder_programmer](ref:decoder_programmer), and back:

```leah check
$base = dcc_base("/dev/ttyACM0")
$r = roster()
$p = decoder_programmer(decoder_definition("nmra"), $base)
$p.on_done = function($what, $error) {
    global $p, $r
    $e = $r.entry("UP 792") ?? $r.add("UP 792", address: 792)
    $e.decoder = $p.definition.name
    $e.cvs = $p.cvs                 ` keep them
    $r.save()
}
$p.read_all()
` later, into another decoder: $p.load_cvs($r.entry("UP 792").cvs), then $p.write_all()
```

## Properties

| Name | Type | Read-only |
|---|---|---|
| [address](#address) | number | |
| [attributes](#attributes) | array | |
| [cvs](#cvs) | array | |
| [decoder](#decoder) | text | |
| [functions](#functions) | array | ✓ |
| [groups](#groups) | array | |
| [long](#long) | boolean | |
| [model](#model) | text | |
| [name](#name) | text | |
| [notes](#notes) | text | |
| [picture](#picture) | text | |
| [road_name](#road_name) | text | |
| [road_number](#road_number) | text | |
| [speed_limit](#speed_limit) | number | |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [attribute](#attribute) | name as text | any value |
| [set_attribute](#set_attribute) | name as text, value | |
| [set_function](#set_function) | number, label as text, momentary: boolean | |

## Property descriptions

### address

**address** · number

Its decoder's address, 1 to 10239 (3 by default). 128 and over makes [long](#long) `true`.

---

### attributes

**attributes** · array

Fields of your own, as values by name: `["Era": "1950s", "Bought": "2019-06-02"]`. Set it whole, or one at a time with [set_attribute](#set_attribute).

---

### cvs

**cvs** · array

Every decoder setting kept for it, each CV's value by its number: what a [decoder_programmer](ref:decoder_programmer#cvs) read, to load into a decoder again with its [load_cvs](ref:decoder_programmer#load_cvs).

---

### decoder

**decoder** · text

The name of its decoder's [decoder_definition](ref:decoder_definition): `"soundtraxx_tsunami2_diesel"`, or `""` when not known.

---

### functions

**functions** · array

What its function keys do, by number: each an array of `label` (`"Horn"`) and `momentary` (`true` for a key that is on only while pressed, such as a horn). Only the keys given a label or made momentary are in it. Set with [set_function](#set_function).

---

### groups

**groups** · array

The groups it is in, a list of names: `["Steam", "Club layout"]`.

---

### long

**long** · boolean

`true` when its decoder answers to its long address. `false` with an address of 128 or more is a `range_error`.

---

### model

**model** · text

What model it is, in your words: `"Athearn Genesis SD70ACe"`.

---

### name

**name** · text

Its name in the roster, unique there (upper and lower case alike: another entry's name is a `range_error`).

---

### notes

**notes** · text

Anything you want to write about it.

---

### picture

**picture** · text

The path of its picture.

---

### road_name

**road_name** · text

Its railroad's name: `"Union Pacific"`.

---

### road_number

**road_number** · text

Its number on the railroad, as text: `"792"`, `"4014-B"`.

---

### speed_limit

**speed_limit** · number

The fastest speed step it may be given, 1 to 126 (126, the top, by default): for a locomotive that runs too fast for its scale.

## Function descriptions

### attribute

**attribute**(name) · any value

One of [attributes](#attributes), by name (upper and lower case alike), or `null`.

---

### set_attribute

**set_attribute**(name, value)

Sets one of [attributes](#attributes); `null` takes it away.

---

### set_function

**set_function**(number, label, momentary: false)

Says what a function key (F0 to F68) does: its label, and with `momentary: true`, that it is on only while pressed. An empty label, latching, makes the key plain again.

```leah check
$e.set_function(0, "Headlight")
$e.set_function(1, "Bell")
$e.set_function(2, "Horn", momentary: true)
```

## See also

[roster](ref:roster), [locomotive](ref:locomotive), [decoder_programmer](ref:decoder_programmer), [Model railroading](ref:Model railroading)
