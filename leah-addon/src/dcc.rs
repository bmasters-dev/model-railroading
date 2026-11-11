//! DCC as the NMRA's standards give it: the packets (S-9.2, S-9.2.1), their bits and timing (S-9.1),
//! and a decoder that reads packets back from the timing, as a decoder on the rails does
//! (design/model_railroading.md, "Stage 1a").

/// A half of a 1 bit and of a 0 bit, in microseconds, as a command station sends them.
pub const ONE_HALF_US: u32 = 58;
pub const ZERO_HALF_US: u32 = 100;
/// The ones before each packet (at least 14 from a command station; a decoder needs 10).
pub const PREAMBLE: usize = 14;

/// The highest short address, and the highest long one.
pub const SHORT_MOST: u16 = 127;
pub const LONG_MOST: u16 = 10239;
/// The functions a locomotive has here, F0 to F28.
pub const FUNCTIONS: usize = 29;

/// The packet: the data, then the error byte (every byte XOR-ed).
pub fn packet(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    out.push(data.iter().fold(0, |a, b| a ^ b));
    out
}

/// The address's bytes: one for a short address, two (`11AAAAAA AAAAAAAA`) for a long one.
pub fn address_bytes(address: u16, long: bool) -> Vec<u8> {
    if long { vec![0xC0 | (address >> 8) as u8, (address & 0xFF) as u8] } else { vec![address as u8] }
}

/// The 128-step speed and direction (`0x3F`, then `DSSSSSSS`): 0 stop, 1 emergency stop, 2 to 127
/// the steps 1 to 126.
pub fn speed_128(address: u16, long: bool, speed: u8, forward: bool, emergency: bool) -> Vec<u8> {
    let value = if emergency {
        1
    } else if speed == 0 {
        0
    } else {
        speed.min(126) + 1
    };
    let mut data = address_bytes(address, long);
    data.push(0x3F);
    data.push(if forward { 0x80 } else { 0 } | value);
    packet(&data)
}

/// The basic speed and direction for a decoder in 28- or 14-step mode (S-9.2: `01DCSSSS`). In 28
/// steps, step n is sent as n + 3 over SSSS and C (its lowest bit); in 14 steps, step n is SSSS = n + 1,
/// and C carries the headlight (F0), which a 14-step decoder takes from here. 0 stop, 1 emergency.
pub fn speed_basic(address: u16, long: bool, speed: u8, forward: bool, emergency: bool, steps: u8, light: bool) -> Vec<u8> {
    let d = if forward { 0x20 } else { 0 };
    let low = if steps == 14 {
        let s = if emergency { 1 } else if speed == 0 { 0 } else { speed.min(14) + 1 };
        (if light { 0x10 } else { 0 }) | s
    } else {
        let v = if emergency { 2 } else if speed == 0 { 0 } else { speed.min(28) + 3 };
        ((v & 1) << 4) | (v >> 1)
    };
    let mut data = address_bytes(address, long);
    data.push(0x40 | d | low);
    packet(&data)
}

/// The function group holding function `n` (0 to 28): F0 to F4, F5 to F8, F9 to F12, F13 to F20,
/// F21 to F28.
pub fn group_of(n: usize) -> usize {
    match n {
        0..=4 => 0,
        5..=8 => 1,
        9..=12 => 2,
        13..=20 => 3,
        _ => 4,
    }
}

/// A function group's packet, from the states of every function.
pub fn functions(address: u16, long: bool, group: usize, on: &[bool; FUNCTIONS]) -> Vec<u8> {
    let bit = |n: usize, shift: u8| u8::from(on[n]) << shift;
    let mut data = address_bytes(address, long);
    match group {
        // 100 F0 F4 F3 F2 F1
        0 => data.push(0x80 | bit(0, 4) | bit(4, 3) | bit(3, 2) | bit(2, 1) | bit(1, 0)),
        // 1011 F8 F7 F6 F5
        1 => data.push(0xB0 | bit(8, 3) | bit(7, 2) | bit(6, 1) | bit(5, 0)),
        // 1010 F12 F11 F10 F9
        2 => data.push(0xA0 | bit(12, 3) | bit(11, 2) | bit(10, 1) | bit(9, 0)),
        3 => data.extend([0xDE, (0..8).map(|i| bit(13 + i, i as u8)).sum()]),
        _ => data.extend([0xDF, (0..8).map(|i| bit(21 + i, i as u8)).sum()]),
    }
    packet(&data)
}

