// Leah's DCC base station (design/model_railroading.md, "Stage 1b" and "Stage 1c"): the NMRA S-9.1
// signal on pin 12 (Timer1's output OC1B, so every edge comes from the hardware timer), for an Arduino
// Mega 2560. With the EX-MotorShield8874 on it, the signal drives the shield's channel A (its
// direction input is pin 12), which puts it on the rails; without the shield, pin 12 is 5 V logic, for
// checking on the bench.
//
// The shield's channel A, as DCC-EX's own definition of it gives the pins: pin 3 turns its output on
// and off (the track power), pin 9 is its brake (held low: off), A0 its current (5.08 mA a step on a
// 5 V, 10-bit Mega), A4 its fault line (pulled up; low on a fault). Whether the shield is there is
// found as the program starts: with the internal pull-up on, A0 reads high with nothing on it, and low
// with the shield's sense resistor to ground.
//
// A 1 is 58 us high then 58 us low; a 0 is 100 us high then 100 us low. Timer1 runs at 2 MHz
// (0.5 us ticks) in fast PWM with its top in OCR1A: each period is one bit. At the end of each bit the
// overflow interrupt works out the next bit and sets its top and middle, which the timer takes at its
// next top.
//
// What it sends: the packets waiting to go now (each three times), else the refresh table's packets in
// turn (each locomotive's speed, kept repeating as command stations do), an idle packet between every
// two (the copies of one sent now too), so no decoder gets two packets in a row: an idle packet lasts
// about 5.5 ms, NMRA S-9.2's least time between two packets to one address.
//
// Over USB (115200 baud), one line at a time; every packet in hex with its error byte last, checked:
//   K 3 03 3F A9 95    keep this packet in the refresh table's place 3 (0 to 31)
//   D 3                drop place 3
//   S 00 41 41         send this packet now, three times, ahead of the refresh table
//   +   -              track power on, off (off: pin 3 low and pin 12 held low, nothing sent)
//   L 1500             the current limit, in mA (10 to 5000; 1500 at the start)
//   X                  everything dropped, power off
//   ?                  "STATE power on places 2 shield yes current 25 limit 1500"
//   .                  nothing: only that the PC is still there
//   I 22 1             watch pin 22 as an input with the internal pull-up (I 22 0: without it; I 22 -:
//                      no longer): its level said at once, "P 22 1", and again whenever it changes
//                      and stays changed for 3 ms, "P 22 0" (sensors: block detectors, pushbuttons,
//                      track contacts). Pins 2 to 69 (A0 to A15 are 54 to 69), but the program's own
//                      (0, 1, 3, 8, 9, 11, 12, 13, A0, A1, A4, A5). X stops watching every pin.
// It answers "OK" or "ERR ..." to every line but ".", and starts with "LEAH DCC BASE 7" (6 was the
// same without I). With the shield and the power on, it says the current every half second: "A 25" (mA).
//
// The programming track (design/model_railroading.md, "Stage 2"): the shield's channel B, as DCC-EX
// has it (pin 11 its output on and off, pin 13 its direction, which is Timer1's OC1C, so the same
// timer gives it the same signal; pin 8 its brake, held low; A1 its current; A5 its fault line).
//   R 1                read setting (CV) 1 on the programming track: "CV 1 3", "CV 1 none" (nothing
//                      answered), or "CV 1 off <why>"
//   W 3 10             write 10 into setting 3, then verify it: "W 3 10 ok", "W 3 10 unconfirmed
//                      (...)", or "W 3 off <why>"
//   V 3 6              is setting 3 6? "V 3 6 yes" or "V 3 6 no" (no: another value, or no decoder)
//   U 29 5             read bit 5 of setting 29: "U 29 5 1", "U 29 5 0", or "U 29 5 none"
//   Y 29 5 1           write bit 5 of setting 29, then verify it: "Y 29 5 1 ok" or "... unconfirmed"
// Reading turns the main track (A) off first, and B off again after: NMRA S-9.2.3's service mode,
// direct mode. B gets the reset packet as its filler (never the idle packet, which would take a decoder
// out of service mode), 260 of them first (about 1.6 s) for the decoder to start up (up to 1 A for
// its first 1.5 s), then for each of the 8 bits:
// 5 resets (the current's baseline measured over them), 5 "verify bit is 1" packets, 6 resets, a
// decoder's answer, its acknowledgment, being 60 mA or more over the baseline for about 4 ms; then
// the whole byte verified the same way. B's current is limited to 250 mA (S-9.2.3) and its fault line
// watched, as A's are.
//
// Safety: the power turns off by itself, saying why with "OFF ...", when
//   - three seconds pass without a line from the PC (it ended, crashed, or lost the cable);
//   - the current is over the limit for about 5 ms all told (a short on the track: a coin, a derailed
//     wheel): a count that rises twice as fast over the limit as it falls under it, so a short the
//     driver chops on and off still adds up (the first 20 ms after the power comes on are left out,
//     for the track's and decoders' capacitors to charge);
//   - the shield's driver says it has a fault (too hot, a short it caught itself, or no supply in the
//     shield's jack) for about 2 ms all told, counted as the current is (rising twice as fast while
//     it says so as it falls while it doesn't), after the same first 20 ms: the driver flags a fault
//     for a moment whenever it catches a surge itself, and a DCC meter or a sound decoder's
//     capacitors make one as the power comes on (found on the owner's bench, 2026-11-11: cutting at
//     the first flag cut the power whenever a DCC meter was on the track).
// The current is sampled by the analog reader's own interrupt, about 9,600 times a second, which cuts
// the power itself: nothing the PC sends, or the main loop is busy with, delays it. After cutting off
// for the current or a fault, it won't turn the power on again for 2 seconds. Without the shield
// found, pin 3 is never turned on. (No watchdog timer: some Megas' bootloaders never come out of the
// reset it makes.)

