use std::{error::Error, time::Duration};

use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

use crate::{HistoryPanel, SettingsPanel};

pub struct TrayRuntime {
    _icon: TrayIcon,
    _timer: slint::Timer,
}

pub fn initialize(
    settings: slint::Weak<SettingsPanel>,
    history: slint::Weak<HistoryPanel>,
) -> Result<TrayRuntime, Box<dyn Error>> {
    let open_settings = MenuItem::new("Настройки", true, None);
    let open_history = MenuItem::new("История", true, None);
    let exit = MenuItem::new("Выход", true, None);
    let settings_id = open_settings.id().clone();
    let history_id = open_history.id().clone();
    let exit_id = exit.id().clone();
    let menu = Menu::with_items(&[&open_settings, &open_history, &exit])?;
    let icon = TrayIconBuilder::new()
        .with_icon(make_icon()?)
        .with_tooltip("SpeakCursor")
        .with_menu(Box::new(menu))
        .build()?;
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == settings_id {
                    if let Some(panel) = settings.upgrade() {
                        let _ = panel.show();
                    }
                } else if event.id == history_id {
                    if let Some(panel) = history.upgrade() {
                        // Opening via the settings callback refreshes the list.
                        // The history panel's own search callback refreshes it too.
                        let _ = panel.show();
                        panel.set_search_query("".into());
                        panel.invoke_search_changed("".into());
                    }
                } else if event.id == exit_id {
                    let _ = slint::quit_event_loop();
                }
            }
        },
    );
    Ok(TrayRuntime {
        _icon: icon,
        _timer: timer,
    })
}

fn make_icon() -> Result<Icon, tray_icon::BadIcon> {
    let mut pixels = Vec::with_capacity(32 * 32 * 4);
    for y in 0..32_i32 {
        for x in 0..32_i32 {
            let dx = x - 16;
            let dy = y - 16;
            let color = if dx * dx + dy * dy <= 14 * 14 {
                if dx.abs() <= 3 && dy.abs() <= 7 {
                    [255, 255, 255, 255]
                } else {
                    [53, 127, 238, 255]
                }
            } else {
                [0, 0, 0, 0]
            };
            pixels.extend_from_slice(&color);
        }
    }
    Icon::from_rgba(pixels, 32, 32)
}
