# reactionlab

Desktop-based, highly configurable reaction time test app based on egui

---

## Why?

- I can't find a similar app anywhere
- Why a desktop app, it's to remove browser latency (with caveat below)
- Kinda annoying that most reaction time sites can't be configured

## Compiling and running

it's a standard Rust project, so...

```sh
cargo run
```

## AI disclosure

This project is mostly AI-generated right now, it wil be retouched when I've finished experimenting with all ideas I have for this, though theres a chance the final/polished version goes closed source

## Planned features

[x] Click timing exercise, a crosshair in the middle of the screen then a "ball" will pass through the crosshair and you have to click while the ball is inside that crosshair.
[ ] Click timing video exercise, just a click timing exercise based on a video of someone peeking, but it starts playing at a random time
[ ] BallSheet-style test implementation
[ ] Better UI when other planned features are already done

## Caveats

This app is based on egui, which might not be the best for absolute lowest latency, but at available backend settings have been configured to their lowest non-tearing latency options available (desired max frame latency 1, try immediate present mode)

Future migration to gpui is in consideration, if it becomes more documented

## App UI

The top panel contains a tab for each available mode. The available modes are:

- Simple reaction time test, which contains the current reaction-time workflow.
- Click timing reaction test, where a target travels from a configurable wall through a static crosshair and is scored by timing.

The active mode fills the rest of the app window with its own UI.

## Start screen

Start screen is split to two columns.

Left column has line graph of 10 previous results, each point corresponds to average (mean) reaction time in the previous run data in the stored run data (explained below). Below the graph is a "Show all runs" button that when pressed will open a window to show stored run data files, and when each of the files are clicked it shows the saved run configurables, each round, also mean and median of the total rounds. Also a bar graph to visually show the results of each round.

Right column simply has "Settings" button that when clicked, opens up a window to configure the reaction time test's settings, and "Start new run" button to transition to the round screen.

## Round screen

Fills the screen with the wait color, then when a random time comes uses the react color. After clicking, it shows normal app background color with "... ms" and "click to continue".

## End screen

End screen shows each round, also mean and median of the total rounds of the run that has just been played. Also a bar graph to visually show the results of each round. So it's similar to when reviewing stored run data. Also a button to "Try again" to bring the user back to the start screen.

The click timing mode reports hit rate and absolute timing error for successful clicks. Failed attempts are classified as Too soon, Almost there (too soon or too late), Too late click, or Too late without a click. Every failure follows the configured retry or end-run action.

## Stored run data

In the app's config directory (usually `.local/share/reactionlab` on Unix or `%APPDATA\reactionlab` on Windows), the simple reaction time mode stores its settings and run history in the `simple_reaction_time_test` subdirectory. Each run is stored as a JSON file with a `reactionlab-` prefix and timestamp showing when the run started. Each run contains its configurables and all its round information (chosen random wait time, user reaction time) so it can be reviewed in the start screen.

The click timing mode uses a separate `click_timing_reaction_test` subdirectory. Its run files store the full configuration snapshot and every realized attempt, including direction, generated initial velocity, stop behavior, timing offset, logical round, and explicit outcome.

Existing root-level persistence is not migrated.

## Settings window

Settings window contains both configurables and a button to reset history to clear historical run data, with a date picker to pick since what date should data not be deleted.

## Configurables

- Waiting color, red by default. Editable using the egui color picker
- React color, green by default. Editable using the egui color picker
- Minimum wait time, 250 ms by default
- Maximum wait time, 10000 ms by default
- Whether a false click invalidates an entire run or just a round
- Round count, 5 by default

## Click timing configurables

- Static crosshair style, colors, outline, dimensions, gap, T style, and center dot
- Independent left and right wall distances
- Target visual radius, clickable radius, RGBA color, speed, acceleration, deceleration, and miss timeout
- Full, mixed, or zero initial velocity
- Always-stop-on-crosshair behavior
- Left/right direction checkboxes, with random direction when both are selected
- Randomized minimum and maximum target appearance wait
- Round count and retry-round or end-run failure action
