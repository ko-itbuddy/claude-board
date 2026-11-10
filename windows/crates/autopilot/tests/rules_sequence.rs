//! (청크 Sequence 1/2 — 전반 시험만) 흐름 그림 읽기 규칙(`rules::sequence`) 시험. 원본: `Tests/AutopilotRulesTests/ReportTests.swift` 의 `SequenceTests`
//! (`describe`·`seqRows` 표는 같은 파일). Swift 시험 4개 가운데 이 파일이 옮긴 것은 `읽기`(표 25 줄)와 `폭`이다.
//! 남은 둘(`그리기`·`넓히기`, SVG)은 R-Sequence-b 가 옮긴다. 표의 줄은 Swift 에서 기계로 옮겼다(손으로 다시 치지 않았다).
//! 글자 그대로 옮길 수 없는 두 줄(`참여자 25명`·`줄 301개`, 원본이 `map`·`String(repeating:)` 으로 만든 입력)만 시험 함수 안에서 같은 식으로 만든다.
//! 표 길이를 세어 줄이 빠지면 실패한다. 끝의 "추가" 시험은 Swift 시험에 없는 것(경계·Foundation 과의 차이를 지키는 것)이다.

use claude_board_autopilot::rules::sequence::{parse_sequence, text_width, SeqItem, SeqParse};

struct Row {
    note: &'static str,
    src: &'static str,
    want: &'static str,
}

