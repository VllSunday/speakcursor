use std::time::Duration;

use slint::ComponentHandle;

slint::include_modules!();

mod history_ui;
mod settings_ui;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod windows_focus;
#[cfg(windows)]
mod windows_hotkeys;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let assistant = AssistantPanel::new()?;
    let recording = RecordingPill::new()?;
    let settings = SettingsPanel::new()?;
    let history = HistoryPanel::new()?;
    #[cfg(windows)]
    let about = AboutPanel::new()?;
    settings_ui::initialize(&settings)?;
    history_ui::initialize(&history, &settings);
    #[cfg(windows)]
    let _tray = tray::initialize(settings.as_weak(), history.as_weak(), about.as_weak())?;

    // These positions only make the two independent windows easy to inspect in
    // the prototype. Production placement will use the active monitor geometry.
    assistant
        .window()
        .set_position(slint::PhysicalPosition::new(180, 24));
    recording
        .window()
        .set_position(slint::PhysicalPosition::new(300, 450));

    #[cfg(windows)]
    let foreground_before = if std::env::args().any(|arg| arg == "--focus-probe") {
        eprintln!("Focus another application now; the panels will appear in three seconds.");
        std::thread::sleep(Duration::from_secs(3));
        Some(windows_focus::foreground_window())
    } else {
        None
    };

    let assistant_weak = assistant.as_weak();
    assistant.on_close_requested(move || {
        if let Some(panel) = assistant_weak.upgrade() {
            let _ = panel.hide();
        }
    });
    let settings_weak = settings.as_weak();
    let assistant_weak = assistant.as_weak();
    assistant.on_open_error_settings(move || {
        if let (Some(settings), Some(alert)) = (settings_weak.upgrade(), assistant_weak.upgrade()) {
            settings.set_page(alert.get_error_page());
            let _ = settings.show();
            let _ = alert.hide();
        }
    });

    #[cfg(windows)]
    let focus_probe = foreground_before.is_some();
    #[cfg(not(windows))]
    let focus_probe = false;
    if focus_probe {
        assistant.show()?;
        recording.show()?;
    } else {
        settings.show()?;
    }

    #[cfg(windows)]
    windows_hotkeys::start(recording.as_weak(), assistant.as_weak(), settings.as_weak());

    #[cfg(windows)]
    if let Some(expected) = foreground_before {
        windows_focus::report_focus("immediately after showing panels", expected);
    }

    #[cfg(windows)]
    if focus_probe {
        let assistant_weak = assistant.as_weak();
        let recording_weak = recording.as_weak();
        // Slint's native handles are available only after the event loop has
        // created the windows. This is a focus probe, not a proven guarantee
        // against activation during the first show(); verify on Windows.
        slint::Timer::single_shot(Duration::from_millis(10), move || {
            if let Some(panel) = assistant_weak.upgrade() {
                if let Err(error) = windows_focus::make_non_activating(panel.window()) {
                    eprintln!("could not mark assistant panel non-activating: {error}");
                }
            }
            if let Some(panel) = recording_weak.upgrade() {
                if let Err(error) = windows_focus::make_non_activating(panel.window()) {
                    eprintln!("could not mark recording pill non-activating: {error}");
                }
            }
        });

        if let Some(expected) = foreground_before {
            slint::Timer::single_shot(Duration::from_millis(250), move || {
                windows_focus::report_focus("after 250 ms", expected);
            });
        }
    }

    let mut activity = 0;
    let recording_weak = recording.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(65),
        move || {
            if let Some(recording) = recording_weak.upgrade() {
                activity = (activity + 1) % 8;
                recording.set_activity(activity);
            }
        },
    );

    slint::run_event_loop_until_quit()?;
    Ok(())
}
