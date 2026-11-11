#!/usr/bin/env python3
"""Stand-ins for Leah's two DCC boards, for the addon's tests (tests/scripts.rs): no hardware.

Each board is a pseudo-terminal whose far end this program plays, speaking as the board's program
(boards/dcc_base/dcc_base.ino, boards/dcc_reader/dcc_reader.ino) does: the base station answers
its lines (K, D, S, +, -, X, ?, .), keeps its refresh table, and turns the power off after 3 seconds
without a line; while its power is on, what it sends reaches the reader, which says each packet
("P ...", each at most every 2 seconds) and its timing every second ("T ...").

    python3 fake_boards.py [--base normal|silent|old|reader] [--log FILE] [--cut-after SECONDS]
                           [--drop-reader-after SECONDS] [--drop-base-after SECONDS]
                           [--shield] [--current MILLIAMPS]

It prints the two ports' paths ("BASE /dev/pts/5" and "READER /dev/pts/6") and runs until its
standard input closes. `--base silent` is a board that says nothing (not running Leah's program);
`old` one running an older program of Leah's. `--log` writes every line the base station was sent.
`reader` a board running the reader's program where the base station was meant to be (the ports
the wrong way round). `--cut-after` turns the power off that long after it was turned on, as the
board's safety does when the PC goes quiet. `--drop-reader-after` and `--drop-base-after` close that
board's port that long after the base station first heard from the PC, as a pulled cable does.
`--shield` is a base station with the motor shield on it, its track drawing `--current` mA (0 if not
given) while the power is on: it says the current every half second, and turns the power off when
it is over the limit, as the board does.
"""

import os
import select
import sys
import time
import tty


def check_ok(packet):
    x = 0
    for b in packet:
        x ^= b
    return len(packet) >= 3 and x == 0


def main():
    mode = "normal"
    log_path = None
    cut_after = None
    powered_at = 0.0
    drop_reader_after = None
    drop_base_after = None
    first_word = None
    shield = False
    drawing = 0
    limit = 1500
    last_a = 0.0
    args = sys.argv[1:]
    while args:
        a = args.pop(0)
        if a == "--base":
            mode = args.pop(0)
        elif a == "--log":
            log_path = args.pop(0)
        elif a == "--cut-after":
            cut_after = float(args.pop(0))
        elif a == "--drop-reader-after":
            drop_reader_after = float(args.pop(0))
        elif a == "--drop-base-after":
            drop_base_after = float(args.pop(0))
        elif a == "--shield":
            shield = True
        elif a == "--current":
            drawing = int(args.pop(0))
    log = open(log_path, "a", buffering=1) if log_path else None

    base_master, base_slave = os.openpty()
    reader_master, reader_slave = os.openpty()
    for fd in (base_slave, reader_slave):
        tty.setraw(fd)
    print("BASE", os.ttyname(base_slave), flush=True)
    print("READER", os.ttyname(reader_slave), flush=True)

    table = {}
    waiting = []
    power = False
    last_word = time.time()
    partial = b""
    last_printed = {}
    last_t = time.time()
    last_old_hello = 0.0
    packets_this_second = 0
    turn = 0

    def say(fd, text):
        if fd < 0:
            return
        try:
            os.write(fd, (text + "\r\n").encode())
        except OSError:
            pass

    def heard(packet):
        nonlocal packets_this_second
        packets_this_second += 1
        key = bytes(packet)
        now = time.time()
        if now - last_printed.get(key, 0) >= 2:
            last_printed[key] = now
            say(reader_master, "P " + " ".join(f"{b:02X}" for b in packet))

    while True:
        ready, _, _ = select.select([sys.stdin] + ([base_master] if base_master >= 0 else []), [], [], 0.05)
        if sys.stdin in ready and not sys.stdin.readline():
            return
        if base_master in ready:
            try:
                partial += os.read(base_master, 1024)
            except OSError:
                partial = b""
            while b"\n" in partial:
                raw, partial = partial.split(b"\n", 1)
                line = raw.decode(errors="replace").strip()
                if not line:
                    continue
                if log:
                    log.write(line + "\n")
                if first_word is None:
                    first_word = time.time()
                if mode in ("silent", "reader"):
                    continue
                if mode == "old":
                    continue
                last_word = time.time()
                c = line[0]
                if c == ".":
                    continue
                if c == "+":
                    power = True
                    powered_at = time.time()
                elif c == "-":
                    power = False
                elif c == "X":
                    power = False
                    table.clear()
                    waiting.clear()
                elif c == "?":
                    now_ma = drawing if shield and power else 0
                    say(base_master, f"STATE power {'on' if power else 'off'} places {len(table)} shield {'yes' if shield else 'no'} current {now_ma} limit {limit}")
                    continue
                elif c == "L":
                    n = int(line[1:].split()[0])
                    if not 10 <= n <= 5000:
                        say(base_master, "ERR a current limit is 10 to 5000 mA")
                        continue
                    limit = n
                elif c in "KDS":
                    words = line[1:].split()
                    if c in "KD":
                        place = int(words.pop(0))
                    packet = [int(w, 16) for w in words]
                    if c != "D" and not check_ok(packet):
                        say(base_master, "ERR the error byte doesn't match the packet")
                        continue
                    if c == "K":
                        table[place] = packet
                    elif c == "D":
                        table.pop(place, None)
                    else:
                        waiting.append(packet)
                else:
                    say(base_master, "ERR unknown: K, D, S, +, -, X, ?, or .")
                    continue
                say(base_master, "OK")
        now = time.time()
        if mode in ("old", "reader") and now - last_old_hello > 0.5:
            last_old_hello = now
            say(base_master, "LEAH DCC BASE 2" if mode == "old" else "LEAH DCC READER 1")
        if first_word is not None and drop_reader_after is not None and now - first_word > drop_reader_after and reader_master >= 0:
            os.close(reader_slave)
            os.close(reader_master)
            reader_master = -1
        if first_word is not None and drop_base_after is not None and now - first_word > drop_base_after and base_master >= 0:
            os.close(base_slave)
            os.close(base_master)
            base_master = -1
            power = False
        if power and shield and drawing > limit:
            power = False
            say(base_master, f"OFF the current was over the limit ({drawing} mA, the limit {limit} mA): is something shorting the track?")
        if power and shield and now - last_a >= 0.5:
            last_a = now
            say(base_master, f"A {drawing}")
        if power and (now - last_word > 3 or (cut_after is not None and now - powered_at > cut_after)):
            power = False
            say(base_master, "OFF no word from the PC for 3 seconds")
        if power:
            # (What the board sends: the waiting packets first, then the table in turn, idle between.)
            heard([0xFF, 0x00, 0xFF])
            if waiting:
                heard(waiting.pop(0))
            elif table:
                places = sorted(table)
                heard(table[places[turn % len(places)]])
                turn += 1
        if now - last_t >= 1:
            last_t = now
            if power:
                say(reader_master, f"T one 57.5-58.0 zero 99.5-100.0 packets {packets_this_second} bad 0 errors 0")
            else:
                say(reader_master, "T one none zero none packets 0 bad 0 errors 0")
            packets_this_second = 0


if __name__ == "__main__":
    main()
