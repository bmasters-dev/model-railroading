# dcc_program_job

## Description

A program being loaded onto a board in the background, as [load_dcc_program](ref:DCC board functions#load_dcc_program) gives it back: it says how it went with its [on_finished](#on_finished) event, and can be waited for with [wait](#wait). Keep it in a variable (or a list) until it has finished, or its event has no one to tell. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)), and isn't shown.

```leah check
$job = load_dcc_program("/dev/ttyUSB0", "reader")
if $job.wait() {
    print("ready")
} else {
    print($job.message)
}
```

## Properties

| Name | Type | Read-only |
|---|---|---|
| [board](#board) | text | ✓ |
| [finished](#finished) | boolean | ✓ |
| [message](#message) | text | ✓ |
| [port](#port) | text | ✓ |
| [role](#role) | text | ✓ |
| [succeeded](#succeeded) | boolean | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [wait](#wait) | | boolean |

## Events

| Name | Parameters |
|---|---|
| [on_finished](#on_finished) | succeeded as boolean, message as text |

## Property descriptions

### board

**board** · text · read-only

arduino-cli's name for the board it is loading onto: `"arduino:avr:mega"`.

---

### finished

**finished** · boolean · read-only

`true` once loading is over, whether it worked or not.

---

### message

**message** · text · read-only

How it went, in words: `"Leah's base station program is on the board on /dev/ttyACM0 (4376 of the board's 253952 bytes)"`, or what went wrong and what to do (`"the board on /dev/ttyUSB0 didn't answer the uploader: is it the board you named (arduino:avr:uno)? ..."`). `null` until it has finished.

---

### port

**port** · text · read-only

The board's port.

---

### role

**role** · text · read-only

The program being loaded: `"base"` or `"reader"`.

---

### succeeded

**succeeded** · boolean · read-only

`true` once the program is on the board; `false` until then, and when it failed.

## Function descriptions

### wait

**wait**() · boolean

Waits until loading is over, and gives back whether it worked. For a script without windows: in a window's code, waiting freezes the window, so use [on_finished](#on_finished) there.

## Event descriptions

### on_finished

**on_finished**(succeeded, message)

Loading is over: `succeeded` is whether the program is on the board, and `message` says how it went, as [message](#message) does.

## See also

[DCC board functions](ref:DCC board functions), [dcc_base](ref:dcc_base), [Model railroading](ref:Model railroading)
