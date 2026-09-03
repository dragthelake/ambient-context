import Foundation
import AppKit
import ApplicationServices
import CoreGraphics
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

/// Seconds since the last keyboard or mouse event anywhere in the session,
/// so a machine left alone stops extending the open block. Returns -1 when
/// the "any input" event type cannot be formed, which the caller reads as
/// "no idle reading on this platform" rather than as zero seconds idle.
@_cdecl("ambient_ax_seconds_since_input")
public func ambientAxSecondsSinceInput() -> Double {
    guard let anyInput = CGEventType(rawValue: ~0) else { return -1 }
    return CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: anyInput)
}

// Content in browsers and Electron apps nests 15 to 30 levels deep, so the
// depth limit exists only to stop runaway recursion, not to bound work; the
// visit cap is what bounds walk time.
private let maxDepth = 40
private let maxElements = 2000
private let maxVisited = 20000

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
    depth: Int,
    visited: inout Int,
    webUrl: inout String?
) {
    visited += 1
    if depth > maxDepth || texts.count >= maxElements || visited > maxVisited { return }

    // A secure field and everything inside it is skipped entirely. This is
    // the first of the redaction layers and the only one that can be done
    // before the text is ever read.
    if isSecure(element) { return }

    // The page URL lives on the web area, not the window. First one wins:
    // the focused window's main web area is reached before any embedded one.
    if webUrl == nil,
       let role = copyAttribute(element, kAXRoleAttribute as String) as? String,
       role == "AXWebArea",
       let url = copyAttribute(element, "AXURL") {
        webUrl = (url as? NSURL)?.absoluteString
    }

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
            collectText(from: child, into: &texts, depth: depth + 1, visited: &visited, webUrl: &webUrl)
        }
    }
}

// Chromium builds its accessibility tree only when assistive technology asks
// for it, so a capture tool must ask. AXManualAccessibility is the attribute
// Electron added for exactly this per-application switch; Chrome itself only
// responds to AXEnhancedUserInterface, the VoiceOver signal. Native apps
// return an error for both, harmlessly. Once per pid, so the poll loop does
// not spam IPC; a relaunched app gets a new pid and is enabled again.
private let enabledPidsLock = NSLock()
private var enabledPids = Set<pid_t>()