// 아래 표는 Swift 시험 파일에서 기계로 옮긴 것이다: 글자 값은 그대로 복사했다(Swift 의 \n·\" 는 Rust 에서도 같은 뜻이다).
const SEQ_ROWS: [Row; 23] = [
    Row { note: "가장 작은 꼴", src: "sequenceDiagram\nA->>B: 안녕", want: "P:A,B | 0->>1:안녕" },
    Row { note: "선언한 차례와 이름, actor", src: "sequenceDiagram\n actor U as 사용자\n participant A as API 서버\n A->>U: 답", want: "P:@U=사용자,A=API 서버 | 1->>0:답" },
    Row { note: "선언 없이 나온 차례", src: "sequenceDiagram\nB->>A: x\nC-->>B: y", want: "P:B,A,C | 0->>1:x · 2-->>0:y" },
    Row { note: "화살표 열 가지", src: "sequenceDiagram\nA->B: 1\nA-->B: 2\nA->>B: 3\nA-->>B: 4\nA-xB: 5\nA--xB: 6\nA-)B: 7\nA--)B: 8\nA<<->>B: 9\nA<<-->>B: 10", want: "P:A,B | 0-1:1 · 0--1:2 · 0->>1:3 · 0-->>1:4 · 0-x1:5 · 0--x1:6 · 0-)1:7 · 0--)1:8 · 0<->>1:9 · 0<-->>1:10" },
    Row { note: "켜기 표지 +·- 는 무시", src: "sequenceDiagram\nA->>+B: 가\nB-->>-A: 나", want: "P:A,B | 0->>1:가 · 1-->>0:나" },
    Row { note: "쌍점 뒤의 화살표 꼴은 글", src: "sequenceDiagram\nA->>B: a->b 로", want: "P:A,B | 0->>1:a->b 로" },
    Row { note: "글 없는 메시지", src: "sequenceDiagram\nA->>B", want: "P:A,B | 0->>1:" },
    Row { note: "Note 셋", src: "sequenceDiagram\nparticipant A\nparticipant B\nNote left of A: 왼\nnote right of B: 오\nNote over A,B: 둘", want: "P:A,B | N left 0:왼 · N right 1:오 · N over 0,1:둘" },
    Row { note: "묶음과 갈래, 빈 줄·주석·끝 쌍반점", src: "sequenceDiagram\n%% 주석\n\nalt 맞음;\nA->>B: x\nelse 틀림\nA-xB: y\nend\nloop 매시\nB->>B: 다시\nend", want: "P:A,B | [alt 맞음 · 0->>1:x · |else 틀림 · 0-x1:y · ] · [loop 매시 · 1->>1:다시 · ]" },
    Row { note: "par/and·critical/option·break·opt", src: "sequenceDiagram\npar 가\nA->>B: 1\nand 나\nA->>B: 2\nend\ncritical 꼭\nA->>B: 3\noption 실패\nA->>B: 4\nend\nbreak 멈춤\nA->>B: 5\nend\nopt 골라\nA->>B: 6\nend", want: "P:A,B | [par 가 · 0->>1:1 · |and 나 · 0->>1:2 · ] · [critical 꼭 · 0->>1:3 · |option 실패 · 0->>1:4 · ] · [break 멈춤 · 0->>1:5 · ] · [opt 골라 · 0->>1:6 · ]" },
    Row { note: "rect·box 의 색 글자는 버림", src: "sequenceDiagram\nbox rgb(1,2,3) 묶음\nparticipant A\nend\nrect rgba(0,0,255,.1)\nA->>A: x\nend", want: "P:A | [box · ] · [rect · 0->>0:x · ]" },
    Row { note: "닫지 않은 묶음은 끝에서 닫음", src: "sequenceDiagram\nloop 계속\nA->>B: x", want: "P:A,B | [loop 계속 · 0->>1:x · ]" },
    Row { note: "autonumber·title·activate·destroy·create", src: "sequenceDiagram\ntitle: 제목\nautonumber\ncreate participant C as 새것\nactivate A\nA->>C: x\ndeactivate A\ndestroy C", want: "P:C=새것,A | 1->>0:x | # | 제목 제목" },
    Row { note: "다른 그림 꼴", src: "graph TD; A-->B; click A \"javascript:alert(1)\"", want: "못 그림 1: sequenceDiagram 으로 시작하지 않습니다" },
    Row { note: "빈 원문", src: "  \n", want: "못 그림 1: 비어 있습니다" },
    Row { note: "참여자 없음", src: "sequenceDiagram\nautonumber", want: "못 그림 1: 참여자가 없습니다" },
    Row { note: "읽지 못한 줄은 그리지 않음(말없이 빠지지 않게)", src: "sequenceDiagram\nA->>B: x\nclick A call()", want: "못 그림 3: 읽지 못한 줄입니다" },
    Row { note: "link 줄", src: "sequenceDiagram\nA->>B: x\nlink A: 대시보드 @ https://example.com", want: "못 그림 3: 읽지 못한 줄입니다" },
    Row { note: "alt 밖의 else", src: "sequenceDiagram\nA->>B: x\nelse 틀림", want: "못 그림 3: else 가 맞는 묶음 안에 있지 않습니다" },
    Row { note: "여는 묶음 없는 end", src: "sequenceDiagram\nA->>B: x\nend", want: "못 그림 3: 여는 묶음 없이 end 가 있습니다" },
    Row { note: "Note 대상 셋", src: "sequenceDiagram\nNote over A,B,C: x", want: "못 그림 2: Note 줄을 읽지 못했습니다" },
    Row { note: "Note 쌍점 없음", src: "sequenceDiagram\nNote over A x", want: "못 그림 2: Note 줄에 쌍점(:)이 없습니다" },
    Row { note: "참여자 이름에 꺾쇠·따옴표", src: "sequenceDiagram\nparticipant \"A\"", want: "못 그림 2: 참여자 이름을 읽지 못했습니다" },
];

