use keyring::Entry;
fn main() {
    let entry = Entry::new("nodirust_app", "steam_token").unwrap();
    println!("Get: {:?}", entry.get_password());
}
