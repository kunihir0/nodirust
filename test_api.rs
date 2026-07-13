use std::fs;
use std::env;

fn main() {
    let home = env::var("HOME").unwrap();
    let config_path = format!("{}/Library/Application Support/com.nodirust.nodirust/config.json", home);
    let contents = fs::read_to_string(config_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let token = config["steam_token"].as_str().unwrap();
    
    println!("Token length: {}", token.len());
    
    // Test API
    let url = "https://companion-rust.facepunch.com/api/servers";
    let client = reqwest::blocking::Client::new();
    let res = client.get(url).header("x-facepunch-token", token).send().unwrap();
    println!("Status: {}", res.status());
    println!("Headers: {:?}", res.headers());
    println!("Body: {}", res.text().unwrap());
}
