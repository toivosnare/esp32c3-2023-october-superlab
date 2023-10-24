#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use panic_rtt_target as _;

#[rtic::app(device = esp32c3)]
mod app {
    use rtt_target::{rprintln, rtt_init_print};

    use esp32c3_hal::{
        clock::ClockControl,
        gpio::{Gpio7, Gpio9, Input, Output, PullUp, PushPull},
        peripherals::{Peripherals, TIMG0},
        prelude::*,
        timer::{TimerGroup, Wdt},
        IO,
    };
    use rtic_monotonics::{
        esp32c3_systimer::{ExtU64, Systimer},
        Monotonic,
    };
    use shared::shift_register::ShiftRegister;

    #[shared]
    struct Shared {
        off_delay: <Systimer as Monotonic>::Duration,
        on_delay: <Systimer as Monotonic>::Duration,
    }

    #[local]
    struct Local {
        led: Gpio7<Output<PushPull>>,
        button: Gpio9<Input<PullUp>>,
        watchdoggy: Wdt<TIMG0>,
        off_delays: ShiftRegister,
        on_delays: ShiftRegister,
        last_instant: <Systimer as Monotonic>::Instant,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        rtt_init_print!();
        rprintln!(env!("CARGO_CRATE_NAME"));

        let peripherals = Peripherals::take();
        let mut system = peripherals.SYSTEM.split();

        let clocks = ClockControl::max(system.clock_control).freeze();
        let timer_group0 = TimerGroup::new(
            peripherals.TIMG0,
            &clocks,
            &mut system.peripheral_clock_control,
        );

        let mut watchdoggy = timer_group0.wdt;
        watchdoggy.start(30u64.secs());

        let systemtimer_token = rtic_monotonics::create_systimer_token!();
        Systimer::start(cx.core.SYSTIMER, systemtimer_token);

        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        let mut led = io.pins.gpio7.into_push_pull_output();
        led.set_high().unwrap();

        let mut button = io.pins.gpio9.into_pull_up_input();
        button.listen(esp32c3_hal::gpio::Event::FallingEdge);
        button.listen(esp32c3_hal::gpio::Event::RisingEdge);

        let off_delays = ShiftRegister::default();
        let on_delays = ShiftRegister::default();
        let last_instant = Systimer::now();

        blink::spawn().unwrap();

        (
            Shared {
                off_delay: 0u64.secs(),
                on_delay: 0u64.secs(),
            },
            Local {
                led,
                button,
                watchdoggy,
                off_delays,
                on_delays,
                last_instant,
            },
        )
    }

    // #[idle]
    // fn idle(cx: idle::Context) -> ! {
    //     loop {
    //         unsafe {
    //             wfi();
    //         }
    //     }
    // }

    #[task(local = [led], shared = [off_delay, on_delay])]
    async fn blink(mut cx: blink::Context) {
        loop {
            cx.local.led.set_high().unwrap();
            let on_delay = cx.shared.on_delay.lock(|d| *d);
            Systimer::delay(on_delay).await;
            cx.local.led.set_low().unwrap();
            let off_delay = cx.shared.off_delay.lock(|d| *d);
            Systimer::delay(off_delay).await;
        }
    }

    #[task(binds = GPIO, local = [button, watchdoggy, off_delays, on_delays, last_instant], shared = [off_delay, on_delay])]
    fn button(mut cx: button::Context) {
        rprintln!("button press");
        let now = Systimer::now();
        let diff = now - *cx.local.last_instant;

        if cx.local.button.is_low().unwrap() {
            cx.local.off_delays.insert(diff.to_millis());
            cx.shared.off_delay.lock(|d| *d = cx.local.off_delays.avg().millis());
        } else {
            cx.local.on_delays.insert(diff.to_millis());
            cx.shared.on_delay.lock(|d| *d = cx.local.on_delays.avg().millis());
        }
        *cx.local.last_instant = now;
        cx.local.watchdoggy.feed();
        cx.local.button.clear_interrupt();
    }
}
