use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct TrayMenu {
    pub dashboard_id: String,
    pub quit_id: String,
}

pub fn setup_tray() -> (TrayIcon, TrayMenu) {
    let tray_menu = Menu::new();
    let dashboard_item = MenuItem::new("Open Dashboard", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let dashboard_id = dashboard_item.id().0.clone();
    let quit_id = quit_item.id().0.clone();

    let _ = tray_menu.append_items(&[
        &dashboard_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]);

    // Load the actual app icon
    let icon_data = include_bytes!("../../assets/app.ico");
    let image = image::load_from_memory_with_format(icon_data, image::ImageFormat::Ico)
        .expect("Failed to parse app.ico")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    let icon = Icon::from_rgba(rgba, width, height).expect("Failed to build tray icon");

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Rust+ Companion")
        .with_icon_as_template(true)
        .with_icon(icon)
        .build()
        .expect("Failed to build system tray");

    (
        tray,
        TrayMenu {
            dashboard_id,
            quit_id,
        },
    )
}

pub fn run_event_loop() {
    let (_tray_icon, tray_menu) = setup_tray();
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let mut ui_process: Option<std::process::Child> = None;

    #[allow(deprecated)]
    event_loop
        .run(move |_event, target| {
            target.set_control_flow(winit::event_loop::ControlFlow::Wait);
            handle_menu_event(target, &tray_menu, &mut ui_process);
            handle_tray_click(&mut ui_process);
        })
        .unwrap();
}

fn handle_menu_event(
    target: &winit::event_loop::ActiveEventLoop,
    menu: &TrayMenu,
    ui_process: &mut Option<std::process::Child>,
) {
    let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() else {
        return;
    };
    if event.id.0 == menu.quit_id {
        if let Some(mut child) = ui_process.take() {
            let _ = child.kill();
        }
        target.exit();
    } else if event.id.0 == menu.dashboard_id {
        open_settings(ui_process);
    }
}

fn handle_tray_click(ui_process: &mut Option<std::process::Child>) {
    let Ok(tray_icon::TrayIconEvent::Click {
        button: tray_icon::MouseButton::Left,
        button_state: tray_icon::MouseButtonState::Up,
        ..
    }) = tray_icon::TrayIconEvent::receiver().try_recv()
    else {
        return;
    };
    open_settings(ui_process);
}

fn open_settings(ui_process: &mut Option<std::process::Child>) {
    let should_spawn = match ui_process {
        Some(child) => child.try_wait().map_or(true, |status| status.is_some()),
        None => true,
    };
    if should_spawn && let Ok(executable) = std::env::current_exe() {
        *ui_process = std::process::Command::new(executable)
            .arg("--ui")
            .spawn()
            .ok();
    }
}
