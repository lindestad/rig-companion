# Rig inputs — prototype A

Open **rig-inputs.kicad_sch** in KiCad, or print **rig-inputs.pdf** on A3. `render/rig-inputs.svg` is zoomable. `wire-list.csv` lists every connection. Matching labels in the schematic are connected electrically; the empty module rectangles represent purchased assemblies, not bare chips. This is a wiring schematic, **not a PCB layout or a physical header pinout**.

## Minimum build

| Quantity | Part | What to choose |
|---|---|---|
| 1 | ESP32-S3 DevKitC-1 style board | Assembled regulator and native USB/OTG socket, exposed GPIOs listed below, preferably presoldered 2.54 mm headers |
| 1 | Analog two-axis joystick | A verified 3.3 V compatible assembled breakout is easiest; existing Switch replacement flex needs identification and an assembled matching FPC adapter |
| 8 | Cherry MX style mechanical switches + caps | Ordinary contact switches, not optical or magnetic keyboard switches; no LEDs needed |
| 2 | Passive mechanical quadrature encoders with push | Through-hole EC11 style, threaded shaft with washer/nut; buy knobs to match actual shaft |
| 1 | Small perfboard | Optional hub for sockets, shared ground and removable harnesses; individual pad board avoids accidental stripboard shorts |
| As needed | Stranded wire, heat-shrink, connectors | 26–28 AWG for short internal signal harnesses; use precrimped keyed pigtails if you want removable modules |
| 1 | USB data cable | Fits the board's native USB socket; route through a cable clamp |
| Optional 2 | 100 nF ceramic capacitors | Through-hole, each ADC signal to ground near board if analog readings are noisy |

No diode matrix, external pull-up resistors, level shifter, extra regulator or input expander is needed for the short-wire first prototype. Firmware enables internal pull-ups. If longer cables prove noisy, add stronger external pull-ups after measuring; spare perfboard space makes that easy. Keep the controller near the inputs, initially within about 30 cm of wiring. This is an assembly target, not a validated cable-length limit.

## Pin and control assignment

| Control | GPIO / connection | Proposed action |
|---|---|---|
| Joystick X / Y | 4 / 5 (ADC1) | Relative mouse velocity with dead zone and acceleration |
| Optional joystick click | 21 to GND when pressed | Left mouse; omit if stick has no independent switch |
| SW1 / SW2 | 6 / 7 to GND | Restore 98 cm seated height / dashboard |
| SW3 / SW4 | 8 / 9 to GND | Desktop request / undo |
| SW5 / SW6 | 10 / 11 to GND | Left / right mouse |
| SW7 / SW8 | 12 / 13 to GND | Windows / Escape |
| Height encoder A / B / push | 15 / 16 / 17 | Signed height steps; hold push for fine steps |
| Scroll encoder A / B / push | 18 / 1 / 2 | Mouse wheel; push is middle mouse |
| Joystick supply | Board 3V3 and GND | Never use the board's 5V pin for these analog inputs |

GPIO19/20 stay reserved for native USB. This allocation avoids the S3 strapping pins and common onboard RGB/memory pins. GPIO14 remains an easy spare on the reference board. **An arbitrary AliExpress S3 board may expose different pins or attach peripherals to them: compare its schematic/markings before soldering.** USB-UART-only boards are unsuitable unless native USB is separately brought out. Use the board's onboard USB connector rather than hand-wiring USB data over perfboard.

ESP32-S3 supports composite USB through TinyUSB: mouse/keyboard reports can operate independently of the companion; VR commands need companion integration. Those firmware/USB command paths are not implemented yet. The existing app has height, undo and dashboard actions; its direct desktop-panel selection remains future work. Physical mouse control of each SteamVR panel also needs a runtime test.

## The Switch joystick is the unresolved part

Four contacts alone do **not** establish a pinout or prove an independent click is present. We have not identified this part. The drawing uses **functional names**, deliberately not FPC contact numbers. Do not solder according to their order in the rectangle.

For the existing module, confirm part number, whether it is potentiometer or active Hall/TMR, contact count, flex pitch, exposed-contact side, operating voltage, and connector latch orientation. Use a matching FPC socket already soldered onto a 2.54 mm breakout. Avoid soldering wires directly onto the flex. A generic four-contact adapter is not automatically compatible. For a confirmed passive potentiometer stick, an unpowered multimeter can identify track ends and moving wipers; do not apply that assumption to an unidentified active sensor.

The easiest substitution is a **complete analog joystick breakout** exposing VCC, GND, X, Y and SW and supporting 3.3 V operation. The extra legs of a bare PlayStation-style stick are handled by its PCB. Power the breakout at 3.3 V even if its silkscreen says “5V”, but only after checking its circuit supports that: a passive two-pot board normally does; an active module may not. If there is no click, leave GPIO21 open with pull-up and disable its action; SW5 already provides left-click.

