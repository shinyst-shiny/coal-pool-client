use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

pub fn get_config_dir() -> PathBuf {
    let mut config_dir = dirs::config_dir().expect("Failed to get config directory");
    config_dir.push("excalivator");
    std::fs::create_dir_all(&config_dir).expect("Failed to create config directory");
    config_dir
}

pub fn save_last_pubkey(pubkey: &str) {
    let mut config_file = get_config_dir();
    config_file.push("last_pubkey.txt");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(config_file)
        .expect("Failed to open config file");
    file.write_all(pubkey.as_bytes()).expect("Failed to write pubkey");
}

pub fn get_last_pubkey() -> Option<String> {
    let mut config_file = get_config_dir();
    config_file.push("last_pubkey.txt");
    let mut file = File::open(config_file).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;
    Some(contents.trim().to_string())
}