# WebSocket hover events (codes 5 and 6)

What the site's WebSocket carries while a battle is in progress, and what was
verified against the 133 MB `ur_log.jsonl` and the 62 captured battles. The live
capture path now records the remote hover transitions and the advisor renders
them; the behavioural analysis below explains why they remain a display hint
rather than solver evidence.

## Shape

Two directions, same payload:

```jsonc
// ws_in  — relayed to us by the server (the opponent's mouse)
{"code": 5, "values": ["3"]}
// ws_out — sent by our own client (our mouse)
{"type": "toclient", "to": 6688465, "code": 5, "values": ["6"]}
```

`values` is always a single string, `"1"` .. `"8"`. In one log: 20,142 `ws_in`
and 12,502 `ws_out` frames, of which codes 5 and 6 are by far the most common
(5,611 / 5,106 in, 4,104 / 3,830 out). Every frame appears **twice**, back to
back with the same value.

## Code 5 is hover-enter, code 6 is hover-leave

Not hover-versus-click. Consecutive frames bracket each other on the same value,
and the timings read like a mouse crossing cards:

```
+0.00s  code 5  value 6     enter their card
+0.07s  code 6  value 6     leave it
+0.11s  code 5  value 2     enter another
+0.20s  code 6  value 2     leave it
```

`5 -> 6` on the same value is the most common transition in both directions
(2,526 in, 1,718 out), and the two codes occur in near-equal numbers, as an
enter/leave pair must.

## The index is by seat, not by perspective

**`values` 1-4 are player0's hand, 5-8 are player1's** — hand index =
`value - 1` or `value - 5`. It is _absolute_: both directions use the same
numbering, so no remapping is needed between what we send and what we receive.

This is the one thing worth being careful about, because the obvious reading —
"1-4 are mine, 5-8 are theirs, from each client's own point of view" — is wrong
and looks right at first. Correlating the last card hovered before
`battles.play` against the card actually played, split by which seat we
occupied:

|                 | values 1-4         | values 5-8        |
| --------------- | ------------------ | ----------------- |
| we were player0 | **63.1%** (65/103) | 38.5% (10/26)     |
| we were player1 | 23.8% (5/21)       | **61.5%** (59/96) |

Pooled across both seats the two halves score identically (56.5% vs 56.6%),
which is what hid it: the signal only separates once the games are split by
seat.

~62% rather than ~100% is expected — the last card hovered before committing is
usually but not always the one played, since people hover the opponent's card or
a rejected option last. Chance is 25%.

## Capture and display

`extractFromRecord` accepts `ws_in` codes 5 and 6 while a battle is active,
converts the absolute value to `{ side, index }`, and suppresses repeated
transitions. Userscript 0.7 also serialises incoming WebSocket log posts: the socket
itself is ordered, but independent localhost fetches previously allowed an enter and
leave to reach the capture server in the opposite order. Since one remote pointer can
only occupy one card, entering a new card now also closes any stale active slot.

The secret-free event is written to `captures/battles/*.jsonl` and broadcast on the
advisor feed. The TUI shows the hovered card with a cyan double outline and holds a
very short hover for at least 350 ms so it survives a screen refresh and can actually
be seen. A committed/played card retains its existing selection or result colour, so
a transient mouse position is never presented as a locked selection. Hover changes
only repaint the current board and do not restart or influence the search.

## Possible future use

- Dwell time is probably the signal, not the raw enter event — pair each 5 with
  its 6 and measure. A card hovered for three seconds means more than one
  crossed in 70 ms.
- Remember the duplicate frames: every event arrives twice and will double any
  count.
- It is information the _solver_ could use, not just the display: a prior over
  which card they are about to play would sharpen `SearchMode.SECOND`, which
  currently averages over their hidden pillz with a flat prior.
