# The model railroading addon

A 3rd-party addon for [Leah](https://github.com/bmasters-dev/leah-language): Leah **as** a model
railroad's DCC command station.

**Built:** a simulated DCC base station (`dcc_base(simulate: true)`) and its locomotives
(`locomotive()`). The packets are made as the NMRA's standards give them and read back from their
signal by simulated decoders, so a script can be tried with no board and no power on the track.
`examples/dcc_simulator.leah` is a throttle with a train going round an oval as the decoder
understood each packet. And **Leah's own base station board**, so far with no track power: an
Arduino Mega making the real signal on a pin (`dcc_base("/dev/ttyACM0")`), checked by an Uno or Nano
reading it back (`reader:`), its programs loaded by `load_dcc_program()` through arduino-cli;
`examples/dcc_bench.leah` drives it and shows what the reader heard. **Next:** the motor shield and a
power supply, still with no locomotive (`design/model_railroading.md`, the safe ladder's step 3).

| | |
|---|---|
| `src/` | The addon: `dcc.rs` (the NMRA packets, their bits and timing, a decoder reading them back), `base.rs` (`dcc_base`, `locomotive`, `dcc_ports`, `load_dcc_program`), `board.rs` (the boards' ports, the link to them, loading their programs), `lib.rs` (what Leah is told, through the addon kit) |
| `boards/` | The boards' programs, which `load_dcc_program()` loads: `dcc_base/` (the Mega) and `dcc_reader/` (the Uno or Nano) |
| `docs/` | Its pages for the Leah Language Reference, and its examples |
| `design/` | The plan and the decisions |
| `tests/` | Scripts run by Leah with this addon, the examples on Leah's hidden test screen, and stand-ins for the boards (`fake_boards.py`) |
| `tools/publish_release.py` | Builds it and publishes a release (`leah-addon-v<version>`) |
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
