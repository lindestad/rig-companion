# Supplied parts assessment

Based on the user's six listing screenshots and pasted titles, not a verified seller schematic. The listings' AI summaries are not electrical specifications. The revision A schematic models bare encoders: use the module mapping below for the supplied assembled encoder boards. No physical header order or joystick pinout has been newly verified.

## Encoders

The rectangular KY-040 photo shows **GND, +, SW, DT, CLK**. The round module shows **GND, S1, S2, KEY, 5V**. Both appear to be mechanical quadrature encoders with a push switch on a breakout PCB. The photo does not show the complete underside circuit.

Use the rectangular KY-040 first: it has clear labels, a threaded mounting bushing, and PCB mounting holes. Keep its assembled PCB. The round one is also a candidate, especially if its shape packages better; the photos do not establish a quality difference.

| Function | Rectangular module label | Round module label | Height GPIO | Scroll GPIO |
|---|---|---|---|---|
| Ground | GND | GND | GND | GND |
| Pull-up supply | + | 5V* | 3V3 | 3V3 |
| Quadrature A | CLK | S1 | 15 | 18 |
| Quadrature B | DT | S2 | 16 | 1 |
| Press | SW | KEY | 17 | 2 |

*The proposed supply is **3.3 V**, including at the round board's terminal labeled “5V”, conditional on confirming it is a passive switch/pull-up breakout without circuitry requiring 5 V. Do not power a resistor-pulled output module from 5 V and connect its outputs directly to the ESP32. The intended wiring needs **five wires per module**, replacing revision A's four-wire bare-encoder harness. Do not confuse the module supply pin with the bare encoder's common terminal.*

With power disconnected, check that the supply connects through resistors to phase outputs and that the contacts switch those outputs to ground. Check push-switch behavior too; enable internal pull-ups in firmware. If another active component is present, identify it before powering. A/B order sets direction; invert in software if needed. Joy-IT documents a comparable KY-040 with 3.3 V Raspberry Pi wiring, but this does not certify these unbranded clones: [KY-040 documentation](https://sensorkit.joy-it.net/de/sensors/ky-040).

## HUANO mouse switches

The photo shows conventional three-terminal, plunger-operated mouse microswitch packages. Use two for left/right in place of SW5/SW6 MX switches, leaving six MX keys in the design. GPIO10 and GPIO11 remain assigned to left/right. Use the normally-open contact pair: **COM to ground, NO to GPIO**, with the third terminal unused and insulated. Exact physical COM/NO/NC terminal positions are not legible in the supplied image; identify with markings or an unpowered continuity test (open at rest, closed when clicked).

The selected listing option reads **Yellow dot 30Million**; the hero image shows pink plungers. Do not infer the actual actuation force or lifetime from that mixed screenshot. Either electrically compatible variant can be tested mechanically.

### Printed click paddles

Make two rigid paddles with separate replaceable rear flexures, a boss over each plunger, and a positive housing stop. Support the switch body firmly in a pocket: solder legs must not carry finger force. Use removable shims or an adjustable seat to tune the rest gap so the switch releases reliably. Tune the stop only after measuring actuation and allowed overtravel; no travel specification is established for this exact part.

The switch supplies the tactile snap; the flexure only guides and returns the paddle. A long, gently bending flexure is a better prototype than a very thin crease. Print a small coupon to tune stiffness and layer orientation. Material fatigue and creep need testing; a pivoted paddle with the switch's return force is a fallback if the flexure is inconsistent. No final flexure dimensions or durable-life claim is made yet.

## S3 board

The selected variant reads **S3-N8R2**. Under Espressif's naming this means **8 MB flash and 2 MB PSRAM**, not the 16 MB / 8 MB claimed in the generic AI summary. This is ample for the proposed USB input firmware. Reference: [ESP32-S3-WROOM-1 datasheet](https://www.espressif.com/sites/default/files/documentation/esp32-s3-wroom-1_wroom-1u_datasheet_en.pdf).

The hero image shows two USB-C sockets and a DevKitC-like 44-pin layout. The proposed GPIOs are visibly present, but the image is generic (its module marking differs from the selected variant). Native-USB suitability is promising, not yet electrically proven. Confirm which socket connects to GPIO19/20 versus the USB-UART bridge from the actual board labels or a device-enumeration test; do not assign left/right socket solely from this photo. Use the native port for composite HID. The existing GPIO allocation can remain provisional.

## PS5 TMR stick

This is a **bare active magnetic replacement**, not an assembled five-pin joystick breakout and not a pair of passive potentiometers. Its through-hole legs avoid flex soldering, but its supply voltage, ground/output order, decoupling needs and output range are still unknown. “For PS5” alone does not establish standalone 3.3 V compatibility. Do not connect it to revision A's joystick supply or treat it as a resistor-divider circuit until those details are known.

A breakout or small carrier could join shared supply/ground and expose X, Y and click as five logical wires once the device is characterized. The metal supports are mounting features; not every leg is an independent signal. A conventional passive analog joystick already mounted on a labeled breakout remains the simplest first bring-up choice if buying another part is acceptable.

## Switch stick

The photo shows the expected compact Joy-Con-style package with mounting ears and a flex tail. It does not expose the contacts clearly enough to count them or establish pitch/contact side. Standard Joy-Con-style sticks often have an independent click, but that cannot be assigned to this exact part from the image. Keep the click/pinout pending rather than building around the earlier four-contact assumption.

The screenshot's **selected variant is “Tool kit”**, with a tools-only thumbnail; the large image and AI summary show joysticks too. If using this listing to order, check that the selected variant actually includes sticks. This observation says nothing about which parts the user already owns.

A confirmed matching, preassembled flex-to-header adapter would make this an attractive low-profile option. Do not purchase an arbitrary four-pin FPC adapter based on the previous assumption. Need a close view of the actual flex contacts or a reliable part drawing before selecting the socket.

## Next revision

Rev B should explicitly draw two powered five-terminal encoder modules, six MX keys, and two HUANO COM/NO switch pairs. Keep the joystick carrier provisional. The GPIO mapping stays the same; encoder supplies and mouse-switch terminal labeling change. Revision A's PDF has **not** been revised by this assessment: do not follow its bare-encoder terminal names at an assembled module header.
