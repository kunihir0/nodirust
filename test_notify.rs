use notify_rust::Notification;
fn main() {
    Notification::new().summary("Test").body("This is a test").appname("Finder").show().unwrap();
}
