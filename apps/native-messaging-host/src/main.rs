use lnwdeck_native_messaging_host::process_message;
use lnwdeck_native_messaging_host::protocol::{read_message, write_message};
use std::io::{self, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    loop {
        match read_message(&mut reader) {
            Ok(msg) => {
                let response = process_message(&msg);
                if let Err(e) = write_message(&mut writer, &response) {
                    eprintln!("native host write error: {e}");
                    break;
                }
                writer.flush().ok();
            }
            Err(e) => {
                eprintln!("native host read error: {e}");
                break;
            }
        }
    }
}
