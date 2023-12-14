# EX3 Critical System

This is the firmware for the sensor nodes. Had to be its own package because it is using the ESP IDF framework. It uses the riscv32imc-esp-espidf build target among things so I couldn't get it working in the same package as the other ESP32 code. To build and run:
```
cargo build
espflash flash target/riscv32imc-esp-espidf/debug/ex3
espflash monitor
```
