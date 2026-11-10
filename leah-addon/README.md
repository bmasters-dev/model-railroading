# The model railroading addon

A 3rd-party addon for [Leah](https://github.com/bmasters-dev/leah-language): Leah **as** a model
railroad's DCC command station.

**Built:** a simulated DCC base station (`dcc_base(simulate: true)`) and its locomotives
(`locomotive()`). The packets are made as the NMRA's standards give them and read back from their
signal by simulated decoders, so a script can be tried with no board and no power on the track.
`examples/dcc_simulator.leah` is a throttle with a train going round an oval as the decoder
understood each packet. **Next:** Leah's own base station board (an Arduino Mega with a motor
shield), tried first with no track power (`design/model_railroading.md`, "Stage 1a": the safe ladder).

| | |
|---|---|
| `src/` | The addon: `dcc.rs` (the NMRA packets, their bits and timing, a decoder reading them back), `base.rs` (`dcc_base` and `locomotive`), `lib.rs` (what Leah is told, through the addon kit) |
| `docs/` | Its pages for the Leah Language Reference, and its examples |
| `design/` | The plan and the decisions |
| `tests/` | Scripts run by Leah with this addon, and the example on Leah's hidden test screen |
| `tools/publish_release.py` | Builds it and publishes a release (`leah-addon-v<version>`) |

## Building and testing

It builds with Leah's addon kit, from Leah's source beside this repository (`~/Leah_Dev/kit`), until
the kit is published:

```
cargo build
cargo test        # with Leah's development build, ~/Leah_Dev/target/debug/leah (or LEAH=...)
```

To try it in your own Leah without publishing: copy `target/debug/libleah_addon_model_railroading.so`
into a 3rd-party addons folder as `model_railroading.so`.
