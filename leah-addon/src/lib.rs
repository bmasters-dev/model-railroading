//! The model railroading addon (design/model_railroading.md): Leah as the command station. Its first
//! stage is a simulated DCC base station (`dcc_base(simulate: true)`) and the locomotives it drives
//! (`locomotive()`), the packets made as the NMRA's standards give them and read back from their
//! signal by simulated decoders.
//!
//! A 3rd-party addon, built with Leah's addon kit: Leah loads it from a 3rd-party addons folder, where
//! `leah --install-addon https://github.com/bmasters-dev/model-railroading` puts it.

mod base;
mod dcc;

use leah_addon_kit::leah_addon;

leah_addon! {
    name: "model_railroading",
    version: env!("CARGO_PKG_VERSION"),
    summary: "Model railroading: a DCC base station (simulated for now) and the locomotives it drives",
    // (Each function, what it needs from a script runner's rules (none: nothing leaves the program
    // while the base station is simulated), and the Rust function running it.)
    functions: [
        ("dcc_base", [], base::new_base),
        ("locomotive", [], base::new_locomotive),
    ],
    objects: [
        ("dcc_base", ["simulate", "power", "packets"], ["emergency_stop", "decoder", "clear_packets"]),
        ("locomotive", ["address", "long", "base", "speed", "direction"], ["set_function", "function", "stop", "emergency_stop"]),
    ],
}
