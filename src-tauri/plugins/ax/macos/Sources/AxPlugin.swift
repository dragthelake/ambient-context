import Foundation
import AppKit
import ApplicationServices
import SwiftRs

// Status codes crossing the C boundary: 0 = not granted, 1 = granted.
// A codes-based contract is simpler to keep in sync by hand than a
// marshalled enum.

/// Reads the current Accessibility trust state. Does not prompt.
@_cdecl("ambient_ax_permission_status")
public func ambientAxPermissionStatus() -> Int32 {
    return AXIsProcessTrusted() ? 1 : 0
}

/// Raises the system prompt if the app is not yet trusted. The prompt only
/// offers to open System Settings; the grant itself happens there, so this
/// almost always returns 0 on first call and the caller must poll
/// `ambient_ax_permission_status` afterwards.
@_cdecl("ambient_ax_request_permission")
public func ambientAxRequestPermission() -> Int32 {
    let key = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
    let options = [key: true] as CFDictionary
    return AXIsProcessTrustedWithOptions(options) ? 1 : 0
}

private let maxDepth = 6
private let maxElements = 2000

private func copyAttribute(_ element: AXUIElement, _ attribute: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success else {
        return nil
    }
    return value
}

private func isSecure(_ element: AXUIElement) -> Bool {
    if let role = copyAttribute(element, kAXRoleAttribute as String) as? String,
       role == "AXSecureTextField" {
        return true
    }
    if let subrole = copyAttribute(element, kAXSubroleAttribute as String) as? String,
       subrole == (kAXSecureTextFieldSubrole as String) {
        return true
    }
    return false
}

private func collectText(
    from element: AXUIElement,
    into texts: inout [String],
    depth: Int
) {
    if depth > maxDepth || texts.count >= maxElements { return }

    // A secure field and everything inside it is skipped entirely. This is
    // the first of the redaction layers and the only one that can be done
    // before the text is ever read.
    if isSecure(element) { return }

    for attribute in [kAXValueAttribute as String, kAXTitleAttribute as String] {
        if let value = copyAttribute(element, attribute) as? String {
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty && trimmed.count < 8000 {
                texts.append(trimmed)
            }
        }
    }

    if let children = copyAttribute(element, kAXChildrenAttribute as String) as? [AXUIElement] {
        for child in children {
            collectText(from: child, into: &texts, depth: depth + 1)
        }
    }
}

/// Reads the focused window of the frontmost application and returns JSON.
/// Only the focused window is touched: background windows, minimised windows
/// and unfocused displays are never walked.
@_cdecl("ambient_ax_snapshot")
public func ambientAxSnapshot() -> SRString {
    guard AXIsProcessTrusted() else {
        return SRString("ERROR: permission not granted")
    }

    // A locked screen is not work. Skipping here also lets the capture loop
    // flush the open block, so the morning's last block ends at the lock
    // rather than spanning lunch.
    if let session = CGSessionCopyCurrentDictionary() as? [String: Any],
       session["CGSSessionScreenIsLocked"] as? Bool == true {
        return SRString("ERROR: screen locked")
    }

    guard let frontmost = NSWorkspace.shared.frontmostApplication else {
        return SRString("ERROR: no frontmost application")
    }

    let appName = frontmost.localizedName ?? "Unknown"
    let appElement = AXUIElementCreateApplication(frontmost.processIdentifier)

    // AX calls are synchronous IPC into the target process. Without a
    // timeout, a hung application freezes this thread with it; with one, a
    // slow target costs a bounded slice of a single tick.
    AXUIElementSetMessagingTimeout(appElement, 0.5)

    guard let focused = copyAttribute(appElement, kAXFocusedWindowAttribute as String) else {
        return SRString("ERROR: no focused window")
    }
    let window = focused as! AXUIElement

    let title = copyAttribute(window, kAXTitleAttribute as String) as? String

    var texts: [String] = []
    collectText(from: window, into: &texts, depth: 0)

    let payload: [String: Any] = [
        "app": appName,
        "window_title": title as Any,
        "text": texts
    ]

    guard let data = try? JSONSerialization.data(withJSONObject: payload),
          let json = String(data: data, encoding: .utf8) else {
        return SRString("ERROR: could not serialise snapshot")
    }
    return SRString(json)
}

/// Census scaffolding. Enables accessibility in the frontmost application:
/// AXManualAccessibility for Electron, falling back to AXEnhancedUserInterface
/// for Chrome itself.
@_cdecl("ambient_ax_enable_frontmost")
public func ambientAxEnableFrontmost() -> Int32 {
    guard let frontmost = NSWorkspace.shared.frontmostApplication else { return 0 }
    let appElement = AXUIElementCreateApplication(frontmost.processIdentifier)
    if AXUIElementSetAttributeValue(
        appElement, "AXManualAccessibility" as CFString, kCFBooleanTrue) == .success {
        return 1
    }
    return AXUIElementSetAttributeValue(
        appElement, "AXEnhancedUserInterface" as CFString, kCFBooleanTrue) == .success ? 1 : 0
}
