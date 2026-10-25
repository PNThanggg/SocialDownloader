use std::sync::Mutex;

use tauri::window::{Effect, EffectsBuilder};
use tauri::{Manager, State, WebviewWindow};

/// Holds the effect that was applied to the main window ("mica" | "acrylic" | "none").
pub struct WindowEffect(pub Mutex<&'static str>);

impl Default for WindowEffect {
    fn default() -> Self {
        Self(Mutex::new("none"))
    }
}

#[cfg(windows)]
fn windows_build() -> u32 {
    use windows::Wdk::System::SystemServices::RtlGetVersion;
    use windows::Win32::System::SystemInformation::OSVERSIONINFOW;

    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a valid, correctly sized OSVERSIONINFOW.
    let status = unsafe { RtlGetVersion(&mut info) };
    if status.is_ok() { info.dwBuildNumber } else { 0 }
}

#[cfg(not(windows))]
fn windows_build() -> u32 {
    0
}

fn try_effect(window: &WebviewWindow, effect: Effect) -> bool {
    window
        .set_effects(EffectsBuilder::new().effect(effect).build())
        .is_ok()
}

/// Apply Mica (Windows 11) or Acrylic, falling back to "none".
pub fn apply(window: &WebviewWindow) -> &'static str {
    let result = if windows_build() >= 22000 && try_effect(window, Effect::Mica) {
        "mica"
    } else if try_effect(window, Effect::Acrylic) {
        "acrylic"
    } else {
        "none"
    };
    if let Some(state) = window.try_state::<WindowEffect>() {
        *state.0.lock().unwrap() = result;
    }
    result
}

#[tauri::command]
pub fn window_effect(state: State<'_, WindowEffect>) -> String {
    state.0.lock().unwrap().to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_effect_is_none() {
        assert_eq!(*super::WindowEffect::default().0.lock().unwrap(), "none");
    }
}
