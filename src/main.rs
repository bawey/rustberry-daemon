extern crate dht22_pi;

fn main() {
    loop {
        let result = dht22_pi::read(26);
        match result{
            Ok(reading) => println!("Read the following: {:?}", reading),
            Err(error) => println!("Got an error {:?}", error)
        };
    }
}