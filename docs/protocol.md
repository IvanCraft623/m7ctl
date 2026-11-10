# Protocol

Implemented in `src/protocol/`.

## Devices
| Mode | USB id |
|---|---|
| Wired | `248a:5b2e` |
| Wireless receiver | `248a:5b2f` |

Interface 1 holds the configuration channel: vendor usage page `0xff00`, usage 1, feature report `0x0c` (33 bytes
with the report id) and input report `5`. Interface 2 (usage page `0xffef`) is never used.

## Frames
Settings are HID feature reports, 33 bytes, report id `0x0c`:

```
[0] 0x0c  [1] 0x01  [2] register  [3] 0x00  [4] sequence  [5] 0x01  [6] payload length  [7..] payload
```
After each write, wait about 60 ms and read the report back: it echoes the register and sequence. **Byte 3 of the echo is 1
when the wireless mouse is asleep**: the receiver acknowledges but the mouse never got the frame, so resend until it is 0.

## Registers
| Reg | Payload | Meaning |
|---|---|---|
| 0x20 | none | status query |
| 0x05 | 18 bytes | DPI: `[0]=(active+1)<<4 \| count`, `[1]=(1<<count)-1`, eight LE16 codes |
| 0x06 | 24 bytes | eight RGB scroll-light colours, one per DPI stage |
| 0x07 | 1 byte | polling interval in ms: 1 = 1000 Hz, 2 = 500, 4 = 250, 8 = 125 |
| 0x08 | mode, nibbles, ... | main lights |
| 0x04 | button, 3-byte code | button function |
| 0x0e | chunks | macro upload |
| 0x09 | `[mm, 0]` | lift-off distance, 1 or 2 mm |
| 0x0b | `[minutes, lights_off_while_moving]` | power-down time (3-10) |
| 0x0c | 0 or 1 | wake on motion |

**Status reply (0x20):** bytes 8-11 model (`M263`); 15 sensor variant (`0x95` wired, `0xff` via the receiver); 18 battery
percent; 19 charge (1 charging, 2 full); 21 online.

**Lights (0x08):** `[mode, ((brightness+1)<<4) | (speed+1), ...]`. Modes: 0 off, 2 colour cycle (count + up to 7 RGB),
3 steady (RGB), 6 breathing (RGB), 7 flash (RGB). Brightness 0-3. Other modes are accepted but do nothing.

**Buttons (0x04: `button index, code[3]`):** buttons are numbered right, middle, back, forward, dpi (left is protected).

| Code | Function |
|---|---|
| `10 mask 00` | click (1 left, 2 right, 4 middle, 8 back, 0x10 forward); tilt left `10 40 00`, right `10 80 00` |
| `40 01/02/03 00` | DPI loop / plus / minus |
| `30 interval count` | Fire Key; Three Click is `30 0a 03` |
| `70 modifiers usage` | keyboard shortcut |
| `80 lo hi` | consumer key (media, volume, mail, ...) |
| `90 slot\|mode<<4 times` | run macro |
| `60 01 00` / `60 02 00` / `60 04 00` | lights on/off, switch polling rate, switch effect |
| `00 00 00` | disable the button |

**Macros (0x0e):** events are three bytes: `type|state|delay_hi`, `delay_lo`, `code` (about 1 ms per delay unit). At most 125
events per macro, 15 slots. Upload frame: byte 5 = chunk index<<4 | chunk count, byte 6 = chunk length + 1, byte 7 = slot,
then up to 25 data bytes. Repeat modes (button code `90`): 0 loop, 1 repeat while held, 2 N times.

## Input reports
DPI stage change (interface 1, report id 5), one per press of a DPI button:
`05 06 01 02 <stage, 1-based> <DPI code, LE16> ...`

## DPI codes
A sensor variant has a table of DPI/50 values; the code sent is the position of the value in the table, plus 1 on variants
0x12, 0x25 and 0x27. The status reply selects the table; the receiver reports `0xff`, so the last known variant is remembered
(default `0x95`).

| Variant | Values (DPI/50) | Entries | Offset |
|---|---|---|---|
| 0x04 | 2-80 step 2, 90, 96, 110-160 step 10 | 48 | 0 |
| 0x12 | 4-48 step 2, 60-96 step 2 | 42 | 1 |
| 0x25 | 4-100 step 2, 110, 120, 128, 140-200 step 10 | 59 | 1 |
| 0x27 | 4-124 step 2, 130-240 step 10, 248 | 74 | 1 |
| 0x35, 0x89 | 2-320 step 2 | 160 | 0 |
| 0x60 | 2-240 step 2, 480 | 121 | 0 |
| 0x70 | 1-200, 202-380 step 2 | 290 | 0 |
| 0x95 (M7 Pro) | 1-520 | 520 | 0 |
| 0x99 | 1-400 | 400 | 0 |