/// The emergency stop every decoder obeys (the broadcast address 0, `01000001`).
pub fn broadcast_emergency_stop() -> Vec<u8> {
    packet(&[0x00, 0x41])
}

/// The packet's bits on the rails: the preamble, then each byte after a 0, then a 1.
pub fn bits(packet: &[u8]) -> Vec<bool> {
    let mut out = vec![true; PREAMBLE];
    for byte in packet {
        out.push(false);
        out.extend((0..8).rev().map(|i| byte >> i & 1 == 1));
    }
    out.push(true);
    out
}

/// The bits as text, grouped as a packet reads: `11111111111111 0 00000011 0 00111111 ... 1`.
pub fn bits_text(packet: &[u8]) -> String {
    let mut out = "1".repeat(PREAMBLE);
    for byte in packet {
        out.push_str(&format!(" 0 {byte:08b}"));
    }
    out.push_str(" 1");
    out
}

/// The signal: each bit two halves, 58 µs for a 1, 100 µs for a 0.
pub fn timings(bits: &[bool]) -> Vec<u32> {
    bits.iter().flat_map(|b| if *b { [ONE_HALF_US; 2] } else { [ZERO_HALF_US; 2] }).collect()
}

/// The bytes as hex, `03 3F A9 95`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// A decoder's reading of the signal: half-bits into bits, bits into packets, the error byte checked
/// (NMRA S-9.1 and S-9.2: a half of 52 to 64 µs is a 1, of 90 to 10,000 µs a 0, and a packet starts
/// after at least 10 ones).
#[derive(Default)]
pub struct Reader {
    /// A half-bit waiting for its other half: whether it was a 1.
    half: Option<bool>,
    /// The ones in a row so far, while looking for a packet's start.
    ones: usize,
    /// Inside a packet: its bytes so far, and the bits of the byte being read.
    bytes: Option<Vec<u8>>,
    byte: Vec<bool>,
}

impl Reader {
    /// Reads half-bits, giving back each whole packet with a right error byte (without it).
    pub fn read(&mut self, halves: &[u32]) -> Vec<Vec<u8>> {
        let mut found = Vec::new();
        for &us in halves {
            let one = match us {
                52..=64 => true,
                90..=10_000 => false,
                // (Neither: noise. What was being read is let go.)
                _ => {
                    *self = Reader::default();
                    continue;
                }
            };
            match self.half.take() {
                None => self.half = Some(one),
                Some(first) if first == one => {
                    if let Some(p) = self.bit(one) {
                        found.push(p);
                    }
                }
                // (Two different halves: out of step; this half starts the next bit.)
                Some(_) => self.half = Some(one),
            }
        }
        found
    }

    fn bit(&mut self, one: bool) -> Option<Vec<u8>> {
        let Some(bytes) = self.bytes.as_mut() else {
            if one {
                self.ones += 1;
            } else {
                if self.ones >= 10 {
                    self.bytes = Some(Vec::new());
                    self.byte.clear();
                }
                self.ones = 0;
            }
            return None;
        };
        if self.byte.len() < 8 {
            self.byte.push(one);
            if self.byte.len() == 8 {
                bytes.push(self.byte.iter().fold(0u8, |a, b| a << 1 | u8::from(*b)));
            }
            return None;
        }
        // After a byte: a 0 starts another, a 1 ends the packet.
        if !one {
            self.byte.clear();
            return None;
        }
        let packet = self.bytes.take().unwrap_or_default();
        self.byte.clear();
        // (The end bit may begin the next preamble.)
        self.ones = 1;
        let good = packet.len() >= 3 && packet.iter().fold(0, |a, b| a ^ b) == 0;
        good.then(|| packet[..packet.len() - 1].to_vec())
    }
}

