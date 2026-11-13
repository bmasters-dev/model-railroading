# Speed functions

## Description

A model's speed as a real train's: the speed it would be going at full size. They come with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

The scales, by name or by ratio:

| Scale | Ratio |
|---|---|
| `"HO"` | 1:87.1 |
| `"N"` | 1:160 |
| `"O"` | 1:48 |
| `"S"` | 1:64 |
| `"TT"` | 1:120 |
| `"Z"` | 1:220 |
| `"OO"` | 1:76.2 |
| `"G"` | 1:32 (gauge 1) |
| `"large"` | 1:22.5 |

```leah
print(scale_speed(30, scale: "N"))
```

Output:

```
["kmh": 172.8, "mph": 107.4]
```

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [scale_speed](#scale_speed) | cm_per_second, scale: | array |

## Function descriptions

### scale_speed

**scale_speed**(cm_per_second, scale: "HO") · array

A model's speed, in centimeters a second, as kilometers and miles an hour at full size: `["kmh": 94.1, "mph": 58.5]` (each to a tenth).

| Part | Description |
|---|---|
| cm_per_second | Required. The model's speed: 0 or more. |
| scale | Optional. A scale from the table above, or its ratio as a number (87.1 for 1:87.1). Another name is a `range_error`. |

## See also

[speedometer](ref:speedometer)
