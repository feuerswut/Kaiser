#![cfg(target_os = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use monarch::{DisplayBackend, DisplayInfo, Layout, ManagerError};
use serde::{Deserialize, Serialize};

use super::apply::{
    active_color_state_signature, capture_sdr_gamma_ramps, force_topology_extend,
    gamma_ramp_looks_identity, reapply_color_calibration_for_active_with_cached_sdr,
    GammaRampKey, GammaRampWords,
};
use super::enumerate::query_active_topology;
use super::win32_types::TopologySnapshot;

#[derive(Debug, Default, Serialize, Deserialize)]
struct SerializableRawSnapshot {
    paths: Vec<serde_json::Value>,
    modes: Vec<serde_json::Value>,
}

struct BackendCache {
    last_snapshot: Option<TopologySnapshot>,
    last_color_state_signature: Option<String>,
    sdr_gamma_cache: HashMap<GammaRampKey, GammaRampWords>,
}

impl BackendCache {
    fn new() -> Self {
        Self {
            last_snapshot: None,
            last_color_state_signature: None,
            sdr_gamma_cache: HashMap::new(),
        }
    }
}

pub struct KaiserBackend {
    cache: Mutex<BackendCache>,
    snapshot_path: PathBuf,
}

impl std::fmt::Debug for KaiserBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KaiserBackend").finish()
    }
}

impl KaiserBackend {
    pub fn new() -> Self {
        let snapshot_path = kaiser_data_dir().join("topology_snapshot.json");
        Self { cache: Mutex::new(BackendCache::new()), snapshot_path }
    }

    fn ensure_snapshot(&self) -> Result<(), ManagerError> {
        let mut cache = self.cache.lock().unwrap();
        if cache.last_snapshot.is_some() {
            return Ok(());
        }
        let snapshot = query_active_topology()?;
        let color_sig = active_color_state_signature(&snapshot);
        let sdr_gamma = capture_sdr_gamma_ramps(&snapshot);
        cache.last_snapshot = Some(snapshot);
        cache.last_color_state_signature = Some(color_sig);
        cache.sdr_gamma_cache = sdr_gamma;
        Ok(())
    }

    fn refresh_snapshot(&self, snapshot: TopologySnapshot) {
        let mut cache = self.cache.lock().unwrap();
        let new_sig = active_color_state_signature(&snapshot);
        let old_sig = cache.last_color_state_signature.as_deref().unwrap_or("");

        if new_sig != old_sig {
            let fresh_sdr = capture_sdr_gamma_ramps(&snapshot);
            let merged: HashMap<GammaRampKey, GammaRampWords> = cache
                .sdr_gamma_cache
                .iter()
                .filter(|(k, ramp)| !gamma_ramp_looks_identity(ramp) && !fresh_sdr.contains_key(k))
                .map(|(k, v)| (*k, *v))
                .chain(fresh_sdr.iter().filter(|(_, v)| !gamma_ramp_looks_identity(v)).map(|(k, v)| (*k, *v)))
                .collect();
            cache.sdr_gamma_cache = merged;
            cache.last_color_state_signature = Some(new_sig);
        }
        cache.last_snapshot = Some(snapshot);
    }
}

impl Default for KaiserBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayBackend for KaiserBackend {
    fn color_state_signature(&self) -> Result<Option<String>, ManagerError> {
        self.ensure_snapshot()?;
        let cache = self.cache.lock().unwrap();
        Ok(cache.last_color_state_signature.clone())
    }

    fn reapply_color_calibration(&self) -> Result<(), ManagerError> {
        let sdr_cache = {
            let cache = self.cache.lock().unwrap();
            cache.sdr_gamma_cache.clone()
        };
        reapply_color_calibration_for_active_with_cached_sdr(&sdr_cache)
    }