/// What a locomotive's decoder holds: its speed (0 to 126), direction, and functions.
#[derive(Clone)]
pub struct DecoderState {
    pub speed: u8,
    pub forward: bool,
    pub functions: [bool; FUNCTIONS],
    pub heard: u64,
    /// The settings written to it on the main (CV number, value).
    pub cvs: std::collections::BTreeMap<u16, u8>,
    /// A write on the main heard once, waiting for its identical second (S-9.2.1 2.3.7.3: two
    /// identical packets, no other packet to this decoder between them, a broadcast among them).
    pub pending_write: Option<Vec<u8>>,
    /// The speed steps of the last speed it heard: 128, 28, or 14.
    pub steps: u8,
}

impl Default for DecoderState {
    fn default() -> Self {
        DecoderState { speed: 0, forward: true, functions: [false; FUNCTIONS], heard: 0, cvs: Default::default(), pending_write: None, steps: 128 }
    }
}

/// Writing a whole setting on the main (S-9.2.1's long form: `1110 11VV`, `VVVVVVVV`, the value).
pub fn ops_write_byte(address: u16, long: bool, cv: u16, value: u8) -> Vec<u8> {
    let mut data = address_bytes(address, long);
    let v = cv - 1;
    data.extend([0xEC | (v >> 8) as u8 & 0x03, (v & 0xFF) as u8, value]);
    packet(&data)
}

/// Writing one bit of a setting on the main (`1110 10VV`, `VVVVVVVV`, `1111 DBBB`).
pub fn ops_write_bit(address: u16, long: bool, cv: u16, bit: u8, on: bool) -> Vec<u8> {
    let mut data = address_bytes(address, long);
    let v = cv - 1;
    data.extend([0xE8 | (v >> 8) as u8 & 0x03, (v & 0xFF) as u8, 0xF0 | u8::from(on) << 3 | bit & 0x07]);
    packet(&data)
}

/// Whom a packet is for.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Target {
    Broadcast,
    Idle,
    Locomotive(u16, bool),
    /// An address this decoder doesn't take (accessory decoders, ...).
    Other,
}

/// A packet (without its error byte) read: whom it is for, and its instruction bytes.
pub fn target(data: &[u8]) -> (Target, &[u8]) {
    match data.first() {
        Some(0x00) => (Target::Broadcast, &data[1..]),
        Some(0xFF) => (Target::Idle, &data[1..]),
        Some(&a @ 1..=127) => (Target::Locomotive(u16::from(a), false), &data[1..]),
        Some(&a @ 0xC0..=0xE7) if data.len() >= 2 => (Target::Locomotive((u16::from(a & 0x3F) << 8) | u16::from(data[1]), true), &data[2..]),
        _ => (Target::Other, data),
    }
}

