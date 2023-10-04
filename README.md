# esp32c3-rtic-tau

Repository for `esp32c3-rtic-tau` demonstration and assignments.

- `esp32c3`, code for the target.
- `host`, code for the host.
- `shared`, library for shared data structures and communication between the host and target.

## Software requirements

- We flash these examples using `cargo embed`, cargo-subcommand. Obtain the tools by running the following:
  - `cargo install probe-rs --features cli`
- Setup udev rules for probe-rs: <https://probe.rs/docs/getting-started/probe-setup/>
- Refresh udev rules
  - `sudo udevadm control --reload-rules && sudo udevadm trigger`
  - WSL2 only: if the above fails on WSL2, run `sudo service udev restart` then try again

## Running the examples

ESP32-C3 programs can be run on the target device as follows.

- Change to target directory:
  - `cd esp32c3`
- Use `cargo embed` to build & run an example, e.g.,
  - `cargo embed --example blinky`

## Using FTDI to connect serial to USB

You cannot put serial wires into a USB port and expect it to work. Therefore we must use a small FTDI2232HL board to
fill in the gaps.

We setup the board based on the FTDI2232H/HL's datasheet:
<https://ftdichip.com/wp-content/uploads/2020/07/DS_FT2232H.pdf>

For example, for a setup where IO pin 0 is allocated for UART TX and IO pin 1 is allocated for UART RX, the connections
between the FTDI and the ESP32-C3 are as follows:

| ESP32-C3 | FTDI |
| - | - |
| GND | GND |
| IO0 | AD1 |
| IO1 | AD0 |

Be mindful of the fact that the TX from the microcontroller will be the RX for the host and vice versa.
