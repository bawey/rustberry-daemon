extern crate dht22_pi;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use dht22_pi::ReadingError;
use rand::Rng;
use rppal::gpio::{Gpio, Mode};
use rppal::hal::Delay;
use rppal_dht11::Dht11;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    dummy_sensor: bool,
    dht11_pin: u8,
    dht22_pin: u8,
    sensor_query_interval_secs: u64,
    listen_on_loopback_only: bool,
    listen_on_port: u16,
    max_readings_kept: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dummy_sensor: false,
            dht11_pin: 0,
            dht22_pin: 26,
            sensor_query_interval_secs: 60,
            listen_on_loopback_only: false,
            listen_on_port: 8080,
            max_readings_kept: usize::MAX,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct SensorData {
    sensor: &'static str,
    timestamp: u64,
    temperature: f32,
    humidity: f32,
}

impl SensorData {
    fn new(sensor: &'static str, temperature: f32, humidity: f32) -> Self {
        let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        return SensorData { sensor, timestamp, temperature, humidity };
    }
}

// TODO: change the method signature to use more general error type
fn read_data_dummy(_config: &Config) -> Result<SensorData, ReadingError> {
    let sensor = "dummy";
    let mut rng = rand::thread_rng();
    let temperature: f32 = 50f32 - rng.gen::<f32>() * 100f32;
    let humidity: f32 = rng.gen::<f32>() * 100f32;
    Ok(SensorData::new(sensor, temperature, humidity))
}

fn read_data_dht11(config: &Config) -> Result<SensorData, ReadingError> {
    let pin = Gpio::new()?.get(config.dht11_pin)?.into_output_low();
    let pin = Gpio::new()?.get(config.dht11_pin)?.into_io(Mode::Input);
    // Create an instance of the DHT11 device
    let mut dht11 = Dht11::new(pin);
    let mut delay = Delay::new();
    // Perform a sensor reading
    let measurement = dht11.perform_measurement(&mut delay).unwrap();
    println!("{:?}", measurement);
    Ok(SensorData::new("dht11", measurement.temperature as f32, measurement.humidity as f32))
}

fn read_data_dht22(config: &Config) -> Result<SensorData, ReadingError> {
    let result = dht22_pi::read(config.dht22_pin);
    match result {
        Ok(reading) => Ok(SensorData::new("dht22", reading.temperature, reading.humidity)),
        Err(error) => Err(error)
    }
}

pub fn main() {
    const APP_NAME: &str = "rustberry-daemon";
    const CONFIG_NAME: &str = "default";

    let config_file_path = confy::get_configuration_file_path(APP_NAME, CONFIG_NAME).unwrap();
    println!("Using {} as config file path", &config_file_path.as_path().as_os_str().to_str().unwrap());
    let config: Config = confy::load_path(&config_file_path).unwrap();

    // the data to be shared between consumer(s) and producer
    let state = Arc::new(Mutex::new(vec![]));
    let consumer_data = Arc::clone(&state);
    let producer_data = Arc::clone(&state);

    thread::spawn(move || {
        let address = format!("{}:{}",
                              if config.listen_on_loopback_only { "127.0.0.1" } else { "0.0.0.0" },
                              &config.listen_on_port
        );
        println!("Listening on http://{}", address);
        let tcp_listener: TcpListener = TcpListener::bind(address).expect("Failed to bind");
        loop {
            let (mut stream, _) = tcp_listener.accept().expect("Failed to accept");
            let mut buffer = [0; 1024];
            let _ = stream.read(&mut buffer);
            let data_points = match consumer_data.lock() {
                Err(_) => vec![],
                Ok(data) => data.to_vec()
            };
            let serialized = serde_json::to_string(&data_points).unwrap();
            let content_length = serialized.len();
            let headers = format!("Content-Type: application/json; charset=utf-8\r\nContent-Length: {content_length}\r\n");
            let response = format!("HTTP/1.1 200 OK\r\n{headers}\r\n{serialized}");
            let _ = stream.write_all(response.as_bytes());
        }
    });


    loop {
        let mut providers: Vec<fn(&Config) -> Result<SensorData, ReadingError>> = vec![];
        if config.dht22_pin != 0 { providers.push(read_data_dht22) }
        if config.dummy_sensor { providers.push(read_data_dummy) }
        if config.dht11_pin != 0 { providers.push(read_data_dht11) }

        for func in providers {
            match (func)(&config) {
                Ok(reading) => {
                    let mut store = producer_data.lock().unwrap();
                    if store.len() == config.max_readings_kept {
                        store.remove(0);
                    }
                    store.push(reading);
                }
                Err(error) => println!("Failed to read sensor: {:?}", error)
            }
        }
        sleep(Duration::from_secs((&config).sensor_query_interval_secs));
    }
}

