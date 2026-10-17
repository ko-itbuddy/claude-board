// stack <출력.png> <입력1.png> <입력2.png> ... — 세로로 이어 붙인다
import AppKit
let a = Array(CommandLine.arguments.dropFirst())
let imgs = a.dropFirst().map { NSImage(contentsOfFile: $0)! }
let reps = imgs.map { NSBitmapImageRep(data: $0.tiffRepresentation!)! }
let w = reps.map(\.pixelsWide).max()!, h = reps.map(\.pixelsHigh).reduce(0, +)
let out = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: out)
var y = h
for r in reps { y -= r.pixelsHigh; r.draw(in: NSRect(x: 0, y: y, width: r.pixelsWide, height: r.pixelsHigh)) }
NSGraphicsContext.restoreGraphicsState()
try! out.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: a[0]))
