// Claude Code 세션 보드 — 작업 경로별 카드로 세션 상태를 보여 주는 네이티브 창
// 상태 파일: ~/.claude/notifier/sessions/*.json (state.py 가 hook 에서 기록)
import AppKit
import AutopilotRules
import BoardRules
import Combine
import SwiftUI

// MARK: - 자율주행 · 글자 표와 공용 부품

// 자율주행 화면 가독성 고치기 1단계(2026-11-10): 자율주행 화면의 글자 크기를 이 표 하나로 정하고, 공용 부품(꼬리표·단추·고른 값·막대·상자·접는 줄)을 여기 모았다.
// 전에는 .boardFont(10.5) 같은 숫자가 320곳에 흩어져 본문이 보드 기준 글자(13pt)보다 2~3pt 작았다. 화면 쪽은 숫자 대신 .boardFont(APType.body) 처럼 이름으로 쓴다.
// 크기는 모두 보드 글자 비율(⌥⌘+, BoardFontScale)을 곱한다. 미니 크기 화면과 접힌 한 줄(\.apMini)은 공간이 없어 1pt 작게(고치기 전보다 +1pt 안쪽).

// 글자 표 — 이름 하나에 크기·미니 크기·글꼴 모양·여러 줄일 때의 줄 간격
struct APType: Sendable {
    let size: CGFloat                   // 보드 글자 비율 1.0 에서의 pt
    let mini: CGFloat                   // 미니 크기 화면·접힌 한 줄에서의 pt
    var design: Font.Design = .default
    var spacing: CGFloat = 0            // 줄 간격(여러 줄 글만 벌어진다)

    static let caption = APType(size: 11.5, mini: 10.5, spacing: 1)                         // 꼬리표·시각·작은 표시
    static let note = APType(size: 12, mini: 11, spacing: 2)                                // 보조 설명·두 번째 줄
    static let body = APType(size: 13, mini: 12, spacing: 2)                                // 본문·줄 제목·단추
    static let label = APType(size: 14, mini: 13, spacing: 2)                               // 상자 제목·강조 값
    static let heading = APType(size: 15, mini: 14, spacing: 1)                             // 단위 제목·카드 제목
    static let title = APType(size: 17, mini: 16)                                           // 시트 머리
    static let screen = APType(size: 18, mini: 17)                                          // 화면 머리(자율주행 중·지난 실행)
    static let mono = APType(size: 12.5, mini: 11.5, design: .monospaced, spacing: 1)       // 고정폭 본문(가지·경로·파일)
    static let monoSmall = APType(size: 11.5, mini: 10.5, design: .monospaced, spacing: 1)  // 고정폭 작은 것(시각·번호)
    static let value = APType(size: 12.5, mini: 11.5)                                       // 막대 값(APMeter) — 고정폭이 아닌 12.5
}

// 미니 크기 화면(BoardTier.mini 의 자율주행 중·지난 실행)과 접힌 한 줄(실행 줄 compact)에서 켠다 — APType 이 mini 크기로 그린다
private struct APMiniKey: EnvironmentKey { static let defaultValue = false }
extension EnvironmentValues {
    var apMini: Bool {
        get { self[APMiniKey.self] }
        set { self[APMiniKey.self] = newValue }
    }
}

struct APFont: ViewModifier {
    @ObservedObject var fontScale = BoardFontScale.shared
    @Environment(\.apMini) private var mini
    let type: APType
    let weight: Font.Weight
    let design: Font.Design
    func body(content: Content) -> some View {
        content
            .font(.system(size: (mini ? type.mini : type.size) * fontScale.scale, weight: weight, design: design))
            .lineSpacing(type.spacing)
    }
}

extension View {
    // 자율주행 글자 — 글자 표 이름으로. design 을 주지 않으면 표의 모양(mono·monoSmall 은 고정폭)
    func boardFont(_ type: APType, weight: Font.Weight = .regular, design: Font.Design? = nil) -> some View {
        modifier(APFont(type: type, weight: weight, design: design ?? type.design))
    }
}

