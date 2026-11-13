//! 자율주행 읽기(4단계 ⑩-2): 링크 → 읽을 대상·그림 호스트 시험.
//! 원본: `Tests/AutopilotRulesTests/ReadingTests.swift` 의 `TargetTests`(`@Suite("자율주행 읽기: 링크 → 읽을 대상·그림 호스트")` — MARK 링크 → 읽을 대상).
//! 옮긴 시험(같은 이름): `대상`(표 16줄) · `그림`(표 12줄) · `줄`. 표는 Swift 시험 파일에서 기계가 옮긴 것이다(글자 그대로).
//! 나머지 ReadLinkTests(읽기 유스케이스, MARK 읽기 유스케이스(가짜 HTTP) 362줄~)는 R-Reading-b 청크와 같이 옮긴다.
//! 끝의 `extra_` 시험은 Swift 시험에 없는 것(경계)이다.
//! 시각은 원본의 서일(t0 = 1793340000)을 그대로 쓰지만 시험 시각이 서울·UTC 어느 쪽 같은 날짜라 시간대는 `Tz::Utc` 로 둔다(connections 시험과 같은 이유).

use claude_board_autopilot::rules::connections::{Connection, Service};
use claude_board_autopilot::rules::reading::{
    confluence_read_request, figma_image_request, figma_read_request, image_url_ok, is_png, jira_issue_request, link_state, notion_children_request,
    notion_id, notion_page_get_request, read_target, ReadTarget, JIRA_READ_FIELDS,
};
use claude_board_autopilot::rules::stopline::Tz;
use std::collections::HashSet;

const T0: f64 = 1_793_340_000.0;
const SITE: &str = "https://example.atlassian.net";

fn atl_read() -> Connection {
    Connection {
        id: "atlassian".into(),
        service: Service::Atlassian,
        site: SITE.into(),
        account: "kim@example.com".into(),
        scopes: vec!["jira".into(), "read".into()],
        expires: "2027-01-31".into(),
        ..Connection::default()
    }
}

fn notion_read() -> Connection {
    Connection { id: "notion".into(), service: Service::Notion, account: "김요청".into(), scopes: vec!["read".into()], ..Connection::default() }
}

fn figma_read() -> Connection {
    Connection { id: "figma".into(), service: Service::Figma, account: "kim@example.com".into(), scopes: vec!["read".into()], ..Connection::default() }
}

