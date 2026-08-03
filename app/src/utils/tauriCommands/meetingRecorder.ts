/**
 * Meeting recorder commands — Tauri custom commands (cpal capture pipeline),
 * wired in `app/src-tauri/src/meeting_recorder/recorder.rs`.
 */
import { invoke } from '@tauri-apps/api/core';

import { isTauri } from './common';

export interface AudioDeviceInfo {
  name: string;
  deviceType: 'input' | 'output';
}

/**
 * Wire shape returned by `meeting_recorder_list_devices`. The Rust
 * `AudioDeviceInfo` struct (`recorder.rs`) has no `#[serde(rename_all = ...)]`
 * attribute, so it serializes its `device_type` field name as-is
 * (snake_case) rather than camelCase — unlike command *arguments*, Tauri
 * does not auto-convert return-value casing. Mapped to `deviceType` below
 * to match the rest of the app's camelCase TS conventions.
 */
interface RawAudioDeviceInfo {
  name: string;
  device_type: 'input' | 'output';
}

export async function meetingRecorderListDevices(): Promise<AudioDeviceInfo[]> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  const raw = await invoke<RawAudioDeviceInfo[]>('meeting_recorder_list_devices');
  return raw.map(device => ({ name: device.name, deviceType: device.device_type }));
}

export async function meetingRecorderStart(
  micDeviceName?: string,
  systemDeviceName?: string,
  title?: string
): Promise<string> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  return await invoke<string>('meeting_recorder_start', {
    micDeviceName: micDeviceName ?? null,
    systemDeviceName: systemDeviceName ?? null,
    title: title ?? null,
  });
}

export async function meetingRecorderStop(): Promise<void> {
  if (!isTauri()) {
    throw new Error('Not running in Tauri');
  }
  await invoke('meeting_recorder_stop');
}