// 꼬리표 — 상태는 strong(진하게)
struct APChip: View {
    let text: String
    var color: Color = APColor.muted
    var strong = false      // 상태 꼬리표는 진하게
    var mono = false
    var body: some View {
        Text(text)
            .boardFont(APType.caption, weight: .semibold, design: mono ? .monospaced : .default)
            .foregroundStyle(color)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .background(Capsule().fill(color.opacity(strong ? 0.20 : 0.09)))
            .overlay(Capsule().stroke(color.opacity(strong ? 0.38 : 0.18), lineWidth: 0.5))
            .lineLimit(1)
            .fixedSize()
    }
}

// 자율주행 표지 — 실행 줄 앞머리
struct APMark: View {
    @Environment(\.apMini) private var mini
    var body: some View {
        HStack(spacing: 3) {
            Image(systemName: "steeringwheel").font(.system(size: mini ? 10 : 11, weight: .bold))
            Text("자율주행")
        }
        .boardFont(APType.caption, weight: .bold)
        .foregroundStyle(Palette.accent)
        .padding(.horizontal, 7).padding(.vertical, 2)
        .background(Capsule().fill(Palette.accent.opacity(0.16)))
        .fixedSize()
    }
}

// 이름 · 값 · 막대(멈출 선 눈금)
struct APMeter: View {
    let label: String
    let value: String
    var fraction: Double? = nil
    var tick: Double? = nil
    var color: Color = APColor.work
    var note: String? = nil
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .firstTextBaseline, spacing: 4) {
                Text(label).boardFont(APType.note).foregroundStyle(.secondary).lineLimit(1)
                Spacer(minLength: 4)
                Text(value).boardFont(APType.value, weight: .semibold).monospacedDigit().lineLimit(1)
            }
            if let f = fraction {
                // 막대·눈금을 Canvas 하나로 그린다 — GeometryReader 와 겹친 보기 넷보다 배치가 가볍다(보드는 매초 다시 그린다)
                Canvas { ctx, size in
                    let y: CGFloat = tick == nil ? 0 : 2, h: CGFloat = 6, w = size.width
                    ctx.fill(Path(roundedRect: CGRect(x: 0, y: y, width: w, height: h), cornerRadius: h / 2), with: .color(.white.opacity(0.08)))
                    let fw = max(3, w * CGFloat(min(max(f, 0), 1)))
                    ctx.fill(Path(roundedRect: CGRect(x: 0, y: y, width: fw, height: h), cornerRadius: h / 2), with: .color(color))
                    if let t = tick {
                        ctx.fill(Path(CGRect(x: w * CGFloat(min(max(t, 0), 1)) - 0.75, y: 0, width: 1.5, height: 10)), with: .color(.white.opacity(0.8)))
                    }
                }
                .frame(height: tick == nil ? 6 : 10)
            } else if let note {
                Text(note).boardFont(APType.caption).foregroundStyle(.secondary).monospacedDigit().lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// 글자 단추 — 역할(role)로 꼴을 나눈다. 잠긴 단추는 흐리게, 도움말에 까닭.
//  주 단추(.primary, 또는 primary: true): 꽉 찬 주황 바탕 + 진한 글 — 한 화면에 하나가 기본(다음·출발·합치기)
//  보통(.normal): 흐린 흰 바탕 + 테두리
//  위험(.destructive): 붉은 글·붉은 테두리(중단·버리기·지우기)
//  tint 를 주면(역할 없이) 그 색의 흐린 알약 — 주의(APColor.limit)·기다리는 것(APColor.wait) 표시용. 고른 값은 단추가 아니라 APChoice 로 그린다
struct APButton: View {
    enum Role { case normal, primary, destructive }
    let title: String
    var icon: String? = nil
    var tint: Color? = nil
    var primary = false
    var role: Role? = nil
    var enabled = true
    var help = ""
    var axid = ""
    let action: @MainActor () -> Void
    @Environment(\.apMini) private var mini
    @State private var hover = false

    private var kind: Role { role ?? (primary ? .primary : .normal) }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 5) {
                if let icon { Image(systemName: icon).font(.system(size: mini ? 10.5 : 12, weight: .bold)) }
                Text(title).boardFont(APType.body, weight: .semibold).lineLimit(1)
            }
            .fixedSize()
            .foregroundStyle(fg)
            .padding(.horizontal, mini ? 9 : 12).frame(height: mini ? 26 : 30)
            .background(Capsule().fill(fill))
            .overlay(Capsule().stroke(stroke, lineWidth: kind == .destructive && enabled ? 1 : 0.5))
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .modifier(ClickableIf(active: enabled, hover: $hover))
        .hint(help.isEmpty ? title : help)
        .accessibilityIdentifier(axid)
    }

    private var fg: Color {
        guard enabled else { return .secondary }
        switch kind {
        case .primary: return Color(white: 0.07)
        case .destructive: return APColor.fail
        case .normal: return tint ?? .primary
        }
    }
    private var fill: Color {
        switch kind {
        case .primary: return enabled ? Palette.accent.opacity(hover ? 1 : 0.92) : Palette.accent.opacity(0.10)
        case .destructive: return APColor.fail.opacity(enabled ? (hover ? 0.16 : 0.06) : 0.03)
        case .normal:
            let c = tint ?? .white
            return c.opacity(enabled ? (hover ? 0.22 : (tint == nil ? 0.08 : 0.16)) : 0.04)
        }
    }
    private var stroke: Color {
        switch kind {
        case .primary: return enabled ? Color.white.opacity(hover ? 0.35 : 0.18) : Palette.accent.opacity(0.22)
        case .destructive: return APColor.fail.opacity(enabled ? 0.60 : 0.18)
        case .normal: return (tint ?? .white).opacity(enabled ? (tint == nil ? 0.20 : 0.30) : 0.10)
        }
    }
}

