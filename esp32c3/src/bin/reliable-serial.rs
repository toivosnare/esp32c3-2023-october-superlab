#![no_main]
#![no_std]
#![feature(type_alias_impl_trait, exclusive_range_pattern)]

use panic_rtt_target as _;

#[rtic::app(device = esp32c3)]
mod app {
    use chrono::{self, DateTime, TimeZone, Timelike, Utc};
    use esp32c3_hal::{
        clock::ClockControl,
        gpio::{Gpio7, Output, PushPull},
        peripherals::{Peripherals, TIMG0, TIMG1},
        prelude::*,
        rmt::{Channel0, Rmt},
        timer::{Timer, Timer0, TimerGroup},
        Rtc, IO,
    };
    use esp_hal_smartled::{smartLedAdapter, SmartLedsAdapter};
    use rgb::RGB8;
    use rtt_target::{rprintln, rtt_init_print};
    use smart_leds::{brightness, gamma, SmartLedsWrite};

    type BlinkLed = Gpio7<Output<PushPull>>;
    type OnOffTimer = Timer<Timer0<TIMG0>>;
    type PeriodTimer = Timer<Timer0<TIMG1>>;
    type RgbLed = SmartLedsAdapter<Channel0<0>, 0, 25>;
    type Duration = fugit::MicrosDurationU64;

    pub struct TimeReference {
        date_time: DateTime<Utc>,
        rtc_value: u64,
    }

    pub struct Blinker {
        led: BlinkLed,
        on_off_timer: OnOffTimer,
        period_timer: PeriodTimer,
        on: bool,
        saved_duration: Duration,
        saved_frequency: Duration,
    }

    #[shared]
    struct Shared {
        blinker: Blinker,
    }

    #[local]
    struct Local {
        time_reference: TimeReference,
        rtc: Rtc<'static>,
        rgb_led: RgbLed,
    }

    #[init]
    fn init(_: init::Context) -> (Shared, Local) {
        rtt_init_print!();
        rprintln!(env!("CARGO_CRATE_NAME"));

        let peripherals = Peripherals::take();
        let mut system = peripherals.SYSTEM.split();
        let clocks = ClockControl::max(system.clock_control).freeze();
        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        let rmt = Rmt::new(
            peripherals.RMT,
            80u32.MHz(),
            &mut system.peripheral_clock_control,
            &clocks,
        )
        .unwrap();

        let time_reference = TimeReference {
            date_time: Utc.timestamp_nanos(0),
            rtc_value: 0,
        };

        let led = io.pins.gpio7.into_push_pull_output();
        let timer_group0 = TimerGroup::new(
            peripherals.TIMG0,
            &clocks,
            &mut system.peripheral_clock_control,
        );
        let on_off_timer = timer_group0.timer0;
        let timer_group1 = TimerGroup::new(
            peripherals.TIMG1,
            &clocks,
            &mut system.peripheral_clock_control,
        );
        let period_timer = timer_group1.timer0;

        let blinker = Blinker {
            led,
            on_off_timer,
            period_timer,
            on: false,
            saved_duration: Duration::from_ticks(0),
            saved_frequency: Duration::from_ticks(0),
        };

        let rtc = Rtc::new(peripherals.RTC_CNTL);
        let rgb_led = <smartLedAdapter!(0, 1)>::new(rmt.channel0, io.pins.gpio2);

        (
            Shared { blinker },
            Local {
                time_reference,
                rtc,
                rgb_led,
            },
        )
    }

    #[task(binds = UART0, local = [time_reference, rtc, rgb_led], shared = [blinker])]
    fn broker(mut cx: broker::Context) {
        // TODO: get messages from UART.
        #[allow(dead_code)]
        enum Message {
            SetTimeReference(DateTime<Utc>),
            TurnBlinkerOff,
            TurnBlinkerOnNow(Duration, Duration),
            TurnBlinkerOnAfterDelay(Duration, Duration, Duration),
            TurnRgbLedOff,
            TurnRgbLedOn,
        }
        let message = Message::SetTimeReference(Utc.timestamp_nanos(0));

        match message {
            Message::SetTimeReference(new_reference_date_time) => {
                set_time_reference(
                    cx.local.time_reference,
                    cx.local.rtc,
                    new_reference_date_time,
                );
            }
            Message::TurnBlinkerOff => {
                cx.shared.blinker.lock(|blinker| turn_blinker_off(blinker));
            }
            Message::TurnBlinkerOnNow(duration, frequency) => {
                cx.shared
                    .blinker
                    .lock(|blinker| turn_blinker_on_now(blinker, duration, frequency));
            }
            Message::TurnBlinkerOnAfterDelay(duration, frequency, delay) => {
                cx.shared.blinker.lock(|blinker| {
                    turn_blinker_on_after_delay(blinker, duration, frequency, delay)
                });
            }
            Message::TurnRgbLedOff => turn_rgb_led_off(cx.local.rgb_led),
            Message::TurnRgbLedOn => {
                turn_rgb_led_on(cx.local.time_reference, cx.local.rtc, cx.local.rgb_led)
            }
        }
    }

