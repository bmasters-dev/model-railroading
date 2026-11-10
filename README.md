# Model railroading

Model railroading with [Leah](https://github.com/bmasters-dev/leah-language): Leah **as** the command
station, with its own DCC base station, then dead rail (battery locomotives with ESP32s), throttles,
panels, CVs, and a roster. This repository holds the Leah addon and, in time, other things for the
layout.

| Folder | What it holds |
|---|---|
| [leah-addon/](leah-addon/) | The model railroading addon for Leah: `dcc_base()` and `locomotive()`. A 3rd-party addon, built with Leah's addon kit. |

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
