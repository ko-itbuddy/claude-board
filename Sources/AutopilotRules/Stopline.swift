// 자율주행 규칙(AutopilotRules) — 계정 멈출 선(설계 4장 "사용률", 6장 6단계).
// 2026-10-30 6단계 ⑫-6 첫 커밋에서 엔진 stoplineGate 의 판정(사용률·멈출 선 견주기, 기다릴 때와 그 글)과 usageFrom(apiJSON:)(퍼센트 꼴 사용량 풀기)을
// 같은 동작으로 옮겼다(옮기기 커밋, 덤프 같음). 사용률 읽기(stream·캐시·사용량 API)·상태 바꾸기·기다리기는 엔진에 남는다
import Foundation

// 사용량 값의 수 — 엔진 num() 과 같다(NSNumber, 수로 읽히는 글자)
func apStopNum(_ v: Any?) -> Double? { (v as? NSNumber)?.doubleValue ?? (v as? String).flatMap { Double($0) } }

// ISO 8601 시각(초) — 엔진 isoDate 와 같다. 소수 초 자릿수가 많으면(…00.318890+00:00) 떼고 읽는다
public func apISODate(_ s: String?) -> Double? {
    guard var s else { return nil }
    let f = ISO8601DateFormatter()
    if let d = f.date(from: s) { return d.timeIntervalSince1970 }
    if let r = s.range(of: #"\.\d+"#, options: .regularExpression) { s.removeSubrange(r) }
    return f.date(from: s)?.timeIntervalSince1970
}

// 사용량 API 응답·보드와 상태줄이 함께 쓰는 캐시(/tmp/.claude-usage-cache.json) 꼴({five_hour:{utilization 84.0, resets_at}, seven_day:{…}}) →
// 엔진 run.json usage 꼴({at, from, five_hour 0.84, five_hour_resets, seven_day, seven_day_resets}). 퍼센트를 비율로 맞춘다.
// 2026-11(5시간은 있을 때만): 5시간 한도가 없는 요금제(Ultra)는 five_hour 가 null 로 온다(보드 parseUsage 3.2.7 과 같은 까닭). 객체인 창만 넣고 없는 창은 칸을 넣지 않는다.
// 두 창 모두 사용률(utilization)을 읽지 못하면 nil(사용률을 모름). 그 전에는 five_hour 객체가 없으면 nil 이라 Ultra 계정에서 사용률을 늘 몰랐다
public func apUsageFromPercent(_ j: [String: Any], from: String, at: Double) -> [String: Any]? {
    var u: [String: Any] = ["at": (at * 1000).rounded() / 1000, "from": from]
    var read = false
    for key in ["five_hour", "seven_day"] {
        guard let w = j[key] as? [String: Any] else { continue }
        if let v = apStopNum(w["utilization"]) { u[key] = v / 100; read = true }
        if let r = apISODate(w["resets_at"] as? String) { u[key + "_resets"] = r }
    }
    return read ? u : nil
}

// 5시간 칸(사용률 막대·멈출 선 고르기·status 줄)을 보일까 — 사용률을 읽었는데(known) 5시간 값이 없으면 그 계정은 5시간 한도가 없는 요금제라 숨긴다.
// 아직 읽지 못했으면 지금처럼 보인다(모름). 멈출 선 설정의 five_hour 값은 그대로 남는다(5시간 한도가 있는 계정으로 바꾸면 다시 본다)
public func apShowFiveHour(known: Bool, hasFive: Bool) -> Bool { !known || hasFive }
// run.json usage 꼴로 — 주간이나 5시간 사용률 가운데 하나라도 수면 읽은 것
public func apShowFiveHour(usage: [String: Any]?) -> Bool {
    let five = apStopNum(usage?["five_hour"]) != nil
    return apShowFiveHour(known: five || apStopNum(usage?["seven_day"]) != nil, hasFive: five)
}

// 엔진 when() 과 같은 꼴("M월 d일 HH:mm", 한국어)
func apWhenText(_ t: Double, tz: TimeZone) -> String {
    let f = DateFormatter()
    f.locale = Locale(identifier: "ko_KR")
    f.timeZone = tz
    f.dateFormat = "M월 d일 HH:mm"
    return f.string(from: Date(timeIntervalSince1970: t))
}

// 멈출 선에 걸려 기다릴 때 — 다시 볼 시각과 진행 기록·run.json reason 의 글
public struct APStoplineWait: Equatable, Sendable {
    public let until: Double
    public let reason: String
    public init(until: Double, reason: String) { self.until = until; self.reason = reason }
}

// 새 단위를 시작해도 되는가(엔진 stoplineGate 의 판정). usage 는 run.json usage 꼴(비율, nil 이면 사용률을 모름), stopline 은 고정 설정의 멈출 선
// ({seven_day 0.80, five_hour 0.95} 이 기본). 초기화 시각이 지난 창은 0 으로 본다. 주간이 먼저, 그다음 5시간. 넘으면 초기화 시각(지났거나 없으면 10분 뒤) + 2분까지.
// usage 에 5시간 칸이 없으면(5시간 한도가 없는 요금제, apUsageFromPercent) 5시간 선은 보지 않고 주간 선만 본다(2026-11, 판정은 그 전과 같음).
// 사용률을 모르면 10분 뒤 다시. 시작해도 되면 nil
public func apStoplineWait(usage: [String: Any]?, stopline: [String: Any], now t: Double, tz: TimeZone) -> APStoplineWait? {
    let l7 = apStopNum(stopline["seven_day"]) ?? 0.80, l5 = apStopNum(stopline["five_hour"]) ?? 0.95
    guard let u = usage else {
        return APStoplineWait(until: t + 600, reason: "계정 사용률을 알 수 없어 새 단위를 시작하지 않고 10분 뒤 다시 확인합니다.")
    }
    let r7 = apStopNum(u["seven_day_resets"]) ?? 0, r5 = apStopNum(u["five_hour_resets"]) ?? 0
    let v7 = r7 > 0 && r7 <= t ? 0 : apStopNum(u["seven_day"]) ?? 0     // 초기화 시각이 지났으면 그 창은 0 으로 본다
    let v5 = r5 > 0 && r5 <= t ? 0 : apStopNum(u["five_hour"]) ?? 0
    if v7 >= l7 {
        let until = (r7 > t ? r7 : t + 600) + 120
        return APStoplineWait(until: until, reason: "계정 주간 사용률 \(Int((v7 * 100).rounded()))% 가 멈출 선 \(Int((l7 * 100).rounded()))% 에 닿아 새 단위를 시작하지 않고 \(apWhenText(until, tz: tz)) 까지 기다립니다.")
    }
    if v5 >= l5 {
        let until = (r5 > t ? r5 : t + 600) + 120
        return APStoplineWait(until: until, reason: "계정 5시간 사용률 \(Int((v5 * 100).rounded()))% 가 멈출 선 \(Int((l5 * 100).rounded()))% 에 닿아 새 단위를 시작하지 않고 \(apWhenText(until, tz: tz)) 까지 기다립니다.")
    }
    return nil
}
