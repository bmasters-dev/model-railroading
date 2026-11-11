# Model railroading

## Description

Leah runs model trains as a DCC command station: it makes the packets a locomotive's decoder obeys (its speed, its direction, its lights and sounds), as the NMRA's DCC standards give them. A [dcc_base](ref:dcc_base) is the base station, and each [locomotive](ref:locomotive) is a train on it, by its DCC address.

A base station can be simulated, with no board and no power on any rails: each packet is turned into its bits and timing, and a simulated decoder for each address reads it back from that signal, as a decoder on the rails does. So a script, a throttle, or a whole panel can be tried, and its packets checked, with nothing that can harm a locomotive. Or it can be **Leah's base station board**, an Arduino Mega making the real signal, checked by a second board that reads it back (below). What each decoder understood is given by the base station's [decoder](ref:dcc_base#decoder) function, and every packet sent, with its bytes and bits, by its [packets](ref:dcc_base#packets).

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

## Leah's base station board

Leah's own base station is an **Arduino Mega** running Leah's base station program, which makes the DCC signal with the board's hardware timer, so its timing doesn't depend on what the computer is doing. With an **EX-MotorShield8874** plugged onto the Mega and a 12 to 15 V supply in the shield's barrel jack, the signal goes onto the rails from the shield's channel A (its screw terminals), the track's current is watched, and the board turns the power off by itself on a short ([current_limit](ref:dcc_base#current_limit)). Without the shield, the signal is 5 V logic on the Mega's **pin 12**, for checking on the bench before anything goes near a track. A second board, the **reader** (an **Arduino Uno or Nano** running Leah's reader program), reads the signal back as a decoder would: the packets it heard, and how long each half-bit lasted.

**Joining them**, with both unplugged from the computer: the Mega's pin 12 (on the shield's own row of sockets when it is on) to one leg of a **1 kΩ resistor**, the resistor's other leg to the reader's **pin 8**, and a wire from a **GND** pin of one board to a **GND** pin of the other. Then plug both into the computer's USB ports, which power them. Nothing else is connected.

**Once per board**, load Leah's programs onto them with [load_dcc_program](ref:DCC board functions#load_dcc_program), which needs **arduino-cli** (Arduino's command-line tool: [its releases](https://github.com/arduino/arduino-cli/releases), then `arduino-cli core install arduino:avr` once). **On Linux**, this account must be allowed to use the boards' USB ports: in the `dialout` group ([dcc_ports](ref:DCC board functions#dcc_ports) says whether it is).

```leah check
$base = dcc_base("/dev/ttyACM0", reader: "/dev/ttyUSB0")
$base.power = true
$loco = locomotive(3)
$loco.speed = 40
sleep(1500)
print($base.decoder(3)["speed"])
print($base.timing["one"])
```

## The programming track

The motor shield's **B** screws are the **programming track**: a piece of track (or a treadmill) on its own, where a decoder's settings are read and written, its address among them ([read_cv](ref:dcc_base#read_cv), [read_address](ref:dcc_base#read_address), [write_cv](ref:dcc_base#write_cv), [write_address](ref:dcc_base#write_address)). Put the locomotive on it, read, then move it to the main track (the **A** screws) to drive. The main track's power is off while reading.

## Pages

| Page | What it holds |
|---|---|
| [dcc_base](ref:dcc_base) | The base station: track power, the packets sent, what each decoder understood, the emergency stop |
| [locomotive](ref:locomotive) | A train by its address: its speed, direction, and functions |
| [DCC board functions](ref:DCC board functions) | The boards plugged in, and loading Leah's programs onto them |
| [dcc_program_job](ref:dcc_program_job) | A program being loaded onto a board |

## Sample code

The example app `dcc_simulator.leah`, which comes with the addon, is a throttle: track power, a speed slider, a reverse box, the headlight, and an emergency stop button. A train goes round an oval of track as the simulated decoder understood the packets, and a list shows each packet's bytes, its meaning, and the bits of the last one.

The example app `dcc_bench.leah` is the same throttle on Leah's base station board, with the reader: it finds the two boards, loads Leah's programs onto them, connects, and shows what the reader heard, how long the half-bits were, and whether that is within the NMRA's limits. When Linux doesn't let the account use the boards yet, it says so in plain words and offers to fix it.

The example app `dcc_throttle.leah` drives a real locomotive on Leah's base station with its motor shield: the locomotive's address (a long one chosen by itself past 127), the track power with its current, a speed slider, Forward and Reverse (sent as soon as it connects, as some decoders keep their headlight off until they are told a direction), Stop and Emergency stop (also the Escape key), and the functions F0 to F12; its **Read address** button reads the address of the locomotive on the programming track and drives that one, and its **Decoder settings** row reads and writes any setting there, its name shown for the usual ones.

## See also

[Addons](ref:Addons), [dcc_base](ref:dcc_base), [locomotive](ref:locomotive), [DCC board functions](ref:DCC board functions), [dcc_program_job](ref:dcc_program_job)
