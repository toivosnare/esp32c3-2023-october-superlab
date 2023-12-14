use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        delay::{Delay, FreeRtos},
        i2c::{I2cConfig, I2cDriver},
        prelude::*,
    },
    mqtt::client::{EspMqttClient, EspMqttMessage, Event, MqttClientConfiguration, QoS},
    nvs::EspDefaultNvsPartition,
    sys::EspError,
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use serde::Serialize;
use shtcx::PowerMode;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const MQTT_URI: &str = env!("MQTT_URI");

fn callback(_: &Result<Event<EspMqttMessage>, EspError>) {}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let i2c_config = I2cConfig::new().baudrate(KiloHertz(100).into());
    let i2c_driver = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio10,
        peripherals.pins.gpio8,
        &i2c_config,
    )?;
    let mut delay = Delay::new_default();
    let mut sht = shtcx::shtc3(i2c_driver);

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASSWORD.into(),
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    log::info!("Wifi started");

    wifi.connect()?;
    log::info!("Wifi connected");

    wifi.wait_netif_up()?;
    log::info!("Wifi netif up");

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    log::info!("Wifi DHCP info: {:?}", ip_info);

    let conf = MqttClientConfiguration::default();
    let mut client = EspMqttClient::new(MQTT_URI, &conf, callback).unwrap();

    #[derive(Serialize)]
    pub enum Sensor {
        Rear,
        Top,
        FrontLeft,
        FrontRight,
    }

    #[derive(Serialize)]
    pub struct Message<'a> {
        pub client_id: &'a str,
        pub sensor: Sensor,
        pub temperature: f32,
        pub humidity: f32,
    }

    let mut message = Message {
        client_id: "client0",
        sensor: Sensor::Rear,
        temperature: 0.0,
        humidity: 0.0,
    };

    loop {
        FreeRtos::delay_ms(3000);
        let measurement = match sht.measure(PowerMode::NormalMode, &mut delay) {
            Err(_) => continue,
            Ok(m) => m,
        };
        message.temperature = measurement.temperature.as_degrees_celsius();
        message.humidity = measurement.humidity.as_percent();
        let message_string = serde_json::to_string(&message).unwrap();
        log::info!("msg={}", message_string);

        client
            .publish(
                "measurements",
                QoS::AtMostOnce,
                false,
                message_string.as_bytes(),
            )
            .unwrap();
    }
}
