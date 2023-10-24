#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use panic_rtt_target as _;

#[rtic::app(device = esp32c3)]
mod app {
    use rtt_target::{rprintln, rtt_init_print};

    use esp32c3_hal::{
        gpio::{Gpio7, Gpio9, Input, Output, PullUp, PushPull},
        peripherals::{Peripherals, TIMG0},
        prelude::*,
        IO,
        timer::{TimerGroup, Wdt},
        clock::ClockControl,
    };
    use rtic_monotonics::{
        esp32c3_systimer::{ExtU64, Systimer},
        Monotonic,
    };
    use shared::shift_register::ShiftRegister;

    #[shared]
    struct Shared {
        delay: <Systimer as Monotonic>::Duration,
    }

    #[local]
    struct Local {
        led: Gpio7<Output<PushPull>>,
        button: Gpio9<Input<PullUp>>,
        register: ShiftRegister,
        last_instant: <Systimer as Monotonic>::Instant,
        watchdoggy: Wdt<TIMG0>,
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

        let register = ShiftRegister::default();
        let last_instant = Systimer::now();

        blink::spawn().unwrap();

        (
            Shared { delay: 0u64.secs() },
            Local {
                led,
                button,
                register,
                last_instant,
                watchdoggy,
            },
        )
    }

    #[task(local = [led], shared = [delay])]
    async fn blink(mut cx: blink::Context) {
        loop {
            cx.local.led.toggle().unwrap();
            let delay = cx.shared.delay.lock(|d| *d);
            Systimer::delay(delay).await;
        }
    }

    #[task(binds = GPIO, local = [button, register, last_instant, watchdoggy], shared = [delay])]
    fn button(mut cx: button::Context) {
        rprintln!("button press");

        let now = Systimer::now();
        let diff = now - *cx.local.last_instant;
        cx.local.register.insert(diff.to_millis());

        cx.shared
            .delay
            .lock(|d| *d = cx.local.register.avg().millis());

        *cx.local.last_instant = now;

        cx.local.watchdoggy.feed();
        cx.local.button.clear_interrupt();
    }
}
