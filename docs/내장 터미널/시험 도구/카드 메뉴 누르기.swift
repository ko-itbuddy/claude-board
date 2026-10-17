// axrow <pid> <카드 제목 글자> — 초점 창에서 제목 글자가 든 카드(AXStaticText)를 찾고, 문서 순서상 그 뒤 첫 "더 보기" 단추를 누른다.
// 제목 글자가 든 카드가 정확히 하나여야 하고, 단추의 세로 위치가 제목과 30pt 안이어야 누른다(다른 카드 단추를 누르지 않게)
import AppKit
let a = CommandLine.arguments
let pid = pid_t(a[1])!, want = a[2]
func attr(_ e: AXUIElement, _ n: String) -> CFTypeRef? { var v: CFTypeRef?; AXUIElementCopyAttributeValue(e, n as CFString, &v); return v }
func s(_ e: AXUIElement, _ n: String) -> String { (attr(e, n) as? String) ?? "" }
func y(_ e: AXUIElement) -> CGFloat { guard let p = attr(e, kAXPositionAttribute) else { return -1 }; var pt = CGPoint.zero; AXValueGetValue(p as! AXValue, .cgPoint, &pt); return pt.y }
var flat: [AXUIElement] = []
func walk(_ e: AXUIElement, _ d: Int) { flat.append(e); guard d < 40, let k = attr(e, kAXChildrenAttribute) as? [AXUIElement] else { return }; for c in k { walk(c, d + 1) } }
let app = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(app, 2)
guard let w = attr(app, kAXFocusedWindowAttribute) else { print("창 없음"); exit(1) }
walk(w as! AXUIElement, 0)
let hits = flat.indices.filter { s(flat[$0], kAXRoleAttribute) == "AXStaticText" && ((attr(flat[$0], kAXValueAttribute) as? String) ?? "").contains(want) }
guard hits.count == 1 else { print("멈춤: 제목이 든 카드 \(hits.count)개"); exit(3) }
let i = hits[0]
guard let j = flat.indices.first(where: { $0 > i && s(flat[$0], kAXRoleAttribute) == "AXButton" && s(flat[$0], kAXDescriptionAttribute) == "더 보기" }) else { print("멈춤: 단추 없음"); exit(3) }
// 사이에 다른 카드 제목(AXStaticText 다음 상태 AXImage)이 끼면 안 된다 — 위치로 확인
let ty = y(flat[i]), by = y(flat[j])
print("카드:", (attr(flat[i], kAXValueAttribute) as? String) ?? "", "y", Int(ty), "단추 y", Int(by))
guard abs(by - ty) < 30 else { print("멈춤: 단추가 카드 줄이 아님"); exit(3) }
let r = AXUIElementPerformAction(flat[j], kAXPressAction as CFString)
print("누름 결과", r.rawValue)
