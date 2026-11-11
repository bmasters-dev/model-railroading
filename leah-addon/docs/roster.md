# roster

## Description

A record of each of your locomotives, kept in a file: its names, its address, its decoder, every one of its decoder's settings, what each function key does, its groups, a picture, notes, fields of your own, and the fastest it may run. Each record is a [roster_entry](ref:roster_entry). [locomotive](ref:locomotive) takes an entry in place of an address, and a [decoder_programmer](ref:decoder_programmer)'s settings go into an entry and back. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

The roster is read when it is made, and changes are kept only when the script calls [save](#save). The file is JSON, made on the first save.

```leah
$r = roster(join_path(make_temp_path(), "roster.json"))
$up = $r.add("UP 792", road_name: "Union Pacific", road_number: "792", address: 792, groups: ["Diesel"])
$up.set_function(2, "Horn", momentary: true)
$r.save()
print([$r.names, $up.long, $up.functions[2]["label"]])
```

Output:

```
[["UP 792"], true, "Horn"]
```

## Making one

**roster**(path) · roster

`path` is the roster's file. Without it, the roster every app shares: `roster.json` in a `model_railroading` folder beside the apps' own data folders (where Leah keeps every app's data: in the app's folder, or the portable folder when one is set). A file that isn't a roster is a `type_error`; one that can't be read is a `file_error`.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [changed](#changed) | boolean | ✓ |
| [count](#count) | number | ✓ |
| [groups](#groups) | array | ✓ |
| [names](#names) | array | ✓ |
| [path](#path) | text | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [add](#add) | name as text, parts by name | roster_entry |
| [entries](#entries) | group: text, address: number | array |
| [entry](#entry) | name as text | roster_entry |
| [export_entries](#export_entries) | path as text, names: array | number |
| [import_entries](#import_entries) | path as text | array |
| [remove](#remove) | name as text | |
| [save](#save) | | |

## Property descriptions

### changed

**changed** · boolean

`true` when something changed since the roster was read or saved.

---

### count

**count** · number

How many entries it has.

---

### groups

**groups** · array

Every group its entries are in, in alphabetical order.

---

### names

**names** · array

Its entries' names, in the order they were added.

---

### path

**path** · text

Its file.

## Function descriptions

### add

**add**(name, parts…) · roster_entry

Adds an entry and gives it back. `name` names it in the roster (upper and lower case alike: a name the roster has already is a `range_error`). Any of the entry's parts can be given by name: `road_name:`, `road_number:`, `address:` (3 by default), `long:`, `decoder:`, `model:`, `notes:`, `picture:`, `groups:`, `cvs:`, `speed_limit:`, `attributes:`, as [roster_entry](ref:roster_entry) has them; the function keys' labels are set with the entry's [set_function](ref:roster_entry#set_function).

---

### entries

**entries**(group: ..., address: ...) · array

Its entries, as a list of [roster_entry](ref:roster_entry): all of them, those in a group, or those at an address (what [read_address](ref:dcc_base#read_address) found, say).

```leah check
$r = roster()
foreach $r.entries(group: "Steam") as $e {
    print($e.name + ": " + $e.address)
}
```

---

### entry

**entry**(name) · roster_entry

The entry with that name (upper and lower case alike), or `null` when there is none.

---

### export_entries

**export_entries**(path, names: ...) · number

Writes entries to a file of their own, as a roster file keeps them, to take to another computer or share: every entry, or those in `names:`. Gives back how many it wrote. A name the roster doesn't have is a `range_error`.

---

### import_entries

**import_entries**(path) · array

Adds the entries in a file (one [export_entries](#export_entries) wrote, or another roster's), and gives back their names. One whose name the roster has already gets a number after it: `"UP 792 (2)"`.

---

### remove

**remove**(name)

Takes an entry out of the roster. A name it doesn't have is a `range_error`. The entry's object, if the script still holds it, can no longer be read or changed.

---

### save

**save**()

Writes the roster to its file (made, with its folder, when it isn't there), replacing it safely: a failure leaves the old file as it was.

## See also

[roster_entry](ref:roster_entry), [locomotive](ref:locomotive), [decoder_programmer](ref:decoder_programmer), [Model railroading](ref:Model railroading)