#include <Arduino.h>

const uint16_t ONE_TOP = 231, ONE_MIDDLE = 115;    // 232 ticks = 116 us, high for 116 ticks = 58 us
const uint16_t ZERO_TOP = 399, ZERO_MIDDLE = 199;  // 400 ticks = 200 us, high for 200 ticks = 100 us
const uint8_t MOST_BYTES = 6, PLACES = 32, WAITING = 8, PREAMBLE = 14, TIMES_NOW = 3;
const uint32_t QUIET_LIMIT = 3000;
const uint8_t SIGNAL_PIN = 12, POWER_PIN = 3, BRAKE_PIN = 9, SENSE_PIN = A0, FAULT_PIN = A4;
const uint8_t PROG_SIGNAL_PIN = 13, PROG_POWER_PIN = 11, PROG_BRAKE_PIN = 8, PROG_FAULT_PIN = A5;
const float MA_PER_STEP = 5.08;

struct Packet { uint8_t bytes[MOST_BYTES]; uint8_t count; };

const Packet IDLE = {{0xFF, 0x00, 0xFF}, 3};
const Packet RESET = {{0x00, 0x00, 0x00}, 3};
// The programming track: whether it is on (and the main track off), the service packet to send and
// how many times more, the packets sent so far (counted by the timer's interrupt), and the analog
// reader's watch for a decoder's acknowledgment.
volatile bool prog = false;
Packet prog_packet;
volatile uint8_t prog_left = 0;
volatile uint16_t packets_sent = 0;
// An acknowledgment: about 2 ms all told (19 samples) 60 mA or more over the baseline within the
// window, not necessarily in one run (a wheel flickering on its rail breaks a run); the byte verified
// at the end guards against a false one.
const uint16_t PROG_LIMIT_STEPS = 250 / 5.08, PROG_START_LIMIT_STEPS = 1000 / 5.08, ACK_STEPS = 60 / 5.08, ACK_SAMPLES = 19;
// For the first 1.5 s (about 14,400 samples) the programming track takes up to 1 A: a sound decoder's
// stay-alive capacitors and amplifier draw far more than 250 mA as it starts, for most of a second
// (782 and 833 mA were cut on the owner's bench, 2026-11-11, at 20 and 400 ms), as DCC-EX lets it
// too (its "prog boost").
const uint16_t PROG_START_SAMPLES = 14400;
// On the programming track the limit must be over for about 50 ms all told before it cuts: a
// decoder's acknowledgment is its motor pulsed for 6 ms, often at its full stall current (about 800 mA
// on the owner's bench, which cut every reading at the first "yes" with 5 ms). A short is still cut in
// about 2 ms by the driver's own fault line.
const uint16_t PROG_TRIP_COUNT = 960;
volatile uint16_t prog_samples = 0;
volatile bool ack_watch = false, ack = false;
volatile uint16_t ack_threshold = 0, ack_run = 0, ack_peak = 0;
uint16_t last_baseline = 0;
volatile uint32_t base_sum = 0;
volatile uint16_t base_count = 0;
Packet table[PLACES];                // count 0: the place is free
Packet waiting[WAITING];             // the packets to send now, a ring
volatile uint8_t waiting_head = 0, waiting_tail = 0;
volatile bool power = false;
bool shield = false;
volatile uint16_t limit_steps = 1500 / 5.08;   // the limit, in the analog reader's steps
uint16_t limit_ma = 1500;
// The analog reader's interrupt's own: the over-limit count, the samples since the power came on, the
// sum for the average said, and why it cut the power (0: it didn't; 1: the current; 2: a fault).
const uint16_t TRIP_COUNT = 96, FAULT_COUNT = 40, BLANK_SAMPLES = 192;
volatile uint16_t over_count = 0, fault_count = 0, since_on = 0, tripped_steps = 0;
volatile uint32_t sum_steps = 0;
volatile uint16_t sum_count = 0;
volatile uint8_t tripped = 0;
uint32_t tripped_at = 0;
float current_ma = 0;                // the average over the last half second, for saying

