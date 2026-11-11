# decoder_definition

## Description

A decoder's settings by name: for each one, the CVs and bits it is kept in, what kind of value it is (a number, one of a few choices, true or false), its range, its group, and when it applies. A [decoder_programmer](ref:decoder_programmer) uses one to read and write a decoder by these names, and a script can use one on its own to turn CVs' values into settings and back. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

Every definition starts from the **NMRA's standard settings**, the ones every decoder has: its addresses, the motor's start, middle, and top voltages, acceleration and deceleration, its configuration (CV 29) by name, its consist, the speed table, and the decoder lock. A maker's definition adds that maker's own settings over them.

```leah
$d = decoder_definition("soundtraxx_tsunami2_diesel")
print([$d.maker, count($d.settings), $d.groups[0]])
print($d.setting("Coupler")["choices"])
print([$d.value_of("Coupler", [126: 0x81]), $d.value_of("Train brake rate", [118: 0x85])])
print($d.cvs_of("Speed steps", "14", [29: 6]))
```

Output:

```
[141, 115, "Address"]
["medium", "heavy", "link-and-pin"]
["heavy", -5]
[29: 4]
```

## Making one

**decoder_definition**(name) · decoder_definition

**decoder_definition**(settings: ..., name: ..., maker: null, versions: [], standard: true) · decoder_definition

