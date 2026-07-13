import { invoke } from "@tauri-apps/api/core";
import type {
  AudioDevice,
  AudioSetting,
  DisplayId,
  DisplayInfo,
  DisplayMode,
  Layout,
  ProfileDto,
  SnapshotDto,
} from "./types";

export const api = {
  getSnapshot(): Promise<SnapshotDto> {
    return invoke("get_snapshot");
  },

  listDisplays(): Promise<DisplayInfo[]> {
    return invoke("list_displays");
  },

  toggleDisplay(display_id: DisplayId): Promise<void> {
    return invoke("toggle_display", { display_id });
  },

  applyLayout(layout: Layout): Promise<void> {
    return invoke("apply_layout", { layout });
  },

  saveProfile(name: string, audio?: AudioSetting[]): Promise<void> {
    return invoke("save_profile", { name, audio: audio ?? [] });
  },

  applyProfile(name: string): Promise<void> {
    return invoke("apply_profile", { name });
  },

  deleteProfile(name: string): Promise<void> {
    return invoke("delete_profile", { name });
  },

  listProfiles(): Promise<ProfileDto[]> {
    return invoke("list_profiles");
  },

  listAudioDevices(): Promise<AudioDevice[]> {
    return invoke("list_audio_devices");
  },

  setAudioVolume(device_id: string, volume: number): Promise<void> {
    return invoke("set_audio_volume", { device_id, volume });
  },

  setAudioMute(device_id: string, muted: boolean): Promise<void> {
    return invoke("set_audio_mute", { device_id, muted });
  },

  setDefaultAudioDevice(device_id: string): Promise<void> {
    return invoke("set_default_audio_device", { device_id });
  },

  listDisplayModes(gdi_device_name: string): Promise<DisplayMode[]> {
    return invoke("list_display_modes", { gdi_device_name });
  },

  setDisplayMode(gdi_device_name: string, mode: DisplayMode): Promise<void> {
    return invoke("set_display_mode", { gdi_device_name, mode });
  },
};
