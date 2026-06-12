// Print the on-screen windows owned by a given app: "<windowID> <width> <height>"
// (one per line). Used by scripts/screenshot.sh to capture the floating overlay
// panels by window id (`screencapture -l<id>`), which yields the panel WITH its
// alpha channel — a transparent background, so the shot can't leak whatever is
// behind it. Run: `swift scripts/winid.swift <app-process-name>`  (macOS only).
import CoreGraphics
import Foundation

let owner = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : ""
guard !owner.isEmpty,
      let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]]
else { exit(1) }

for info in list {
    guard let o = info[kCGWindowOwnerName as String] as? String, o == owner,
          let num = info[kCGWindowNumber as String] as? Int,
          let b = info[kCGWindowBounds as String] as? [String: Any],
          let w = b["Width"] as? CGFloat, let h = b["Height"] as? CGFloat
    else { continue }
    print("\(num) \(Int(w)) \(Int(h))")
}