/// A decoder's state changed by an instruction, and what it understood, in words (None when it
/// isn't one this decoder knows).
pub fn obey(state: &mut DecoderState, instruction: &[u8]) -> Option<String> {
    let set = |state: &mut DecoderState, first: usize, bits: u8, count: usize| {
        for i in 0..count {
            state.functions[first + i] = bits >> i & 1 == 1;
        }
    };
    // A write on the main acts on its second identical packet; any other packet to this decoder
    // between them (a broadcast too) cancels it.
    if let [g, v, d, ..] = instruction
        && (g & 0xFC == 0xEC || (g & 0xFC == 0xE8 && d & 0xF0 == 0xF0))
    {
        let cv = (u16::from(g & 0x03) << 8 | u16::from(*v)) + 1;
        if state.pending_write.as_deref() != Some(&instruction[..3]) {
            state.pending_write = Some(instruction[..3].to_vec());
            return None;
        }
        state.pending_write = None;
        let words = if g & 0xFC == 0xEC {
            state.cvs.insert(cv, *d);
            format!("CV {cv} = {d} (on the main)")
        } else {
            let bit = d & 0x07;
            let on = d & 0x08 != 0;
            let old = state.cvs.get(&cv).copied().unwrap_or(0);
            state.cvs.insert(cv, if on { old | 1 << bit } else { old & !(1 << bit) });
            format!("CV {cv} bit {bit} = {} (on the main)", u8::from(on))
        };
        state.heard += 1;
        return Some(words);
    }
    state.pending_write = None;
    let words = match instruction {
        [0x3F, ds, ..] => {
            state.steps = 128;
            state.forward = ds & 0x80 != 0;
            let direction = if state.forward { "forward" } else { "reverse" };
            match ds & 0x7F {
                0 => {
                    state.speed = 0;
                    format!("stop, {direction}")
                }
                1 => {
                    state.speed = 0;
                    format!("emergency stop, {direction}")
                }
                v => {
                    state.speed = v - 1;
                    format!("speed {}, {direction}", v - 1)
                }
            }
        }
        // 01DCSSSS: the basic speed instruction, read as the decoder's CV 29 says (bit 1 clear: 14
        // steps, C its headlight; else 28 steps, C the step's lowest bit). Its stops (the broadcast
        // emergency stop among them) leave the direction as it was.
        [b, ..] if b & 0xC0 == 0x40 => {
            let fourteen = state.cvs.get(&29).is_some_and(|c| c & 0x02 == 0);
            let ssss = b & 0x0F;
            if fourteen {
                state.functions[0] = b & 0x10 != 0;
            }
            if ssss <= 1 {
                state.speed = 0;
                if ssss == 0 { "stop".to_string() } else { "emergency stop".to_string() }
            } else {
                state.forward = b & 0x20 != 0;
                let direction = if state.forward { "forward" } else { "reverse" };
                if fourteen {
                    state.steps = 14;
                    state.speed = ssss - 1;
                } else {
                    state.steps = 28;
                    state.speed = ((ssss << 1) | (b >> 4 & 1)) - 3;
                }
                format!("speed {} of {}, {direction}", state.speed, state.steps)
            }
        }
        [b, ..] if b & 0xE0 == 0x80 => {
            state.functions[0] = b & 0x10 != 0;
            set(state, 1, b & 0x0F, 4);
            on_words(state, 0, 4)
        }
        [b, ..] if b & 0xF0 == 0xB0 => {
            set(state, 5, b & 0x0F, 4);
            on_words(state, 5, 8)
        }
        [b, ..] if b & 0xF0 == 0xA0 => {
            set(state, 9, b & 0x0F, 4);
            on_words(state, 9, 12)
        }
        [0xDE, b, ..] => {
            set(state, 13, *b, 8);
            on_words(state, 13, 20)
        }
        [0xDF, b, ..] => {
            set(state, 21, *b, 8);
            on_words(state, 21, 28)
        }
        _ => return None,
    };
    state.heard += 1;
    Some(words)
}

