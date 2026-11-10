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