/// 원본 `targetRows`(ReadingTests.swift 298~315) — (참고, 주소, atlassian_site, 기대). site 는 None 이면 연결한 사이트 없이 판정한다
#[test]
fn 대상() {
    let site = Some(SITE);
    let local = Some("http://127.0.0.1:8123");
    let rows: [(&str, &str, Option<&str>, ReadTarget); 16] = [
        ("Jira 이슈", "https://example.atlassian.net/browse/TODO-41", site, ReadTarget::Jira { key: "TODO-41".into() }),
        (
            "Jira 보드의 고른 이슈",
            "https://example.atlassian.net/jira/software/projects/TODO/boards/1?selectedIssue=TODO-7",
            site,
            ReadTarget::Jira { key: "TODO-7".into() },
        ),
        ("Confluence 페이지", "https://example.atlassian.net/wiki/spaces/OPS/pages/48213/정책", site, ReadTarget::Confluence { page: "48213".into() }),
        ("Confluence pageId", "https://example.atlassian.net/wiki/pages/viewpage.action?pageId=777", site, ReadTarget::Confluence { page: "777".into() }),
        (
            "Confluence 짧은 주소는 아직",
            "https://example.atlassian.net/wiki/x/AbCd",
            site,
            ReadTarget::Bad("Confluence 페이지 주소에서 페이지 번호를 찾지 못했습니다(…/pages/<번호>/… 꼴로 붙여 넣습니다. 짧은 주소 /wiki/x/… 는 아직 읽지 않습니다)".into()),
        ),
        ("시험 빌드 사이트(127.0.0.1)", "http://127.0.0.1:8123/wiki/spaces/OPS/pages/9", local, ReadTarget::Confluence { page: "9".into() }),
        ("다른 포트의 127.0.0.1 은 그냥 웹", "http://127.0.0.1:9999/wiki/spaces/OPS/pages/9", local, ReadTarget::Web),
        (
            "Notion 페이지",
            "https://www.notion.so/team/회의실-예약-0123456789abcdef0123456789abcdef",
            site,
            ReadTarget::Notion { page: "01234567-89ab-cdef-0123-456789abcdef".into() },
        ),
        (
            "Notion ?p=",
            "https://www.notion.so/team/db?p=0123456789ABCDEF0123456789ABCDEF",
            site,
            ReadTarget::Notion { page: "01234567-89ab-cdef-0123-456789abcdef".into() },
        ),
        ("Notion 사이트", "https://x.notion.site/Page-0123456789abcdef0123456789abcdef", site, ReadTarget::Notion { page: "01234567-89ab-cdef-0123-456789abcdef".into() }),
        (
            "Figma 프레임",
            "https://www.figma.com/design/AbCdEfGhIjKlMn/회의실?node-id=12-4&t=x",
            site,
            ReadTarget::Figma { file: "AbCdEfGhIjKlMn".into(), node: Some("12:4".into()) },
        ),
        ("Figma 가지", "https://www.figma.com/design/AbCdEfGhIjKlMn/branch/ZyXwVuTsRqPo/x", site, ReadTarget::Figma { file: "ZyXwVuTsRqPo".into(), node: None }),
        ("Figma 파일(프레임 없음)", "https://figma.com/file/AbCdEfGhIjKlMn/x", site, ReadTarget::Figma { file: "AbCdEfGhIjKlMn".into(), node: None }),
        ("그 밖 웹", "https://wiki.example.com/page/1", site, ReadTarget::Web),
        ("아이디·암호 든 주소", "https://u:p@example.atlassian.net/browse/A-1", site, ReadTarget::Bad("사용자 이름·비밀번호가 든 주소는 받지 않습니다".into())),
        ("꼴 아님", "javascript:alert(1)", site, ReadTarget::Bad("웹 주소 꼴이 아닙니다(https:// 로 시작하는 주소를 붙여 넣습니다)".into())),
    ];
    for (note, url, atlassian_site, want) in rows {
        assert_eq!(read_target(url, atlassian_site), want, "{note}");
    }
}

/// 원본 `imageRows`(ReadingTests.swift 318~331) — (참고, 주소, 시험 빌드, 기대)
#[test]
fn 그림() {
    for (note, url, dev, ok) in [
        ("Figma 내보내기 저장소", "https://figma-alpha-api.s3.us-west-2.amazonaws.com/images/abc", false, true),
        ("*.figma.com", "https://s3-alpha-sig.figma.com/img/abc?Expires=1&Signature=x", false, true),
        ("figma.com 그 자체는 아님", "https://figma.com/img/abc", false, false),
        ("꼬리만 같은 다른 호스트", "https://evilfigma.com/img", false, false),
        ("figma.com 을 앞에 둔 다른 호스트", "https://figma.com.evil.example/img", false, false),
        ("다른 S3 버킷", "https://evil.s3.us-west-2.amazonaws.com/images/abc", false, false),
        ("http 는 아님", "http://figma-alpha-api.s3.us-west-2.amazonaws.com/images/abc", false, false),
        ("다른 포트", "https://figma-alpha-api.s3.us-west-2.amazonaws.com:8443/images/abc", false, false),
        ("아이디 든 주소", "https://u@figma-alpha-api.s3.us-west-2.amazonaws.com/images/abc", false, false),
        ("설치판 127.0.0.1 은 아님", "http://127.0.0.1:9/img/a.png", false, false),
        ("시험 빌드 127.0.0.1(다른 포트도)", "http://127.0.0.1:9/img/a.png", true, true),
        ("시험 빌드라도 localhost 는 아님", "http://localhost:9/img/a.png", true, false),
    ] {
        assert_eq!(image_url_ok(url, dev), ok, "{note}");
    }
}

