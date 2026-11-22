# Controller test bench

Tests for the controllers OpenController reads from their own HID reports, next to SDL or instead
of it. Nobody needs the controller: each one is a JSON file in `pads/` that describes its report,
and `cargo test -p open-controller-core testbench` builds reports from that file and checks every
decoder that reads it.

## What a file holds

| Field | What |
|---|---|
| `ids` | `vvvv:pppp` for every way it connects (cable, receiver). |
| `connections` | Which interface carries the report on each, and who reads it (SDL's driver or not). |
| `sources` | Where each fact came from: SDL's driver, the maker's app, other projects, a user's report. |
| `rest` | The report at rest, in hex, report id first. Best taken from a user's report. |
| `signature` | How many leading bytes say which report it is (kept as they are when making reports). |
| `controls` | Each button: `byte`, `mask`, optional `value` (for a hat or a mode field), and `to`. |
| `axes` | Each byte that is a stick or an analog trigger. |
| `noise` | Bytes that move with no button behind them: motion, counters, battery. |
| `not_input` | Reports that are not input, such as replies to commands. |
| `captures` | A user's report's "bytes that changed" line, pasted as is, with what they did. |
| `unseen` | What the file says that no real controller has shown yet. |
| `decoders` | Which of OpenController's readers this report goes through (`decoder()` in `mod.rs`). |
| `motion` | For a report OpenController reads motion from: its reader (`motion_decoder()`) and `samples`, reports whose motion is known. |

A motion sample gives `gyro` as pitch, yaw and roll, each `0` (still), `+`, `-` (SDL's sense:
tilting the top up and turning left are positive) or `any`, and `accel_g`, the accelerometer's
length in g, for a reader that has one. The checks: every sample reads as it says, no button or
stick moves the gyro, and random or cut reports never make the reader panic or give a value that
is not a number. `the_bench_catches_a_gyro_turned_the_wrong_way` keeps that honest.

`to` is one of `south east west north back guide start left_stick right_stick left_shoulder
right_shoulder dpad_up dpad_down dpad_left dpad_right misc1..misc6 touchpad left_paddle1
right_paddle1 left_paddle2 right_paddle2 left_trigger right_trigger left_x left_y right_x
right_y none`. Write what the part *is*, even when SDL reads it: a decoder only answers for the
parts its `owns` claims, and must leave the rest as SDL had them.

## What gets checked, for every file and button decoder

1. The file holds together: known targets, bytes inside the report, nothing held at rest, no two
   buttons on the same bit.
2. At rest the decoder reads nothing.
3. Each control alone gives exactly its button (or nothing, when it is SDL's), and all of them at
   once give all of them.
4. Sticks and noise bytes, at their ends and at random, never change a button.
5. Replies and reports too short for the bytes the decoder reads are not input.
6. Thousands of random reports never make it panic.
7. Every bit a user's report caught moving is explained by the file. When this fails, the user
   pressed something the file does not know: that is where a missing button hides.

The test `the_bench_catches_the_apex_2_bug_and_a_gap_in_a_file` keeps the bench honest: a decoder
that reads the Apex 2's wheel as triggers, as SDL 3.4 did, must fail it.

## Adding a controller

1. Collect what is known (see `scripts/research/README.md`): SDL's driver for it, the maker's app,
   other projects, and above all a user's report with the raw bytes.
2. Write `pads/<maker>-<model>.json`. Put the rest report and the capture in exactly as the report
   printed them.
3. Run the tests. `what_users_reports_caught_is_all_explained` says which bits nobody accounted
   for yet.
4. When OpenController gets a reader for it, add it to `decoder()` and to the file's `decoders`.

When SDL itself misreads the controller, also port the part of SDL's driver that reads it into a
test, as `flydigi/sim.rs` does for the Flydigi V1 pads, and replay the user's session through it:
when the port gives back the user's report byte for byte, the cause is proven, and the fix can be
tested against the same session.

A report holds no personal data in these bytes, but a full report does (names, paths, serials):
copy only the bytes into the file.
