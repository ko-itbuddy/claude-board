import AppKit
// 앱 아이콘: 짙은 남색 둥근 사각형 + 주황 sparkle + 상태 점 세 개
let out = CommandLine.arguments[1]
let fm = FileManager.default
try? fm.createDirectory(atPath: out, withIntermediateDirectories: true)
let specs: [(String, Int)] = [("16x16",16),("16x16@2x",32),("32x32",32),("32x32@2x",64),("128x128",128),("128x128@2x",256),("256x256",256),("256x256@2x",512),("512x512",512),("512x512@2x",1024)]
for (name, px) in specs {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let s = CGFloat(px)
    let inset = s * 0.09
    let rect = NSRect(x: inset, y: inset, width: s - inset * 2, height: s - inset * 2)
    let path = NSBezierPath(roundedRect: rect, xRadius: s * 0.2, yRadius: s * 0.2)
    NSGradient(colors: [NSColor(red: 0.18, green: 0.17, blue: 0.29, alpha: 1), NSColor(red: 0.07, green: 0.07, blue: 0.11, alpha: 1)])!.draw(in: path, angle: -90)
    NSGradient(colors: [NSColor(red: 0.85, green: 0.47, blue: 0.34, alpha: 0.35), .clear])!.draw(in: path, relativeCenterPosition: NSPoint(x: -0.4, y: 0.5))
    NSColor(white: 1, alpha: 0.10).setStroke(); path.lineWidth = max(1, s * 0.006); path.stroke()
    let cfg = NSImage.SymbolConfiguration(pointSize: s * 0.34, weight: .semibold).applying(.init(paletteColors: [NSColor(red: 0.95, green: 0.55, blue: 0.40, alpha: 1)]))
    if let sym = NSImage(systemSymbolName: "sparkle", accessibilityDescription: nil)?.withSymbolConfiguration(cfg) {
        let sz = sym.size
        sym.draw(in: NSRect(x: (s - sz.width) / 2, y: s * 0.56 - sz.height / 2, width: sz.width, height: sz.height))
    }
    let colors = [NSColor(red: 1.0, green: 0.66, blue: 0.36, alpha: 1), NSColor(red: 0.45, green: 0.66, blue: 1.0, alpha: 1), NSColor(red: 0.42, green: 0.85, blue: 0.60, alpha: 1)]
    let d = s * 0.075, gap = s * 0.06
    let total = d * 3 + gap * 2
    for (i, c) in colors.enumerated() {
        c.setFill()
        NSBezierPath(ovalIn: NSRect(x: (s - total) / 2 + CGFloat(i) * (d + gap), y: s * 0.2, width: d, height: d)).fill()
    }
    NSGraphicsContext.restoreGraphicsState()
    try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: "\(out)/icon_\(name).png"))
}