// 그림 값을 견주기 쉬운 글로: "P:A,@U=사용자 | 0->>1:글 · N over 0,1:글 · [alt 라벨 · |else 라벨 · ] · #". 원본 `describe` 와 같다
fn describe(p: &SeqParse) -> String {
    match p {
        SeqParse::Unsupported { line, why } => format!("못 그림 {line}: {why}"),
        SeqParse::Ok(d) => {
            let ps = d
                .participants
                .iter()
                .map(|p| format!("{}{}{}", if p.actor { "@" } else { "" }, p.id, if p.label == p.id { String::new() } else { format!("={}", p.label) }))
                .collect::<Vec<_>>()
                .join(",");
            let items = d
                .items
                .iter()
                .map(|it| match it {
                    SeqItem::Message { from, to, text, dotted, head, both } => {
                        let tail = match head.as_str() {
                            "none" => "",
                            "arrow" => ">>",
                            "cross" => "x",
                            _ => ")",
                        };
                        format!("{from}{}{}{tail}{to}:{text}", if *both { "<" } else { "" }, if *dotted { "--" } else { "-" })
                    }
                    SeqItem::Note { at, side, text } => {
                        format!("N {} {}:{text}", side.as_str(), at.iter().map(usize::to_string).collect::<Vec<_>>().join(","))
                    }
                    SeqItem::Begin { kind, label } => format!("[{kind}{}", if label.is_empty() { String::new() } else { format!(" {label}") }),
                    SeqItem::Divide { kind, label } => format!("|{kind}{}", if label.is_empty() { String::new() } else { format!(" {label}") }),
                    SeqItem::End => "]".to_string(),
                })
                .collect::<Vec<_>>()
                .join(" · ");
            format!("P:{ps} | {items}{}{}", if d.autonumber { " | #" } else { "" }, if d.title.is_empty() { String::new() } else { format!(" | 제목 {}", d.title) })
        }
    }
}

fn read(src: &str) -> String {
    describe(&parse_sequence(src))
}

// Swift 시험 `읽기`(sequenceDiagram 의 작은 부분만 읽고 그 밖은 원문으로) — 표 23 줄 + 계산으로 만든 2 줄
#[test]
fn 읽기() {
    for r in SEQ_ROWS.iter() {
        assert_eq!(read(r.src), r.want, "{}", r.note);
    }
    // 원본: .init(note: "참여자 25명", src: "sequenceDiagram\n" + (1...25).map { "participant P\($0)" }.joined(separator: "\n"), want: "못 그림 26: 참여자가 너무 많습니다(24까지)")
    let src = format!("sequenceDiagram\n{}", (1..=25).map(|i| format!("participant P{i}")).collect::<Vec<_>>().join("\n"));
    assert_eq!(read(&src), "못 그림 26: 참여자가 너무 많습니다(24까지)", "참여자 25명");
    // 원본: .init(note: "줄 301개", src: "sequenceDiagram\n" + String(repeating: "A->>B: x\n", count: 301), want: "못 그림 302: 줄이 너무 많습니다(300줄까지 그립니다)")
    let src = format!("sequenceDiagram\n{}", "A->>B: x\n".repeat(301));
    assert_eq!(read(&src), "못 그림 302: 줄이 너무 많습니다(300줄까지 그립니다)", "줄 301개");
}

// Swift 시험 `폭`
#[test]
fn 폭() {
    assert_eq!(text_width("가나", 10.0), 20.0);
    assert_eq!(text_width("ab", 10.0), 12.0);
    assert_eq!(text_width("", 10.0), 0.0);
}

#[test]
fn 표_길이() {
    // Swift seqRows 는 25 줄: 글자 그대로 옮긴 23 줄 + 계산으로 만든 2 줄(읽기 안)
    assert_eq!(SEQ_ROWS.len(), 23);
}

// ---- 추가: Swift 시험에 없는 경계 ----

#[test]
fn 추가_줄바꿈_꼴과_키워드_대소문자() {
    assert_eq!(read("sequenceDiagram\r\nA->>B: x\r\n"), "P:A,B | 0->>1:x");
    assert_eq!(read("sequenceDiagram\rA->>B: x"), "P:A,B | 0->>1:x");
    assert_eq!(read("sequenceDiagram\nALT 갈래\nA->>B: x\nELSE 틀림\nEnd"), "P:A,B | [alt 갈래 · 0->>1:x · |else 틀림 · ]");
    assert_eq!(read("sequenceDiagram\nelse 틀림"), "못 그림 2: else 가 맞는 묶음 안에 있지 않습니다");
    assert_eq!(read("sequenceDiagram\nELSE 틀림"), "못 그림 2: ELSE 가 맞는 묶음 안에 있지 않습니다");
}

#[test]
fn 추가_참여자_다시_선언과_이름() {
    assert_eq!(read("sequenceDiagram\nparticipant A\nactor A as 에이\nA->>B: x"), "P:@A=에이,B | 0->>1:x");
    assert_eq!(read("sequenceDiagram\nparticipant A as 가 as 나\nA->>A: x"), "P:A=가 as 나 | 0->>0:x");
    assert_eq!(read("sequenceDiagram\ncreate note A"), "못 그림 2: 읽지 못한 줄입니다");
}

