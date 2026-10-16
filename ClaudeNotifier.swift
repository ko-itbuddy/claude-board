// Claude Code 작업 완료·승인 대기 알림 패널
// 사용법: claude-notifier --title T --subtitle S --body B [--kind done|wait] [--summary waiting=1,working=2,idle=3]
//        [--sound Glass] [--duration 10] [--activate <bundle id>]
//        [--position center|top-left|top-right|bottom-left|bottom-right]
import AppKit

struct Options {
    var title = "Claude Code"
    var subtitle = ""
    var body = ""
    var sound = "Glass"
    var duration: TimeInterval = 10
    var activate = ""
    var kind = "done"
    var summary: [String: Int] = [:]
    var position = "center"

    var accent: NSColor {
        kind == "wait"
            ? NSColor(srgbRed: 1.00, green: 0.66, blue: 0.25, alpha: 1)
            : NSColor(srgbRed: 0.30, green: 0.85, blue: 0.55, alpha: 1)
    }
    var symbol: String { kind == "wait" ? "hand.raised.fill" : "checkmark.seal.fill" }
}

func parseOptions() -> Options {
    var o = Options()
    var args = Array(CommandLine.arguments.dropFirst())
    while !args.isEmpty {
        let key = args.removeFirst()
        guard !args.isEmpty else { break }
        let value = args.removeFirst()
        switch key {
        case "--title": o.title = value
        case "--subtitle": o.subtitle = value
        case "--body": o.body = value
        case "--sound": o.sound = value
        case "--duration": o.duration = TimeInterval(value) ?? o.duration
        case "--activate": o.activate = value
        case "--kind": o.kind = value
        case "--position": o.position = value
        case "--summary":   // 예: waiting=1,working=2,idle=3
            for pair in value.split(separator: ",") {
                let kv = pair.split(separator: "=")
                if kv.count == 2, let n = Int(kv[1]) { o.summary[String(kv[0])] = n }
            }
        default: break
        }
    }
    return o
}

// 여러 세션이 동시에 알림을 띄우면 슬롯 파일로 순번을 나눠 가운데에서는 가로로 반씩 겹쳐 놓는다.
// 모서리에서는 슬롯 파일에 패널이 차지한 세로 범위를 적어 두고, 새 패널은 같은 모서리의 빈자리에 쌓는다
final class Slot {
    static let dir = NSTemporaryDirectory() + "claude-notifier-slots"
    let index: Int
    private let path: String

    init() {
        let dir = Slot.dir
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        for i in 0..<5 {
            let p = "\(dir)/\(i)"
            if let attrs = try? FileManager.default.attributesOfItem(atPath: p),
               let date = attrs[.modificationDate] as? Date,
               Date().timeIntervalSince(date) > 120 {
                try? FileManager.default.removeItem(atPath: p)
            }
            let fd = open(p, O_CREAT | O_EXCL | O_WRONLY, 0o600)
            if fd >= 0 {
                close(fd)
                index = i
                path = p
                return
            }
        }
        index = 0
        path = ""
    }

    // 슬롯 순번별 가로 위치(패널 너비 배수) — 가운데를 기준으로 오른쪽·왼쪽 번갈아 반씩 비켜 놓는다
    var horizontalShift: CGFloat { [0, 0.5, -0.5, 1, -1][index] }

    // 모서리에 둔 이 패널의 세로 범위 — 예: "top-right 900.0 1013.0"
    func record(_ position: String, _ minY: CGFloat, _ maxY: CGFloat) {
        if !path.isEmpty { try? "\(position) \(minY) \(maxY)".write(toFile: path, atomically: false, encoding: .utf8) }
    }

