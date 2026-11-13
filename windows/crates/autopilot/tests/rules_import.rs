//! 가져오기 시험(할 일 문서) — 원본 `Tests/AutopilotRulesTests/ImportTests.swift` 중 청크 a 몫.
//! 이 파일은 20검사 중 4검사(절·전체·키·경로, suite `ImportTodoTests`)를 옮긴다.
//! 나머지는 원본 순서대로: 바깥 목록 5(질의·지라항목·노션항목·요청·읽기)와 옛 자동화 3(고르기·항목·묶음)은 R-Import-b,
//! 가리기 3(가리기·바뀐본문·상태합치기)와 기계 검사 5(검사·값·대기열·줄파일·글)는 R-Import-c 가 각자의 파일에서 더한다.

use claude_board_autopilot::rules::import::{
    fnv, todo_headings, todo_items, todo_likely, todo_path_ok, ItemState,
};

const TODO_DOC: &str = "# 할 일\n\n## 이번 주\n- [ ] 로그인 실패 문구를 바꾼다\n  - 비밀번호 틀림과 계정 없음을 나눠 보인다\n- [x] README 첫 줄 고침\n- [~] 목록 쪽 나누기\n* 장바구니 합계가 0 일 때 숨긴다\n1. ~~옛 배너 지우기~~\n- 로그인 실패 문구를 바꾼다\n\n```\n- [ ] 울타리 안은 항목이 아님\n```\n\n## 다음 주\n- [ ] 결제 내역 내보내기 (완료)\n- [ ] 알림 설정 쪽";

/// 첫 절만 걷기 — 원본 `ImportTests.swift` 의 `절`(test절)
#[test]
fn 절() {
    let items = todo_items(TODO_DOC, "docs/TODO.md", Some("이번 주"));
    assert_eq!(items.len(), 6);
    assert_eq!(
        items.iter().map(|it| it.title.as_str()).collect::<Vec<_>>(),
        vec![
            "로그인 실패 문구를 바꾼다",
            "README 첫 줄 고침",
            "목록 쪽 나누기",
            "장바구니 합계가 0 일 때 숨긴다",
            "옛 배너 지우기",
            "로그인 실패 문구를 바꾼다",
        ]
    );
    assert_eq!(
        items.iter().map(|it| it.state).collect::<Vec<_>>(),
        vec![ItemState::Open, ItemState::Done, ItemState::Progress, ItemState::Open, ItemState::Done, ItemState::Open]
    );
    assert_eq!(items[0].body, "- 비밀번호 틀림과 계정 없음을 나눠 보인다");
    assert_eq!(items[0].key, "todo:docs/TODO.md#로그인-실패-문구를-바꾼다");
    assert_eq!(items[5].key, "todo:docs/TODO.md#로그인-실패-문구를-바꾼다~2");
    assert_eq!(items[0].place, "docs/TODO.md:4");
    assert_eq!(items[4].state_note, "취소선");
    for it in &items {
        assert!(!it.title.contains("울타리"), "울타리 안은 항목이 아니다: {}", it.title);
    }
}

/// 문서 전체(절 지정 없음) — 원본 `ImportTests.swift` 의 `전체`(test전체)
#[test]
fn 전체() {
    let items = todo_items(TODO_DOC, "docs/TODO.md", None);
    assert_eq!(items.len(), 8);
    assert_eq!(
        todo_headings(TODO_DOC)
            .iter()
            .map(|h| h.title.clone())
            .collect::<Vec<_>>(),
        vec!["할 일", "이번 주", "다음 주"]
    );
    let 결제 = items.iter().find(|it| it.title.starts_with("결제 내역")).expect("결제 내역 항목");
    assert_eq!(결제.state, ItemState::Done);
}

/// 같은 항목을 두 문서에서 걷어도 키가 같고, 본문이 바뀌면 지문만 다르다 — 원본 `ImportTests.swift` 의 `키`(test키)
#[test]
fn 키() {
    let a = todo_items(TODO_DOC, "docs/TODO.md", Some("이번 주"));
    let b = todo_items(
        &format!("\n\n{}", TODO_DOC.replace("나눠 보인다", "나눠 보이고 도움말 고리를 단다")),
        "docs/TODO.md",
        Some("이번 주"),
    );
    assert_eq!(a[0].key, b[0].key);
    assert_ne!(a[0].place, b[0].place);
    assert_ne!(a[0].digest(), b[0].digest());
    assert_eq!(a[1].digest(), b[1].digest());
    assert_eq!(fnv("abc"), "e71fa2190541574b");
}

/// 파일 고르기 — 원본 `ImportTests.swift` 의 `경로`(test경로)
#[test]
fn 경로() {
    for p in ["docs/TODO.md", "plan.txt"] {
        assert!(todo_path_ok(p), "{p} 는 고를 수 있다");
    }
    for p in ["../TODO.md", "/etc/TODO.md", ".github/TODO.md", "app/main.swift"] {
        assert!(!todo_path_ok(p), "{p} 는 고를 수 없다");
    }
    assert!(todo_likely("docs/TODO.md"));
    assert!(!todo_likely("docs/guide.md"));
}