    fn list_displays(&self) -> Result<Vec<DisplayInfo>, ManagerError> {
        self.ensure_snapshot()?;
        let cache = self.cache.lock().unwrap();
        let snapshot = cache.last_snapshot.as_ref().unwrap();
        let mut all_displays = snapshot.displays.clone();

        // Try to add known-inactive displays from the topology snapshot (previously active)
        // so the UI can show them as "off" and let the user re-enable them
        let active_ids: std::collections::HashSet<_> =
            all_displays.iter().map(|d| d.id.clone()).collect();

        if let Ok(content) = std::fs::read_to_string(&self.snapshot_path) {
            if let Ok(persisted) = serde_json::from_str::<PersistedSnapshot>(&content) {
                for display in persisted.displays {
                    if !active_ids.contains(&display.id) {
                        let mut inactive = display;
                        inactive.is_active = false;
                        all_displays.push(inactive);
                    }
                }
            }
        }
        Ok(all_displays)
    }

    fn get_layout(&self) -> Result<Layout, ManagerError> {
        self.ensure_snapshot()?;
        let cache = self.cache.lock().unwrap();
        Ok(cache.last_snapshot.as_ref().unwrap().layout.clone())
    }

    fn apply_layout(&self, layout: Layout) -> Result<(), ManagerError> {
        self.ensure_snapshot()?;

        let snapshot = {
            let cache = self.cache.lock().unwrap();
            cache.last_snapshot.as_ref().unwrap().clone()
        };

        let any_enabled = layout.outputs.iter().any(|o| o.enabled);
        if !any_enabled {
            return Err(ManagerError::Backend(
                "refusing to apply layout with all displays disabled".to_string(),
            ));
        }

        let currently_active: std::collections::HashSet<(u64, u32)> = snapshot
            .raw
            .paths
            .iter()
            .filter(|p| p.flags & 0x1 != 0)
            .map(|p| {
                (
                    super::win32_types::luid_to_u64(
                        p.targetInfo.adapterId.HighPart,
                        p.targetInfo.adapterId.LowPart,
                    ),
                    p.targetInfo.id,
                )
            })
            .collect();

        let needs_to_enable_inactive = layout.outputs.iter().any(|o| {
            o.enabled && !currently_active.contains(&(o.display_id.adapter_luid, o.display_id.target_id))
        });

        let working_snapshot = if needs_to_enable_inactive {
            let restored_paths_snapshot = self.load_persisted_snapshot_raw();
            if let Some(persisted) = restored_paths_snapshot {
                // Use the persisted snapshot that contains paths to inactive monitors
                TopologySnapshot {
                    raw: persisted,
                    layout: snapshot.layout.clone(),
                    displays: snapshot.displays.clone(),
                }
            } else {
                // Best effort: try topology extend first to rediscover monitors
                let _ = force_topology_extend();
                std::thread::sleep(std::time::Duration::from_millis(1500));
                query_active_topology().unwrap_or(snapshot)
            }
        } else {
            snapshot
        };

        let next_snapshot =
            super::apply::apply_layout_against_snapshot(&layout, &working_snapshot)?;

        self.persist_snapshot(&next_snapshot);

        let sdr_cache = {
            let cache = self.cache.lock().unwrap();
            cache.sdr_gamma_cache.clone()
        };
        let _ = reapply_color_calibration_for_active_with_cached_sdr(&sdr_cache);

        self.refresh_snapshot(next_snapshot);
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct PersistedSnapshot {
    displays: Vec<DisplayInfo>,
    layout: Layout,
}

impl KaiserBackend {
    fn persist_snapshot(&self, snapshot: &TopologySnapshot) {
        let data = PersistedSnapshot {
            displays: snapshot.displays.clone(),
            layout: snapshot.layout.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::create_dir_all(self.snapshot_path.parent().unwrap());
            let _ = std::fs::write(&self.snapshot_path, json);
        }
    }

    fn load_persisted_snapshot_raw(&self) -> Option<super::win32_types::RawTopologySnapshot> {
        // We can't serialize the raw Win32 paths/modes, but we can trigger a full rediscover.
        // This returns None to signal the caller to fall back to topology extend.
        None
    }
}

pub fn kaiser_data_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(appdata).join("Kaiser")
}
