//! Prints time in milliseconds from the RTC Timer

#![no_std]
#![no_main]

use esp32c3_hal::{clock::ClockControl, peripherals::Peripherals, prelude::*, Delay, Rtc};
use panic_rtt_target as _;

use rtt_target::{rprintln, rtt_init_print};

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let rtc = Rtc::new(peripherals.RTC_CNTL);
    let mut delay = Delay::new(&clocks);

    rtt_init_print!();
    rprintln!("rtc_time");

    loop {
        rprintln!("rtc time in milliseconds is {}", rtc.get_time_ms());
        delay.delay_ms(1000u32);
    }
}
