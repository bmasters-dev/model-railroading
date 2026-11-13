# speedometer

## Description

A train's speed, measured between two [sensors](ref:sensor) a known distance apart: the time from one becoming active to the other, either way. It gives the model's speed in centimeters a second, and the speed a real train would be going at full size. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

Any two detectors work: two blocks' detectors (the distance between them is the first block's length), or a pair of infrared gates on the board's pins a measured distance apart on a straight. A sensor's [active_delay](ref:sensor#active_delay) adds to the time, so the sensors measured with have none.

```leah check
$meter = speedometer(sensor(22), sensor(23), distance: 50, scale: "HO")
$meter.on_measured = function($cm, $kmh, $mph) {
    print("This train runs at a scale " + to_text($mph) + " mph")
}
```

## Making one

**speedometer**(first, second, distance:, scale: "HO") · speedometer

| Part | Description |
|---|---|
| first | Required. One [sensor](ref:sensor). |
| second | Required. Another. |
| distance | Required. The centimeters between them: more than 0. |
| scale | Optional. The model's scale, for the full-size speed: see [scale_speed](ref:Speed functions#scale_speed). |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [distance](#distance) | number | ✓ |
| [last_scale_kmh](#last_scale_kmh) | number | ✓ |
| [last_scale_mph](#last_scale_mph) | number | ✓ |
| [last_speed](#last_speed) | number | ✓ |
| [scale](#scale) | number | ✓ |
| [sensors](#sensors) | array | ✓ |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_measured](#on_measured) | cm_per_second, scale_kmh, scale_mph | |

## Property descriptions

### distance

**distance** · number · read-only

The centimeters between its two sensors.

---

### last_scale_kmh

**last_scale_kmh** · number · read-only

The last speed measured, as kilometers an hour at full size; `null` before the first.

---

### last_scale_mph

**last_scale_mph** · number · read-only

The last speed measured, as miles an hour at full size; `null` before the first.

---

### last_speed

**last_speed** · number · read-only

The last speed measured, in centimeters a second on the model; `null` before the first.

---

### scale

**scale** · number · read-only

Its scale's ratio: 87.1 for HO.

---

### sensors

**sensors** · array · read-only

Its two sensors.

## Event descriptions

### on_measured

**on_measured**(cm_per_second, scale_kmh, scale_mph)

A train passed both sensors: its speed on the model, in centimeters a second, and at full size, in kilometers and miles an hour (each to a tenth).

## See also

[Speed functions](ref:Speed functions), [sensor](ref:sensor)