// 고른 값(선택지·설정 줄 값) — 단추와 다른 꼴. 고른 것은 흰 바탕 + 밝은 테두리 + 앞에 체크 + 흰 굵은 글, 안 고른 것은 흐린 글·테두리만.
// 주황을 쓰지 않는다(주황은 주 단추·자율주행 표지·주의). 모서리를 둥근 네모로 해 알약 꼴 단추와 갈라 보이게 한다
struct APChoice: View {
    let title: String
    let on: Bool
    var icon: String? = nil
    var enabled = true
    var help = ""
    var axid = ""
    let action: @MainActor () -> Void
    @Environment(\.apMini) private var mini
    @State private var hover = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 5) {
                if on { Image(systemName: "checkmark").font(.system(size: mini ? 9.5 : 11, weight: .heavy)).accessibilityHidden(true) }
                if let icon { Image(systemName: icon).font(.system(size: mini ? 10.5 : 12, weight: .semibold)) }
                Text(title).boardFont(APType.body, weight: on ? .bold : .regular).lineLimit(1)
            }
            .fixedSize()
            .foregroundStyle(enabled ? (on ? Color.white : Color.white.opacity(hover ? 0.85 : 0.62)) : Color.secondary)
            .padding(.horizontal, mini ? 8 : 11).frame(height: mini ? 26 : 30)
            .background(RoundedRectangle(cornerRadius: 7).fill(Color.white.opacity(on ? (enabled ? 0.16 : 0.08) : (hover && enabled ? 0.06 : 0))))
            .overlay(RoundedRectangle(cornerRadius: 7).stroke(Color.white.opacity(on ? (enabled ? 0.62 : 0.25) : 0.16), lineWidth: on ? 1 : 0.5))
            .contentShape(RoundedRectangle(cornerRadius: 7))
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .modifier(ClickableIf(active: enabled, hover: $hover))
        .hint(help.isEmpty ? title : help)
        .accessibilityAddTraits(on ? .isSelected : [])
        .accessibilityIdentifier(axid)
    }
}