// The interrupt's own: the packet going out and where in it, and a packet sent now while it repeats.
Packet going, again;
uint8_t preamble_left = PREAMBLE, byte_at = 0, times_left = 0, place_at = 0;
int8_t bit_at = -1;                  // -1: the start bit before the byte
bool idle_next = false;              // true after every packet but an idle one: an idle one comes next

void next_packet() {
  packets_sent++;
  if (prog) {                                                         // service mode: resets between
    if (prog_left > 0) { prog_left--; going = prog_packet; } else going = RESET;
    return;
  }
  if (idle_next) { going = IDLE; idle_next = false; return; }       // between every two packets
  if (times_left > 0) { times_left--; going = again; idle_next = true; return; }
  if (waiting_head != waiting_tail) {
    again = waiting[waiting_tail];
    waiting_tail = (waiting_tail + 1) % WAITING;
    going = again;
    times_left = TIMES_NOW - 1;
    idle_next = true;
    return;
  }
  for (uint8_t i = 0; i < PLACES; i++) {
    uint8_t p = (place_at + i) % PLACES;
    if (table[p].count) { going = table[p]; place_at = (p + 1) % PLACES; idle_next = true; return; }
  }
  going = IDLE;                                                       // nothing to send: idle
}

uint8_t next_bit() {
  if (preamble_left) { preamble_left--; return 1; }
  if (byte_at < going.count) {
    if (bit_at < 0) { bit_at = 7; return 0; }
    uint8_t b = (going.bytes[byte_at] >> bit_at) & 1;
    if (bit_at == 0) { byte_at++; bit_at = -1; } else bit_at--;
    return b;
  }
  next_packet();                                                      // the end bit, then the next one
  preamble_left = PREAMBLE; byte_at = 0; bit_at = -1;
  return 1;
}

ISR(TIMER1_OVF_vect) {
  if (next_bit()) { OCR1A = ONE_TOP; OCR1B = ONE_MIDDLE; OCR1C = ONE_MIDDLE; }
  else { OCR1A = ZERO_TOP; OCR1B = ZERO_MIDDLE; OCR1C = ZERO_MIDDLE; }
}

// Cuts the power from inside an interrupt (interrupts are off already).
void cut_power_now(uint8_t why) {
  PORTE &= ~_BV(PE5);                                                 // pin 3 low: the shield's output off
  TCCR1A &= ~_BV(COM1B1);
  PORTB &= ~_BV(PB6);                                                 // pin 12 low
  power = false;
  tripped = why;
}

// Cuts the programming track from inside an interrupt.
void cut_prog_now(uint8_t why) {
  PORTB &= ~_BV(PB5);                                                 // pin 11 low: channel B's output off
  TCCR1A &= ~_BV(COM1C1);
  PORTB &= ~_BV(PB7);                                                 // pin 13 low
  prog = false;
  ADMUX = _BV(REFS0);                                                 // back to A0
  tripped = why;
}

// The analog reader, running freely on A0 (A1 while the programming track is on): each sample the
// track's current.
ISR(ADC_vect) {
  uint16_t steps = ADC;
  if (prog) {
    if (since_on < BLANK_SAMPLES) { since_on++; return; }
    if (!(PINF & _BV(PF5))) fault_count += 2; else if (fault_count) fault_count--;           // A5 low: a fault
    if (fault_count >= FAULT_COUNT) { tripped_steps = steps; fault_count = 0; cut_prog_now(4); return; }
    uint16_t prog_limit = PROG_LIMIT_STEPS;
    if (prog_samples < PROG_START_SAMPLES) { prog_samples++; prog_limit = PROG_START_LIMIT_STEPS; }
    if (steps > prog_limit) over_count += 2; else if (over_count) over_count--;
    if (over_count >= PROG_TRIP_COUNT) { tripped_steps = steps; over_count = 0; cut_prog_now(3); return; }
    if (ack_watch) {
      if (steps > ack_peak) ack_peak = steps;
      if (steps > ack_threshold && ++ack_run >= ACK_SAMPLES) ack = true;
    } else {
      base_sum += steps;
      base_count++;
    }
    return;
  }
  if (!power) return;
  sum_steps += steps;
  sum_count++;
  if (since_on < BLANK_SAMPLES) { since_on++; return; }
  if (!(PINF & _BV(PF4))) fault_count += 2; else if (fault_count) fault_count--;             // A4 low: a fault
  if (fault_count >= FAULT_COUNT) { tripped_steps = steps; fault_count = 0; cut_power_now(2); return; }
  if (steps > limit_steps) over_count += 2; else if (over_count) over_count--;
  if (over_count >= TRIP_COUNT) { tripped_steps = steps; over_count = 0; cut_power_now(1); }
}

