use std::time::Duration;

use slint::ComponentHandle;

slint::include_modules!();

#[cfg(windows)]
mod windows_focus;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let assistant = AssistantPanel::new()?;
    let recording = RecordingPill::new()?;

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

    assistant.on_close_requested(|| {
        let _ = slint::quit_event_loop();
    });
    assistant.show()?;
    recording.show()?;

    #[cfg(windows)]
    if let Some(expected) = foreground_before {
        windows_focus::report_focus("immediately after showing panels", expected);
    }

    #[cfg(windows)]
    {
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

    let preview = "Напишите короткий ответ на письмо. Подтвердите встречу в четверг и предложите созвониться после обеда.";
    let mut shown = 0;
    let mut activity = 0;
    let assistant_weak = assistant.as_weak();
    let recording_weak = recording.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(65),
        move || {
            if let Some(assistant) = assistant_weak.upgrade() {
                if shown < preview.chars().count() {
                    shown = (shown + 2).min(preview.chars().count());
                    assistant
                        .set_answer_text(preview.chars().take(shown).collect::<String>().into());
                }
            }
            if let Some(recording) = recording_weak.upgrade() {
                activity = (activity + 1) % 8;
                recording.set_activity(activity);
            }
        },
    );

    slint::run_event_loop()?;
    Ok(())
}
