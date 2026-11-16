# itSnows

A snowy overlay for the Windows desktop. Snow falls over everything, piles up
on the taskbar and on top of your windows, and stays out of the way of your
work.

- Adjustable snowfall, from a few flakes to a blizzard
- Snow settles on the taskbar and on the top edges of open windows; move a
  window and its snow falls off
- A little snow blower drives out and clears the taskbar when the drifts get deep
- Christmas lights along the top of the screen or on every window
- Play mode: roll snowballs, stack three into a snowman, throw them
- Clicks pass straight through; it never takes focus, hides itself for
  full-screen apps and stays out of screen sharing

Windows 10 (1903 or later) and Windows 11, 64-bit. Primary monitor only.

## Get it

1. Open the [Actions tab](../../actions) and pick the newest run with a green tick.
2. Download the `itsnows-windows-x64` artifact and unzip it.
3. Run `itsnows.exe`. There is nothing to install.

The file is not code-signed, so the first launch shows a blue "Windows
protected your PC" box. Choose **More info**, then **Run anyway**.

## Use it

itSnows lives in the notification area (the snowflake icon near the clock).
Click the icon for the menu:

| Menu item | What it does |
|---|---|
| Intensity | Off, Light, Medium, Heavy, Blizzard |
| Lights | Off, top of screen, or on windows; and the blink pattern |
| Call the snow blower | Sends it out now, however little snow there is |
| Clear all snow | Empties every pile (snowmen stay) |
| Play mode | Lets you grab the snow with the mouse |
| Show in screen sharing | Off by default: the snow is left out of screen shares and recordings |
| Start with Windows | Launch at sign-in |
| Pause | Hide everything until you unpause |
| Exit | Quit |

| Hotkey | |
|---|---|
| `Ctrl+Alt+S` | Play mode on / off |
| `Esc` | Leave play mode |
| `Ctrl+Alt+↑` / `Ctrl+Alt+↓` | More / less snow |
| `Ctrl+Alt+P` | Pause |

If another program already uses one of these, that hotkey is skipped; the menu
still works.

### Play mode

A thin frosty frame around the screen shows play mode is on. While it is on,
the mouse belongs to the snow, not to your apps. It switches itself off after
30 seconds without mouse input.

- **Roll:** press on the snow on the taskbar and drag along it. The ball grows
  as it picks up snow.
- **Stack:** pick a ball up and drop it on a bigger one. Three make a snowman.
- **Throw:** flick a ball you are carrying and let go.
- **Crumble:** right-click a ball or snowman to turn it back into snow.

Settings are kept in `%APPDATA%\itSnows\settings.json`.

## Build from source

Needs [Rust](https://rustup.rs) on Windows.

```bash
cargo build --release -p itsnows
```

The result is `target\release\itsnows.exe`.

## How it is put together

| Crate | |
|---|---|
| `crates/snow-core` | The whole simulation as pure logic: inputs in, a list of shapes to draw out. No OS or graphics code. |
| `crates/itsnows` | The Windows shell: overlay window, Direct2D drawing, window tracking, tray, hotkeys. |
| `crates/snow-preview` | Developer tool that renders scripted scenes to PNG on any OS. |

```bash
cargo test --workspace
```

```bash
cargo run --release -p snow-preview -- snowman
```

Preview scenes: `snow`, `piles`, `blower`, `lights`, `snowman`. Frames land in
`preview-out/`.

What can only be checked on a real PC is listed in
[`docs/manual-checklist.md`](docs/manual-checklist.md).
