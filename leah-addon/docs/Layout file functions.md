# Layout file functions

## Description

A [layout](ref:layout) saved to a file and read back: the turnouts, sensors, lights, and blocks it holds, each with its settings, and which sensors each reads and what each light follows, by name. Their states aren't saved: a turnout read back is where its feedback says, or unknown. They come with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

The file is JSON, readable and easy to compare in version control: `"format"` (1), then `"sensors"`, `"turnouts"`, `"blocks"`, and `"lights"`, each a list of objects whose keys are the settings their makers take (a turnout's `"address"`, `"name"`, `"inverted"`, `"sensor"` or `"sensors"` by name, `"move_time"`; a light's `"follow"` by name and `"when"`). One written by hand in the same form is read the same way.

```leah
$base = dcc_base(simulate: true)
$layout = layout()
$layout.add(sensor(22, name: "Siding detector", inactive_delay: 1500), turnout(12, name: "Yard lead"))
write_layout_file($layout, "/tmp/leah_my_layout.json")
$again = read_layout_file("/tmp/leah_my_layout.json")
print([$again.names, $again.find("Siding detector").inactive_delay])
```

Output:

```
[["Siding detector", "Yard lead"], 1500]
```

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [read_layout_file](#read_layout_file) | path, base: | layout |
| [write_layout_file](#write_layout_file) | layout, path | |

## Function descriptions

### read_layout_file

**read_layout_file**(path, base: ...) · layout

A layout made from a file: each object made as its maker makes it, so a setting it doesn't take says so as the maker would, with the file's list and place ("sensors 2"). A file of a newer format is a `type_error` naming it.

| Part | Description |
|---|---|
| path | Required. The file. |
| base | Optional. The [dcc_base](ref:dcc_base) its turnouts, lights, and sensors' pins are on; by default the one made last. |

---

### write_layout_file

**write_layout_file**(layout, path)

Writes the layout to a file, replacing it safely (the old file stays until the new one is whole). An object reading one that isn't in the layout (a turnout's sensor, a light's block) is a `range_error`: add it too.

| Part | Description |
|---|---|
| layout | Required. The [layout](ref:layout). |
| path | Required. The file. |

## See also

[layout](ref:layout)
