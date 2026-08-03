#[tauri::command]
fn greet() -> String {
    "Hello from inwdeck!".to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_returns_inwdeck_message() {
        let result = greet();
        assert!(result.contains("inwdeck"));
    }
}
