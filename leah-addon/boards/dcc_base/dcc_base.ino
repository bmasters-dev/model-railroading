// Leah's DCC base station (design/model_railroading.md, "Stage 1b"): the NMRA S-9.1 signal on pin 12
// (Timer1's output OC1B, so every edge comes from the hardware timer). In the safe ladder's step 2 it is
// 5 V logic only, with no motor shield.
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
//   +   -              track power on, off (off: pin 12 held low, nothing sent)
//   X                  everything dropped, power off
//   ?                  "STATE power on places 2"
//   .                  nothing: only that the PC is still there
// It answers "OK" or "ERR ..." to every line but ".", and starts with "LEAH DCC BASE 2".
//
// Safety: with the power on, three seconds without a line from the PC (the program on the PC ended,
// crashed, or lost the cable) turn the power off, and it says "OFF no word from the PC for 3 seconds".

#include <Arduino.h>

const uint16_t ONE_TOP = 231, ONE_MIDDLE = 115;    // 232 ticks = 116 us, high for 116 ticks = 58 us
const uint16_t ZERO_TOP = 399, ZERO_MIDDLE = 199;  // 400 ticks = 200 us, high for 200 ticks = 100 us
const uint8_t MOST_BYTES = 6, PLACES = 32, WAITING = 8, PREAMBLE = 14, TIMES_NOW = 3;
const uint32_t QUIET_LIMIT = 3000;

struct Packet { uint8_t bytes[MOST_BYTES]; uint8_t count; };

const Packet IDLE = {{0xFF, 0x00, 0xFF}, 3};
Packet table[PLACES];                // count 0: the place is free
Packet waiting[WAITING];             // the packets to send now, a ring
volatile uint8_t waiting_head = 0, waiting_tail = 0;
volatile bool power = false;

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

void set_power(bool on) {
  noInterrupts();
  power = on;
  if (on) TCCR1A |= _BV(COM1B1); else { TCCR1A &= ~_BV(COM1B1); digitalWrite(12, LOW); }
  interrupts();
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

// A place's number from the text (moving past it); -1 when it isn't one.
int read_place(const char *&c) {
  while (*c == ' ') c++;
  int n = 0, digits = 0;
  while (*c >= '0' && *c <= '9') { n = n * 10 + (*c - '0'); c++; digits++; }
  return digits && n < PLACES ? n : -1;
}

void obey(const char *line) {
  const char *c = line + 1;
  switch (line[0]) {
    case '.': return;
    case '+': set_power(true); Serial.println("OK"); return;
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
      Serial.print(" places "); Serial.println(used);
      return;
    }
    case 'K': case 'D': {
      int place = read_place(c);
      if (place < 0) { Serial.println("ERR a place is 0 to 31"); return; }
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
      waiting[waiting_head] = p;
      waiting_head = after;
      Serial.println("OK");
      return;
    }
  }
  Serial.println("ERR unknown: K, D, S, +, -, X, ?, or .");
}

void setup() {
  pinMode(12, OUTPUT);
  digitalWrite(12, LOW);
  going = IDLE;
  noInterrupts();
  TCCR1A = _BV(WGM11) | _BV(WGM10);                 // fast PWM, top OCR1A; OC1B joined when powered
  TCCR1B = _BV(WGM13) | _BV(WGM12) | _BV(CS11);     // 16 MHz / 8 = 2 MHz
  OCR1A = ONE_TOP; OCR1B = ONE_MIDDLE;
  TIMSK1 = _BV(TOIE1);
  interrupts();
  Serial.begin(115200);
  Serial.println("LEAH DCC BASE 2");
}

void loop() {
  static char line[48];
  static uint8_t used = 0;
  static uint32_t last_word = 0;
  while (Serial.available()) {
    char c = Serial.read();
    if (c == '\r') continue;
    if (c == '\n') { line[used] = 0; if (used) { last_word = millis(); obey(line); } used = 0; }
    else if (used < sizeof line - 1) line[used++] = c;
  }
  if (power && millis() - last_word > QUIET_LIMIT) {
    set_power(false);
    Serial.println("OFF no word from the PC for 3 seconds");
  }
}