void set_power(bool on) {
  noInterrupts();
  power = on;
  if (on) {
    over_count = 0;
    fault_count = 0;
    since_on = 0;
    TCCR1A |= _BV(COM1B1);                                            // the signal first...
    if (shield) digitalWrite(POWER_PIN, HIGH);                        // ...then the shield's output on
  } else {
    digitalWrite(POWER_PIN, LOW);                                     // the shield's output off first
    TCCR1A &= ~_BV(COM1B1);
    digitalWrite(SIGNAL_PIN, LOW);
  }
  interrupts();
}

// The programming track on (the main track off first) or off.
void set_prog(bool on) {
  if (on) set_power(false);
  noInterrupts();
  if (on) {
    over_count = 0;
    fault_count = 0;
    since_on = 0;
    prog_samples = 0;
    ack_watch = false;
    base_sum = 0;
    base_count = 0;
    ADMUX = _BV(REFS0) | 1;                                           // A1: channel B's current
    prog_left = 0;
    prog = true;
    TCCR1A |= _BV(COM1C1);
    digitalWrite(PROG_POWER_PIN, HIGH);
  } else {
    digitalWrite(PROG_POWER_PIN, LOW);
    TCCR1A &= ~_BV(COM1C1);
    digitalWrite(PROG_SIGNAL_PIN, LOW);
    prog = false;
    ADMUX = _BV(REFS0);                                               // A0: channel A's
  }
  interrupts();
}

uint16_t sent_so_far() {
  noInterrupts();
  uint16_t n = packets_sent;
  interrupts();
  return n;
}

// Waits while `count` packets go out; false when the programming track was cut meanwhile.
bool wait_packets(uint16_t count) {
  uint16_t start = sent_so_far();
  while ((uint16_t)(sent_so_far() - start) < count) {
    if (!prog) return false;
  }
  return prog;
}

// Sends a service packet as S-9.2.3 has it and says whether the decoder acknowledged it: the baseline
// over 5 resets, then 5 of the packet and 6 resets while watching for the acknowledgment. -1: the
// programming track was cut.
int service(const Packet &p) {
  noInterrupts(); base_sum = 0; base_count = 0; interrupts();
  if (!wait_packets(5)) return -1;
  noInterrupts();
  uint16_t baseline = base_count ? base_sum / base_count : 0;
  last_baseline = baseline;
  ack_threshold = baseline + ACK_STEPS;
  ack = false;
  ack_run = 0;
  ack_watch = true;
  prog_packet = p;
  prog_left = 5;
  interrupts();
  bool ok = wait_packets(11);
  noInterrupts(); ack_watch = false; bool heard = ack; interrupts();
  return ok ? heard : -1;
}

// Reads a setting (CV 1 to 1024) on the programming track: its value, -1 when nothing acknowledged,
// -2 when the programming track was cut (its reason said by the loop).
int read_cv(uint16_t cv) {
  set_prog(true);
  noInterrupts(); ack_peak = 0; interrupts();
  if (!wait_packets(260)) return -2;                                  // the decoder starts up (about 1.6 s)
  uint8_t high = 0x03 & ((cv - 1) >> 8), low = (cv - 1) & 0xFF, value = 0;
  for (uint8_t bit = 0; bit < 8; bit++) {
    Packet p = {{(uint8_t)(0x78 | high), low, (uint8_t)(0xE8 | bit), 0}, 4};   // verify bit `bit` is 1
    p.bytes[3] = p.bytes[0] ^ p.bytes[1] ^ p.bytes[2];
    int a = service(p);
    if (a < 0) return -2;
    if (a) value |= 1 << bit;
  }
  Packet v = {{(uint8_t)(0x74 | high), low, value, 0}, 4};                      // verify the byte
  v.bytes[3] = v.bytes[0] ^ v.bytes[1] ^ v.bytes[2];
  int a = service(v);
  set_prog(false);
  if (a < 0) return -2;
  return a ? value : -1;
}

