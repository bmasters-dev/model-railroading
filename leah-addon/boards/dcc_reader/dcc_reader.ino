// Leah's DCC reader, the safe ladder's step 2: reads the base station's signal on pin 8 (5 V logic,
// through a 1 kOhm resistor, grounds joined) and tells the PC what it heard, as a decoder would.
//
// Every edge's time is taken by Timer1's input capture on pin 8 (ICP1), in hardware, to 0.5 us:
// nothing the Uno is busy with moves it (taking the time in an interrupt on pin 2 was off by up to
// 6 us whenever the Uno was sending to the PC, 2026-11-10). Each half bit is judged by NMRA S-9.1's
// decoder limits: 52 to 64 us is half of a 1, 90 to 10000 us half of a 0, anything else is bad. Two
// matching halves make a bit; packets are a preamble of at least 10 ones, then bytes each after a 0,
// ended by a 1, the last byte the others XOR-ed.
//
// Over USB (115200 baud): "P 03 3F A9 95" for each packet heard (each one at most every 2 seconds), and every second
// "T one 57.5-58.5 zero 99.5-100.5 packets 152 bad 0 errors 0" (the halves' shortest and longest in us,
// the packets heard, halves outside the limits, packets whose error byte was wrong). Starts with
// "LEAH DCC READER 1".

#include <Arduino.h>

volatile uint16_t ring[128];
volatile uint8_t head = 0;
uint8_t tail = 0;
volatile uint16_t last_edge = 0;

ISR(TIMER1_CAPT_vect) {
  uint16_t now = ICR1;
  TCCR1B ^= _BV(ICES1);                 // the other edge next
  ring[head] = now - last_edge;
  last_edge = now;
  head = (head + 1) & 127;
}

uint16_t one_min = 0xFFFF, one_max = 0, zero_min = 0xFFFF, zero_max = 0;
uint32_t packets = 0, bad = 0, errors = 0;
int8_t pending = -1;
enum { PREAMBLE, BYTE, AFTER_BYTE } state = PREAMBLE;
uint8_t ones = 0, bit_count = 0, current_byte = 0, bytes[8], count = 0;
struct Heard { uint8_t bytes[8]; uint8_t count; uint32_t time; };
Heard heard[6];                         // the packets printed lately, so each is printed at most every 2 s

void packet_done() {
  uint8_t check = 0;
  for (uint8_t i = 0; i < count; i++) check ^= bytes[i];
  if (check != 0 || count < 3) { errors++; return; }
  packets++;
  uint32_t now = millis();
  uint8_t oldest = 0;
  for (uint8_t h = 0; h < 6; h++) {
    bool same = heard[h].count == count;
    for (uint8_t i = 0; same && i < count; i++) same = heard[h].bytes[i] == bytes[i];
    if (same) { if (now - heard[h].time < 2000) return; oldest = h; break; }
    if (heard[h].time < heard[oldest].time) oldest = h;
  }
  memcpy(heard[oldest].bytes, bytes, count); heard[oldest].count = count; heard[oldest].time = now;
  Serial.print("P");
  for (uint8_t i = 0; i < count; i++) { Serial.print(bytes[i] < 16 ? " 0" : " "); Serial.print(bytes[i], HEX); }
  Serial.println();
}

void take_bit(uint8_t b) {
  switch (state) {
    case PREAMBLE:
      if (b) { if (ones < 255) ones++; }
      else if (ones >= 10) { state = BYTE; bit_count = 0; current_byte = 0; count = 0; }
      else ones = 0;
      break;
    case BYTE:
      current_byte = (current_byte << 1) | b;
      if (++bit_count == 8) { if (count < sizeof bytes) bytes[count++] = current_byte; state = AFTER_BYTE; }
      break;
    case AFTER_BYTE:
      if (b) { packet_done(); state = PREAMBLE; ones = 1; }
      else { state = BYTE; bit_count = 0; current_byte = 0; }
      break;
  }
}

void take_half(uint16_t ticks) {
  int8_t kind;
  if (ticks >= 104 && ticks <= 128) { kind = 1; if (ticks < one_min) one_min = ticks; if (ticks > one_max) one_max = ticks; }
  else if (ticks >= 180 && ticks <= 20000) { kind = 0; if (ticks < zero_min) zero_min = ticks; if (ticks > zero_max) zero_max = ticks; }
  else { bad++; pending = -1; state = PREAMBLE; ones = 0; return; }
  if (pending == kind) { take_bit(kind); pending = -1; } else pending = kind;
}

void print_us(uint16_t ticks) { Serial.print(ticks / 2); Serial.print(ticks & 1 ? ".5" : ".0"); }

void setup() {
  Serial.begin(115200);
  pinMode(8, INPUT);
  noInterrupts();
  TCCR1A = 0;
  TCCR1B = _BV(ICNC1) | _BV(ICES1) | _BV(CS11);   // 2 MHz, noise canceller, a rising edge first
  TIMSK1 = _BV(ICIE1);
  interrupts();
  Serial.println("LEAH DCC READER 1");
}

void loop() {
  while (tail != head) { take_half(ring[tail]); tail = (tail + 1) & 127; }
  static uint32_t last = 0;
  if (millis() - last >= 1000) {
    last = millis();
    Serial.print("T one ");
    if (one_max) { print_us(one_min); Serial.print("-"); print_us(one_max); } else Serial.print("none");
    Serial.print(" zero ");
    if (zero_max) { print_us(zero_min); Serial.print("-"); print_us(zero_max); } else Serial.print("none");
    Serial.print(" packets "); Serial.print(packets);
    Serial.print(" bad "); Serial.print(bad);
    Serial.print(" errors "); Serial.println(errors);
    one_min = zero_min = 0xFFFF; one_max = zero_max = 0; packets = bad = errors = 0;
  }
}
