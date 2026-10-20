# Hardware attempt 2, 2026-10-20: job rejected with status 2

Second authorized physical print attempt, same `HELLO LINUX` content
(400 feed columns, 400x32 raster), modern profile, with the new content-free
diagnostic mode enabled. The printer received the complete job and actively
rejected it. No label was produced.

## Command and journal

```sh
target/debug/letratag-printer hardware-send test-output/hello-linux.png \
  --address '90:E5:B1:49:DA:AE' \
  --state "$PWD/test-output/hardware-2" --columns 400 \
  --acknowledge-unverified-hardware --diagnostics
```

Fresh journal `test-output/hardware-2` (attempt 1's inflight entry was
archived with the operator decision, see
[hardware-attempt-1-2026-10-20.md](hardware-attempt-1-2026-10-20.md)).
Diagnostics captured to `test-output/hardware-2-diag.log` (content-free:
write progress and raw status bytes only).

## Diagnostic trace (abridged)

```
[diag] connected; usable value size 20 at 2.75s
[diag] write 1/97 len=9 at 2.82s        (header)
[diag] write 2/97 len=18 at 2.90s       (chunk 0)
...
[diag] write 97/97 len=16 at 10.10s     (final chunk, 9 body bytes + trailer)
[diag] status [1b 52 02] at 10.40s
[diag] outcome Err(Uncertain) at 10.40s
print outcome uncertain; operator inspection required, no automatic retry
```

* Wall time 12.3 s (20:48:20-20:48:33 local). Stable adapter window: started
  right after the 60-second adapter power cycle (hci1, cycle at 20:48:13-16),
  next cycle 20:49:13, after the run.
* All 97 writes were host-accepted (ATT Write Request acknowledged), about
  75 ms apart. No write errors, no timeouts.
* First and only status notification: `1b 52 02`, 0.3 s after the last write.
  Code 2 = print failed (S1 `printer.py` Result.FAILED, S2 `status.ts`
  `unknown_failure`).
* Operator observation during the run: motor noise again, and the LED blinked
  fast for a few seconds. No tape emerged from the exit slot.

## Interpretation

The rejection arrived 0.3 s after the job was fully received. A 400-column
label needs roughly 7-9 s of physical printing, so the printer rejected the
job at validation time, before any physical printing. This is a content
rejection, not a mechanical failure and not a mid-print disconnect. The fast
LED blink is consistent with the printer's error indication; the motor blip
is consistent with a brief gear movement on the rejected job (S1 documents
gear movement even without a cassette).

Ruled out by this trace:

* Transport/write path: every write was ATT-acknowledged; usable size 20,
  17-byte body chunks, far below both the BlueZ characteristic MTU (500) and
  S2's bench-confirmed link MTU limit (247).
* Framing: header, chunk indices with the index-27 skip, final `12 34`
  trailer, and reassembled body length (1628) are byte-identical to S1 and
  S2 and were validated offline against the mock capture.
* Write mode: ATT Write Request, matching S1 and the only write flag the TX
  characteristic offers on this device.
* Adapter instability: the run sat inside a stable window.

What remains is the one documented difference between the two known-good
implementations: the wire profile. Modern (S2: copies directive `1B 23 01`,
raster mode `0x81`, cut `1B 70 30`) is rejected with code 2 by this firmware
(`RUTIP20220628`). Legacy (S1: no copies directive, raster mode `0x01`,
form feed `1B 45`) is the other documented, golden-tested format.

## Next

Attempt 3 with `--profile legacy` (explicit operator choice, fresh journal),
still one job, no retries, diagnostics enabled. The legacy body for this
raster is 1624 bytes = 96 chunks + header = 97 writes, same as modern.

The journal `test-output/hardware-2` holds an inflight entry. Before attempt
3 the operator confirms the physical outcome (expected: no tape output, code 2
= failed) and the entry is archived with that decision, per docs/security.md.
