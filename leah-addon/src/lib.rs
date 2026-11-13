//! The model railroading addon (design/model_railroading.md): Leah as the command station. A DCC base
//! station (`dcc_base()`), simulated or Leah's own board on a USB port with a reader board reading its
//! signal back, and the locomotives it drives (`locomotive()`), the packets made as the NMRA's
//! standards give them; the boards' ports (`dcc_ports()`) and loading their programs
//! (`load_dcc_program()`).
//!
//! A 3rd-party addon, built with Leah's addon kit: Leah loads it from a 3rd-party addons folder, where
//! `leah --install-addon https://github.com/bmasters-dev/model-railroading` puts it.

mod auto;
mod base;
mod block;
mod board;
mod clock;
mod consist;
mod dcc;
mod definitions;
mod diagram;
mod layout;
mod light;
mod makers;
mod phone;
mod plan;
mod programmer;
mod roster;
mod route;
mod sensor;
mod sim;
mod speed;
mod switchboard;
mod turnout;
mod wait;

use leah_addon_kit::{Function, Needs, Value};

/// What each function needs from a script runner's rules: a simulated base station nothing (nothing
/// leaves the program), one on a board's port "device"; the ports list "device"; loading a program
/// "programs" (arduino-cli) and "device".
fn needs(name: &str, options: &[(String, Value, i64)]) -> Vec<Needs> {
    match name {
        "dcc_base" if options.iter().any(|(n, v, _)| n == "simulate" && *v == Value::Bool(true)) => vec![],
        "dcc_base" | "dcc_ports" => vec![Needs::Device],
        "load_dcc_program" => vec![Needs::Programs, Needs::Device],
        "read_decoder_definition_file" | "roster" | "write_layout_file" | "read_layout_file" => vec![Needs::Files],
        _ => vec![],
    }
}

