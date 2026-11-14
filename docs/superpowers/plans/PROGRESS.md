# itSnows — progress (paused 2026-11-14)

Plan: `2026-11-14-itsnows.md`. Execution: inline, task by task, TDD.

## Done and pushed (all tests green)

- Task 1 falling snow core · Task 2 PNG preview · Task 3 overlay + Direct2D + CI
- Task 4 tray, hotkeys, settings, auto-hide · Task 5 height maps · Task 6 window-top visibility
- Task 7 accumulation · Task 8 window tracker · Task 9 snow blower · Task 10 Christmas lights

CI: first run green with `.exe` artifact (commit fc9ba58). Later pushes not yet checked.

## Next

- **Task 11** snowballs and snowmen (core). Tests are written and were seen failing; parked in
  `task-11-play-tests.wip.rs` so the branch stays green. Design notes: `play.rs` with
  `Play { stacks: Vec<Stack{balls, look}>, held, flying }`, `Ball { x, y, r, area }` (Clone),
  `Play::total_area()`, `World.play` field, `World::ball_count()/snowman_count()`.
  Balls rest on the ground line (not on the pile). Only a carried ball can be thrown; a rolled one is set down.
- **Task 12** play mode in the app (mouse input, Esc hotkey already wired, `set_play` exists).
- **Task 13** README + manual checklist.
- Final whole-branch review by a fresh reviewer, then finish.

## Rulings made so far (deviations from the plan)

1. `Flake` has a `passed` list (once-per-window landing decision).
2. No `png` crate; tiny-skia writes PNG.
3. Overlay is 1 px shorter than the screen so the shell does not treat it as a full-screen app.
4. No `Gfx::resize`; graphics are rebuilt on display change or device loss.
5. Frame loop is in `app.rs`; `SetLayeredWindowAttributes(255)` is called on the overlay (unverified on a real PC).
6. Settings parsed field by field (no serde derive); tray icon on a hidden helper window; snow freezes while the tray menu is open.
7. Overlay re-asserted top-most every 2 s.
8. Window-top overflow handled by limiting end columns to 3 px (snow slides off the ends) instead of per-flake respawn.
9. Shed flakes use a fractional accumulator (fixes an over-dense trickle seen in the preview).
10. Taskbar not used as an occluder; foreign click-through windows skipped; idle = ~4 fps instead of stopped.
11. Ground pile is not relaxed while the blower runs; spray outlives the vehicle; manual call ignores cooldown.
12. Pile outline drawn as a 5-column moving average.

## For the owner to check on a real PC

- Whether the overlay shows at all and is click-through (item 5 above).
- Ground pile height: at spec values it is an opaque band up to 54–90 px above the taskbar.
