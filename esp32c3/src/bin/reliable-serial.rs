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
        peripherals::{Peripherals, TIMG0, TIMG1, UART0},
        prelude::*,
        rmt::{Channel0, Rmt},
        timer::{Timer, Timer0, TimerGroup},
        uart::{
            config::{Config, DataBits, Parity, StopBits},
            TxRxPins, UartRx, UartTx,
        },
        Rtc, Uart, IO,
    };
    use esp_hal_smartled::{smartLedAdapter, SmartLedsAdapter};
    use rgb::RGB8;
    use rtt_target::{rprintln, rtt_init_print};
    use smart_leds::{brightness, gamma, SmartLedsWrite};

    use core::mem::size_of;
    use corncobs::max_encoded_len;
    const IN_SIZE: usize = max_encoded_len(size_of::<shared::Command>() + size_of::<u32>());
    const OUT_SIZE: usize = max_encoded_len(size_of::<Response>() + size_of::<u32>());

    // use shared::date_time::UtcDateTime;
    // use shared::{deserialize_crc_cobs, serialize_crc_cobs, Message, Response};
    use shared::{Message, Response};
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
        tx: UartTx<'static, UART0>,
        rx: UartRx<'static, UART0>,
        // sender: Sender<'static, u8, CAPACITY>,
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

        let config = Config {
            baudrate: 115200,
            data_bits: DataBits::DataBits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::STOP1,
        };

        let pins = TxRxPins::new_tx_rx(
            io.pins.gpio0.into_push_pull_output(),
            io.pins.gpio1.into_floating_input(),
        );

        let mut uart0 = Uart::new_with_config(
            peripherals.UART0,
            config,
            Some(pins),
            &clocks,
            &mut system.peripheral_clock_control,
        );

        // !!!
        uart0.set_rx_fifo_full_threshold(1).unwrap();
        uart0.listen_rx_fifo_full();

        let (tx, rx) = uart0.split();

        (
            Shared { blinker },
            Local {
                time_reference,
                rtc,
                rgb_led,
                tx,
                rx,
                // sender,
            },
        )
    }

    #[task(binds = UART0, local = [time_reference, rtc, rgb_led, rx, tx], shared = [blinker])]
    fn broker(mut cx: broker::Context) {
        let mut rx_buffer = [0u8; IN_SIZE];
        let mut tx_buffer = [0u8; OUT_SIZE];
        let mut index: usize = 0;

        while let Ok(byte) = nb::block!(cx.local.rx.read()) {
            // overflow
            if index >= IN_SIZE {
                send_error_response(cx.local.tx, Response::NotOK, &mut tx_buffer);
                index = 0;
                continue;
            }

            rx_buffer[index] = byte;
            index += 1;

            if byte == 0 {
                match shared::deserialize_crc_cobs::<shared::Command>(&mut rx_buffer[..index]) {
                    Ok(command) => {
                        let response = process_command(&mut cx, command);
                        let cobbed_response_data =
                            shared::serialize_crc_cobs(&response, &mut tx_buffer);
                        match cobbed_response_data {
                            Ok(cobbed_response_data) => {
                                for &byte in cobbed_response_data {
                                    nb::block!(cx.local.tx.write(byte)).unwrap();
                                }
                            }
                            Err(_) => {
                                send_error_response(
                                    cx.local.tx,
                                    Response::SerializationError,
                                    &mut tx_buffer,
                                );
                            }
                        }
                    }
                    Err(_) => {
                        send_error_response(cx.local.tx, Response::ParseError, &mut tx_buffer);
                    }
                }
                index = 0;
            }
        }

        cx.local.rx.reset_rx_fifo_full_interrupt();
    }

    // redundant, could have been generalised into common send function
    fn send_error_response(
        tx: &mut UartTx<UART0>,
        response: Response,
        tx_buffer: &mut [u8; OUT_SIZE],
    ) {
        let cobbed_response = shared::serialize_crc_cobs(&response, tx_buffer);
        if let Ok(cobbed_response) = cobbed_response {
            for &byte in cobbed_response {
                nb::block!(tx.write(byte)).unwrap();
            }
        }
    }

    fn process_command(cx: &mut broker::Context, command: shared::Command) -> Response {
        use rtic::Mutex;

        match command {
            shared::Command::Set(id, message, dev_id) => match message {
                Message::SetTimeReference(datetime) => {
                    set_time_reference(
                        &mut *cx.local.time_reference,
                        cx.local.rtc,
                        datetime.into(),
                    );
                    // set_time_reference(&cx.local.time_reference, cx.local.rtc, datetime.into());
                    Response::SetOk
                }
                Message::TurnBlinkerOff => {
                    cx.shared.blinker.lock(|blinker| turn_blinker_off(blinker));
                    Response::SetOk
                }
                Message::TurnBlinkerOnNow(dur, freq) => {
                    let duration = Duration::from_ticks(dur);
                    let frequency = Duration::from_ticks(freq);
                    cx.shared
                        .blinker
                        .lock(|blinker| turn_blinker_on_now(blinker, duration, frequency));
                    Response::SetOk
                }
                Message::TurnBlinkerOnAfterDelay(dur, freq, delay) => {
                    let duration = Duration::from_ticks(dur);
                    let frequency = Duration::from_ticks(freq);
                    let delay = Duration::from_ticks(delay);
                    cx.shared.blinker.lock(|blinker| {
                        turn_blinker_on_after_delay(blinker, duration, frequency, delay)
                    });
                    Response::SetOk
                }
                Message::TurnRgbLedOff => {
                    turn_rgb_led_off(cx.local.rgb_led);
                    Response::SetOk
                }
                Message::TurnRgbLedOn => {
                    turn_rgb_led_on(cx.local.time_reference, cx.local.rtc, cx.local.rgb_led);
                    Response::SetOk
                }
            },
            shared::Command::Get(id, parameter, dev_id) => match id {
                // ranges for room for future expansion close by
                10..=19 => get_id1_data(parameter, dev_id),
                // 20..=29 => get_id2_data(parameter, dev_id),
                // 30..=39 => get_id3_data(parameter, dev_id),
                // 40..=49 => get_id4_data(parameter, dev_id),
                _ => Response::Illegal,
            },
        }
    }

    fn get_id1_data(parameter: u32, dev_id: u32) -> Response {
        // logic to handle data retrieval for message id 1
        let data = 0; // placeholder
        Response::Data(10, parameter, data, dev_id)
    }

    // fn get_id2_data(parameter: u32, dev_id: u32) -> Response {
    // fn get_id3_data(parameter: u32, dev_id: u32) -> Response {
    // fn get_id4_data(parameter: u32, dev_id: u32) -> Response {

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
