# Model railroading addon: examples

Short examples of the model railroading addon, as raw material for its guide (the form of Leah's own
docs/guide/examples.md). Leah's `cargo test` runs every `leah` block and compares its output with the
**Output** block under it.

## A simulated base station

A base station with no board: each packet is read back from its signal by a simulated decoder.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$loco.speed = 40
$loco.direction = "reverse"
$d = $base.decoder(3)
print([$d["speed"], $d["direction"]])
print(last($base.packets)["bytes"])
```

Output:
```
[40, "reverse"]
03 3F 29 15
```

## A decoder's maker and its configuration

CV 8's maker by the NMRA's list, and CV 29's bits by name, both ways.

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

## A decoder definition

A decoder's settings by name: the NMRA's standard set built in, a maker's over it, and the bit work
both ways.

```leah
$d = decoder_definition("soundtraxx_tsunami2_diesel")
print([$d.maker, count($d.settings), $d.groups[0]])
print($d.setting("Coupler")["choices"])
print([$d.value_of("Coupler", [126: 0x81]), $d.value_of("Train brake rate", [118: 0x85])])
print($d.cvs_of("Speed steps", "14", [29: 6]))
print(decoder_definition_for(["maker_id": 141, "version": 82]).name)
```

Output:
```
[141, 115, "Address"]
["medium", "heavy", "link-and-pin"]
["heavy", -5]
[29: 4]
nmra
```

## A roster

Each locomotive's record, kept in a file; a locomotive made from its entry.

```leah
$r = roster(join_path(make_temp_path(), "roster.json"))
$up = $r.add("UP 792", road_name: "Union Pacific", address: 792, groups: ["Diesel"])
$up.set_function(2, "Horn", momentary: true)
$r.save()
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive($r.entry("up 792"))
$loco.speed = 30
print([$r.names, $loco.address, $loco.long, $base.decoder(792, long: true)["speed"]])
```

Output:
```
[["UP 792"], 792, true, 30]
```

## Speed steps, a speed limit, and momentary keys

A locomotive's own speed steps (28 here, in the NMRA's basic packets), its speed limit, and a horn on
only while its key is held.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(5, speed_steps: 28)
$loco.speed_limit = 20
$loco.speed = 28
$loco.momentary = [2]
$loco.press_function(2)
$on = $loco.function(2)
$loco.release_function(2)
print([$loco.speed, $base.decoder(5)["speed"], $loco.speed_percent, $on, $loco.function(2)])
```

Output:
```
[20, 20, 71, true, false]
```

## A consist

Two locomotives as one train, the second back to back with the first; in an advanced consist the
decoders take the speed from the consist address.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$a = locomotive(7)
$b = locomotive(8)
$train = consist(address: 74)
$train.add($a)
$train.add($b, reversed: true)
$train.speed = 40
print([$base.decoder(7)["speed"], $base.decoder(8)["direction"], $base.decoder(8)["cvs"][19]])
$train.break_up()
print($base.decoder(8)["cvs"][19])
```

Output:
```
[40, "reverse", 202]
0
```
