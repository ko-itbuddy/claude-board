// crop <입력.png> <출력.png> x y w h  (픽셀, 왼쪽 위 기준)
import AppKit
let a = CommandLine.arguments
let img = NSImage(contentsOfFile: a[1])!
var r = NSRect(origin: .zero, size: img.size)
let cg = img.cgImage(forProposedRect: &r, context: nil, hints: nil)!
let c = cg.cropping(to: CGRect(x: Int(a[3])!, y: Int(a[4])!, width: Int(a[5])!, height: Int(a[6])!))!
try! NSBitmapImageRep(cgImage: c).representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: a[2]))
