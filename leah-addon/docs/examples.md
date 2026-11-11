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
