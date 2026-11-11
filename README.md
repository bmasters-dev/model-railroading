# Model railroading

Model railroading with [Leah](https://github.com/bmasters-dev/leah-language): Leah **as** the command
station, with its own DCC base station, then dead rail (battery locomotives with ESP32s), throttles,
panels, CVs, and a roster. This repository holds the Leah addon and, in time, other things for the
layout.

| Folder | What it holds |
|---|---|
| [leah-addon/](leah-addon/) | The model railroading addon for Leah: `dcc_base()` and `locomotive()`. A 3rd-party addon, built with Leah's addon kit. |

## What you need

- **For the simulator** (`dcc_base(simulate: true)`): only Leah and this addon.
- **For a real base station board** (being built: the safe ladder's step 2):
  - **`arduino-cli`** (1.5 or newer), Arduino's command-line tool, which compiles Leah's board
    programs and loads them onto the boards. It isn't in Debian: get it from
    [its releases](https://github.com/arduino/arduino-cli/releases), then add the boards' support once
    with `arduino-cli core install arduino:avr`.
  - An **Arduino Mega 2560** (the base station) and, for checking the signal with no track power, an
    **Arduino Uno or Nano** (the reader), on USB.
  - **Permission to open the boards' USB ports**: on Debian and Ubuntu, your account in the `dialout`
    group (`sudo usermod -aG dialout $USER`, then log out and back in). The example app explains this
    and offers to do it when it is missing.

## Installing the addon

```
leah --install-addon https://github.com/bmasters-dev/model-railroading
```

Then start Leah again. `leah --update-addon model_railroading` gets a newer release, and
`leah --remove-addon model_railroading` takes it away. Then try the throttle:

```
leah leah-addon/examples/dcc_simulator.leah
```

The addon's releases are tagged `leah-addon-v<version>`, apart from anything else this repository
releases.

## Licence

MIT or Apache 2.0, at your choice ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
