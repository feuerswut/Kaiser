use serde::{Deserialize, Serialize};
use tauri::State;

use kaiser_core::{
    set_display_mode as core_set_display_mode, AudioDevice, AudioFlow, AudioSetting, DisplayMode,
    KaiserProfile,
};
use monarch::{DisplayId, DisplayInfo, Layout, Profile};

use crate::state::AppState;

// ---- DTOs ---------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SnapshotDto {
    pub displays: Vec<DisplayInfo>,
    pub layout: Layout,
    pub profiles: Vec<ProfileDto>,
    pub pending_confirmation: bool,
    pub pending_confirmation_remaining_secs: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileDto {
    pub name: String,
    pub layout: Layout,
    pub audio: Vec<AudioSetting>,
}

fn to_profile_dto(profile: &Profile, store: &kaiser_core::KaiserConfigStore) -> ProfileDto {
    let audio = store
        .load_kaiser_profile(&profile.name)
        .map(|kp| kp.audio)
        .unwrap_or_default();
    ProfileDto { name: profile.name.clone(), layout: profile.layout.clone(), audio }
}

// ---- Display commands ---------------------------------------------------

#[tauri::command]
pub fn get_snapshot(state: State<AppState>) -> Result<SnapshotDto, String> {
    let manager = state.manager.lock().unwrap();
    let store = state.new_store();
    let displays = manager.list_displays().map_err(|e| e.to_string())?;
    let layout = manager.get_layout().map_err(|e| e.to_string())?;
    let profiles = manager
        .list_profiles()
        .iter()
        .map(|p| to_profile_dto(p, &store))
        .collect();
    let pending = manager.has_pending_confirmation();
    let remaining = manager
        .pending_confirmation_remaining()
        .map(|d| d.as_secs_f64());
    Ok(SnapshotDto { displays, layout, profiles, pending_confirmation: pending, pending_confirmation_remaining_secs: remaining })
}

#[tauri::command]
pub fn list_displays(state: State<AppState>) -> Result<Vec<DisplayInfo>, String> {
    let manager = state.manager.lock().unwrap();
    manager.list_displays().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_display(display_id: DisplayId, state: State<AppState>) -> Result<(), String> {
    let mut manager = state.manager.lock().unwrap();
    manager.toggle_display(&display_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_layout(layout: Layout, state: State<AppState>) -> Result<(), String> {
    let mut manager = state.manager.lock().unwrap();
    manager.apply_layout(layout).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_profile(
    name: String,
    audio: Option<Vec<AudioSetting>>,
    state: State<AppState>,
) -> Result<(), String> {
    let manager = state.manager.lock().unwrap();
    let layout = manager.get_layout().map_err(|e| e.to_string())?;
    drop(manager);

    let store = state.new_store();
    let audio = audio.unwrap_or_default();
    store.save_kaiser_profile(&name, KaiserProfile { layout, audio })
        .map_err(|e| e.to_string())?;

    // Also update the Monarch profile list so the manager knows about it
    let mut manager = state.manager.lock().unwrap();
    manager.save_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_profile(name: String, state: State<AppState>) -> Result<(), String> {
    let mut manager = state.manager.lock().unwrap();
    manager.apply_profile(&name).map_err(|e| e.to_string())?;
    drop(manager);

    // Apply audio settings for this profile
    let store = state.new_store();
    if let Some(kaiser_profile) = store.load_kaiser_profile(&name) {
        if !kaiser_profile.audio.is_empty() {
            let audio = state.audio.lock().unwrap();
            apply_audio_settings(&audio, &kaiser_profile.audio);
        }
    }

    Ok(())
}

fn apply_audio_settings(audio: &kaiser_core::AudioManager, settings: &[AudioSetting]) {
    let devices = match audio.list_devices() {
        Ok(d) => d,
        Err(_) => return,
    };
    for setting in settings {
        let matching: Vec<&AudioDevice> = devices
            .iter()
            .filter(|d| {
                d.name.to_lowercase().contains(&setting.pattern.to_lowercase())
                    && match (d.flow, setting.flow) {
                        (AudioFlow::Render, AudioFlow::Render) => true,
                        (AudioFlow::Capture, AudioFlow::Capture) => true,
                        _ => false,
                    }
            })
            .collect();
        for device in matching {
            if let Some(vol) = setting.volume {
                let _ = audio.set_volume(&device.id, vol);
            }
            if let Some(muted) = setting.muted {
                let _ = audio.set_mute(&device.id, muted);
            }
            if setting.set_default == Some(true) {
                let _ = audio.set_default(&device.id);
            }
        }
    }
}

#[tauri::command]
pub fn delete_profile(name: String, state: State<AppState>) -> Result<(), String> {
    let mut manager = state.manager.lock().unwrap();
    manager.delete_profile(&name).map_err(|e| e.to_string())?;
    drop(manager);
    let store = state.new_store();
    store.delete_kaiser_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_profiles(state: State<AppState>) -> Result<Vec<ProfileDto>, String> {
    let manager = state.manager.lock().unwrap();
    let store = state.new_store();
    Ok(manager
        .list_profiles()
        .iter()
        .map(|p| to_profile_dto(p, &store))
        .collect())
}

// ---- Audio commands -----------------------------------------------------

#[tauri::command]
pub fn list_audio_devices(state: State<AppState>) -> Result<Vec<AudioDevice>, String> {
    let audio = state.audio.lock().unwrap();
    audio.list_devices().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_audio_volume(device_id: String, volume: f32, state: State<AppState>) -> Result<(), String> {
    let audio = state.audio.lock().unwrap();
    audio.set_volume(&device_id, volume).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_audio_mute(device_id: String, muted: bool, state: State<AppState>) -> Result<(), String> {
    let audio = state.audio.lock().unwrap();
    audio.set_mute(&device_id, muted).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_default_audio_device(device_id: String, state: State<AppState>) -> Result<(), String> {
    let audio = state.audio.lock().unwrap();
    audio.set_default(&device_id).map_err(|e| e.to_string())
}

// ---- Resolution commands ------------------------------------------------

#[tauri::command]
pub fn list_display_modes(gdi_device_name: String) -> Result<Vec<DisplayMode>, String> {
    kaiser_core::list_display_modes(&gdi_device_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_display_mode(gdi_device_name: String, mode: DisplayMode) -> Result<(), String> {
    core_set_display_mode(&gdi_device_name, &mode).map_err(|e| e.to_string())
}
