# decoder_programmer

## Description

Reads and writes a whole decoder by its settings' names, as a [decoder_definition](ref:decoder_definition) gives them: everything at once, a few settings, or only what was changed. It keeps each setting's value and its **state**, which says whether that value can be trusted. It comes with the **model_railroading** addon ([Model railroading](ref:Model railroading)).

It works in one of two places:

- **On the programming track** (made with a [dcc_base](ref:dcc_base)): it reads and writes, each reading and writing in the background, with [on_progress](#on_progress) after each CV and [on_done](#on_done) at the end, so a window never waits. As for [read_cv](ref:dcc_base#read_cv), the main track's power is off meanwhile, and only Leah's base station board with its motor shield can do it.
- **On the main** (made with a [locomotive](ref:locomotive)): it can only write, to that locomotive's address, each CV as [locomotive.write_cv](ref:locomotive#write_cv) writes it, half a second apart. The writes go out one after another before the function gives back, then the events come.

| State | The value |
|---|---|
| `"unknown"` | isn't known: not read yet, or the decoder didn't answer for it |
| `"read"` | was read from the decoder |
| `"written"` | was written to the decoder |
| `"edited"` | was changed by the script ([set_value](#set_value)) and not written yet |
| `"loaded"` | came from [load_cvs](#load_cvs) and isn't written yet |

```leah check
$base = dcc_base("/dev/ttyACM0")
$p = decoder_programmer(decoder_definition("nmra"), $base)
$p.on_done = function($what, $error) {
    global $p
    if $what == "read_all" {
        print("address " + $p.value("Short address") + ", acceleration " + $p.value("Acceleration"))
        $p.set_value("Acceleration", 20)
        $p.write_changes()
    }
}
$p.read_all()
```

## Making one

**decoder_programmer**(definition, base) · decoder_programmer

**decoder_programmer**(definition, locomotive) · decoder_programmer

`definition` is a [decoder_definition](ref:decoder_definition); the second value is a [dcc_base](ref:dcc_base), for its programming track, or a [locomotive](ref:locomotive), to write to it on the main. Anything else is a `type_error`.

## Properties

| Name | Type | Read-only |
|---|---|---|
| [busy](#busy) | boolean | ✓ |
| [cvs](#cvs) | array | ✓ |
| [definition](#definition) | decoder_definition | ✓ |
| [main_track](#main_track) | boolean | ✓ |
| [problems](#problems) | array | ✓ |
| [states](#states) | array | ✓ |
| [values](#values) | array | ✓ |

## Functions

| Name | Parameters | Returns |
|---|---|---|
| [cancel](#cancel) | | |
| [load_cvs](#load_cvs) | cvs as array | |
| [read](#read) | names, tries: number | |
| [read_all](#read_all) | tries: number | |
| [set_value](#set_value) | name as text, value | |
| [state](#state) | name as text | text |
| [value](#value) | name as text | any value |
| [write_all](#write_all) | | |
| [write_changes](#write_changes) | | |

## Events

| Name | Parameters |
|---|---|
| [on_done](#on_done) | what, error |
| [on_progress](#on_progress) | done, total, cv |

## Property descriptions

### busy

**busy** · boolean

`true` while it reads or writes. One reading or writing at a time: starting another meanwhile is a `programming_error`.

---

### cvs

**cvs** · array

Every CV's value it knows, by number: what was read or written, with the changes and loaded values not yet written over them.

---

### definition

**definition** · decoder_definition

The definition it was made with.

---

### main_track

**main_track** · boolean

`true` when it was made with a locomotive, to write on the main.

---

### problems

**problems** · array

The CVs the last reading couldn't read, each with why, by number: `[128: "the decoder didn't answer for this CV (it may not have it)"]`. Decoders leave out many of the NMRA's optional settings, so a few problems after [read_all](#read_all) are usual: those settings stay `"unknown"`.

---

### states

**states** · array

Each setting's state, by name.

---

### values

**values** · array

Each setting's value, by name: a number, a choice's words, or `true` or `false`; `null` while it isn't known.

## Function descriptions

### cancel

**cancel**()

Stops the reading or writing going on after the CV it is at: [on_done](#on_done) comes with the error `"cancelled"`. What was done before it is kept.

---

### load_cvs

**load_cvs**(cvs)

Takes CVs' values from elsewhere (a saved copy of a locomotive's settings, as an array of each CV's value by its number), to be written: their settings' state is `"loaded"` until [write_changes](#write_changes) or [write_all](#write_all) writes them. CV 7 and 8 (the decoder's version and maker) and CV 15 and 16 (its lock) are never taken: they belong to the decoder in hand.

---

### read

**read**(names, tries: 2)

Reads some settings on the programming track: one setting's name, or a list of them. The changes to other settings that share a CV with them are kept, over what was read. On the main, it is a `programming_error`.

---

### read_all

**read_all**(tries: 2)

Reads every setting on the programming track, in the background: a few seconds per CV, so a minute or two for a whole decoder. Each CV the decoder didn't answer for is asked `tries` times (1 to 10), then left unknown and listed in [problems](#problems), and the reading goes on; with no decoder answering at all (nothing on the track), it stops at once. What was read replaces any change not yet written. On the main, it is a `programming_error`.

---

### set_value

**set_value**(name, value)

Changes a setting, to be written by [write_changes](#write_changes): its state is `"edited"` until then. The value is checked as the definition says (a number in its range, one of its choices' words or values, `true` or `false`), or it is a `range_error`. A setting that is part of a CV (a bit, or a few) needs that CV known first, read or loaded, so its other bits are kept: otherwise it is a `range_error` saying so. A setting that can only be read is a `range_error` too.

---

### state

**state**(name) · text

One setting's state: `"unknown"`, `"read"`, `"written"`, `"edited"`, or `"loaded"`.

---

### value

**value**(name) · any value

One setting's value, or `null` while it isn't known.

---

### write_all

**write_all**()

Writes every setting, as known, read and changed: to copy a whole decoder's settings, or to write loaded ones. Never CV 7 and 8, the decoder lock (CV 15 and 16), or a setting that can only be read. Every CV it writes must be known first, or it is a `programming_error` naming them.

---

### write_changes

**write_changes**()

Writes what was changed ([set_value](#set_value)) and loaded ([load_cvs](#load_cvs)), and nothing else: the quick way, and the usual one on the main. With nothing to write, [on_done](#on_done) comes at once and nothing is sent.

On the main, a change of the address the locomotive answers to (CV 1, CV 17 and 18, or CV 29's address bit) is a `programming_error`, with nothing sent: the decoder would stop hearing the writes after it. Change an address on the programming track.

## Event descriptions

### on_done

**on_done**(what, error)

A reading or writing is done: `what` is `"read_all"`, `"read"`, `"write_changes"`, or `"write_all"`, and `error` is `null`, or why it stopped or what it couldn't do, in words. A script without windows waits for it.

---

### on_progress

**on_progress**(done, total, cv)

After each CV: how many are done, out of how many, and the CV just read or written. For a progress bar.

```leah check
$p.on_progress = function($done, $total, $cv) {
    global $bar
    $bar.maximum_value = $total
    $bar.value = $done
}
```

## See also

[decoder_definition](ref:decoder_definition), [dcc_base](ref:dcc_base), [locomotive](ref:locomotive), [Model railroading](ref:Model railroading)
