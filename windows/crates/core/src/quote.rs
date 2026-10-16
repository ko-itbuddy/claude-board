//! 명령줄 인용 — `resume_cmd` 문자열을 만들 때 쓴다 (SPEC §2.1 `resume_cmd`, §3.3).
//!
//! - Windows: `CommandLineToArgvW` 가 원래 인자로 되돌리는 규칙. 공백·특수문자가 있으면 큰따옴표로 감싸고 `"` 는 `\"`.
//! - 그 밖: Python `shlex.quote`/`shlex.join` 과 같다(macOS `state.py` 와 같은 결과).

use std::borrow::Cow;

/// `shlex.quote` — `[\w@%+=:,./-]`(ASCII) 밖의 글자가 있으면 작은따옴표로 감싼다.
pub fn posix(arg: &str) -> Cow<'_, str> {
    if arg.is_empty() {
        return Cow::Borrowed("''");
    }
    let safe = arg.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if safe {
        Cow::Borrowed(arg)
    } else {
        Cow::Owned(format!("'{}'", arg.replace('\'', "'\"'\"'")))
    }
}

/// Windows 인자 하나. 빈 인자·공백·`"`·cmd 특수문자(`&|<>^()%!`)가 있으면 큰따옴표로 감싼다.
/// 감쌀 때 `"` 앞과 끝의 `\` 는 두 배로 늘려 `CommandLineToArgvW` 가 원래 값으로 되돌리게 한다.
pub fn windows(arg: &str) -> Cow<'_, str> {
    let needs = arg.is_empty() || arg.chars().any(|c| matches!(c, ' ' | '\t' | '\n' | '\x0b' | '"' | '&' | '|' | '<' | '>' | '^' | '(' | ')' | '%' | '!'));
    if !needs {
        return Cow::Borrowed(arg);
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut slashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', slashes));
                out.push(c);
                slashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    Cow::Owned(out)
}

/// `shlex.join`
pub fn join_posix<S: AsRef<str>>(args: &[S]) -> String {
    args.iter().map(|a| posix(a.as_ref())).collect::<Vec<_>>().join(" ")
}

/// Windows 명령줄로 잇는다.
pub fn join_windows<S: AsRef<str>>(args: &[S]) -> String {
    args.iter().map(|a| windows(a.as_ref())).collect::<Vec<_>>().join(" ")
}

/// 이 플랫폼의 규칙으로 잇는다 — Windows 빌드는 Windows 규칙, 그 밖은 shlex.
pub fn join<S: AsRef<str>>(args: &[S]) -> String {
    if cfg!(windows) {
        join_windows(args)
    } else {
        join_posix(args)
    }
}

/// `CommandLineToArgvW` 규칙으로 명령줄을 나눈다(인용 시험용 되돌리기, argv[0] 특례는 무시).
#[cfg(test)]
fn split_windows(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let (mut out, mut cur, mut i, mut quoted, mut has) = (Vec::new(), String::new(), 0, false, false);
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let start = i;
            while i < chars.len() && chars[i] == '\\' {
                i += 1;
            }
            let n = i - start;
            if i < chars.len() && chars[i] == '"' {
                cur.extend(std::iter::repeat_n('\\', n / 2));
                if n % 2 == 1 {
                    cur.push('"');
                    i += 1;
                }
            } else {
                cur.extend(std::iter::repeat_n('\\', n));
            }
            has = true;
            continue;
        }
        if c == '"' {
            if quoted && chars.get(i + 1) == Some(&'"') {
                cur.push('"');
                i += 2;
                continue;
            }
            quoted = !quoted;
            has = true;
        } else if (c == ' ' || c == '\t') && !quoted {
            if has {
                out.push(std::mem::take(&mut cur));
                has = false;
            }
        } else {
            cur.push(c);
            has = true;
        }
        i += 1;
    }
    if has {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_like_shlex() {
        // 기대값은 python3 shlex.quote 로 만든 것
        assert_eq!(posix(""), "''");
        assert_eq!(posix("claude-opus-5-5"), "claude-opus-5-5");
        assert_eq!(posix("--model=a:b,c/d.e@f%g+h"), "--model=a:b,c/d.e@f%g+h");
        assert_eq!(posix("a b"), "'a b'");
        assert_eq!(posix("it's"), "'it'\"'\"'s'");
        assert_eq!(posix("한글"), "'한글'");
        assert_eq!(posix("{\"a\":1}"), "'{\"a\":1}'");
        assert_eq!(join_posix(&["claude", "--resume", "x y"]), "claude --resume 'x y'");
    }

    #[test]
    fn windows_quoting() {
        assert_eq!(windows("claude-opus-5-5"), "claude-opus-5-5");
        assert_eq!(windows(r"C:\dir\file"), r"C:\dir\file");
        assert_eq!(windows(""), "\"\"");
        assert_eq!(windows("a b"), "\"a b\"");
        assert_eq!(windows("{\"a\":1}"), r#""{\"a\":1}""#);
        assert_eq!(windows(r"C:\my dir\"), r#""C:\my dir\\""#);
        assert_eq!(windows(r#"a\"b"#), r#""a\\\"b""#);
        assert_eq!(windows("x&y"), "\"x&y\"");
        assert_eq!(join_windows(&["claude", "--model", "claude-opus-5-5", "--resume", "id"]), "claude --model claude-opus-5-5 --resume id");
    }

    #[test]
    fn windows_round_trip() {
        let cases = [
            "plain",
            "",
            "a b",
            r"C:\Program Files\x\",
            r#"{"hooks":{"a":"b c"}}"#,
            r#"tail\\"#,
            r#"q"uote"#,
            r#"\\server\share dir\"#,
            "탭\t과 한글",
        ];
        let line = join_windows(&cases);
        assert_eq!(split_windows(&line), cases.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }
}
