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
// It answers "OK" or "ERR ..." to every line but ".", and starts with "LEAH DCC BASE 3". With the
// shield and the power on, it says the current every half second: "A 25" (mA).
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
const float MA_PER_STEP = 5.08;

struct Packet { uint8_t bytes[MOST_BYTES]; uint8_t count; };

const Packet IDLE = {{0xFF, 0x00, 0xFF}, 3};
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
  if (next_bit()) { OCR1A = ONE_TOP; OCR1B = ONE_MIDDLE; } else { OCR1A = ZERO_TOP; OCR1B = ZERO_MIDDLE; }
}

// Cuts the power from inside an interrupt (interrupts are off already).
void cut_power_now(uint8_t why) {
  PORTE &= ~_BV(PE5);                                                 // pin 3 low: the shield's output off
  TCCR1A &= ~_BV(COM1B1);
  PORTB &= ~_BV(PB6);                                                 // pin 12 low
  power = false;
  tripped = why;
}

// The analog reader, running freely on A0: each sample the track's current.
ISR(ADC_vect) {
  uint16_t steps = ADC;
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
      set_power(false);
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
  Serial.println("ERR unknown: K, D, S, +, -, L, X, ?, or .");
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
  Serial.println("LEAH DCC BASE 3");
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