// Writes a setting (CV 1 to 1024) on the programming track (S-9.2.3's direct mode, write byte): the
// decoder started as for reading, the write sent 5 times with 6 resets after (its acknowledgment
// watched, though not every decoder gives one for a write), 30 more resets for it to store the value,
// then the byte verified as a reading's last question is, twice at most. 1: confirmed; 0: written but
// not confirmed; -2: the programming track was cut.
int write_cv(uint16_t cv, uint8_t value) {
  set_prog(true);
  noInterrupts(); ack_peak = 0; interrupts();
  if (!wait_packets(260)) return -2;
  uint8_t high = 0x03 & ((cv - 1) >> 8), low = (cv - 1) & 0xFF;
  Packet w = {{(uint8_t)(0x7C | high), low, value, 0}, 4};
  w.bytes[3] = w.bytes[0] ^ w.bytes[1] ^ w.bytes[2];
  if (service(w) < 0) return -2;
  if (!wait_packets(30)) return -2;
  Packet v = {{(uint8_t)(0x74 | high), low, value, 0}, 4};
  v.bytes[3] = v.bytes[0] ^ v.bytes[1] ^ v.bytes[2];
  for (uint8_t tries = 0; tries < 2; tries++) {
    int a = service(v);
    if (a < 0) return -2;
    if (a) { set_prog(false); return 1; }
  }
  set_prog(false);
  return 0;
}

// One service packet built for CV `cv` (1 to 1024): `kind` 0x74 verify byte, 0x78 bit (verify or write),
// 0x7C write byte, with its third byte.
Packet service_packet(uint8_t kind, uint16_t cv, uint8_t third) {
  Packet p = {{(uint8_t)(kind | (0x03 & ((cv - 1) >> 8))), (uint8_t)((cv - 1) & 0xFF), third, 0}, 4};
  p.bytes[3] = p.bytes[0] ^ p.bytes[1] ^ p.bytes[2];
  return p;
}

// Verifies a whole setting: 1 the decoder said yes, 0 nothing answered (not that value, or no
// decoder), -2 the programming track was cut.
int verify_cv(uint16_t cv, uint8_t value) {
  set_prog(true);
  noInterrupts(); ack_peak = 0; interrupts();
  if (!wait_packets(260)) return -2;
  int a = service(service_packet(0x74, cv, value));
  set_prog(false);
  return a < 0 ? -2 : a;
}

// Reads one bit: asks "is it 1?" then "is it 0?". 1 or 0; -1 when neither was answered; -2 cut.
int read_cv_bit(uint16_t cv, uint8_t bit) {
  set_prog(true);
  noInterrupts(); ack_peak = 0; interrupts();
  if (!wait_packets(260)) return -2;
  int one = service(service_packet(0x78, cv, 0xE8 | bit));
  if (one < 0) return -2;
  int zero = one ? 0 : service(service_packet(0x78, cv, 0xE0 | bit));
  set_prog(false);
  if (zero < 0) return -2;
  return one ? 1 : (zero ? 0 : -1);
}

// Writes one bit, then verifies it (twice at most). 1 confirmed, 0 not, -2 cut.
int write_cv_bit(uint16_t cv, uint8_t bit, uint8_t on) {
  set_prog(true);
  noInterrupts(); ack_peak = 0; interrupts();
  if (!wait_packets(260)) return -2;
  if (service(service_packet(0x78, cv, 0xF0 | (on << 3) | bit)) < 0) return -2;
  if (!wait_packets(30)) return -2;
  for (uint8_t tries = 0; tries < 2; tries++) {
    int a = service(service_packet(0x78, cv, 0xE0 | (on << 3) | bit));
    if (a < 0) return -2;
    if (a) { set_prog(false); return 1; }
  }
  set_prog(false);
  return 0;
}

// Says, after a reading or a writing, why the programming track was cut (the rest of its answer line).
void say_prog_cut() {
  uint8_t why;
  uint16_t steps;
  noInterrupts(); why = tripped; steps = tripped_steps; tripped = 0; interrupts();
  if (why == 3) {
    Serial.print(" off the programming track's current was over its limit (");
    Serial.print((long)(steps * MA_PER_STEP));
    Serial.println(" mA, the limit 250 mA, 1 A as the decoder starts): is something shorting it?");
  } else if (why == 4) {
    Serial.println(" off the motor shield's channel B driver reported a fault (too hot, a short it caught itself, or no supply in the shield's jack)");
  } else {
    Serial.println(" off the programming track was cut off");
  }
}

void power_off_because(const char *why) {
  set_power(false);
  Serial.print("OFF ");
  Serial.println(why);
}

int hex_digit(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}

// Reads a packet's bytes from the text; false (with the answer said) when they aren't one.
bool read_packet(const char *c, Packet &p) {
  p.count = 0;
  uint8_t check = 0;
  while (*c) {
    if (*c == ' ') { c++; continue; }
    int high = hex_digit(c[0]), low = c[1] ? hex_digit(c[1]) : -1;
    if (high < 0 || low < 0) { Serial.println("ERR each byte is two hex digits"); return false; }
    if (p.count == MOST_BYTES) { Serial.println("ERR a packet is at most 6 bytes"); return false; }
    p.bytes[p.count++] = high * 16 + low;
    check ^= high * 16 + low;
    c += 2;
  }
  if (p.count < 3) { Serial.println("ERR a packet is at least 3 bytes, its error byte last"); return false; }
  if (check != 0) { Serial.println("ERR the error byte doesn't match the packet"); return false; }
  return true;
}