    // 같은 모서리에 떠 있는 다른 패널들의 세로 범위 — 낡은 슬롯(120초 넘게 갱신 없음)은 뺀다
    func taken(_ position: String) -> [(minY: CGFloat, maxY: CGFloat)] {
        let names = (try? FileManager.default.contentsOfDirectory(atPath: Slot.dir)) ?? []
        return names.compactMap { name in
            let p = "\(Slot.dir)/\(name)"
            guard p != path,
                  let date = (try? FileManager.default.attributesOfItem(atPath: p))?[.modificationDate] as? Date,
                  Date().timeIntervalSince(date) <= 120,
                  let f = (try? String(contentsOfFile: p, encoding: .utf8))?.split(separator: " "),
                  f.count == 3, f[0] == position, let a = Double(f[1]), let b = Double(f[2]) else { return nil }
            return (CGFloat(a), CGFloat(b))
        }
    }

    // 마우스를 올려 오래 머무는 동안 다른 알림이 이 슬롯을 낡은 것으로 보고 가져가지 않도록 수정 시각을 갱신한다
    func touch() {
        if !path.isEmpty { try? FileManager.default.setAttributes([.modificationDate: Date()], ofItemAtPath: path) }
    }

    func release() {
        if !path.isEmpty { try? FileManager.default.removeItem(atPath: path) }
    }
}

// 클릭과 마우스 올림을 받는 루트 뷰 — 마우스를 올려 두는 동안에는 카운트다운을 멈춘다
final class InteractiveView: NSView {
    var onClick: (() -> Void)?
    var onHover: ((Bool) -> Void)?

    // 글자·상자 같은 하위 뷰가 클릭을 가로채지 않도록 패널 안의 모든 클릭을 이 뷰가 받는다
    override func hitTest(_ point: NSPoint) -> NSView? {
        let local = superview.map { convert(point, from: $0) } ?? point
        return bounds.contains(local) ? self : nil
    }
    override func mouseDown(with event: NSEvent) { onClick?() }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseEntered(with event: NSEvent) { onHover?(true) }
    override func mouseExited(with event: NSEvent) { onHover?(false) }
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: bounds,
                                       options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                       owner: self, userInfo: nil))
    }
    override func resetCursorRects() { addCursorRect(bounds, cursor: .pointingHand) }
}

// 바탕색: 짙은 남색 그라데이션 (블러가 살짝 비쳐 보이도록 거의 불투명) — 세션 보드와 같은 색
final class BackdropView: NSView {
    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        let g = CAGradientLayer()
        g.colors = [NSColor(srgbRed: 0.12, green: 0.12, blue: 0.19, alpha: 0.94).cgColor,
                    NSColor(srgbRed: 0.07, green: 0.07, blue: 0.11, alpha: 0.94).cgColor]
        g.startPoint = CGPoint(x: 0.5, y: 1)
        g.endPoint = CGPoint(x: 0.5, y: 0)
        layer = g
    }
    required init?(coder: NSCoder) { nil }
}

// 배경 위에 은은한 강조색 빛을 깔아 주는 뷰
final class GlowView: NSView {
    init(color: NSColor) {
        super.init(frame: .zero)
        wantsLayer = true
        let g = CAGradientLayer()
        g.type = .radial
        g.colors = [color.withAlphaComponent(0.28).cgColor, color.withAlphaComponent(0).cgColor]
        g.startPoint = CGPoint(x: 0.08, y: 1.0)
        g.endPoint = CGPoint(x: 0.75, y: -0.4)
        layer = g
    }
    required init?(coder: NSCoder) { nil }
}

func makeLabel(_ text: String, size: CGFloat, weight: NSFont.Weight = .regular,
               color: NSColor = .labelColor, lines: Int = 1, lineSpacing: CGFloat = 0) -> NSTextField {
    let l = NSTextField(wrappingLabelWithString: "")
    let style = NSMutableParagraphStyle()
    style.lineSpacing = lineSpacing
    style.lineBreakMode = .byWordWrapping
    l.attributedStringValue = NSAttributedString(string: text, attributes: [
        .font: NSFont.systemFont(ofSize: size, weight: weight),
        .foregroundColor: color,
        .paragraphStyle: style,
    ])
    l.isSelectable = false
    l.maximumNumberOfLines = lines
    l.cell?.truncatesLastVisibleLine = true
    l.cell?.lineBreakMode = .byTruncatingTail
    l.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
    return l
}

