extern crate dht22_pi;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Mutex, Arc};
use std::thread;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use dht22_pi::ReadingError;

use rand::Rng;
use serde::Serialize;

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
fn read_data_dummy() -> Result<SensorData, ReadingError> {
    let sensor = "dummy";
    let mut rng = rand::thread_rng();
    let temperature: f32 = 50f32 - rng.gen::<f32>() * 100f32;
    let humidity: f32 = rng.gen::<f32>() * 100f32;
    Ok(SensorData::new(sensor, temperature, humidity))
}

fn read_data_dht22() -> Result<SensorData, ReadingError> {
    // TODO: make the pin number configurable
    let result = dht22_pi::read(26);
    return match result {
        Ok(reading) => Ok(SensorData::new("dht22", reading.temperature, reading.humidity)),
        Err(error) => Err(error)
    };
}

pub fn main() {
    // the data to be shared between consumer(s) and producer
    let state = Arc::new(Mutex::new(vec![]));

    let consumer_data = Arc::clone(&state);
    let producer_data = Arc::clone(&state);

    thread::spawn(move || {
        // TODO: make configurable!
        let addr: SocketAddr = ([0, 0, 0, 0], 8080).into();
        println!("Listening on http://{}", addr);
        let tcp_listener: TcpListener = TcpListener::bind(&addr).expect("Failed to bind");
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
            let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\n\r\n{serialized}");
            let _ = stream.write_all(response.as_bytes());
        }
    });

    loop {
        // TODO: make the list configurable
        for func in [read_data_dht22, read_data_dummy] {
            match (func)() {
                Ok(reading) => {
                    let mut store = producer_data.lock().unwrap();
                    // TODO: use a fixed-size of buffer (configurable)
                    store.push(reading);
                }
                Err(error) => println!("Failed to read sensor: {:?}", error)
            }
        }
        //TODO: make the interval configurable
        sleep(Duration::from_secs(15));
    }
}