// 돌아가기·건너가기 글자 단추 — "← 보드" 는 앞에, "합치기·승격 →" 는 뒤에 꺾쇠 아이콘을 그린다(글의 화살표는 빼고 그리되 접근성 이름은 받은 글 그대로)
struct APBack: View {
    let title: String
    let axid: String
    let action: @MainActor () -> Void
    @Environment(\.apMini) private var mini
    @State private var hover = false
    var body: some View {
        let back = title.hasPrefix("← "), fwd = title.hasSuffix(" →")
        let text = back ? String(title.dropFirst(2)) : fwd ? String(title.dropLast(2)) : title
        Button(action: action) {
            HStack(spacing: 4) {
                if back { Image(systemName: "chevron.left").font(.system(size: mini ? 10.5 : 12, weight: .bold)) }
                Text(text).boardFont(APType(size: 13.5, mini: 12.5), weight: .semibold)
                if fwd { Image(systemName: "chevron.right").font(.system(size: mini ? 10.5 : 12, weight: .bold)) }
            }
            .fixedSize()
            .foregroundStyle(Palette.accent.opacity(hover ? 1 : 0.88))
            .padding(.vertical, mini ? 3 : 5).padding(.trailing, mini ? 3 : 5)   // 누르는 자리를 글보다 넓게(앞쪽은 내용 줄 맞춤 그대로)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .accessibilityLabel(title)
        .accessibilityIdentifier(axid)
    }
}

// 제목이 있는 상자
struct APBox<Content: View>: View {
    let title: String
    var note = ""
    var tint: Color? = nil
    var axid = ""
    @ViewBuilder let content: () -> Content
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(title).boardFont(APType.label, weight: .bold).fixedSize()
                if !note.isEmpty { Text(note).boardFont(APType.note).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true) }
                Spacer(minLength: 0)
            }
            content()
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 12).fill((tint ?? Color.white).opacity(tint == nil ? 0.04 : 0.08)))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke((tint ?? Color.white).opacity(tint == nil ? 0.08 : 0.32), lineWidth: 0.5))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(axid)
    }
}

// 회색 설명 글 — 12pt, 여러 줄이면 줄 간격을 벌린다
func apNote(_ s: String, _ color: Color = .secondary) -> some View {
    Text(s).boardFont(APType.note).foregroundStyle(color).fixedSize(horizontal: false, vertical: true)
}

// 고르는 줄(동그라미 또는 네모) — 잠긴 줄은 흐리게, 도움말에 까닭
struct APPickRow<Label: View>: View {
    let on: Bool
    var square = false
    var enabled = true
    var help = ""
    let axid: String
    let action: @MainActor () -> Void
    @ViewBuilder let label: () -> Label
    @State private var hover = false
    var body: some View {
        Button(action: action) {
            HStack(alignment: .top, spacing: 9) {
                Image(systemName: square ? (on ? "checkmark.square.fill" : "square") : (on ? "largecircle.fill.circle" : "circle"))
                    .font(.system(size: 14)).foregroundStyle(on ? Palette.accent : .secondary).padding(.top, 1)
                label()
                Spacer(minLength: 0)
            }
            .padding(10)
            .background(RoundedRectangle(cornerRadius: 10).fill(Color.white.opacity(on ? 0.08 : (hover && enabled ? 0.05 : 0.02))))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(on ? Palette.accent.opacity(0.5) : Color.white.opacity(0.06), lineWidth: on ? 1 : 0.5))
            .contentShape(Rectangle())
            .opacity(enabled ? 1 : 0.55)
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .modifier(ClickableIf(active: enabled, hover: $hover))
        .hint(help)
        .accessibilityIdentifier(axid)
    }
}

// 접히는 줄 — 머리에 요약(접혀 있어도 보임)
struct APFold<Content: View>: View {
    let title: String
    let summary: String
    let axid: String
    @Binding var open: Bool
    var tint: Color? = nil
    var flash = false
    var tag: String? = nil
    @ViewBuilder let content: () -> Content
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Button { withAnimation(foldSpring) { open.toggle() } } label: {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: 11, weight: .bold)).foregroundStyle(.secondary).frame(width: 12)
                    Text(title).boardFont(APType.label, weight: .bold).fixedSize()
                    Text(summary).boardFont(APType.body).foregroundStyle(flash ? Palette.accent : .secondary).lineLimit(2)
                    if let tag { APChip(text: tag, color: Palette.accent, strong: true) }
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier(axid)
            if open { content().padding(.leading, 20) }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 11).fill(flash ? Palette.accent.opacity(0.16) : (tint ?? Color.white).opacity(tint == nil ? 0.035 : 0.07)))
        .overlay(RoundedRectangle(cornerRadius: 11).stroke((tint ?? Color.white).opacity(tint == nil ? 0.07 : 0.28), lineWidth: 0.5))
    }
}