func symbolView(_ name: String, size: CGFloat, weight: NSFont.Weight, color: NSColor) -> NSImageView {
    let image = NSImage(systemSymbolName: name, accessibilityDescription: nil)?
        .withSymbolConfiguration(NSImage.SymbolConfiguration(pointSize: size, weight: weight)) ?? NSImage()
    let v = NSImageView(image: image)
    v.contentTintColor = color
    v.translatesAutoresizingMaskIntoConstraints = false
    return v
}

func roundedBox(color: NSColor, radius: CGFloat) -> NSView {
    let v = NSView()
    v.wantsLayer = true
    v.layer?.backgroundColor = color.cgColor
    v.layer?.cornerRadius = radius
    v.translatesAutoresizingMaskIntoConstraints = false
    return v
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    let opts = parseOptions()
    let slot = Slot()
    let width: CGFloat = 440
    let pad: CGFloat = 16
    let margin: CGFloat = 12   // 모서리에 둘 때 화면 가장자리와의 간격
    let gap: CGFloat = 8       // 모서리에 쌓인 패널 사이 간격

    var panel: NSPanel!
    var progressFill: NSView!
    var progressWidth: NSLayoutConstraint!
    var elapsed: TimeInterval = 0
    var hovered = false
    var closing = false
    var ticks = 0

    func applicationDidFinishLaunching(_ notification: Notification) {
        let content = buildContent()
        content.layoutSubtreeIfNeeded()
        let height = content.fittingSize.height

        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main ?? NSScreen.screens[0]
        let frame = screen.visibleFrame
        var x = frame.midX - width / 2 + slot.horizontalShift * width
        var y = frame.midY - height / 2
        // 모서리: 가로로 비켜 놓지 않고 화면 안쪽(위 모서리는 아래로, 아래 모서리는 위로) 쌓는다.
        // 패널 높이가 본문 길이에 따라 달라 순번으로 자리를 정하지 않고, 가장자리에서 가까운 패널부터 보며 겹치면 그 너머로 옮긴다
        if ["top-left", "top-right", "bottom-left", "bottom-right"].contains(opts.position) {
            let top = opts.position.hasPrefix("top")
            x = opts.position.hasSuffix("left") ? frame.minX + margin : frame.maxX - width - margin
            y = top ? frame.maxY - margin - height : frame.minY + margin
            // 동시에 뜬 패널이 서로의 기록을 못 보고 같은 자리를 고르지 않도록, 범위를 읽고 적는 동안 잠근다
            let lock = open(Slot.dir + "/lock", O_CREAT | O_RDWR, 0o600)
            flock(lock, LOCK_EX)
            for r in slot.taken(opts.position).sorted(by: { top ? $0.maxY > $1.maxY : $0.minY < $1.minY })
            where y < r.maxY + gap && y + height > r.minY - gap {
                y = top ? r.minY - gap - height : r.maxY + gap
            }
            slot.record(opts.position, y, y + height)
            close(lock)
        }
        let target = NSRect(x: min(max(x, frame.minX + 8), frame.maxX - width - 8),
                            y: y,
                            width: width, height: height)

        panel = NSPanel(contentRect: target.offsetBy(dx: 0, dy: -14),
                        styleMask: [.nonactivatingPanel, .borderless],
                        backing: .buffered, defer: false)
        panel.level = .statusBar
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.appearance = NSAppearance(named: .darkAqua)
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.contentView = content
        panel.alphaValue = 0
        // 키 창으로 만들지 않는다 — 프롬프트를 입력하던 터미널이 키보드 입력을 그대로 받는다(닫기는 클릭·시간 경과)
        panel.orderFrontRegardless()

        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.28
            ctx.timingFunction = CAMediaTimingFunction(name: .easeOut)
            panel.animator().alphaValue = 1
            panel.animator().setFrame(target, display: true)
        }
        if !opts.sound.isEmpty { NSSound(named: NSSound.Name(opts.sound))?.play() }

        let timer = Timer(timeInterval: 0.05, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(timer, forMode: .common)
    }

    func buildContent() -> NSView {
        let accent = opts.accent
        let root = InteractiveView()
        root.onClick = { [weak self] in self?.clicked() }
        root.onHover = { [weak self] inside in
            self?.hovered = inside
            if inside { self?.panel.orderFrontRegardless() }
        }
        root.translatesAutoresizingMaskIntoConstraints = false

        // 바탕: 반투명 블러 + 짙은 남색 + 강조색 빛 + 얇은 테두리
        let blur = NSVisualEffectView()
        blur.material = .hudWindow
        blur.blendingMode = .behindWindow
        blur.state = .active
        blur.wantsLayer = true
        blur.layer?.cornerRadius = 14
        blur.layer?.masksToBounds = true
        blur.layer?.borderWidth = 1
        blur.layer?.borderColor = accent.withAlphaComponent(0.35).cgColor
        blur.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(blur)

        let backdrop = BackdropView()
        backdrop.translatesAutoresizingMaskIntoConstraints = false
        blur.addSubview(backdrop)
        let glow = GlowView(color: accent)
        glow.translatesAutoresizingMaskIntoConstraints = false
        blur.addSubview(glow)

        // 1줄: 아이콘 · "<프로젝트> <상태>" · 시각
        let badge = roundedBox(color: accent.withAlphaComponent(0.18), radius: 9)
        let icon = symbolView(opts.symbol, size: 15, weight: .semibold, color: accent)
        badge.addSubview(icon)

        let title = NSTextField(labelWithString: "")
        let titleText = NSMutableAttributedString(string: opts.subtitle.isEmpty ? "" : opts.subtitle + "  ", attributes: [
            .font: NSFont.systemFont(ofSize: 15, weight: .bold), .foregroundColor: NSColor.white])
        titleText.append(NSAttributedString(string: opts.title, attributes: [
            .font: NSFont.systemFont(ofSize: 15, weight: .semibold), .foregroundColor: accent]))
        title.attributedStringValue = titleText
        title.lineBreakMode = .byTruncatingTail
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let time = DateFormatter()
        time.dateFormat = "HH:mm"
        let clock = makeLabel(time.string(from: Date()), size: 12, weight: .medium, color: NSColor.white.withAlphaComponent(0.45))
        clock.setContentCompressionResistancePriority(.required, for: .horizontal)
        let spacer = NSView()
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let header = NSStackView(views: [badge, title, spacer, clock])
        header.spacing = 10
        header.alignment = .centerY

        // 2줄: 응답 요약 한 줄
        let body = makeLabel(opts.body, size: 13, color: NSColor.white.withAlphaComponent(0.82), lines: 2, lineSpacing: 2)
        body.isHidden = opts.body.isEmpty

        // 3줄: 전체 세션 현황 (색 점 + 개수)
        var parts: [NSView] = []
        let states: [(String, String, NSColor)] = [
            ("waiting", "승인 대기", NSColor(srgbRed: 1.00, green: 0.66, blue: 0.36, alpha: 1)),
            ("working", "작업 중", NSColor(srgbRed: 0.45, green: 0.66, blue: 1.00, alpha: 1)),
            ("idle", "대기", NSColor(srgbRed: 0.42, green: 0.85, blue: 0.60, alpha: 1)),
        ]
        for (key, name, color) in states {
            guard let n = opts.summary[key], n > 0 else { continue }
            let dot = roundedBox(color: color, radius: 3)
            dot.widthAnchor.constraint(equalToConstant: 6).isActive = true
            dot.heightAnchor.constraint(equalToConstant: 6).isActive = true
            let label = makeLabel("\(name) \(n)", size: 11.5, weight: .medium, color: NSColor.white.withAlphaComponent(0.6))
            let item = NSStackView(views: [dot, label])
            item.spacing = 5
            item.alignment = .centerY
            parts.append(item)
        }
        let summary = NSStackView(views: parts)
        summary.spacing = 12
        summary.isHidden = parts.isEmpty

        let track = roundedBox(color: NSColor.white.withAlphaComponent(0.08), radius: 1)
        progressFill = roundedBox(color: accent.withAlphaComponent(0.85), radius: 1)
        track.addSubview(progressFill)

        let text = NSStackView(views: [body, summary])
        text.orientation = .vertical
        text.alignment = .leading
        text.spacing = 7

        let column = NSStackView(views: [header, text, track])
        column.orientation = .vertical
        column.alignment = .leading
        column.spacing = 6
        column.setCustomSpacing(12, after: text)
        column.translatesAutoresizingMaskIntoConstraints = false
        blur.addSubview(column)

        let inner = width - pad * 2
        let indent: CGFloat = 30 + 10
        progressWidth = progressFill.widthAnchor.constraint(equalToConstant: inner)
        NSLayoutConstraint.activate([
            root.widthAnchor.constraint(equalToConstant: width),
            blur.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            blur.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            blur.topAnchor.constraint(equalTo: root.topAnchor),
            blur.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            backdrop.leadingAnchor.constraint(equalTo: blur.leadingAnchor),
            backdrop.trailingAnchor.constraint(equalTo: blur.trailingAnchor),
            backdrop.topAnchor.constraint(equalTo: blur.topAnchor),
            backdrop.bottomAnchor.constraint(equalTo: blur.bottomAnchor),
            glow.leadingAnchor.constraint(equalTo: blur.leadingAnchor),
            glow.trailingAnchor.constraint(equalTo: blur.trailingAnchor),
            glow.topAnchor.constraint(equalTo: blur.topAnchor),
            glow.bottomAnchor.constraint(equalTo: blur.bottomAnchor),

            column.leadingAnchor.constraint(equalTo: blur.leadingAnchor, constant: pad),
            column.trailingAnchor.constraint(equalTo: blur.trailingAnchor, constant: -pad),
            column.topAnchor.constraint(equalTo: blur.topAnchor, constant: 14),
            column.bottomAnchor.constraint(equalTo: blur.bottomAnchor, constant: -12),

            header.widthAnchor.constraint(equalToConstant: inner),
            badge.widthAnchor.constraint(equalToConstant: 30),
            badge.heightAnchor.constraint(equalToConstant: 30),
            icon.centerXAnchor.constraint(equalTo: badge.centerXAnchor),
            icon.centerYAnchor.constraint(equalTo: badge.centerYAnchor),

            text.leadingAnchor.constraint(equalTo: column.leadingAnchor, constant: indent),
            text.widthAnchor.constraint(equalToConstant: inner - indent),

            track.widthAnchor.constraint(equalToConstant: inner),
            track.heightAnchor.constraint(equalToConstant: 2),
            progressFill.leadingAnchor.constraint(equalTo: track.leadingAnchor),
            progressFill.topAnchor.constraint(equalTo: track.topAnchor),
            progressFill.bottomAnchor.constraint(equalTo: track.bottomAnchor),
            progressWidth,
        ])
        body.preferredMaxLayoutWidth = inner - indent
        return root
    }

    func tick() {
        guard !closing else { return }
        if !hovered { elapsed += 0.05 }
        ticks += 1
        if ticks % 20 == 0 { slot.touch() }
        let remaining = max(0, opts.duration - elapsed)
        progressWidth.constant = (width - pad * 2) * CGFloat(remaining / opts.duration)
        if remaining <= 0 { dismiss() }
    }

    func clicked() {
        if !opts.activate.isEmpty,
           let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: opts.activate) {
            NSWorkspace.shared.openApplication(at: url, configuration: NSWorkspace.OpenConfiguration())
        }
        dismiss()
    }

    func dismiss() {
        guard !closing else { return }
        closing = true
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.25
            panel.animator().alphaValue = 0
            panel.animator().setFrame(panel.frame.offsetBy(dx: 0, dy: 10), display: true)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in
            self?.slot.release()
            exit(0)
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