// A whole number from the text (moving past it); -1 when there is none.
long read_number(const char *&c) {
  while (*c == ' ') c++;
  long n = 0;
  int digits = 0;
  while (*c >= '0' && *c <= '9' && digits < 6) { n = n * 10 + (*c - '0'); c++; digits++; }
  return digits ? n : -1;
}

// The pins watched as inputs: whether each is, the level last said, and a change waiting to last 3 ms.
const uint8_t PINS = 70;
bool watched[PINS];
uint8_t said_level[PINS], new_level[PINS];
uint8_t changed_at[PINS];             // millis()'s low byte when the waiting change was first seen

bool own_pin(uint8_t pin) {
  return pin < 2 || pin == POWER_PIN || pin == PROG_BRAKE_PIN || pin == BRAKE_PIN || pin == PROG_POWER_PIN
      || pin == SIGNAL_PIN || pin == PROG_SIGNAL_PIN || pin == SENSE_PIN || pin == A1 || pin == FAULT_PIN || pin == PROG_FAULT_PIN;
}

void say_pin(uint8_t pin) {
  Serial.print("P "); Serial.print(pin); Serial.print(' '); Serial.println(said_level[pin]);
}

void stop_watching() {
  for (uint8_t pin = 0; pin < PINS; pin++) {
    if (watched[pin]) { watched[pin] = false; pinMode(pin, INPUT); }
  }
}

// Each watched pin's level, said when it has changed and stayed changed for 3 ms.
void watch_pins() {
  uint8_t now = millis();
  for (uint8_t pin = 2; pin < PINS; pin++) {
    if (!watched[pin]) continue;
    uint8_t level = digitalRead(pin);
    if (level == said_level[pin]) { new_level[pin] = level; continue; }
    if (level != new_level[pin]) { new_level[pin] = level; changed_at[pin] = now; continue; }
    if ((uint8_t)(now - changed_at[pin]) >= 3) { said_level[pin] = level; say_pin(pin); }
  }
}

