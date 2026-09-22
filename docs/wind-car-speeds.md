# Car speed estimates and airflow curve

Wind mapping lives in Rig Companion, not controller firmware. No firmware flash is
needed to change a curve, add cars or revise speed estimates.

The curve maps normalized speed to a fraction of the selected fan range:

```
full_speed = max(10, estimated_car_top_speed_kmh - 15)
x = clamp(vehicle_speed_kmh / full_speed, 0, 1)
fan_percent = minimum + (maximum - minimum) * curve(x)
```

The **Balanced** preset approximates the previous exponent 0.60 response. Presets
also include **Linear**, **Early boost**, **Gentle** and **S-curve**. Choosing a preset
replaces the draft points and smoothing, leaving run mode, min/max and car settings alone.

Click empty graph space to add a point at that speed/output; drag an interior point
up or down to change output without shifting its speed. **Add point** inserts at the
middle of the largest speed gap, initially on the curve. **Remove selected** removes
an interior point. The two endpoints stay at minimum and maximum. There can be 2–16
points, separated by at least 1% of the speed range. Point speeds scale with the
active car's threshold. Selection shows the corresponding km/h and actual fan percent.

**Smoothing** blends straight piecewise-linear segments (0%) with a shape-preserving
cubic Hermite interpolant (100%). Interior tangents use the weighted harmonic mean
of neighboring secants, with zero tangents at plateaus/turns; endpoint tangents use
the endpoint secant. It passes through every control point, with output bounded by
each segment's endpoints. This rounds the graph; it is not a time-delay filter.
Increasing points remain increasing, while deliberate dips are allowed. All presets
are monotonic. Both graph and controller use the same sampling function.

All edits are drafts and affect running fans only after **Apply changes**.
The graph is a commanded PWM curve, not a calibrated measurement of air velocity.
The maximum setting remains an absolute cap. An 80% maximum still means 80% fan
demand at/above the car's threshold. Equal minimum/maximum produces constant airflow.

## Car identity and coverage

SimHub bridge protocol v2 sends `game`, `car_id`, `car_model` and `car_class` from
`GameData.GameName` and `GameData.NewData`. These properties were checked against
the installed SimHub 9.12.4 SDK. Speed remains `SpeedKmh`, independent of display units.
Car strings use a JSON serializer and UTF-8. No player name, account ID or session
credentials are sent. Transport stays on localhost UDP 29814.

`data/iracing-car-identities.json` snapshots all 177 rows from iRacing's published
[car filepath list](https://support.iracing.com/support/solutions/articles/31000172625-filepath-for-active-iracing-cars)
retrieved on 22 September 2026 (the source page was last updated in May). Duplicate
rows are merged; `data/wind-car-speeds.json` contains 189 car entries after additions.
New model names are supplemented from the official
[2026 S3 release notes](https://support.iracing.com/support/solutions/articles/31000179016-2026-season-3-initial-release-notes-2026-06-09-01-)
and [2026 S4 release notes](https://support.iracing.com/support/solutions/articles/31000179517-2026-season-4-initial-release-notes-2026-09-09-01-).
The public list's `bmwrn2csr` typo has an additional `bmwm2csr` alias.

Exact normalized filepath/display-name aliases take precedence, then an unambiguous
longest contained alias, then a small class fallback. Numeric IDs are not guessed.
Other games cannot accidentally match iRacing cars. Unknown/new cars use the visible,
editable fallback estimate, so coverage remains functional without silently claiming
that every future car has been individually researched. Identity is refreshed with
each heartbeat; stale data drops to minimum and does not reuse the last car's estimate.

## Meaning and maintenance of speeds

**These are approximate car/class estimates, not measured top speeds for every car.**
Most values are engineering estimates rounded to 5–10 km/h. They describe plausible
long-straight performance, not the top speed of the current circuit, qualifying record,
or speed achieved so far this session. Setup, weather, BoP, drafting, power rulesets,
and gearing can change attainable speeds substantially, especially oval/dirt cars.

Examples: Vee 165→150 km/h; MX-5 200→185; GT3 285→270; IR18 380→365.
The BMW M2 Racing's 270 km/h comes from iRacing's S3 announcement, but the Rookie
power ruleset can be slower. Radical SR10's 290 km/h is rounded from the manufacturer's
[published 180 mph](https://radicalmotorsport.com/news/50th-production-sr10).
These explicit sources do not certify the other estimated values.

Update the reviewed rules/variants in `scripts/build-wind-car-catalogue.py`, run it
with Python, inspect the generated JSON diff, and run `cargo test --lib`. The build
bundles the catalogue; no network/account connection is used at runtime. Tests require
every official identity/alias to resolve and distinguish Cup/GT3 and newer variants.
For a setup that differs, turn automatic car speed off and adjust the manual top speed.
The 15 km/h margin still applies. No automatic learning from an out-lap or replay peak.

## Upgrade and validation

Upgrade both Rig Companion and the SimHub bridge DLL together (v1 heartbeats lack
identity and are rejected by the new receiver). The point editor requires no further
bridge or firmware update. Existing v1/v2 saved settings migrate in memory to v3,
retaining enable/run mode, limits, ports and channel selection.
The default 180 km/h threshold becomes a 250 km/h fallback estimate; a customized
old threshold is converted to an approximate top speed by adding 15 km/h. The file
is not rewritten until the user applies settings. The former exponent curve is
sampled into eight editable points with full smoothing; this is a close approximation,
not an exact exponent function, particularly near zero speed. Invalid/future settings
remain protected. Point coordinates and smoothing round-trip through JSON without
floating-point drift.

Unit tests exercise monotonicity, limits, the 15 km/h margin, car changes, malformed
frames, stale data, settings migration, run gates and catalog coverage. Editor tests
cover click-to-add, vertical dragging, clamping, fixed endpoints and release outside
the canvas. Curve tests cover presets, smoothing, peaks/plateaus without overshoot,
point count/spacing, invalid saved data and custom-curve persistence. A synthetic
SDK/UDP check verifies encoding; it does not establish real-game property values for
every car. Check the displayed car and estimate on entering the next iRacing session.