// MARK: 2단계에서 더한 부품(보드 칸 화면) — 아래 부품은 출발 준비·합치기 시트에서도 쓴다

// ⓘ 도움말 — 늘 붙어 있던 화면 사용법 글을 제목 옆 아이콘으로 옮긴다(마우스를 올리면 .hint). 정보(값·까닭·경고)는 글로 남기고 이것에 넣지 않는다
struct APInfo: View {
    let text: String
    var axid = ""
    @Environment(\.apMini) private var mini
    @State private var hover = false
    var body: some View {
        Image(systemName: "info.circle")
            .font(.system(size: mini ? 11 : 12.5, weight: .medium))
            .foregroundStyle(Color.secondary.opacity(hover ? 1 : 0.7))
            .padding(2)
            .contentShape(Rectangle())
            .onHover { hover = $0 }
            .hint(text)
            .accessibilityLabel("도움말")
            .accessibilityIdentifier(axid)
    }
}

// 덜 눈에 띄는 글자 단추 — 자주 쓰지 않는 행동(멈추고 다시 출발·터미널로 이어받기·원문 지우기)을 주 단추 줄 아래 작은 줄에.
// 바탕 없이 아이콘 + 12pt 글, 올리면 밝아진다. tint 를 주면 그 색(위험은 APColor.fail). 잠기면 흐리게, 도움말에 까닭
struct APMinorButton: View {
    let title: String
    var icon: String? = nil
    var tint: Color? = nil
    var enabled = true
    var help = ""
    var axid = ""
    let action: @MainActor () -> Void
    @Environment(\.apMini) private var mini
    @State private var hover = false
    var body: some View {
        Button(action: action) {
            HStack(spacing: 4) {
                if let icon { Image(systemName: icon).font(.system(size: mini ? 10 : 11.5, weight: .semibold)) }
                Text(title).boardFont(APType.note, weight: .semibold).lineLimit(1)
            }
            .fixedSize()
            .foregroundStyle(enabled ? (tint ?? Color.white).opacity(hover ? 1 : (tint == nil ? 0.72 : 0.85)) : Color.secondary.opacity(0.6))
            .padding(.vertical, 4).padding(.horizontal, 2)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .modifier(ClickableIf(active: enabled, hover: $hover))
        .hint(help.isEmpty ? title : help)
        .accessibilityIdentifier(axid)
    }
}

// 얇은 막대(높이 4) — 상한까지 얼마나 썼는지. 값 글자는 옆이나 위에 따로 쓴다
struct APThinBar: View {
    let fraction: Double
    var color: Color = APColor.work
    var body: some View {
        Canvas { ctx, size in
            let h = size.height, w = size.width
            ctx.fill(Path(roundedRect: CGRect(x: 0, y: 0, width: w, height: h), cornerRadius: h / 2), with: .color(.white.opacity(0.08)))
            let fw = max(h, w * CGFloat(min(max(fraction, 0), 1)))
            ctx.fill(Path(roundedRect: CGRect(x: 0, y: 0, width: fw, height: h), cornerRadius: h / 2), with: .color(color))
        }
        .frame(height: 4)
        .accessibilityHidden(true)
    }
}

// 누르는 줄 끝의 "건너가기" 알약 — 줄 전체가 단추일 때 무엇을 하는 줄인지 보인다(예: 끝난 실행 줄의 "합치기·승격 ›"). 단추가 아니라 꼴만
struct APGoPill: View {
    let title: String
    var color: Color = Palette.accent
    var hover = false
    @Environment(\.apMini) private var mini
    var body: some View {
        HStack(spacing: 3) {
            Text(title).boardFont(APType.note, weight: .bold)
            Image(systemName: "chevron.right").font(.system(size: mini ? 9 : 10, weight: .heavy))
        }
        .fixedSize()
        .foregroundStyle(color)
        .padding(.horizontal, mini ? 7 : 9).frame(height: mini ? 22 : 25)
        .background(Capsule().fill(color.opacity(hover ? 0.26 : 0.16)))
        .overlay(Capsule().stroke(color.opacity(0.45), lineWidth: 0.5))
    }
}