void obey(const char *line) {
  const char *c = line + 1;
  switch (line[0]) {
    case '.': return;
    case '+':
      if (tripped_at && millis() - tripped_at < 2000) {
        Serial.println("OFF it cut the power off a moment ago: it waits 2 seconds before the power can come on again");
        return;
      }
      set_power(true);
      Serial.println("OK");
      return;
    case '-': set_power(false); Serial.println("OK"); return;
    case 'X':
      set_prog(false);
      set_power(false);
      stop_watching();
      noInterrupts();
      for (uint8_t i = 0; i < PLACES; i++) table[i].count = 0;
      waiting_tail = waiting_head;
      interrupts();
      Serial.println("OK");
      return;
    case '?': {
      uint8_t used = 0;
      for (uint8_t i = 0; i < PLACES; i++) if (table[i].count) used++;
      Serial.print("STATE power "); Serial.print(power ? "on" : "off");
      Serial.print(" places "); Serial.print(used);
      Serial.print(" shield "); Serial.print(shield ? "yes" : "no");
      Serial.print(" current "); Serial.print(shield && power ? (long)(current_ma + 0.5) : 0);
      Serial.print(" limit "); Serial.println(limit_ma);
      return;
    }
    case 'B': {
      // The programming track on for that many seconds (1 to 30) with reset packets, its current said
      // four times a second ("B 812"), limited to 1 A throughout: for checking a decoder and the
      // track on the bench.
      long seconds = read_number(c);
      if (seconds < 1 || seconds > 30) { Serial.println("ERR the programming track is on for 1 to 30 seconds"); return; }
      if (!shield) { Serial.println("ERR there is no motor shield"); return; }
      set_prog(true);
      noInterrupts(); prog_samples = 0; interrupts();
      uint32_t end = millis() + seconds * 1000UL, next = millis();
      while ((long)(millis() - end) < 0 && prog) {
        noInterrupts(); prog_samples = 0; base_sum = 0; base_count = 0; interrupts();
        while ((long)(millis() - next) < 0 && prog) {}
        next += 250;
        uint32_t sum; uint16_t n;
        noInterrupts(); sum = base_sum; n = base_count; base_sum = 0; base_count = 0; interrupts();
        Serial.print("B "); Serial.println(n ? (long)((float)sum / n * MA_PER_STEP) : 0);
      }
      bool cut = !prog;
      set_prog(false);
      Serial.println(cut ? "B off cut" : "B done");
      return;
    }
    case 'V': case 'U': case 'Y': {
      // V cv value: verify a setting ("V 3 6 yes" / "no"); U cv bit: read a bit ("U 29 5 1" / "0" /
      // "none"); Y cv bit 0|1: write a bit and verify it ("Y 29 5 1 ok" / "unconfirmed").
      char what = line[0];
      long cv = read_number(c);
      long second = read_number(c);
      long third = what == 'Y' ? read_number(c) : 0;
      if (cv < 1 || cv > 1024) { Serial.println("ERR a setting (CV) is 1 to 1024"); return; }
      if (what == 'V' && (second < 0 || second > 255)) { Serial.println("ERR a setting's value is 0 to 255"); return; }
      if (what != 'V' && (second < 0 || second > 7)) { Serial.println("ERR a bit is 0 to 7"); return; }
      if (what == 'Y' && (third < 0 || third > 1)) { Serial.println("ERR a bit's value is 0 or 1"); return; }
      if (!shield) { Serial.print(what); Serial.print(" "); Serial.print(cv); Serial.println(" off there is no motor shield"); return; }
      if (power) Serial.println("OFF reading a setting: the main track's power is off while the programming track is on");
      int done = what == 'V' ? verify_cv(cv, second) : what == 'U' ? read_cv_bit(cv, second) : write_cv_bit(cv, second, third);
      set_prog(false);
      Serial.print(what); Serial.print(" "); Serial.print(cv); Serial.print(" "); Serial.print(second);
      if (what == 'Y') { Serial.print(" "); Serial.print(third); }
      if (done == -2) { say_prog_cut(); return; }
      if (what == 'V') Serial.println(done ? " yes" : " no");
      else if (what == 'U') { if (done < 0) Serial.println(" none"); else { Serial.print(" "); Serial.println(done); } }
      else Serial.println(done ? " ok" : " unconfirmed");
      return;
    }
    case 'W': {
      long cv = read_number(c);
      long value = read_number(c);
      if (cv < 1 || cv > 1024) { Serial.println("ERR a setting (CV) is 1 to 1024"); return; }
      if (value < 0 || value > 255) { Serial.println("ERR a setting's value is 0 to 255"); return; }
      if (!shield) { Serial.print("W "); Serial.print(cv); Serial.println(" off there is no motor shield"); return; }
      if (power) Serial.println("OFF reading a setting: the main track's power is off while the programming track is on");
      int done = write_cv(cv, value);
      set_prog(false);
      Serial.print("W "); Serial.print(cv); Serial.print(" "); Serial.print(value);
      if (done == 1) Serial.println(" ok");
      else if (done == 0) {
        Serial.print(" unconfirmed (baseline ");
        Serial.print((long)(last_baseline * MA_PER_STEP));
        Serial.println(" mA)");
      }
      else say_prog_cut();
      return;
    }
    case 'R': {
      long cv = read_number(c);
      if (cv < 1 || cv > 1024) { Serial.println("ERR a setting (CV) is 1 to 1024"); return; }
      if (!shield) { Serial.print("CV "); Serial.print(cv); Serial.println(" off there is no motor shield"); return; }
      if (power) Serial.println("OFF reading a setting: the main track's power is off while the programming track is on");
      int value = read_cv(cv);
      set_prog(false);
      Serial.print("CV "); Serial.print(cv);
      if (value >= 0) { Serial.print(" "); Serial.println(value); }
      else if (value == -1) {
        uint16_t peak;
        noInterrupts(); peak = ack_peak; interrupts();
        Serial.print(" none (baseline ");
        Serial.print((long)(last_baseline * MA_PER_STEP));
        Serial.print(" mA, highest ");
        Serial.print((long)(peak * MA_PER_STEP));
        Serial.println(" mA)");
      }
      else say_prog_cut();
      return;
    }
    case 'I': {
      long pin = read_number(c);
      while (*c == ' ') c++;
      if (pin < 2 || pin >= PINS || own_pin(pin)) { Serial.println("ERR a sensor's pin is 2 to 69, but 3, 8, 9, 11, 12, 13, A0, A1, A4, A5 (the program's own)"); return; }
      if (*c == '-') { if (watched[pin]) { watched[pin] = false; pinMode(pin, INPUT); } Serial.println("OK"); return; }
      if (*c != '0' && *c != '1') { Serial.println("ERR I pin 1 (pull-up), I pin 0 (none), or I pin -"); return; }
      pinMode(pin, *c == '1' ? INPUT_PULLUP : INPUT);
      delayMicroseconds(50);
      said_level[pin] = new_level[pin] = digitalRead(pin);
      watched[pin] = true;
      Serial.println("OK");
      say_pin(pin);
      return;
    }
    case 'L': {
      long n = read_number(c);
      if (n < 10 || n > 5000) { Serial.println("ERR a current limit is 10 to 5000 mA"); return; }
      limit_ma = n;
      noInterrupts(); limit_steps = n / MA_PER_STEP; interrupts();
      Serial.println("OK");
      return;
    }
    case 'K': case 'D': {
      long place = read_number(c);
      if (place < 0 || place >= PLACES) { Serial.println("ERR a place is 0 to 31"); return; }
      Packet p = {{0}, 0};
      if (line[0] == 'K' && !read_packet(c, p)) return;
      noInterrupts(); table[place] = p; interrupts();
      Serial.println("OK");
      return;
    }
    case 'S': {
      Packet p;
      if (!read_packet(c, p)) return;
      uint8_t after = (waiting_head + 1) % WAITING;
      if (after == waiting_tail) { Serial.println("ERR too many packets waiting"); return; }
      noInterrupts();
      waiting[waiting_head] = p;
      waiting_head = after;
      interrupts();
      Serial.println("OK");
      return;
    }
  }
  Serial.println("ERR unknown: K, D, S, +, -, L, R, W, V, U, Y, B, I, X, ?, or .");
}

