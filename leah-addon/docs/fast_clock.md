# fast_clock

## Description

A layout's own time of day, running faster than the real one, for timetables and operating sessions: at a rate of 4, four layout minutes pass in each real one, so a day's traffic fits an evening. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

It says each layout minute ([on_minute](#on_minute)) and the times listed in [alarms](#alarms) ([on_alarm](#on_alarm)). Phones show it when it is given to a [phone_throttle_server](ref:phone_throttle_server) ([set_clock](ref:phone_throttle_server#set_clock)), and a [track_diagram](ref:track_diagram) shows it with [add_clock](ref:track_diagram#add_clock). While it runs, a script without windows goes on running.

```leah
$clock = fast_clock(time: "06:58:30", rate: 60, alarms: ["07:00"])
$clock.on_minute = function($time) {
    print($time)
}
$clock.on_alarm = function($time) {
    global $clock
    print("The 07:00 local leaves")
    $clock.stop()
}
$clock.start()
```

Output:

```
06:59
07:00
The 07:00 local leaves
```

## Making one

**fast_clock**(time: "06:00", rate: 4, alarms: []) · fast_clock

It starts stopped: [start](#start) sets it running.

| Part | Description |
|---|---|
| time | Optional. The time of day it starts at: `"06:00"` or `"06:00:30"`. |
| rate | Optional. How much faster than real time it runs: 0.1 to 60. |
| alarms | Optional. A list of times of day [on_alarm](#on_alarm) says: `["07:15", "18:40"]`. |

## Properties

| Name | Type | Read-only |
|---|---|---|
| [alarms](#alarms) | array | |
| [hour](#hour) | number | ✓ |
| [minute](#minute) | number | ✓ |
| [rate](#rate) | number | |
| [running](#running) | boolean | |
| [time](#time) | text | |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [set_time](#set_time) | time | |
| [start](#start) | | |
| [stop](#stop) | | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_alarm](#on_alarm) | time | |
| [on_minute](#on_minute) | time | |

## Property descriptions

### alarms

**alarms** · array

The times of day [on_alarm](#on_alarm) says, as `"HH:MM"`.

---

### hour

**hour** · number · read-only

Its hour now, 0 to 23.

---

### minute

**minute** · number · read-only

Its minute now, 0 to 59.

---

### rate

**rate** · number

How much faster than real time it runs: 0.1 to 60.

---

### running

**running** · boolean

`true` while it runs; setting it starts or stops it.

---

### time

**time** · text

Its time of day now, `"HH:MM"`; setting it, `"07:30"` or `"07:30:15"`, moves it there.

## Function descriptions

### set_time

**set_time**(time)

Moves it to a time of day, `"07:30"`, running or not.

---

### start

**start**()

Sets it running from where it is.

---

### stop

**stop**()

Stops it where it is.

## Event descriptions

### on_alarm

**on_alarm**(time)

A time in [alarms](#alarms) came: `time` is it, `"07:15"`.

---

### on_minute

**on_minute**(time)

A layout minute passed: `time` is the new one, `"06:59"`.

## See also

[phone_throttle_server](ref:phone_throttle_server), [track_diagram](ref:track_diagram)
