// Leah's DCC base station, the safe ladder's step 2: the NMRA S-9.1 signal as 5 V logic on pin 12
// (Timer1's output OC1B, so every edge comes from the hardware timer), with no motor shield.
//
// A 1 is 58 us high then 58 us low; a 0 is 100 us high then 100 us low. Timer1 runs at 2 MHz
// (0.5 us ticks) in fast PWM with its top in OCR1A: each period is one bit. At the end of each bit the
// overflow interrupt sets the next bit's top and middle, which the timer takes at its next top.
//
// Over USB (115200 baud), one line at a time:
//   P 03 3F A9      the packet to repeat (the error byte is added here), between idle packets
//   I               idle packets only
// It answers each line with "OK ..." or "ERR ...", and starts with "LEAH DCC BASE 1".

#include <Arduino.h>

const uint16_t ONE_TOP = 231, ONE_MIDDLE = 115;    // 232 ticks = 116 us, high for 116 ticks = 58 us
const uint16_t ZERO_TOP = 399, ZERO_MIDDLE = 199;  // 400 ticks = 200 us, high for 200 ticks = 100 us
const uint8_t MAX_BYTES = 6, MAX_BITS = 14 + MAX_BYTES * 9 + 1;

struct Packet { uint8_t bits[MAX_BITS]; uint8_t length; };

Packet idle_packet, commands[2];
volatile uint8_t command = 0;        // which of the two command buffers is in use
volatile bool have_command = false;
volatile const Packet *current = &idle_packet;
volatile uint8_t position = 0;
volatile bool next_is_command = false;

void build(Packet &p, const uint8_t *bytes, uint8_t count) {
  uint8_t n = 0, check = 0;
  for (uint8_t i = 0; i < 14; i++) p.bits[n++] = 1;              // the preamble
  for (uint8_t i = 0; i <= count; i++) {
    uint8_t b = i < count ? bytes[i] : check;                     // the error byte last
    if (i < count) check ^= b;
    p.bits[n++] = 0;                                              // the start bit before each byte
    for (int8_t k = 7; k >= 0; k--) p.bits[n++] = (b >> k) & 1;
  }
  p.bits[n++] = 1;                                                // the packet's end bit
  p.length = n;
}

ISR(TIMER1_OVF_vect) {
  uint8_t bit = current->bits[position++];
  if (position >= current->length) {
    position = 0;
    if (next_is_command && have_command) current = &commands[command];
    else current = &idle_packet;
    next_is_command = !next_is_command;
  }
  if (bit) { OCR1A = ONE_TOP; OCR1B = ONE_MIDDLE; } else { OCR1A = ZERO_TOP; OCR1B = ZERO_MIDDLE; }
}

int hex_digit(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}

void answer(const char *word, const uint8_t *bytes, uint8_t count) {
  Serial.print(word);
  uint8_t check = 0;
  for (uint8_t i = 0; i <= count; i++) {
    uint8_t b = i < count ? bytes[i] : check;
    if (i < count) check ^= b;
    Serial.print(b < 16 ? " 0" : " "); Serial.print(b, HEX);
  }
  Serial.println();
}

void obey(char *line) {
  if (line[0] == 'I' && line[1] == 0) { have_command = false; Serial.println("OK idle"); return; }
  if (line[0] != 'P') { Serial.println("ERR say P and the packet's bytes in hex, or I"); return; }
  uint8_t bytes[MAX_BYTES], count = 0;
  for (char *c = line + 1; *c; ) {
    if (*c == ' ') { c++; continue; }
    int high = hex_digit(c[0]), low = c[1] ? hex_digit(c[1]) : -1;
    if (high < 0 || low < 0) { Serial.println("ERR each byte is two hex digits"); return; }
    if (count == MAX_BYTES - 1) { Serial.println("ERR at most 5 bytes (the error byte is added)"); return; }
    bytes[count++] = high * 16 + low;
    c += 2;
  }
  if (count < 2) { Serial.println("ERR a packet has at least 2 bytes"); return; }
  uint8_t spare = 1 - command;
  build(commands[spare], bytes, count);
  noInterrupts(); command = spare; have_command = true; interrupts();
  answer("OK", bytes, count);
}

void setup() {
  const uint8_t idle[] = {0xFF, 0x00};
  build(idle_packet, idle, 2);
  pinMode(12, OUTPUT);
  noInterrupts();
  TCCR1A = _BV(COM1B1) | _BV(WGM11) | _BV(WGM10);   // fast PWM, top OCR1A, OC1B high until the middle
  TCCR1B = _BV(WGM13) | _BV(WGM12) | _BV(CS11);     // 16 MHz / 8 = 2 MHz
  OCR1A = ONE_TOP; OCR1B = ONE_MIDDLE;
  TIMSK1 = _BV(TOIE1);
  interrupts();
  Serial.begin(115200);
  Serial.println("LEAH DCC BASE 1");
}

void loop() {
  static char line[40];
  static uint8_t used = 0;
  while (Serial.available()) {
    char c = Serial.read();
    if (c == '\r') continue;
    if (c == '\n') { line[used] = 0; if (used) obey(line); used = 0; }
    else if (used < sizeof line - 1) line[used++] = c;
  }
}
