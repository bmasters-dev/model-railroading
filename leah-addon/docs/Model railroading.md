# Model railroading

## Description

Leah runs model trains as a DCC command station: it makes the packets a locomotive's decoder obeys (its speed, its direction, its lights and sounds), as the NMRA's DCC standards give them. A [dcc_base](ref:dcc_base) is the base station, and each [locomotive](ref:locomotive) is a train on it, by its DCC address.

The base station is a simulated one, with no board and no power on any rails: each packet is turned into its bits and timing, and a simulated decoder for each address reads it back from that signal, as a decoder on the rails does. So a script, a throttle, or a whole panel can be tried, and its packets checked, with nothing that can harm a locomotive. What each decoder understood is given by the base station's [decoder](ref:dcc_base#decoder) function, and every packet sent, with its bytes and bits, by its [packets](ref:dcc_base#packets).

All of this comes with the **model_railroading** addon, a 3rd-party addon installed by its address: `leah --install-addon https://github.com/bmasters-dev/model-railroading` (then start Leah again). A Leah without it says `unknown function` when a script uses one of its names.

```leah
$base = dcc_base(simulate: true)
$base.power = true
$loco = locomotive(3)
$loco.speed = 40
$loco.set_function(0, true)
print($base.decoder(3)["speed"])
foreach $base.packets as $p {
    print($p["bytes"] + "  " + $p["meaning"])
}
```

Output:

```
40
03 3F A9 95  locomotive 3: speed 40, forward
03 90 93  locomotive 3: functions F0 to F4: F0 on, the rest off
```

## DCC in a few words

- **An address** picks the locomotive: 1 to 127 is a short address, sent as one byte; 128 to 10239 a long one, sent as two. A decoder answers to one or the other: 3 and the long address 3 are two locomotives.
- **Speed** goes in 128 steps: 0 is stopped and 126 is full speed. The direction is sent with it.
- **Functions** are numbered from F0, the headlight; F1 to F28 are what the decoder makes them: often the bell, the horn, and other lights and sounds.
- **A packet** is the address, an instruction, and an error byte that lets a decoder throw away a packet that was damaged on the way.
- **The signal** on the rails is the packet's bits: a preamble of at least 14 ones, then each byte after a 0, then a 1. Each bit is two halves of the track voltage switched one way and then the other: 58 microseconds each for a 1, 100 for a 0.
- **An emergency stop** to the broadcast address stops every locomotive at once.

## Pages

| Page | What it holds |
|---|---|
| [dcc_base](ref:dcc_base) | The base station: track power, the packets sent, what each decoder understood, the emergency stop |
| [locomotive](ref:locomotive) | A train by its address: its speed, direction, and functions |

## Sample code

The example app `dcc_simulator.leah`, which comes with the addon, is a throttle: track power, a speed slider, a reverse box, the headlight, and an emergency stop button. A train goes round an oval of track as the simulated decoder understood the packets, and a list shows each packet's bytes, its meaning, and the bits of the last one.

## See also

[Addons](ref:Addons), [dcc_base](ref:dcc_base), [locomotive](ref:locomotive)