/// "functions F0 to F4: F0 on, the rest off".
fn on_words(state: &DecoderState, first: usize, last: usize) -> String {
    let on: Vec<String> = (first..=last).filter(|n| state.functions[*n]).map(|n| format!("F{n}")).collect();
    let which = if on.is_empty() { "all off".to_string() } else { format!("{} on, the rest off", on.join(", ")) };
    format!("functions F{first} to F{last}: {which}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_basic_speed_packets_are_the_nmras_and_read_back() {
        // 28 steps: step 1 is 0010/0, step 28 1111/1; 14 steps: step n is n + 1, the headlight in C.
        assert_eq!(speed_basic(3, false, 1, true, false, 28, false)[1], 0x62);
        assert_eq!(speed_basic(3, false, 28, true, false, 28, false)[1], 0x7F);
        assert_eq!(speed_basic(3, false, 0, true, true, 28, false)[1], 0x61);
        assert_eq!(speed_basic(3, false, 5, false, false, 14, true)[1], 0x56);
        let mut d = DecoderState::default();
        obey(&mut d, &speed_basic(3, false, 17, false, false, 28, false)[1..2]);
        assert_eq!((d.speed, d.forward, d.steps), (17, false, 28));
        d.cvs.insert(29, 4);
        obey(&mut d, &speed_basic(3, false, 9, true, false, 14, true)[1..2]);
        assert_eq!((d.speed, d.forward, d.steps, d.functions[0]), (9, true, 14, true));
        obey(&mut d, &speed_basic(3, false, 0, true, false, 14, false)[1..2]);
        assert_eq!((d.speed, d.functions[0]), (0, false));
    }

    #[test]
    fn packets_are_the_nmras() {
        // S-9.2: address 3, 128 steps, speed 40 forward; a long address; F0 on; the broadcast stop.
        assert_eq!(hex(&speed_128(3, false, 40, true, false)), "03 3F A9 95");
        assert_eq!(hex(&speed_128(1234, true, 126, true, false)), "C4 D2 3F FF D6");
        let mut on = [false; FUNCTIONS];
        on[0] = true;
        assert_eq!(hex(&functions(3, false, 0, &on)), "03 90 93");
        assert_eq!(hex(&broadcast_emergency_stop()), "00 41 41");
        // S-9.2.1 2.3.7.3: CV 3 = 20 on address 3 (1110 1100, CV-1 = 2, 20); CV 29 bit 5 set.
        assert_eq!(hex(&ops_write_byte(3, false, 3, 20)), "03 EC 02 14 F9");
        assert_eq!(hex(&ops_write_bit(3, false, 29, 5, true)), "03 E8 1C FD 0A");
        assert_eq!(hex(&ops_write_byte(1234, true, 300, 7)), "C4 D2 ED 2B 07 D7");
    }

    #[test]
    fn a_write_on_the_main_takes_two_identical_packets() {
        let write = ops_write_byte(3, false, 3, 20);
        let instruction = &write[1..write.len() - 1];
        let mut state = DecoderState::default();
        assert_eq!(obey(&mut state, instruction), None);
        assert!(state.cvs.is_empty());
        assert_eq!(obey(&mut state, instruction).as_deref(), Some("CV 3 = 20 (on the main)"));
        assert_eq!(state.cvs.get(&3), Some(&20));
        // Another packet to the decoder between the two cancels it.
        let other = ops_write_byte(3, false, 4, 9);
        let other_instruction = &other[1..other.len() - 1];
        obey(&mut state, other_instruction);
        obey(&mut state, &[0x3F, 0x80]);
        obey(&mut state, other_instruction);
        assert_eq!(state.cvs.get(&4), None);
        // A bit write.
        let bit = ops_write_bit(3, false, 29, 5, true);
        let b = &bit[1..bit.len() - 1];
        obey(&mut state, b);
        obey(&mut state, b);
        assert_eq!(state.cvs.get(&29), Some(&0x20));
    }

    #[test]
    fn a_decoder_reads_packets_back_from_their_timing() {
        let sent = speed_128(3, false, 40, true, false);
        let mut reader = Reader::default();
        let read = reader.read(&timings(&bits(&sent)));
        assert_eq!(read, vec![sent[..sent.len() - 1].to_vec()]);
        // A wrong error byte: the packet is let go of.
        let mut bad = sent.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(Reader::default().read(&timings(&bits(&bad))).is_empty());
        // Halves outside S-9.1's limits are noise.
        let mut halves = timings(&bits(&sent));
        halves[40] = 75;
        assert!(Reader::default().read(&halves).is_empty());
    }
}
