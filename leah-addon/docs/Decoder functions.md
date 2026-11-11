# Decoder functions

## Description

Functions for a decoder's settings that need no base station: the maker of a CV 8 value, a decoder's configuration (CV 29) by name, both ways, and its definitions (its settings by name: [decoder_definition](ref:decoder_definition)). They come with the **model_railroading** addon ([Model railroading](ref:Model railroading)). A base station's [identify_decoder](ref:dcc_base#identify_decoder) uses them on what it reads.

| Function | What it does |
|---|---|
| [decoder_maker_name](#decoder_maker_name) | The maker's name for a CV 8 value |
| [cv29_parts](#cv29_parts) | CV 29's bits by name |
| [make_cv29](#make_cv29) | CV 29's value from its parts by name |
| [decoder_definitions](#decoder_definitions) | The built-in decoder definitions |
| [decoder_definition_for](#decoder_definition_for) | The built-in definition for a decoder identify_decoder read |
| [read_decoder_definition_file](#read_decoder_definition_file) | A decoder definition kept in a JSON file |

```leah
print(decoder_maker_name(151))
$c = cv29_parts(38)
print([$c["long_address"], $c["speed_steps"], $c["analog"], $c["reversed"]])
print(make_cv29(long_address: true))
print(make_cv29(reversed: true, speed_steps: 14, analog: false))
```

Output:

```
Electronic Solutions Ulm GmbH
[true, 28, true, false]
38
1
```

## Function descriptions

### decoder_maker_name

**decoder_maker_name**(id) · text

The maker's name for a CV 8 value (0 to 255), as the NMRA's list of manufacturer IDs gives it: `"Throttle-Up (Soundtraxx)"` for 141, `"Electronic Solutions Ulm GmbH"` (ESU) for 151, `"Train Control Systems"` for 153, `"Digitrax"` for 129. `null` for a number the NMRA hasn't given out. A number outside 0 to 255 is a `range_error`.

---

### cv29_parts

**cv29_parts**(value) · array

The NMRA's meaning of each bit of CV 29, a locomotive decoder's configuration (0 to 255), as an array:

| Key | Bit | Description |
|---|---|---|
| `reversed` | 0 | `true` when forward and reverse are swapped |
| `speed_steps` | 1 | `28` (which also takes 128 speed steps) or `14` |
| `analog` | 2 | `true` when it also runs on a plain DC layout |
| `railcom` | 3 | `true` when it answers by RailCom |
| `speed_table` | 4 | `true` when it uses its own speed table (CV 67 to 94) rather than CV 2, 5, and 6 |
| `long_address` | 5 | `true` when it answers to its long address (CV 17 and 18) rather than CV 1 |
| `accessory` | 7 | `true` for an accessory decoder (a turnout's), whose CV 29 means other things |

---

### make_cv29

**make_cv29**(reversed: false, speed_steps: 28, analog: true, railcom: false, speed_table: false, long_address: false) · number

CV 29's value from its parts, the other way from [cv29_parts](#cv29_parts). `speed_steps:` is 14, 28, or 128 (28 and 128 are the same bit); anything else is a `range_error`. With nothing given, it is 6: 28 or 128 speed steps, running on DC too, as most decoders come.

```leah check
$base = dcc_base("/dev/ttyACM0")
$base.write_cv(29, make_cv29(long_address: true, analog: false))
```

---

### decoder_definitions

**decoder_definitions**() · array

The built-in decoder definitions, each an array of its `name` (as [decoder_definition](ref:decoder_definition) takes it) and `about` (what it is for).

```leah
foreach decoder_definitions() as $d {
    print($d["name"])
}
```

Output:

```
nmra
soundtraxx_tsunami2_diesel
```

---

### decoder_definition_for

**decoder_definition_for**(identity) · decoder_definition

The built-in [decoder_definition](ref:decoder_definition) for a decoder, from what [identify_decoder](ref:dcc_base#identify_decoder) read: its maker's, by the maker, the version, and the kind of sound, or the NMRA's standard one when Leah has none for that decoder yet. Anything other than what `identify_decoder` gives is a `type_error`.

```leah check
$base = dcc_base("/dev/ttyACM0")
$d = decoder_definition_for($base.identify_decoder())
print($d.name)
```

---

### read_decoder_definition_file

**read_decoder_definition_file**(path) · decoder_definition

A [decoder_definition](ref:decoder_definition) kept in a JSON file: an object with `name`, `settings` (a list of settings, each an object of the parts [decoder_definition](ref:decoder_definition#making-one) takes), and, when they are needed, `maker`, `versions`, and `standard`. A file that isn't JSON, or holds something else, is a `type_error`, and so is a mistake in a setting, as for `decoder_definition`; a file that can't be read is a `file_error`.

```json
{"name": "my sound decoder", "maker": 141, "versions": [82],
 "settings": [{"name": "Master volume", "cv": 128, "group": "Sound"}]}
```

```leah check
$d = read_decoder_definition_file("my_decoder.json")
print(count($d.settings))
```

## See also

[decoder_definition](ref:decoder_definition), [decoder_programmer](ref:decoder_programmer), [dcc_base](ref:dcc_base), [locomotive](ref:locomotive), [Model railroading](ref:Model railroading)
