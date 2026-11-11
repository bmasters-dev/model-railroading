# phone_throttle_server

## Description

A server that lets phone throttle apps drive the locomotives on a [dcc_base](ref:dcc_base) over the local network: apps that work with WiThrottle, such as Engine Driver on Android and WiThrottle on iPhone. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

A phone takes a locomotive by its address, or by its name from a [roster](ref:roster) (its keys then labelled as the roster entry says, and its momentary keys, such as a horn, on only while pressed). One phone can run several locomotives at once, on several throttles. The phones also switch the track power, throw the script's turnouts, set its routes, and show its fast clock. What one phone changes, every other phone holding that locomotive sees, and so does the script: a phone drives the base station's own [locomotive](ref:locomotive) at that address when the script has one.

The server is announced on the local network, so the phones list it by its name; a phone can also be given the computer's address and the [port](#port). It serves until [close](#close) is called or the program ends, whether or not the script still holds it, so a script with no windows that only starts one goes on serving, and one the script let go of never leaves phones driving with nothing to stop them.

For safety, a phone that asked for a heartbeat and then goes quiet for longer than [heartbeat](#heartbeat) seconds and a little more (half of them, at least two seconds, so one saying it right on time over a slow Wi-Fi is never stopped) has its locomotives stopped at once, and is told why; a phone that disconnects has its locomotives stopped, eased by their decoders. A locomotive a phone lets go of runs on as it was, and is the same one when a phone takes it again.

```leah check
$base = dcc_base("/dev/ttyACM0")
$server = phone_throttle_server(name: "My Railroad", roster: roster())
$server.add_turnout("T1", label: "Yard lead")
$server.on_turnout_request = function($name, $state) {
    print("throw " + $name + " " + $state)
}
```

## Making one

**phone_throttle_server**(port: 12090, name: "Leah", base: ..., roster: ..., heartbeat: 10, announce: true) · phone_throttle_server

| Part | Description |
|---|---|
| port | Optional. The network port it listens on: 12090, the usual one for these servers, which most apps try first; `0` for any free one (the [port](#port) property gives it). One in use is a `network_error`. |
| name | Optional. Its name, as the phones list it: `"My Railroad"` (`"Leah"` when not given). |
| base | Optional. The [dcc_base](ref:dcc_base) the phones drive; by default the one made last. With none made, it is a `hardware_error`. |
| roster | Optional. A [roster](ref:roster): the phones list its entries, and take a locomotive by its entry's name, with its labels, momentary keys, speed steps, and speed limit. Without one, the phones take locomotives by their addresses only. |
| heartbeat | Optional. The seconds a phone that asked for a heartbeat may go quiet before its locomotives are stopped, 1 to 3600; `0` for none. 10 by default. |
| announce | Optional. `true` (the default) to announce it on the local network, so phones list it by its name; `false` for a phone given the address and port by hand. |

Starting it needs Leah 0.1.0-alpha.50 or later; an older Leah says so (an `api_error`).

## Properties

| Name | Type | Read-only |
|---|---|---|
| [base](#base) | dcc_base | ✓ |
| [clients](#clients) | array | ✓ |
| [clock](#clock) | array | ✓ |
| [heartbeat](#heartbeat) | number | |
| [name](#name) | text | ✓ |
| [port](#port) | number | ✓ |
| [roster](#roster) | roster | ✓ |
| [routes](#routes) | array | ✓ |
| [running](#running) | boolean | ✓ |
| [turnouts](#turnouts) | array | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [add_route](#add_route) | name as text, label: text | |
| [add_turnout](#add_turnout) | name as text, label: text, state: text | |
| [close](#close) | | |
| [message](#message) | text, alert: boolean | |
| [remove_route](#remove_route) | name as text | |
| [remove_turnout](#remove_turnout) | name as text | |
| [set_clock](#set_clock) | time as text, rate: number | |
| [set_route](#set_route) | name as text, state as text | |
| [set_turnout](#set_turnout) | name as text, state as text | |
| [stop_clock](#stop_clock) | | |

## Events

| Name | Parameters | Returns |
|---|---|---|
| [on_client](#on_client) | name | |
| [on_client_left](#on_client_left) | name | |
| [on_route_request](#on_route_request) | name | |
| [on_turnout_request](#on_turnout_request) | name, state | |

## Property descriptions

### base

**base** · dcc_base · read-only

The base station the phones drive.

---

### clients

**clients** · array · read-only

The phones connected, each an array of `name` (as the phone calls itself, such as "John's Phone"), `device` (the phone's own id), `address` (its network address), and `locomotives` (how many it holds).

---

### clock

**clock** · array · read-only

The fast clock, as [set_clock](#set_clock) set it: `time` (its time now, `"06:12:30"`) and `rate` (0 when stopped); `null` when none was set.

---

### heartbeat

**heartbeat** · number

The seconds a phone that asked for a heartbeat may go quiet before its locomotives are stopped (0: never; a little more is allowed, as above). Phones are told it as they connect, and again when it is set.

---

### name

**name** · text · read-only

Its name, as the phones list it.

---

### port

**port** · number · read-only

The port it listens on: the one given, or the free one chosen for `port: 0`.

---

### roster

**roster** · roster · read-only

The roster the phones list, or `null`.

---

### routes

**routes** · array · read-only

Its routes, each an array of `name`, `label`, and `state` (`"active"` or `"inactive"`).

---

### running

**running** · boolean · read-only

`true` until [close](#close).

---

### turnouts

**turnouts** · array · read-only

Its turnouts, each an array of `name`, `label`, and `state` (`"closed"`, `"thrown"`, or `"unknown"`).

## Function descriptions

### add_route

**add_route**(name, label: name)

Adds a route the phones can set: `name` is how they ask for it, `label` what they show. It starts inactive. The phones connected are told. A name it has already is a `range_error`.

---

### add_turnout

**add_turnout**(name, label: name, state: "closed")

Adds a turnout the phones can throw: `name` is how they ask for it (`"T1"`), `label` what they show (`"Yard lead"`), `state` `"closed"`, `"thrown"`, or `"unknown"`. The phones connected are told. A name it has already is a `range_error`.

---

### close

**close**()

Stops serving: each phone connected is let go of (its locomotives stopped, eased; [on_client_left](#on_client_left) runs for it), and the server is no longer announced.

---

### message

**message**(text, alert: false)

Shows words on every phone connected: as information, or with `alert: true` as an alert.

---

### remove_route

**remove_route**(name)

Takes a route away; the phones connected are told. One it doesn't have is a `range_error`.

---

### remove_turnout

**remove_turnout**(name)

Takes a turnout away; the phones connected are told. One it doesn't have is a `range_error`.

---

### set_clock

**set_clock**(time, rate: 1)

Starts the fast clock the phones show, at a time of day (`"06:00"` or `"06:00:30"`), running `rate` times as fast as a real clock (0.1 to 60; 4 for four minutes to each real one). Every phone is told at once, and again at each fast minute.

```leah check
$server.set_clock("06:00", rate: 4)
```

---

### set_route

**set_route**(name, state)

Sets a route `"active"` or `"inactive"`, and tells every phone. One it doesn't have is a `range_error`.

---

### set_turnout

**set_turnout**(name, state)

Sets a turnout's state, `"closed"`, `"thrown"`, or `"unknown"` (one the script threw itself, or a turnout the script found was thrown), and tells every phone. One it doesn't have is a `range_error`.

---

### stop_clock

**stop_clock**()

Stops the fast clock where it is; the phones show it stopped.

## Event descriptions

### on_client

**on_client**(name)

A phone connected and said its name.

---

### on_client_left

**on_client_left**(name)

A phone went: it closed its connection, or the connection broke. Its locomotives were stopped, eased by their decoders.

---

### on_route_request

**on_route_request**(name)

A phone set a route: the route shows active on every phone already, and the script sets the turnouts it holds.

---

### on_turnout_request

**on_turnout_request**(name, state)

A phone threw a turnout: `state` is `"closed"` or `"thrown"`. The turnout shows its new state on every phone already, and the script throws the real one (an accessory decoder, a servo, a relay); it can call [set_turnout](#set_turnout) to show otherwise.

```leah check
$server.on_turnout_request = function($name, $state) {
    print($name + " to " + $state)
}
```

## See also

[dcc_base](ref:dcc_base), [locomotive](ref:locomotive), [roster](ref:roster), [Model railroading](ref:Model railroading)
