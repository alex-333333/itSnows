# itSnows — design

Date: 2026-11-14. Status: awaiting approval.

## Intent

A festive snow overlay for the Windows desktop with New Year vibes, made for
the owner and their friends. It should be a small toy that makes the desktop
feel like winter and is fun to poke at, while staying out of the way of real
work on the PC.

## Goals

1. Snow falls over the desktop; intensity is adjustable.
2. Snow piles up on the taskbar and on the top edges of open windows.
3. Christmas lights can be strung along window tops.
4. When a lot of snow has piled up, a small snow blower drives out and clears
   a large part of it.
5. The user can roll snowballs out of the fallen snow and build a snowman.
6. It does not interfere with work on the PC.
7. Room for more festive extras later.

## Non-goals (v1)

- No installer, no auto-update, no code signing. One `.exe` passed around.
- No macOS/Linux version.
- No multi-monitor support: v1 covers the primary monitor only. *(builder's call)*
- No sound. *(builder's call — sound is the fastest way to annoy at work)*

## Anti-goals

It is a failure, even if everything works, if the app:

- ever swallows a click or keystroke meant for another app outside play mode;
- steals focus, appears in Alt-Tab or on the taskbar;
- makes the PC noticeably slower, spins the fans, or drains a laptop battery;
- covers a fullscreen game, video, or presentation;
- shows up in a friend's work screen share by surprise.

## Constraints

- Development happens in a Linux cloud session with Rust but no Windows. The
  overlay can only be seen on a real Windows PC.
- Friends must be able to run it by double-clicking one file. Windows 10
  (1903+) and Windows 11, x64.
- The `.exe` will be unsigned, so Windows SmartScreen shows a "More info → Run
  anyway" warning on first launch. Accepted for a friends-only tool.

## Approaches

| | A. Rust, native overlay | B. C# / .NET | C. Web tech (Electron/Tauri) |
|---|---|---|---|
| Size for friends | few MB, one file | 60–70 MB self-contained | 10–150 MB |
| Idle cost | lowest | moderate | browser engine running all day |
| Testable in the dev session | logic yes | no toolchain | visuals in a browser |
| Effort for polish | highest | medium | lowest |

**Chosen: A**, picked by the owner on the builder's recommendation. "Must not
interfere" is the requirement that is hardest to retrofit, and A is the
lightest. Its weakness (visuals can't be seen in the dev session) is covered
by the draw-list design below.

## Architecture

A Cargo workspace with three crates:

```
crates/
  snow-core/     pure simulation, no OS or graphics dependencies
  snow-preview/  dev-only: renders the core's draw list to PNG (runs on Linux)
  itsnows/       the Windows app: overlay window, Direct2D, tray, hotkeys
```

**`snow-core`** owns all behaviour. Each frame the app feeds it inputs and gets
back a draw list:

- Inputs: elapsed time, screen size, list of *surfaces* (horizontal segments
  snow can land on, each with a stable id), settings, and in play mode the
  pointer position and button state.
- Output: a flat list of primitives (circle, soft sprite, line, polyline,
  rounded rect, each with colour and alpha), in back-to-front order.

Because every visual is expressed as draw-list primitives, the two renderers
are thin: `itsnows` replays the list with Direct2D, `snow-preview` replays it
with a software rasteriser. The developer can therefore look at snow, piles,
the blower, lights and snowmen as PNG frames on Linux, and only the Windows
plumbing is left to check by eye on a real PC.

Randomness comes from a seeded RNG owned by the core, so tests are
deterministic.

**`itsnows`** (Windows only):

- One borderless window covering the primary monitor, always on top, never
  activatable, hidden from Alt-Tab and the taskbar
  (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW |
  WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP`).
- Drawing through DirectComposition with a premultiplied-alpha swap chain and
  Direct2D, so the desktop compositor blends it on the GPU and nothing is
  copied through system memory.
- Click-through is the `WS_EX_TRANSPARENT` style. Play mode removes it; leaving
  play mode restores it.
- Window tracker: about four times a second, enumerate top-level windows in
  z-order (visible, not minimised, not cloaked, real frame bounds from DWM) and
  compute which parts of each window's top edge are actually visible, i.e. not
  covered by a window above it. Those segments plus the taskbar's top edge are
  the surfaces. If the taskbar is auto-hidden, the bottom of the screen is the
  ground instead. Maximised windows have no top edge on screen and get no snow.
- Tray icon with a right-click menu, global hotkeys, settings file.

Uses Microsoft's `windows` crate for all OS calls.

## Behaviour

### Snow

- Flakes in three depth layers (far: small, slow, faint; near: larger, faster),
  with gentle sideways sway and occasional wind gusts that lean the whole
  field.
- Intensity is 0–100 %. It sets the spawn rate; the live flake count is capped
  (about 1,500 at 100 % on a 1080p screen, scaled by screen area).
- Tray menu presets: Off, Light, Medium, Heavy, Blizzard. Fine steps with
  hotkeys. *(builder's call: a tray menu can't hold a slider, and a settings
  window is more than v1 needs)*

### Accumulation

- Each surface carries a height map in 4 px columns. A flake that touches a
  surface adds to the column under it and disappears.
- Piles relax toward a natural slope so they look like drifts, not spikes.
- Only near-layer flakes land on window tops, and only some of them, so snow
  still reaches the taskbar behind a wall of windows. Everything that reaches
  the taskbar lands.
- Caps: about 24 px on window tops (excess slides off the ends as falling
  flakes), about 90 px on the taskbar.
- When a window moves, resizes, closes, or gets covered, the snow on the
  affected segment falls off as a burst of flakes. *(builder's call)*
- Piles very slowly melt when intensity is Off.

### Snow blower

- Triggers when the taskbar pile's average height passes about 60 % of its cap,
  or on demand from the tray menu.
- A small vehicle drives in from one side along the taskbar, removes about
  80 % of the snow in the columns it passes, and throws it in an arc out of its
  chute as particles that fly off-screen. Then it drives out the other side.
- It works on the taskbar only; window tops are kept in check by their cap.
- It drives behind snowmen and leaves them standing. *(builder's call)*

### Christmas lights

- A garland of coloured bulbs hanging in shallow swags, with a soft glow.
- Tray setting: Off / Top of screen / On windows. "On windows" hangs a garland
  from each visible window-top segment wide enough to hold one.
  *(builder's call on the three modes)*
- Patterns: steady, slow twinkle, chase. Cycled from the tray menu.

### Play mode

- `Ctrl+Alt+S` toggles it; `Esc` also leaves it. It turns itself off after
  30 s without mouse input, so a forgotten play mode can't block work.
  *(builder's call: toggle rather than hold, plus the timeout)*
- While on: a thin frosty border glows around the screen, and the overlay
  receives the mouse.
- **Roll:** press on a pile and drag along it. A snowball appears under the
  pointer and grows as it travels, taking snow out of the columns it passes.
  Release to set it down.
- **Stack:** drag a ball and drop it on top of another. Balls only stack
  smaller-on-larger; otherwise the ball rolls off and sits beside it.
- **Snowman:** three stacked balls become a snowman: coal eyes, carrot nose,
  twig arms, a hat or scarf picked at random.
- **Throw:** flicking a held ball and releasing throws it; it bursts into
  flakes where it lands.
- Right-click a ball or snowman to crumble it back into snow.
- Balls and snowmen live on the taskbar ground and persist until crumbled or
  the app exits. Snow keeps falling on their heads.

### Not interfering

- Click-through and no-focus at all times outside play mode.
- Fully hidden and paused while a fullscreen app, game, or presentation is in
  front (via the shell's notification state), and while the session is locked.
- Hidden from screen capture and screen sharing by default
  (`WDA_EXCLUDEFROMCAPTURE`), with a tray toggle to show it.
  *(builder's call on the default)*
- 60 fps on mains power, 30 fps on battery. Rendering stops entirely when
  intensity is Off and nothing on screen is moving.
- Budget on an ordinary laptop at Medium: under 2 % CPU, under 60 MB RAM.
  These are targets to measure on a real PC, not guarantees.

### Tray menu and hotkeys

Menu: Intensity ▸ presets · Lights ▸ mode, pattern · Call the snow blower ·
Clear all snow · Play mode · Show in screen sharing · Start with Windows ·
Pause · Exit.

Hotkeys: `Ctrl+Alt+S` play mode · `Ctrl+Alt+↑/↓` intensity ±10 % ·
`Ctrl+Alt+P` pause. If a hotkey is already taken by another app, it is skipped
and the tray menu still works.

Settings are saved to `%APPDATA%\itSnows\settings.json`. "Start with Windows"
writes a per-user Run entry; no admin rights anywhere.

## Testing and delivery

- `snow-core`: unit tests written first for each behaviour (spawn rate vs.
  intensity, landing and height maps, slope relaxation, caps, shake-off on
  surface change, blower trigger and removal share, rolling conserves snow,
  stacking rules, snowman formation, play-mode timeout). Runs on Linux.
- `snow-preview`: renders scripted scenes to PNG for eyeballing in the dev
  session.
- `itsnows`: type-checked against the Windows target from Linux where the
  toolchain allows; built and tested for real by GitHub Actions on a Windows
  runner on every push, with the `.exe` attached to the run as a download.
- Manual checklist on a real PC for what can't be automated: click-through,
  focus, Alt-Tab, fullscreen hiding, screen-share hiding, CPU and RAM.

## Build order

Each step ends with something runnable:

1. Overlay window, falling snow, intensity, tray, CI producing an `.exe`.
2. Window tracking and accumulation on the taskbar and window tops.
3. Snow blower.
4. Christmas lights.
5. Play mode: snowballs, stacking, snowmen, throwing.
6. Polish from real-PC feedback.

## Later ideas (not in v1)

- New Year countdown with fireworks at midnight on 31 December.
- Multi-monitor support.
- Footprints or a cat walking across the taskbar snow.
- Frost creeping in from the screen corners at Blizzard.
- Snow reacting to fast mouse movement like wind.

## Left to the builder

Exact visuals (flake sprites, blower and snowman art, colours), physics
constants, the software rasteriser used by `snow-preview`, internal module
layout, and tuning of caps and thresholds after seeing it run.