#[test]
fn 추가_note_대상_쪼개기() {
    // 빈 조각은 버리고 공백뿐인 조각은 남아 이름 검사에서 걸린다
    assert_eq!(read("sequenceDiagram\nNote over A,,B: x"), "P:A,B | N over 0,1:x");
    assert_eq!(read("sequenceDiagram\nNote over A, : x"), "못 그림 2: Note 줄을 읽지 못했습니다");
    assert_eq!(read("sequenceDiagram\nNote left of A,B: x"), "못 그림 2: Note 줄을 읽지 못했습니다");
    assert_eq!(read("sequenceDiagram\nNote left A: x"), "못 그림 2: Note 줄을 읽지 못했습니다");
    assert_eq!(read("sequenceDiagram\nNote under A: x"), "못 그림 2: Note 줄을 읽지 못했습니다");
}

#[test]
fn 추가_end_뒤에_글이_있으면_메시지로_읽는다() {
    assert_eq!(read("sequenceDiagram\nend A->>B: x"), "못 그림 2: 메시지 줄을 읽지 못했습니다");
    assert_eq!(read("sequenceDiagram\nend"), "못 그림 2: 여는 묶음 없이 end 가 있습니다");
}

#[test]
fn 추가_제목과_빈칸_깎기() {
    assert_eq!(read("sequenceDiagram\ntitle 제목\nA->B"), "P:A,B | 0-1: | 제목 제목");
    assert_eq!(read("sequenceDiagram\ntitle : 제목\nA->B"), "P:A,B | 0-1: | 제목 제목");
    // 전각 빈칸·NBSP 는 Foundation `.whitespaces`(Zs)라 깎인다
    assert_eq!(read("sequenceDiagram\n\u{3000}A->>B: x\u{a0}"), "P:A,B | 0->>1:x");
    let long = format!("sequenceDiagram\ntitle: {}\nA->B", "가".repeat(301));
    match parse_sequence(&long) {
        SeqParse::Ok(d) => assert_eq!(d.title.chars().count(), 300),
        other => panic!("읽지 못함: {other:?}"),
    }
}

#[test]
fn 추가_한도_경계() {
    let ok24 = format!("sequenceDiagram\n{}", (1..=24).map(|i| format!("participant P{i}")).collect::<Vec<_>>().join("\n"));
    assert!(matches!(parse_sequence(&ok24), SeqParse::Ok(d) if d.participants.len() == 24));
    let items300 = format!("sequenceDiagram\n{}", "A->>B: x\n".repeat(300));
    assert!(matches!(parse_sequence(&items300), SeqParse::Ok(d) if d.items.len() == 300));
    let label300 = format!("sequenceDiagram\nA->>B: {}", "가".repeat(300));
    assert!(matches!(parse_sequence(&label300), SeqParse::Ok(_)));
    let label301 = format!("sequenceDiagram\nA->>B: {}", "가".repeat(301));
    assert_eq!(read(&label301), "못 그림 2: 메시지 줄을 읽지 못했습니다");
    let id81 = format!("sequenceDiagram\nparticipant {}", "a".repeat(81));
    assert_eq!(read(&id81), "못 그림 2: 참여자 이름을 읽지 못했습니다");
    let id80 = format!("sequenceDiagram\nparticipant {}", "a".repeat(80));
    assert!(matches!(parse_sequence(&id80), SeqParse::Ok(_)));
}

#[test]
fn 추가_글자_폭_범위() {
    assert_eq!(text_width("a가", 10.0), 16.0);
    assert_eq!(text_width("\u{1F600}", 10.0), 10.0); // 그림 글자 1em
    assert_eq!(text_width("\u{FF21}", 10.0), 10.0); // 전각 A
    assert_eq!(text_width("\u{2014}", 10.0), 6.0); // 긴 줄표는 0.6em
    assert_eq!(text_width("a", 0.0), 0.0);
}