/// 원본 `줄`(ReadingTests.swift 341~359) — 자료 링크 줄의 상태(종류와 상태만 비교한다)
#[test]
fn 줄() {
    let have: HashSet<String> = ["atlassian", "figma"].iter().map(|s| s.to_string()).collect();
    let all = [atl_read(), notion_read(), figma_read()];
    let st = |u: &str, conns: &[Connection], mcp: Option<&str>| -> String {
        let l = link_state(u, conns, &have, mcp, false, T0, Tz::Utc);
        format!("{} {}", l.kind, l.state)
    };
    assert_eq!(st("https://example.atlassian.net/wiki/spaces/OPS/pages/1", &all, None), "Confluence connected");
    assert_eq!(st("https://other.atlassian.net/wiki/spaces/OPS/pages/1", &all, None), "Confluence needs_connection");
    assert_eq!(st("https://www.figma.com/design/AbCdEfGhIjKlMn/x?node-id=1-2", &all, None), "Figma connected");
    // 토큰 없음
    assert_eq!(st("https://www.notion.so/x-0123456789abcdef0123456789abcdef", &all, None), "Notion needs_connection");
    assert_eq!(st("https://www.figma.com/design/AbCdEfGhIjKlMn/x", &[atl_read()], None), "Figma needs_connection");
    let no_read = Connection { scopes: vec!["jira".into()], ..atl_read() };
    assert_eq!(st("https://example.atlassian.net/browse/A-1", &[no_read], None), "Jira needs_connection");
    assert_eq!(st("https://wiki.example.com/p", &all, None), "웹 needs_connection");
    assert_eq!(st("https://wiki.example.com/p", &all, Some("wiki")), "웹 mcp");
    assert_eq!(st("https://wiki.example.com/p", &all, Some("bad name")), "웹 needs_connection");
    assert_eq!(st("ftp://x", &all, None), "웹 unreadable");
}

// ── Swift 시험에 없는 것(경계) ───────────────────────

/// 노션 아이디: 16진수 글자만 골라 정확히 32자일 때(원본 filter(isHexDigit) 그대로 — 16진수 아닌 글자는 버려진다), 대소문자 섞여도 같은 답
#[test]
fn extra_노션_아이디_경계() {
    let full = "0123456789ABCDEF0123456789abcdef";
    assert_eq!(
        notion_id(full),
        Some("01234567-89ab-cdef-0123-456789abcdef".to_string())
    );
    assert_eq!(notion_id(""), None);
    assert_eq!(notion_id(&full[..31]), None); // 한 글자 모자람
    // 16진수가 한 글자 더 끼면 33자라 틀린다
    assert_eq!(notion_id(&format!("{}f", "a".repeat(32))), None);
    // 16진수 아닌 글자는 버려진다 — 32자 + 꼬리 'X' 는 그래도 맞다(원본과 같다)
    assert_eq!(notion_id(&format!("{full}X")), Some("01234567-89ab-cdef-0123-456789abcdef".to_string()));
    assert_eq!(notion_id("0123456789ABCDEF0123456789abcde g"), None); // 16진수 아닌 글자 버려지면 31자뿐
}

/// 그림 주소의 꼴 검문: 빈 글자·빈칸·옳지 않은 글자, 그리고 4000자(모양수) 경계
#[test]
fn extra_그림_주소_꼴() {
    let base = "https://figma-alpha-api.s3.us-west-2.amazonaws.com/";
    assert!(!image_url_ok("", false)); // 빈 주소
    assert!(!image_url_ok("https://figma-alpha-api.s3.us-west-2.amazonaws.com/a x", false)); // 빈칸
    assert!(!image_url_ok("https://figma-alpha-api.s3.us-west-2.amazonaws.com/a\"x", false)); // 따옴표
    assert!(image_url_ok(&format!("{base}{}", "a".repeat(4000 - base.len())), false)); // 모양수 4000 — 정확히 참
    assert!(!image_url_ok(&format!("{base}{}", "a".repeat(4001 - base.len())), false)); // 한 글자 넘침 — 거짓
}

