use notify_rust::Notification;
fn main() {
    let handle = Notification::new()
        .summary("Test action")
        .body("Click me")
        .show()
        .unwrap();

    handle.wait_for_action(|action| {
        println!("Action clicked: {}", action);
    });
}