With a name, one of the built-in definitions ([decoder_definitions](ref:Decoder functions#decoder_definitions) lists them):

| Name | For |
|---|---|
| `"nmra"` | Any decoder: the NMRA's standard settings |
| `"soundtraxx_tsunami2_diesel"` | SoundTraxx Tsunami2 diesel sound decoders: the standard settings, and SoundTraxx's volumes for each sound, its choices of horn, bell, engine, and the others, its sound options, brakes, lights, and back-EMF |

A name that isn't one of them is a `range_error`. [decoder_definition_for](ref:Decoder functions#decoder_definition_for) picks the right one for a decoder [identify_decoder](ref:dcc_base#identify_decoder) read.

With `settings:`, a definition of the script's own: `settings:` is a list of settings, each an array of the parts below, and `name:` names it. `maker:` (CV 8's value) and `versions:` (a list of CV 7's values) say which decoders it is for. The standard settings come first, unless `standard: false`; a setting of the same name as a standard one takes its place. [read_decoder_definition_file](ref:Decoder functions#read_decoder_definition_file) reads the same parts from a JSON file.

| Part | Description |
|---|---|
| `name` | Required. The setting's name, as the programmer and the script call it. |
| `cv` | Required. Its CV (1 to 1024), or a list of CVs for a value spread over several, the first the most significant byte. |
| `bit` | One bit (0 to 7) of the CV: a setting that is `true` or `false`, unless it has `choices`. |
| `bits` | A run of bits, `[first, last]`: `[0, 5]`. Without `bit` or `bits`, the whole CV. |
| `kind` | `"number"` (the usual), `"choice"`, `"flag"` (one bit, `true` or `false`), `"signed"` (bit 7 the sign, the others the amount: -127 to 127), or `"long_address"` (two CVs as CV 17 and 18 keep a long address: 1 to 10239). |
| `min`, `max` | A number's range, within what its bits hold. |
| `choices` | The values it takes, each with its words: `[0: "medium", 1: "heavy", 2: "link-and-pin"]`. The words are its value. |
| `group` | The group it is shown in: `"Sound"`. Settings without one are in `"Settings"`. |
| `help` | A sentence saying what it does. |
| `default` | Its value as the decoder comes from the maker. |
| `read_only` | `true` for a setting that can only be read (CV 7 and 8 are). |
| `write_all` | `false` to leave it out of [write_all](ref:decoder_programmer#write_all) (the decoder lock's settings are). |
| `shown_when` | When it applies: another setting's name and value, `["Speed curve", "speed table"]`, or with a comparison, `["Volume", ">", 0]` (`==`, `!=`, `<`, `<=`, `>`, `>=`). |
| `index` | `[CV 31, CV 32]` for a setting kept in an indexed page (CV 257 to 512). One CV number is on one page only. |

A mistake in a definition is a `type_error` saying which setting and what: an unknown part, two settings named the same, two settings using the same bit of a CV, a `shown_when` naming no setting, a choice that doesn't fit its bits.

```leah check
$mine = decoder_definition(name: "my sound decoder", maker: 141, versions: [82], settings: [_
    ["name": "Master volume", "cv": 128, "group": "Sound", "help": "The volume of every sound."], _
    ["name": "Bell type", "cv": 115, "bits": [0, 2], "choices": [0: "brass", 1: "steel", 2: "electronic"], "group": "Sound"]])
```

## Properties

| Name | Type | Read-only |
|---|---|---|
| [cvs](#cvs) | array | ✓ |
| [groups](#groups) | array | ✓ |
| [maker](#maker) | number | ✓ |
| [name](#name) | text | ✓ |
| [settings](#settings) | array | ✓ |
| [versions](#versions) | array | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [cvs_of](#cvs_of) | name as text, value, cvs as array | array |
| [is_shown](#is_shown) | name as text, values as array | boolean |
| [setting](#setting) | name as text | array |
| [value_of](#value_of) | name as text, cvs as array | any value |

## Property descriptions

### cvs

**cvs** · array

Every CV its settings use, in order.

---

### groups

**groups** · array

Its groups' names, in the order their first settings come: `["Address", "Motor", "Identity", ...]`.

---

### maker

**maker** · number

The maker's ID (CV 8) it is for, or `null` for any maker.

---

### name

**name** · text

Its name: a built-in one's (`"nmra"`), or the one given.

---

### settings

**settings** · array

Its settings, in order, each an array of its parts as `settings:` takes them (with `kind`, `group`, and the range always given).

---

### versions

**versions** · array

The CV 7 values it is for: an empty list for any.

## Function descriptions

### cvs_of

**cvs_of**(name, value, cvs: []) · array

The CVs' new values for a setting given a value, as an array of each CV's value by its number: what to write to set it. For a setting that is part of a CV (a bit, or a few), `cvs` gives the CV's value now, so its other bits stay as they are; without it, that is a `range_error` saying to read the CV first. A value the setting doesn't take is a `range_error` saying what it takes.

```leah
$d = decoder_definition("nmra")
print($d.cvs_of("Long address", 730))
print($d.cvs_of("Address used", "long", [29: 6]))
```

Output:

```
[17: 194, 18: 218]
[29: 38]
```

---

### is_shown

**is_shown**(name, values) · boolean

Whether a setting applies, given the other settings' values by name (a [decoder_programmer](ref:decoder_programmer#values)'s `values`): `false` for one whose `shown_when` isn't met, such as the speed table's steps while the decoder uses its three-point curve. A setting whose condition's setting isn't known yet counts as applying.

```leah
$d = decoder_definition("nmra")
print($d.is_shown("Speed step 5", ["Speed curve": "three points"]))
```

Output:

```
false
```

---

### setting

**setting**(name) · array

One setting's parts, by its name (upper and lower case alike). A name the definition doesn't have is a `range_error`.

---

### value_of

**value_of**(name, cvs) · any value

A setting's value from its CVs' values (an array of each CV's value by its number): a number, the choice's words, or `true` or `false`. `null` when one of its CVs isn't in `cvs`.

```leah
$d = decoder_definition("nmra")
print([$d.value_of("Speed steps", [29: 6]), $d.value_of("Analog running", [29: 6]), $d.value_of("Long address", [17: 194, 18: 218])])
```

Output:

```
["28/128", true, 730]
```

## See also

[decoder_programmer](ref:decoder_programmer), [Decoder functions](ref:Decoder functions), [dcc_base](ref:dcc_base), [Model railroading](ref:Model railroading)