    #[task(binds = TG0_T0_LEVEL, local = [], shared = [blinker])]
    fn on_off_timer_isr(mut cx: on_off_timer_isr::Context) {
        rprintln!("Handling on_off_timer interrupt.");
        cx.shared.blinker.lock(|blinker| {
            if blinker.on {
                turn_blinker_off(blinker);
            } else {
                turn_blinker_on_now(blinker, blinker.saved_duration, blinker.saved_frequency);
            }
            blinker.on_off_timer.clear_interrupt();
        });
    }

    #[task(binds = TG1_T0_LEVEL, local = [], shared = [blinker])]
    fn period_timer_isr(mut cx: period_timer_isr::Context) {
        rprintln!("Handling period_timer interrupt.");
        cx.shared.blinker.lock(|blinker| {
            blinker.led.toggle().unwrap();
            blinker.period_timer.clear_interrupt();
            blinker.period_timer.set_alarm_active(true);
        });
    }

    fn set_time_reference(
        time_reference: &mut TimeReference,
        rtc: &Rtc,
        new_reference_date_time: DateTime<Utc>,
    ) {
        time_reference.date_time = new_reference_date_time;
        time_reference.rtc_value = rtc.get_time_ms();
        rprintln!(
            "Associating current RTC value ({}) to date time {}.",
            time_reference.rtc_value,
            time_reference.date_time
        );
    }

    fn turn_blinker_off(blinker: &mut Blinker) {
        rprintln!("Turning blinker off.");
        blinker.led.set_low().unwrap();
        blinker.on_off_timer.unlisten();
        blinker.on_off_timer.set_counter_active(false);
        blinker.period_timer.unlisten();
        blinker.period_timer.set_counter_active(false);
        blinker.on = false;
    }

    fn turn_blinker_on_now(blinker: &mut Blinker, duration: Duration, frequency: Duration) {
        rprintln!(
            "Turning blinker on for {} with period of {}.",
            duration,
            frequency
        );
        blinker.led.set_low().unwrap();
        blinker.on_off_timer.listen();
        blinker.on_off_timer.start(duration);
        blinker.period_timer.listen();
        blinker.period_timer.start(frequency);
        blinker.on = true;
    }

    fn turn_blinker_on_after_delay(
        blinker: &mut Blinker,
        duration: Duration,
        frequency: Duration,
        delay: Duration,
    ) {
        rprintln!(
            "Turning blinker on for {} with period of {} after delay of {}.",
            duration,
            frequency,
            delay
        );
        blinker.led.set_low().unwrap();
        blinker.on_off_timer.listen();
        blinker.on_off_timer.start(delay);
        blinker.period_timer.unlisten();
        blinker.period_timer.set_counter_active(false);
        blinker.on = false;
        blinker.saved_duration = duration;
        blinker.saved_frequency = frequency;
    }

    fn turn_rgb_led_off(rgb_led: &mut RgbLed) {
        rprintln!("Turning RGB LED off.");
        rgb_led
            .write([RGB8 { r: 0, g: 0, b: 0 }].iter().cloned())
            .unwrap();
    }

    fn get_date_time(time_reference: &TimeReference, rtc: &Rtc) -> DateTime<Utc> {
        let diff_ms = rtc.get_time_ms() - time_reference.rtc_value;
        let diff = chrono::Duration::milliseconds(diff_ms as i64);
        return time_reference.date_time.checked_add_signed(diff).unwrap();
    }

    fn turn_rgb_led_on(time_reference: &TimeReference, rtc: &Rtc, rgb_led: &mut RgbLed) {
        rprintln!("Turning RGB LED on.");
        let hour = get_date_time(time_reference, rtc).time().hour();
        let color = match hour {
            3..9 => RGB8 {
                r: 0xF8,
                g: 0xF3,
                b: 0x2B,
            },
            9..15 => RGB8 {
                r: 0x9C,
                g: 0xFF,
                b: 0xFA,
            },
            15..21 => RGB8 {
                r: 0x05,
                g: 0x3C,
                b: 0x5E,
            },
            21..24 | 0..3 => RGB8 {
                r: 0x31,
                g: 0x08,
                b: 0x1F,
            },
            _ => panic!("Invalid hour value."),
        };
        rgb_led
            .write(brightness(gamma([color].iter().cloned()), 10))
            .unwrap();
    }
}
