use winit::event_loop::{EventLoop, ControlFlow, ActiveEventLoop};
use winit::window::{WindowAttributes, Window};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;

#[derive(Default)]
struct App {
    window: Option<Window>,
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attributes = WindowAttributes::default().with_title("Test");
            let window = event_loop.create_window(attributes).unwrap();
            self.window = Some(window);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: winit::window::WindowId, event: WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }
}
fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
