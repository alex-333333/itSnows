# Manual checklist

These need a real Windows PC; nothing here can be automated from CI.

## Shows up and stays out of the way

- [ ] Snow is visible over the desktop and over app windows
- [ ] Clicks, drags, scrolling and right-clicks reach the apps underneath everywhere on screen
- [ ] No entry in Alt-Tab and no button on the taskbar
- [ ] Typing in an editor is never interrupted; the overlay never takes focus
- [ ] The taskbar, Start menu and notifications still open and behave normally
- [ ] A second launch of `itsnows.exe` does nothing

## Snow

- [ ] Intensity presets and `Ctrl+Alt+↑/↓` change the snowfall
- [ ] Snow builds up on the taskbar
- [ ] Snow builds up on the top edge of a normal (not maximised) window
- [ ] The part of a window top hidden behind another window gets no snow
- [ ] Moving, resizing or closing a window makes its snow fall off
- [ ] With intensity Off, piles slowly melt away

## Snow blower and lights

- [ ] "Call the snow blower" sends it across the taskbar and the snow behind it is lower
- [ ] It comes out by itself when the taskbar snow gets deep
- [ ] Lights: top of screen, and on windows (follow windows as they move)
- [ ] Pattern item cycles steady, twinkle, chase

## Play mode

- [ ] `Ctrl+Alt+S` shows the frosty frame; the mouse cursor is a hand
- [ ] Rolling, stacking, a three-ball snowman, throwing, right-click crumble
- [ ] `Esc` leaves play mode and clicks reach apps again
- [ ] It leaves by itself after 30 s without mouse input
- [ ] `Esc` works normally in other apps once play mode is off

## Hiding

- [ ] A full-screen video and a full-screen game hide the snow; it returns afterwards
- [ ] Lock and unlock the PC: snow returns
- [ ] In a screen share or screenshot the snow is absent; with "Show in screen sharing" ticked it is present
- [ ] Pause hides everything; unpause brings it back

## System

- [ ] Restart Explorer (Task Manager → Windows Explorer → Restart): the tray icon comes back
- [ ] Change the screen resolution while it runs
- [ ] Taskbar set to auto-hide: snow settles on the bottom of the screen
- [ ] "Start with Windows" survives a sign-out and sign-in; unticking removes it
- [ ] Settings are remembered after Exit and relaunch
- [ ] Task Manager at Medium: CPU under about 2 %, memory under about 60 MB
- [ ] Laptop on battery: still smooth enough (it draws at half rate)