private func enableAccessibilityOnce(_ pid: pid_t, _ appElement: AXUIElement, _ appName: String) {
    enabledPidsLock.lock()
    let alreadyTried = enabledPids.contains(pid)
    if !alreadyTried { enabledPids.insert(pid) }
    enabledPidsLock.unlock()
    if alreadyTried { return }

    if AXUIElementSetAttributeValue(
        appElement, "AXManualAccessibility" as CFString, kCFBooleanTrue) == .success {
        FileHandle.standardError.write(Data("[ax] enabled AXManualAccessibility for \(appName)\n".utf8))
        return
    }
    if AXUIElementSetAttributeValue(
        appElement, "AXEnhancedUserInterface" as CFString, kCFBooleanTrue) == .success {
        FileHandle.standardError.write(Data("[ax] enabled AXEnhancedUserInterface for \(appName)\n".utf8))
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

    // The tree takes a few seconds to build after this first asks, so the
    // first reads of a freshly enabled app are thin and fill in on later
    // polls. That is fine for a background loop.
    enableAccessibilityOnce(frontmost.processIdentifier, appElement, appName)

    guard let focused = copyAttribute(appElement, kAXFocusedWindowAttribute as String) else {
        return SRString("ERROR: no focused window")
    }
    let window = focused as! AXUIElement

    let title = copyAttribute(window, kAXTitleAttribute as String) as? String

    // The window's backing file, where the app exposes one (Preview, Xcode,
    // TextEdit and most document apps do). A path is worth more than any
    // amount of scraped text: the consuming LLM can open the real document.
    var document = copyAttribute(window, "AXDocument") as? String
    if document == nil, let docUrl = copyAttribute(window, "AXDocument") {
        document = (docUrl as? NSURL)?.absoluteString
    }

    var texts: [String] = []
    var visited = 0
    var webUrl: String? = nil
    collectText(from: window, into: &texts, depth: 0, visited: &visited, webUrl: &webUrl)

    let payload: [String: Any] = [
        "app": appName,
        "window_title": title as Any,
        "document": document as Any,
        "url": webUrl as Any,
        "text": texts
    ]

    guard let data = try? JSONSerialization.data(withJSONObject: payload),
          let json = String(data: data, encoding: .utf8) else {
        return SRString("ERROR: could not serialise snapshot")
    }
    return SRString(json)
}

// MARK: - Window chrome

/// Hides the three traffic light buttons on one window, leaving the rest of
/// the native title bar in place. The window keeps its system corner mask
/// and its edge resizing, which going borderless would both give up; only
/// the buttons go, because the page draws its own.
///
/// Identified by the NSWindow pointer Tauri already holds rather than by
/// title, which would pick the wrong window the moment two of them matched.
/// AppKit is main-thread only, so the caller marshals.
@_cdecl("ambient_hide_window_buttons")
public func ambientHideWindowButtons(_ windowPointer: Int64) -> Int32 {
    guard windowPointer != 0,
          let raw = UnsafeRawPointer(bitPattern: Int(windowPointer)) else {
        return 0
    }
    let window = Unmanaged<NSWindow>.fromOpaque(raw).takeUnretainedValue()
    var hidden: Int32 = 0
    for kind: NSWindow.ButtonType in [.closeButton, .miniaturizeButton, .zoomButton] {
        if let button = window.standardWindowButton(kind) {
            button.isHidden = true
            hidden += 1
        }
    }
    return hidden
}

/// Lightens the traffic lights on an unfocused window. Since macOS 26 the
/// inactive buttons are not a fixed grey but a translucent tint over
/// whatever sits behind them, so on this app's navy title bar the light
/// appearance renders them darker than the bar itself (measured #000058 on
/// #000080) and they read as three black holes. The dark appearance tints
/// the other way and they come out lighter than the bar. The focused red,
/// amber and green are opaque and unchanged either way.
///
/// The appearance goes on the window, which is what the title bar's theme
/// frame draws the buttons from. The content view is then pinned back to
/// light. That second half is belt and braces rather than load bearing:
/// WebKit takes prefers-color-scheme from the page's own `color-scheme`,
/// which this one never declares, so the content renders light either way,
/// and captures of the Settings tab with and without it are identical. It
/// stays because the day someone writes `color-scheme: light dark` the
/// window's appearance becomes the thing that decides, and a page with no
/// dark palette would turn dark with no obvious cause.
///
/// Returns the number of layers set, so a missing content view is visible
/// to the caller rather than passing silently. AppKit is main-thread only,
/// so the caller marshals.
@_cdecl("ambient_lighten_inactive_traffic_lights")
public func ambientLightenInactiveTrafficLights(_ windowPointer: Int64) -> Int32 {
    guard windowPointer != 0,
          let raw = UnsafeRawPointer(bitPattern: Int(windowPointer)) else {
        return 0
    }
    let window = Unmanaged<NSWindow>.fromOpaque(raw).takeUnretainedValue()
    window.appearance = NSAppearance(named: .darkAqua)
    guard let content = window.contentView else { return 1 }
    content.appearance = NSAppearance(named: .aqua)
    return 2
}

/// Moves the traffic-light cluster without touching the title bar container,
/// which keeps the window layout intact. `x` is the close button's leading
/// edge in window coordinates; `yFromTop` is the distance from the window's
/// top edge to the top of the buttons. AppKit is main-thread only.
@_cdecl("ambient_position_traffic_lights")
public func ambientPositionTrafficLights(
    _ windowPointer: Int64,
    x: Double,
    yFromTop: Double
) -> Int32 {
    guard windowPointer != 0,
          let raw = UnsafeRawPointer(bitPattern: Int(windowPointer)) else {
        return 0
    }
    let window = Unmanaged<NSWindow>.fromOpaque(raw).takeUnretainedValue()
    guard let close = window.standardWindowButton(.closeButton),
          let mini = window.standardWindowButton(.miniaturizeButton),
          let zoom = window.standardWindowButton(.zoomButton) else {
        return 0
    }

    let windowHeight = window.frame.size.height
    let spaceBetween = mini.frame.origin.x - close.frame.origin.x
    let buttons = [close, mini, zoom]

    func place() {
        for (index, button) in buttons.enumerated() {
            guard let superview = button.superview else { continue }
            button.isHidden = false
            let originInWindow = NSPoint(
                x: x + Double(index) * spaceBetween,
                y: windowHeight - yFromTop - button.frame.size.height
            )
            button.setFrameOrigin(superview.convert(originInWindow, from: nil))
        }
    }

    // One layout pass after show, so standardWindowButton frames are real.
    DispatchQueue.main.async(execute: place)

    return Int32(buttons.count)
}
