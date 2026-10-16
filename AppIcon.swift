import AppKit
// 앱 아이콘: 짙은 남색 둥근 사각형 위에 보드 — 주황 sparkle 제목 줄 + 상태 점(승인 대기·작업 중·완료)이 붙은 세션 카드 세 장
// SF Symbols 는 약관상 앱 아이콘에 쓸 수 없어 모든 도형을 직접 그린다
let out = CommandLine.arguments[1]
let fm = FileManager.default
try? fm.createDirectory(atPath: out, withIntermediateDirectories: true)
let specs: [(String, Int)] = [("16x16",16),("16x16@2x",32),("32x32",32),("32x32@2x",64),("128x128",128),("128x128@2x",256),("256x256",256),("256x256@2x",512),("512x512",512),("512x512@2x",1024)]
let orange = NSColor(red: 1.0, green: 0.62, blue: 0.36, alpha: 1)
let blue = NSColor(red: 0.45, green: 0.66, blue: 1.0, alpha: 1)
let green = NSColor(red: 0.42, green: 0.85, blue: 0.60, alpha: 1)

// 네 꼭짓점이 오목하게 휜 별
func sparkle(_ c: NSPoint, _ r: CGFloat) -> NSBezierPath {
    let p = NSBezierPath()
    let k = r * 0.16
    p.move(to: NSPoint(x: c.x, y: c.y + r))
    p.curve(to: NSPoint(x: c.x + r, y: c.y), controlPoint1: NSPoint(x: c.x + k, y: c.y + k), controlPoint2: NSPoint(x: c.x + k, y: c.y + k))
    p.curve(to: NSPoint(x: c.x, y: c.y - r), controlPoint1: NSPoint(x: c.x + k, y: c.y - k), controlPoint2: NSPoint(x: c.x + k, y: c.y - k))
    p.curve(to: NSPoint(x: c.x - r, y: c.y), controlPoint1: NSPoint(x: c.x - k, y: c.y - k), controlPoint2: NSPoint(x: c.x - k, y: c.y - k))
    p.curve(to: NSPoint(x: c.x, y: c.y + r), controlPoint1: NSPoint(x: c.x - k, y: c.y + k), controlPoint2: NSPoint(x: c.x - k, y: c.y + k))
    p.close()
    return p
}

func bar(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, _ color: NSColor) {
    color.setFill()
    NSBezierPath(roundedRect: NSRect(x: x, y: y - h / 2, width: w, height: h), xRadius: h / 2, yRadius: h / 2).fill()
}

for (name, px) in specs {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let s = CGFloat(px)
    let small = px < 64   // 작은 크기에서는 제목 줄을 빼고 카드·점을 키워 뭉개지지 않게 한다
    let inset = s * 0.09
    let rect = NSRect(x: inset, y: inset, width: s - inset * 2, height: s - inset * 2)
    let path = NSBezierPath(roundedRect: rect, xRadius: s * 0.2, yRadius: s * 0.2)
    NSGradient(colors: [NSColor(red: 0.18, green: 0.17, blue: 0.29, alpha: 1), NSColor(red: 0.07, green: 0.07, blue: 0.11, alpha: 1)])!.draw(in: path, angle: -90)
    NSGradient(colors: [NSColor(red: 0.85, green: 0.47, blue: 0.34, alpha: 0.35), .clear])!.draw(in: path, relativeCenterPosition: NSPoint(x: -0.4, y: 0.5))
    NSColor(white: 1, alpha: 0.10).setStroke(); path.lineWidth = max(1, s * 0.006); path.stroke()

    // 제목 줄 — sparkle + 제목 글자 자리
    if !small {
        orange.setFill()
        sparkle(NSPoint(x: s * 0.27, y: s * 0.735), s * 0.075).fill()
        bar(s * 0.375, s * 0.735, s * 0.33, s * 0.045, NSColor(white: 1, alpha: 0.85))
    }

    // 세션 카드 — 위에서부터 승인 대기(주황 테두리)·작업 중(파랑)·완료(초록)
    let ys: [CGFloat] = small ? [0.71, 0.5, 0.29] : [0.575, 0.415, 0.255]
    let x0 = s * (small ? 0.17 : 0.19), w = s * (small ? 0.66 : 0.62), h = s * (small ? 0.18 : 0.135)
    let d = s * (small ? 0.12 : 0.07), dx = x0 + s * (small ? 0.1 : 0.08)
    let cards: [(color: NSColor, waiting: Bool)] = [(orange, true), (blue, false), (green, false)]
    for (card, y) in zip(cards, ys) {
        let cy = s * y
        let box = NSBezierPath(roundedRect: NSRect(x: x0, y: cy - h / 2, width: w, height: h), xRadius: s * 0.035, yRadius: s * 0.035)
        (card.waiting ? card.color.withAlphaComponent(0.16) : NSColor(white: 1, alpha: 0.07)).setFill()
        box.fill()
        (card.waiting ? card.color.withAlphaComponent(0.7) : NSColor(white: 1, alpha: 0.12)).setStroke()
        box.lineWidth = max(1, s * (card.waiting ? 0.008 : 0.005))
        box.stroke()
        card.color.setFill()
        NSBezierPath(ovalIn: NSRect(x: dx - d / 2, y: cy - d / 2, width: d, height: d)).fill()
        if small {
            bar(dx + d, cy, x0 + w - (dx + d) - s * 0.07, s * 0.06, NSColor(white: 1, alpha: 0.7))
        } else {
            bar(s * 0.34, cy + s * 0.022, s * 0.39, s * 0.032, NSColor(white: 1, alpha: 0.75))
            bar(s * 0.34, cy - s * 0.028, s * 0.24, s * 0.026, NSColor(white: 1, alpha: 0.32))
        }
    }
    NSGraphicsContext.restoreGraphicsState()
    try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: "\(out)/icon_\(name).png"))
}
