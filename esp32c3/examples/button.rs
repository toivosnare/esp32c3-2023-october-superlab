//! panic
//!
//! Run on target:
//!
//! cargo embed --example panic
//!
//! Showcases basic panic handling

#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

// bring in panic handler
use panic_rtt_target as _;

#[rtic::app(device = esp32c3)]
mod app {
    use rtt_target::{rprintln, rtt_init_print};

    // to bring in interrupt vector initialization
    use esp32c3_hal::{
        self as _,
        clock::ClockControl,
        gpio::{Gpio9, Input, PullUp},
        peripherals::Peripherals,
        prelude::*,
        IO,
    };

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        button: Gpio9<Input<PullUp>>,
    }

    #[init]
    fn init(_: init::Context) -> (Shared, Local) {
        rtt_init_print!();
        rprintln!(env!("CARGO_CRATE_NAME"));

        let peripherals = Peripherals::take();
        let system = peripherals.SYSTEM.split();
        let _ = ClockControl::max(system.clock_control).freeze();

        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
        let mut button = io.pins.gpio9.into_pull_up_input();
        button.listen(esp32c3_hal::gpio::Event::FallingEdge);

        #[allow(unreachable_code)]
        (Shared {}, Local { button })
    }

    #[task(binds = GPIO, local = [button])]
    fn button(cx: button::Context) {
        rprintln!("button press");
        cx.local.button.clear_interrupt();
    }
}