For analog firmware, calibrate center and both endpoints per axis, set ADC attenuation, average samples, and clamp saturated extremes. A 3.3 V pot can exceed the ADC's characterized measurement range near full travel: that is handled as an endpoint, not assumed linear full-scale measurement. Start with a roughly 10% radial dead zone and tune. Reverse axes in software rather than moving power wires.

## Soldering and printed modules

1. Make a controller cradle, a key plate, a joystick pod and two encoder pods, bolted to a common base. Keep the joystick pod interchangeable until the actual module is chosen. Fit the base to your specific 4040 slot/T-nuts; “4040” does not uniquely specify slot hardware.
2. Mount MX switches in the plate and solder only their **two metal electrical legs**. Three-pin vs five-pin MX usually describes added plastic mounting posts; those are not extra electrical inputs. Start with a 14 mm square plate opening and a small tolerance-test coupon, then measure the actual switch and tune the clip lip. Do not force MX legs into ordinary 2.54 mm perfboard: their geometry is not a standard header grid.
3. On the back of the key plate, daisy-chain one contact from all eight switches to ground using insulated wire. Each other contact gets its own named signal. This is a nine-wire key harness. Label both ends; cap colors are insufficient while soldering from the rear.
4. Each passive encoder has A/B/common and a separate two-contact push switch. Identify them from the exact part drawing or continuity test; tabs are mechanical supports, not signal pins. Join common and one push contact locally to ground. Run four wires: GND, A, B, push. Clockwise direction can be inverted in firmware. A powered KY-040-style board is a different assembly: check its pull-up supply and circuitry before substitution.
5. Put the S3 in removable female header sockets on the hub if desired. Run each named GPIO to the corresponding module signal. Distribute ground from the hub to each module; only the joystick needs a 3.3 V supply. Optional connectors do not change the circuit. Avoid loose Dupont friction joints for the final vibrating rig.
6. Add service loops and printed cable clamps before closing each pod. Keep flex stationary, cover exposed solder joints, and give the board antenna some clearance from the aluminum if wireless may be used later. The USB-only design does not need Wi-Fi/Bluetooth enabled.
7. Before USB power, inspect for bridges and meter 3V3-to-GND for an unintended short. Confirm each key closes only its assigned signal to ground. Validate the joystick supply/pinout separately. Bring up USB and digital inputs first, then connect the verified joystick.

Provide screw access and an open/removable back to every pod. Do not finalize holes, board standoffs, flex bends, encoder shaft openings or wire lengths from this schematic: measure the purchased parts first. A Blender exploded assembly can reuse the same net names and CSV; its first useful version should show actual solder contacts, connector orientation, local ground links and strain relief. No dimensionally accurate printable model is included in this revision.

## Firmware bring-up

- Poll/debounce keys without blocking USB; send releases as well as presses. Use a quadrature transition decoder for encoders, rejecting bounce/invalid transitions rather than treating every edge as a detent.
- Start with a diagnostic input view before enabling commands. Confirm simultaneous keys, encoder direction, press-and-turn, joystick range/center, and startup with held keys. Do not trigger calibration automatically on connection.
- Mouse click/scroll/Windows are HID actions. Restore, undo, height delta and dashboard/desktop are explicit companion commands; accumulated encoder height changes should be batched rather than committing SteamVR calibration at every contact bounce.
- Keep profile capture separate from ordinary restore. A hardware press restores the saved 98 cm default, not the currently miscalibrated height.

## Sources and validation

- [Espressif DevKitC-1 pin tables and board documentation](https://documentation.espressif.com/esp-dev-kits/en/latest/esp32s3/esp32-s3-devkitc-1/user_guide_v1.0.html): reference header signals, native USB and memory variants.
- [Espressif composite USB device stack](https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32s3/api-reference/peripherals/usb_device.html): HID and custom/composite functions.
- [Alps Alpine encoder catalog](https://tech.alpsalpine.com/assets/catalog/product-catalog-ec-all.en.pdf): mechanical encoder terminals and part-dependent variants.
- [Adafruit analog mini-stick breakout](https://www.adafruit.com/product/3246): example of converting a small stick to 2.54 mm connections; this is a PSP-like product, **not a verified adapter for the user's Switch part**.

KiCad 10 loads the native schematic and exports SVG/PDF and an XML netlist. ERC reports zero errors and zero warnings. `python hardware/verify.py` checks the exported connectivity against the intended 17 GPIO signals, ground and 3.3 V. Module terminals are modeled as passive wiring endpoints: ERC cannot validate hidden purchased-board circuitry, voltage compatibility, or the unknown joystick flex. No physical electrical testing has been performed.

`python hardware/generate.py` regenerates schematic/library/wire list; it overwrites schematic edits, so edit the generator or keep a manual copy. Export with `kicad-cli sch export pdf -o hardware/rig-inputs.pdf hardware/rig-inputs.kicad_sch` and the equivalent `svg` command. The generator requires Python's standard library only.
