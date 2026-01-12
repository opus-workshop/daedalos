#!/usr/bin/env swift
// Daedalos Terminal.app Theme Installer
// Generates proper NSColor data and installs the theme

import Foundation
import AppKit

// MARK: - Daedalos Color Palette

struct DaedalosColors {
    // Backgrounds
    static let bgMain = NSColor(red: 0.0824, green: 0.0941, blue: 0.1255, alpha: 1) // #151820
    static let bgDeep = NSColor(red: 0.0588, green: 0.0667, blue: 0.0784, alpha: 1) // #0f1114
    static let bgSelection = NSColor(red: 0.1765, green: 0.2275, blue: 0.2902, alpha: 1) // #2d3a4a

    // Foregrounds
    static let fgPrimary = NSColor(red: 0.9098, green: 0.8784, blue: 0.8314, alpha: 1) // #e8e0d4
    static let fgSecondary = NSColor(red: 0.6588, green: 0.6235, blue: 0.5804, alpha: 1) // #a89f94
    static let fgMuted = NSColor(red: 0.4196, green: 0.3961, blue: 0.3765, alpha: 1) // #6b6560

    // Accents
    static let bronze = NSColor(red: 0.8314, green: 0.6471, blue: 0.4549, alpha: 1) // #d4a574
    static let bronzeBright = NSColor(red: 0.9098, green: 0.7686, blue: 0.6039, alpha: 1) // #e8c49a
    static let amber = NSColor(red: 0.8980, green: 0.6588, blue: 0.2941, alpha: 1) // #e5a84b
    static let teal = NSColor(red: 0.2902, green: 0.6039, blue: 0.5490, alpha: 1) // #4a9a8c
    static let tealBright = NSColor(red: 0.4196, green: 0.7686, blue: 0.7059, alpha: 1) // #6bc4b4
    static let copperGreen = NSColor(red: 0.3529, green: 0.6039, blue: 0.4784, alpha: 1) // #5a9a7a
    static let copperBright = NSColor(red: 0.4784, green: 0.7490, blue: 0.6039, alpha: 1) // #7abf9a
    static let terracotta = NSColor(red: 0.7686, green: 0.4784, blue: 0.3529, alpha: 1) // #c47a5a
    static let terracottaBright = NSColor(red: 0.9098, green: 0.6039, blue: 0.4784, alpha: 1) // #e89a7a
    static let plum = NSColor(red: 0.5412, green: 0.4157, blue: 0.5412, alpha: 1) // #8a6a8a
    static let plumBright = NSColor(red: 0.7059, green: 0.5412, blue: 0.7059, alpha: 1) // #b48ab4
    static let olive = NSColor(red: 0.5412, green: 0.6039, blue: 0.4196, alpha: 1) // #8a9a6b
    static let sage = NSColor(red: 0.6431, green: 0.7216, blue: 0.5412, alpha: 1) // #a4b88a
}

// MARK: - Color Data Encoding

func archiveColor(_ color: NSColor) -> Data {
    // Use NSKeyedArchiver for modern macOS
    do {
        return try NSKeyedArchiver.archivedData(withRootObject: color, requiringSecureCoding: false)
    } catch {
        fatalError("Failed to archive color: \(error)")
    }
}

// MARK: - Keybindings

let keybindings: [String: [String: Any]] = [
    "$@L": ["Action": 11, "Text": "loop status\n"],
    "$@V": ["Action": 11, "Text": "verify --quick\n"],
    "$@U": ["Action": 11, "Text": "undo timeline\n"],
    "$@Z": ["Action": 11, "Text": "undo last\n"],
    "$@P": ["Action": 11, "Text": "project info\n"],
    "$@T": ["Action": 11, "Text": "project tree\n"],
    "$@S": ["Action": 11, "Text": "codex search \""],
    "$@E": ["Action": 11, "Text": "error-db match \""],
    "$@A": ["Action": 11, "Text": "agent list\n"],
    "$@C": ["Action": 11, "Text": "undo checkpoint \""],
    "$@X": ["Action": 11, "Text": "context estimate\n"],
    "$@J": ["Action": 11, "Text": "journal what\n"],
]

// MARK: - Build Profile

let profile: [String: Any] = [
    "name": "Daedalos",
    "BackgroundColor": archiveColor(DaedalosColors.bgMain),
    "TextColor": archiveColor(DaedalosColors.fgPrimary),
    "TextBoldColor": archiveColor(DaedalosColors.bronze),
    "CursorColor": archiveColor(DaedalosColors.bronze),
    "SelectionColor": archiveColor(DaedalosColors.bgSelection),

    // ANSI Normal
    "ANSIBlackColor": archiveColor(DaedalosColors.bgDeep),
    "ANSIRedColor": archiveColor(DaedalosColors.terracotta),
    "ANSIGreenColor": archiveColor(DaedalosColors.copperGreen),
    "ANSIYellowColor": archiveColor(DaedalosColors.amber),
    "ANSIBlueColor": archiveColor(DaedalosColors.teal),
    "ANSIMagentaColor": archiveColor(DaedalosColors.plum),
    "ANSICyanColor": archiveColor(DaedalosColors.tealBright),
    "ANSIWhiteColor": archiveColor(DaedalosColors.fgSecondary),

    // ANSI Bright
    "ANSIBrightBlackColor": archiveColor(DaedalosColors.fgMuted),
    "ANSIBrightRedColor": archiveColor(DaedalosColors.terracottaBright),
    "ANSIBrightGreenColor": archiveColor(DaedalosColors.copperBright),
    "ANSIBrightYellowColor": archiveColor(DaedalosColors.bronzeBright),
    "ANSIBrightBlueColor": archiveColor(DaedalosColors.tealBright),
    "ANSIBrightMagentaColor": archiveColor(DaedalosColors.plumBright),
    "ANSIBrightCyanColor": archiveColor(DaedalosColors.sage),
    "ANSIBrightWhiteColor": archiveColor(DaedalosColors.fgPrimary),

    // Terminal settings
    "columnCount": 120,
    "rowCount": 35,
    "CursorBlink": true,
    "CursorType": 1, // Block cursor
    "ShowWindowSettingsNameInTitle": true,
    "shellExitAction": 1,
    "ShouldRestoreContent": true,

    // Keybindings
    "keyMapBoundKeys": keybindings
]

// MARK: - Install

print("Installing Daedalos theme for Terminal.app...")

// Read current Terminal preferences
let defaults = UserDefaults(suiteName: "com.apple.Terminal")!
var windowSettings = defaults.dictionary(forKey: "Window Settings") as? [String: Any] ?? [:]

// Add/update our profile
windowSettings["Daedalos"] = profile
defaults.set(windowSettings, forKey: "Window Settings")
defaults.synchronize()

print("")
print("✓ Daedalos theme installed!")
print("")
print("To use:")
print("  1. Quit and reopen Terminal.app")
print("  2. Terminal > Settings > Profiles > Daedalos")
print("  3. Click 'Default' to make it your default profile")
print("")
print("Keybindings (Cmd+Shift+...):")
print("  L  loop status       V  verify --quick")
print("  Z  undo last         U  undo timeline")
print("  C  checkpoint        P  project info")
print("  T  project tree      S  codex search")
print("  E  error-db match    A  agent list")
print("  X  context estimate  J  journal what")
