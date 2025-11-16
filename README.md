# apitor

This repo contains some Rust code to drive the Apitor Robot Q using a gamepad via BLE; this way you don't need the mobile / tablet app which
is [probably for the best](https://www.ftc.gov/legal-library/browse/cases-proceedings/apitor).

# Usage

- Ensure your bluetooth is on
- Ensure your gamepad is plugged in / paired
- Turn on the Apitor
- `cargo run`

Then you can use the thumb sticks to drive the motors (like a tracked vehicle).

The two LEDs will cycle in some kinda sequence when idle- when the motors are in use, a given LED will be red when the motor of the same side
is being driven forward, blue when the motor of the same side is being driven rearward and green when they're not being driven, but the motor
on the other side is.

The data from the sensors is being consumed, but I have seen that the notification stream sometimes stops after a time (maybe needs re-requesting?)
and at any rate, I haven't attempted to actually decode it (I think my sensors are flaky, only the proximity sensor shows anything and it's only like
3 different values).