/// 읽기 요청 꼴을 문자 그대로 본다(GET, 머리 Accept[·Notion-Version], 인증 꼴)
#[test]
fn extra_요청_꼴() {
    let base = "https://example.atlassian.net";
    let basic = claude_board_autopilot::rules::connections::Auth::Basic { user: "kim@example.com".into() };
    let jira = jira_issue_request("TODO-41", base, basic.clone(), JIRA_READ_FIELDS);
    assert_eq!(
        jira.url,
        format!(
            "{base}/rest/api/3/issue/TODO-41?fields=summary,status,issuetype,assignee,reporter,priority,labels,updated,created,description,attachment,issuelinks,parent,fixVersions,components"
        )
    );
    assert_eq!(jira.method, "GET");
    assert_eq!(jira.headers.get("Accept").map(String::as_str), Some("application/json"));
    assert_eq!(jira.auth, basic);
    // 주소 한 칸 퍼센트 더하기(원본 q — 모두 대문자 두 자리)
    assert_eq!(jira_issue_request("A B-1", base, basic.clone(), JIRA_READ_FIELDS).url, format!("{base}/rest/api/3/issue/A%20B-1?fields=summary,status,issuetype,assignee,reporter,priority,labels,updated,created,description,attachment,issuelinks,parent,fixVersions,components"));
    assert_eq!(
        confluence_read_request("48213", base, basic).url,
        format!("{base}/wiki/rest/api/content/48213?expand=body.storage,version,space,history,children.attachment,metadata.labels")
    );
    let notion_base = "https://api.notion.com";
    let page = notion_page_get_request("01234567-89ab-cdef-0123-456789abcdef", notion_base);
    assert_eq!(page.url, format!("{notion_base}/v1/pages/01234567-89ab-cdef-0123-456789abcdef"));
    assert_eq!(claude_board_autopilot::rules::connections::Auth::Bearer, page.auth);
    assert_eq!(page.headers.get("Notion-Version").map(String::as_str), Some("2022-06-28"));
    assert_eq!(
        notion_children_request("01234567-89ab-cdef-0123-456789abcdef", notion_base, None).url,
        format!("{notion_base}/v1/blocks/01234567-89ab-cdef-0123-456789abcdef/children?page_size=100")
    );
    assert_eq!(
        notion_children_request("b", notion_base, Some("c 2")).url,
        format!("{notion_base}/v1/blocks/b/children?page_size=100&start_cursor=c%202")
    );
    let figma_base = "https://api.figma.com";
    assert_eq!(figma_read_request("AbCdEfGhIjKlMn", None, figma_base).url, format!("{figma_base}/v1/files/AbCdEfGhIjKlMn?depth=2"));
    // API 꼴(콜론)은 퍼센트 더한다
    assert_eq!(figma_read_request("AbCdEfGhIjKlMn", Some("12:4"), figma_base).url, format!("{figma_base}/v1/files/AbCdEfGhIjKlMn/nodes?ids=12%3A4"));
    assert_eq!(
        figma_image_request("AbCdEfGhIjKlMn", "12:4", figma_base).url,
        format!("{figma_base}/v1/images/AbCdEfGhIjKlMn?ids=12%3A4&format=png&scale=1")
    );
}

/// MCP 서버 이름은 64자까지만(문자 그대로 비교 — 64 는 MCP, 65 는 연결 필요)
#[test]
fn extra_mcp_이름_경계() {
    let web = "https://wiki.example.com/p";
    let st = |mcp: Option<&str>| -> String {
        let l = link_state(web, &[], &HashSet::new(), mcp, false, T0, Tz::Utc);
        format!("{} {}", l.kind, l.state)
    };
    assert_eq!(st(Some(&"a".repeat(64))), "웹 mcp");
    assert_eq!(st(Some(&"a".repeat(65))), "웹 needs_connection");
    assert_eq!(st(Some("W-ki_1")), "웹 mcp"); // 마침표·빈칸·점은 안 된다
    assert_eq!(st(Some("w.ki")), "웹 needs_connection");
}

/// PNG 서명: 8바이트만은 아니고(원본 d.count > 8), 9바째가 맞으면 참, 틀리면 거짓
#[test]
fn extra_png_서명() {
    let sig: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    assert!(!is_png(&sig));
    assert!(is_png(&[sig.as_slice(), &[0]].concat()));
    assert!(!is_png(&[0x89, 0x50, 0x4E, 0x48, 0, 0, 0, 0, 0]));
}
