# rustberry-daemon
Rust snippets cobbled together to read some sensor data and expose it.

## Installation steps
1. Build the binary 
   - `cargo build --release`
2. Set [file's capabilities](https://stackoverflow.com/questions/7635515/how-to-set-cap-sys-nice-capability-to-a-linux-user):
   - `sudo setcap 'cap_sys_nice=eip' target/release/rustberry-daemon`
3. Add a `systemctl` unit or whatever