// (As the kit's `leah_addon!` makes it, with the needs given by the options above.)
/// The addon's entry point (Leah's addon interface).
#[unsafe(no_mangle)]
pub extern "C" fn leah_addon_entry(level: u32) -> *const leah_addon_kit::raw::AddonTable {
    // (An outside program it needs that isn't a Debian package, shown by `leah --doctor`, #32.)
    leah_addon_kit::needs_programs(&[("arduino-cli", "building and loading the base station's board program", "https://arduino.github.io/arduino-cli/latest/installation/")]);
    leah_addon_kit::raw::entry(level, || leah_addon_kit::Definition {
        name: "model_railroading",
        version: env!("CARGO_PKG_VERSION"),
        summary: "Model railroading: a DCC base station (simulated, or Leah's own board on USB) and the locomotives it drives",
        requires: &[],
        functions: vec![
            ("dcc_base", &[Needs::Device], base::new_base as Function),
            ("locomotive", &[], base::new_locomotive as Function),
            ("dcc_ports", &[Needs::Device], base::dcc_ports as Function),
            ("load_dcc_program", &[Needs::Programs, Needs::Device], base::load_dcc_program as Function),
            ("decoder_maker_name", &[], base::decoder_maker_name as Function),
            ("cv29_parts", &[], base::cv29_parts as Function),
            ("make_cv29", &[], base::make_cv29 as Function),
            ("decoder_definition", &[], definitions::new_definition as Function),
            ("decoder_definitions", &[], definitions::definitions_list as Function),
            ("decoder_definition_for", &[], definitions::definition_for as Function),
            ("read_decoder_definition_file", &[Needs::Files], definitions::read_definition_file as Function),
            ("decoder_programmer", &[], programmer::new_programmer as Function),
            ("roster", &[Needs::Files], roster::new_roster as Function),
            ("consist", &[], consist::new_consist as Function),
            ("phone_throttle_server", &[Needs::Web], phone::new_server as Function),
            ("turnout", &[], turnout::new_turnout as Function),
            ("sensor", &[], sensor::new_sensor as Function),
            ("light", &[], light::new_light as Function),
            ("block", &[], block::new_block as Function),
            ("layout", &[], layout::new_layout as Function),
            ("route", &[], route::new_route as Function),
            ("track_plan", &[], plan::new_plan as Function),
            ("track_diagram", &[], diagram::new_diagram as Function),
            ("switchboard", &[], switchboard::new_switchboard as Function),
            ("fast_clock", &[], clock::new_clock as Function),
            ("layout_simulator", &[], sim::new_simulator as Function),
            ("auto_train", &[], auto::new_auto_train as Function),
            ("speedometer", &[], speed::new_speedometer as Function),
            ("scale_speed", &[], speed::scale_speed as Function),
            ("wait_for", &[], wait::wait_for as Function),
            ("wait_for_any", &[], wait::wait_for_any as Function),
            ("write_layout_file", &[Needs::Files], layout::write_layout_file as Function),
            ("read_layout_file", &[Needs::Files], layout::read_layout_file as Function),
        ],
        needs: Some(needs),
        objects: vec![
            ("dcc_base", &["simulate", "port", "reader", "power", "packets", "timing", "shield", "current", "current_limit", "locomotives"], &["emergency_stop", "stop_all", "decoder", "accessory", "clear_packets", "close", "read_cv", "read_address", "write_cv", "write_address", "verify_cv", "read_cv_bit", "write_cv_bit", "identify_decoder", "reset_decoder", "find_decoder_locks", "unlock_decoder", "lock_decoder"], &["on_read", "on_write", "on_locomotive_changed"]),
            ("locomotive", &["address", "long", "base", "speed", "direction", "speed_steps", "speed_percent", "speed_limit", "momentary", "labels", "entry"], &["set_function", "function", "press_function", "release_function", "stop", "emergency_stop", "write_cv", "write_cv_bit"], &[]),
            ("decoder_definition", &["name", "maker", "versions", "settings", "groups", "cvs"], &["setting", "value_of", "cvs_of", "is_shown"], &[]),
            ("decoder_programmer", &["definition", "main_track", "values", "states", "cvs", "busy", "problems"], &["value", "set_value", "state", "read_all", "read", "write_changes", "write_all", "load_cvs", "cancel"], &["on_progress", "on_done"]),
            ("consist", &["name", "address", "kind", "members", "speed", "direction", "base"], &["add", "remove", "is_reversed", "set_function", "stop", "emergency_stop", "break_up"], &[]),
            ("roster", &["path", "names", "count", "groups", "changed"], &["add", "entry", "entries", "remove", "save", "export_entries", "import_entries"], &[]),
            ("roster_entry", &["name", "road_name", "road_number", "address", "long", "decoder", "model", "notes", "picture", "groups", "cvs", "functions", "speed_limit", "attributes"], &["set_function", "attribute", "set_attribute"], &[]),
            ("dcc_program_job", &["port", "role", "board", "finished", "succeeded", "message"], &["wait"], &["on_finished"]),
            ("turnout", &["address", "name", "state", "commanded", "inverted", "feedback", "sensors", "move_time", "locked", "base"], &["close", "throw", "toggle"], &["on_changed", "on_mismatch"]),
            ("sensor", &["pin", "name", "state", "active", "inverted", "pull", "active_delay", "inactive_delay", "base"], &["simulate"], &["on_changed", "on_active", "on_inactive"]),
            ("light", &["address", "name", "on", "inverted", "following", "base"], &["toggle", "pulse", "follow"], &["on_changed"]),
            ("block", &["name", "sensor", "state", "occupied", "train", "reserved", "reserved_by"], &[], &["on_changed", "on_occupied", "on_cleared"]),
            ("layout", &["names", "count", "turnouts", "sensors", "lights", "blocks"], &["add", "find", "remove"], &[]),
            ("route", &["name", "settings", "blocks", "delay", "busy", "aligned", "reserved"], &["set", "reserve", "release"], &["on_set", "on_aligned", "on_released"]),
            ("track_plan", &["blocks", "connections"], &["connect", "set_cost", "cost", "neighbours", "find_path"], &[]),
            ("speedometer", &["sensors", "distance", "scale", "last_speed", "last_scale_kmh", "last_scale_mph"], &[], &["on_measured"]),
            ("auto_train", &["locomotive", "route", "state", "block", "max_speed", "approach_speed", "direction"], &["start", "halt", "resume", "emergency_stop"], &["on_departed", "on_block_entered", "on_arrived", "on_blocked"]),
            ("layout_simulator", &["plan", "trains", "top_speed", "block_length", "running"], &["add_train", "remove_train", "set_length", "length", "start", "stop"], &["on_train_entered", "on_train_stopped"]),
            ("fast_clock", &["time", "rate", "running", "alarms", "hour", "minute"], &["start", "stop", "set_time"], &["on_minute", "on_alarm"]),
            ("phone_throttle_server", &["name", "port", "running", "clients", "heartbeat", "turnouts", "routes", "clock", "base", "roster"], &["add_turnout", "set_turnout", "remove_turnout", "add_route", "set_route", "remove_route", "set_clock", "stop_clock", "message", "close"], &["on_client", "on_client_left", "on_turnout_request", "on_route_request"]),
        ],
        controls: vec![("track_diagram", diagram::PROPS, diagram::FUNCS, diagram::EVENTS, (400, 240)), ("switchboard", switchboard::PROPS, switchboard::FUNCS, switchboard::EVENTS, (400, 160))],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_simulated_base_station_needs_nothing_and_a_board_needs_the_device() {
        let simulate = vec![("simulate".to_string(), Value::Bool(true), 1)];
        assert!(needs("dcc_base", &simulate).is_empty());
        assert_eq!(needs("dcc_base", &[]), vec![Needs::Device]);
        assert_eq!(needs("load_dcc_program", &[]), vec![Needs::Programs, Needs::Device]);
        assert!(needs("locomotive", &[]).is_empty());
    }
}