// Whether the shield is there: its sense resistor holds A0 low against the internal pull-up.
bool find_shield() {
  pinMode(SENSE_PIN, INPUT_PULLUP);
  delay(5);
  int with_pullup = analogRead(SENSE_PIN);
  pinMode(SENSE_PIN, INPUT);
  return with_pullup < 600;
}

void setup() {
  pinMode(POWER_PIN, OUTPUT);
  digitalWrite(POWER_PIN, LOW);
  pinMode(BRAKE_PIN, OUTPUT);
  digitalWrite(BRAKE_PIN, LOW);
  pinMode(FAULT_PIN, INPUT_PULLUP);
  pinMode(PROG_POWER_PIN, OUTPUT);
  digitalWrite(PROG_POWER_PIN, LOW);
  pinMode(PROG_BRAKE_PIN, OUTPUT);
  digitalWrite(PROG_BRAKE_PIN, LOW);
  pinMode(PROG_FAULT_PIN, INPUT_PULLUP);
  pinMode(PROG_SIGNAL_PIN, OUTPUT);
  digitalWrite(PROG_SIGNAL_PIN, LOW);
  pinMode(SIGNAL_PIN, OUTPUT);
  digitalWrite(SIGNAL_PIN, LOW);
  shield = find_shield();
  if (shield) {
    // The analog reader on A0 (ADC0), free-running, 16 MHz / 128 = 125 kHz: about 9,600 samples a
    // second, each one's interrupt watching the current.
    ADMUX = _BV(REFS0);                                               // AVcc (5 V), ADC0
    ADCSRB = 0;                                                       // free-running, ADC0..7
    ADCSRA = _BV(ADEN) | _BV(ADSC) | _BV(ADATE) | _BV(ADIE) | _BV(ADPS2) | _BV(ADPS1) | _BV(ADPS0);
  }
  going = IDLE;
  noInterrupts();
  TCCR1A = _BV(WGM11) | _BV(WGM10);                 // fast PWM, top OCR1A; OC1B joined when powered
  TCCR1B = _BV(WGM13) | _BV(WGM12) | _BV(CS11);     // 16 MHz / 8 = 2 MHz
  OCR1A = ONE_TOP; OCR1B = ONE_MIDDLE;
  TIMSK1 = _BV(TOIE1);
  interrupts();
  Serial.begin(115200);
  Serial.println("LEAH DCC BASE 7");
}

void loop() {
  static char line[48];
  static uint8_t used = 0;
  static uint32_t last_word = 0, last_said = 0;
  while (Serial.available()) {
    char c = Serial.read();
    if (c == '\r') continue;
    if (c == '\n') { line[used] = 0; if (used) { last_word = millis(); obey(line); } used = 0; }
    else if (used < sizeof line - 1) line[used++] = c;
  }
  watch_pins();
  if (power && millis() - last_word > QUIET_LIMIT) {
    power_off_because("no word from the PC for 3 seconds");
  }
  if (tripped) {
    uint8_t why;
    uint16_t steps;
    noInterrupts(); why = tripped; steps = tripped_steps; tripped = 0; interrupts();
    tripped_at = millis();
    if (tripped_at == 0) tripped_at = 1;
    if (why == 1) {
      Serial.print("OFF the current was over the limit (");
      Serial.print((long)(steps * MA_PER_STEP));
      Serial.print(" mA, the limit ");
      Serial.print(limit_ma);
      Serial.println(" mA): is something shorting the track?");
    } else {
      Serial.println("OFF the motor shield's driver reported a fault (too hot, a short it caught itself, or no supply in the shield's jack)");
    }
  }
  if (shield && power && millis() - last_said >= 500) {
    last_said = millis();
    uint32_t sum;
    uint16_t count;
    noInterrupts(); sum = sum_steps; count = sum_count; sum_steps = 0; sum_count = 0; interrupts();
    current_ma = count ? (float)sum / count * MA_PER_STEP : 0;
    Serial.print("A ");
    Serial.println((long)(current_ma + 0.5));
  }
}
