# Animation plans

Self-contained implementation plans from the `improve-animations` audit of the
book-style card turn (detail view, 2026-10-07). Each plan lists exact files,
values, steps, boundaries and checks.

| # | Plan | Severity | Status |
| --- | --- | --- | --- |
| 001 | [Make the book turn one object with one table shadow](001-book-turn-spine-and-shadow.md) | HIGH | DONE |

## Execution order

1. 001. It has no dependencies.

## Not planned (from the same audit)

These were offered, and the user did not select them on 2026-10-07:

- Angle-based shading on the turning page.
- A spine crease in the open book.
- A contact shadow from the lifting page.
- Drawing the left page's texture in advance, to remove the one 11 ms frame
  on the first turn.
