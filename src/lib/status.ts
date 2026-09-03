import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type Permission = "granted" | "notGranted";

export type CaptureStatus = {
  running: boolean;
  blocks_today: number;
};

export type AppStatus = {
  capture: CaptureStatus;
  setCapture: (next: CaptureStatus) => void;
  permission: Permission;
  folder: string | null;
  setFolder: (next: string | null) => void;
  /// Permission granted and a folder chosen: the app can actually record.
  ready: boolean;
};

/// The three facts every surface of the app displays: whether capture is
/// running, whether macOS has granted accessibility, and where files go.
/// One hook so a window polls once rather than once per component, and so
/// the status bar and the eye can never disagree about the same state.
///
/// Capture is observed rather than tracked because it is started from the
/// tray, the Overview tab and MCP; a 1s poll is cheaper than the event
/// plumbing that would keep three writers in step.
export function useAppStatus(): AppStatus {
  const [capture, setCapture] = useState<CaptureStatus>({
    running: false,
    blocks_today: 0,
  });
  const [permission, setPermission] = useState<Permission>("notGranted");
  const [folder, setFolder] = useState<string | null>(null);

  // The folder rides the same poll as capture: it can be changed from the
  // Settings tab or the setup window while this window is open, and the
  // status bar must not keep showing the old path.
  useEffect(() => {
    let cancelled = false;
    const read = async () => {
      const [nextCapture, nextFolder] = await Promise.all([
        invoke<CaptureStatus>("capture_status"),
        invoke<string | null>("current_folder"),
      ]);
      if (cancelled) return;
      setCapture(nextCapture);
      setFolder(nextFolder ?? null);
    };
    void read();
    const id = setInterval(read, 1000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  useEffect(() => {
    void invoke<Permission>("permission_status").then(setPermission);
  }, []);

  // A grant arrives while the app is running, from System Settings, with
  // nothing to announce it. Poll until it lands, then stop.
  useEffect(() => {
    if (permission === "granted") return;
    const id = setInterval(async () => {
      setPermission(await invoke<Permission>("permission_status"));
    }, 700);
    return () => clearInterval(id);
  }, [permission]);

  return {
    capture,
    setCapture,
    permission,
    folder,
    setFolder,
    ready: permission === "granted" && folder !== null,
  };
}
