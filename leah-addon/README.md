# The model railroading addon

A 3rd-party addon for [Leah](https://github.com/bmasters-dev/leah-language): Leah **as** a model
railroad's DCC command station.

**Built (Phase A):** a DCC base station, simulated (`dcc_base(simulate: true)`: the packets made as
the NMRA's standards give them and read back from their signal by simulated decoders, so a script can
be tried with no board and no power on the track) or **Leah's own base station board** (an Arduino
Mega with the EX-MotorShield8874, `dcc_base("/dev/ttyACM0")`, checked by an Uno or Nano reading the
signal back); locomotives with 14, 28, or 128 speed steps, speed limits, and momentary keys;
reading, writing, identifying, resetting, and unlocking decoders on the programming track and on the
main; decoder definitions and the `decoder_programmer`; the `roster`; consists, software and
advanced. The samples in `examples/`: a throttle, a game pad throttle, the decoder workshop, a bench
tester, and a simulator. **Next (Phase B):** a server for phone throttle apps (`design/model_railroading.md`).

**Documentation:** <https://github.com/bmasters-dev/model-railroading/wiki>, published from `docs/`
by `tools/docs.py`.

| | |
|---|---|
| `src/` | The addon: `dcc.rs` (the NMRA packets, their bits and timing, a decoder reading them back), `base.rs` (`dcc_base`, `locomotive`, `dcc_ports`, `load_dcc_program`), `board.rs` (the boards' ports, the link to them, loading their programs), `lib.rs` (what Leah is told, through the addon kit) |
| `boards/` | The boards' programs, which `load_dcc_program()` loads: `dcc_base/` (the Mega) and `dcc_reader/` (the Uno or Nano) |
| `docs/` | Its pages, published to this repository's wiki (its own documentation, apart from Leah's reference), and its examples (`examples.md`) |
| `design/` | The plan and the decisions |
| `tests/` | Scripts run by Leah with this addon, the examples on Leah's hidden test screen, and stand-ins for the boards (`fake_boards.py`) |
| `tools/publish_release.py` | Builds it and publishes a release (`leah-addon-v<version>`) |
| `tools/docs.py` | Checks every page's samples and links (`check`) and publishes the pages to the wiki (`wiki`) |
| `tools/make_icons.leah` | Draws its icons into `model_railroading.icons` |

## Building and testing

It builds with Leah's addon kit, from Leah's source beside this repository (`~/Leah_Dev/kit`), until
the kit is published:

```
cargo build
cargo test        # with Leah's development build, ~/Leah_Dev/target/debug/leah (or LEAH=...)
```

To try it in your own Leah without publishing: copy `target/debug/libleah_addon_model_railroading.so`
into a 3rd-party addons folder as `model_railroading.so`.
