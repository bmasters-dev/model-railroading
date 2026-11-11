# DCC board functions

## Description

The functions for Leah's base station board and its reader: the boards plugged in, and loading Leah's programs onto them. They come with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

| Function | What it does |
|---|---|
| [dcc_ports](#dcc_ports) | The boards plugged in, and whether this account may use them |
| [load_dcc_program](#load_dcc_program) | Loads Leah's base station or reader program onto a board |

## Function descriptions

### dcc_ports

**dcc_ports**() · array

The USB serial ports plugged in (the boards: `/dev/ttyACM0`, `/dev/ttyUSB0`, ...), in order, each an array:

| Key | Holds |
|---|---|
| `port` | its whole path, as [dcc_base](ref:dcc_base) and [load_dcc_program](#load_dcc_program) take it |
| `name` | what the board is, from its USB ids: `"Arduino Mega 2560"`, `"Arduino Uno"`, `"USB serial (CH340): an Uno, Nano, or Mega clone"`, or the name the device gives |
| `vendor_id`, `product_id` | its USB ids, four hex digits each: `"2341"`, `"0042"` |
| `allowed` | `true` when this account may use it; `false` when Linux doesn't let it yet |

A board that isn't `allowed` is there, but this account can't open it until it is in the `dialout` group. An app should say so in plain words and offer to fix it: with the **devices** addon, `fix_device_access("serial")` does it (asking for the password); without it, `sudo usermod -aG dialout $USER` in a terminal. Either way the user then logs out and back in once. An empty list means no board is plugged in.

```leah check
foreach dcc_ports() as $p {
    print($p["port"] + ": " + $p["name"] + (if $p["allowed"] then "" else " (not allowed yet)"))
}
```

---

### load_dcc_program

**load_dcc_program**(port, program, board: ..., arduino_cli: ...) · dcc_program_job

Loads one of Leah's board programs onto the board on `port`, in the background: it compiles the program for the board and uploads it, as the Arduino IDE does, through **arduino-cli**, Arduino's command-line tool. It gives back a [dcc_program_job](ref:dcc_program_job) at once, whose [on_finished](ref:dcc_program_job#on_finished) event says how it went (a board takes about 15 seconds the first time, a few after). A program stays on the board until another is loaded, so this is done once per board, and again when a newer version of the addon brings a newer program (then [dcc_base](ref:dcc_base) says so).

| Part | Description |
|---|---|
| port | Required. The board's port, its whole path: `"/dev/ttyACM0"`. |
| program | Required. `"base"`, the base station's program (for an Arduino Mega), or `"reader"`, the reader's (for an Arduino Uno or Nano). Anything else is a `type_error`. |
| board | Optional. arduino-cli's name for the board: by default `"arduino:avr:mega"` for the base station and `"arduino:avr:uno"` for the reader. A Nano is `"arduino:avr:nano"`, or `"arduino:avr:nano:cpu=atmega328old"` with the old bootloader. |
| arduino_cli | Optional. Where arduino-cli is, when it isn't on the PATH. |

arduino-cli isn't part of Leah: get it from [its releases](https://github.com/arduino/arduino-cli/releases), and add the boards' support once with `arduino-cli core install arduino:avr`. Without it, `load_dcc_program` is a `shell_error` saying so. The program, its build, and arduino-cli's build cache go in a temporary folder of the script's own, removed when the script ends. A board a [dcc_base](ref:dcc_base) has open can't be loaded: [close](ref:dcc_base#close) it first.

```leah check
$job = load_dcc_program("/dev/ttyACM0", "base")
$job.on_finished = function($ok, $message) {
    print($message)
}
```

## See also

[dcc_program_job](ref:dcc_program_job), [dcc_base](ref:dcc_base), [Model railroading](ref:Model railroading)
