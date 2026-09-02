// Print the on-screen windows owned by a given process: "<windowID> <width> <height>"
// (one per line). Used by scripts/screenshot.sh to capture the floating overlay
// panels by window id (`screencapture -l<id>`), which yields the panel WITH its
// alpha channel — a transparent background, so the shot can't leak whatever is
// behind it. Run: `swift scripts/winid.swift <process-id>` (macOS only).
import CoreGraphics
import Foundation

let ownerPID = CommandLine.arguments.count > 1 ? Int(CommandLine.arguments[1]) : nil
guard let ownerPID,
      let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]]
else { exit(1) }

for info in list {
    guard let pid = info[kCGWindowOwnerPID as String] as? Int, pid == ownerPID,
          let num = info[kCGWindowNumber as String] as? Int,
          let b = info[kCGWindowBounds as String] as? [String: Any],
          let w = b["Width"] as? CGFloat, let h = b["Height"] as? CGFloat
    else { continue }
    print("\(num) \(Int(w)) \(Int(h))")
}
