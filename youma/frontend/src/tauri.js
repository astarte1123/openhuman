// Tauri v2: __TAURI_INTERNALS__ is injected by the Tauri runtime into every window.
// It provides the low-level IPC invoke function without requiring @tauri-apps/api or
// the withGlobalTauri config option.
export async function invokeCommand(name, args = {}) {
  const internals = window.__TAURI_INTERNALS__;
  if (!internals) {
    throw new Error(
      "[youma] Tauri internals not available. " +
      "This app must run inside the Tauri desktop shell."
    );
  }
  return internals.invoke(name, args);
}
