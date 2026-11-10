//! (청크 Stack 1/2 — 전반 시험만) 기술 스택 칸 규칙(`rules::stack`) 시험. 원본: `Tests/AutopilotRulesTests/TopicTests.swift`
//! (Stack.swift 를 쓰는 시험 파일은 이것뿐이다. `CritiqueTests`·`DraftTests` 는 `APStackSet()` 을 입력으로 만들 뿐이다).
//! TopicTests 의 시험 중 이 파일이 옮긴 것은 `DetectTests.표`(표 10 줄 → `detect_표`)다.
//! 남은 것(R-Stack-b): `TopicWordTests.표`·`낱말`(`apTopicStack`·`apTopicHits`), `DetectTests.읽는자리`(`apDetectStack(root:files:)`·가짜 `APStackFiles`),
//! `ConflictTests.표`(`apStackConflicts`). 같은 스위트의 NewRepo·Link 시험은 다른 원본(NewRepo.swift·Links.swift)의 몫이다.
//! 표와 조각 글(gradleSpring·ktsSpring·pomSpring)은 Swift 에서 변환 스크립트로 옮겼다(손으로 다시 치지 않았다):
//! `#"…"#` → `r#"…"#`, 여러 줄 `"""` → `r#"…"#`. 끝의 `extra_` 시험은 Swift 시험에 없는 것(정규식을 손으로 옮긴 자리의 경계)이다.

use claude_board_autopilot::rules::stack::{detect_stack, stack_candidates, StackSet, StackSlot, STACK_DIRS, STACK_FILE_NAMES};
use std::collections::HashMap;

struct DetectRow {
    note: &'static str,
    files: &'static [(&'static str, &'static str)],
    want: &'static str,
    build: &'static str,
    sources: &'static [&'static str],
}

// 원본 `show` — 칸마다 "키 판"(키가 없으면 이름), 없으면 -, 언어 판은 back 뒤 "/언어 판"
fn show(s: &StackSet) -> String {
    fn one(x: &Option<StackSlot>) -> String {
        match x {
            Some(x) => format!("{} {}", if x.key.is_empty() { &x.name } else { &x.key }, x.framework_version).trim_matches([' ', '\t']).to_string(),
            None => "-".to_string(),
        }
    }
    let lang = match &s.back {
        Some(b) if !b.lang_version.is_empty() => format!("/{} {}", b.lang, b.lang_version),
        _ => String::new(),
    };
    format!("{}|{}{}|{}", one(&s.front), one(&s.back), lang, one(&s.db))
}

fn map(files: &[(&str, &str)]) -> HashMap<String, String> {
    files.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

// 아래 상수·표는 Swift 시험 파일에서 기계로 옮긴 것이다: 글자 값은 그대로 복사했다(Swift 의 \n·\" 는 Rust 에서도 같은 뜻이다).
const GRADLE_SPRING: &str = r#"plugins {
    id 'java'
    id 'org.springframework.boot' version '4.0.8'
    id 'io.spring.dependency-management' version '1.1.7'
}
java { toolchain { languageVersion = JavaLanguageVersion.of(21) } }"#;
const KTS_SPRING: &str = r#"plugins {
    kotlin("jvm") version "2.4.0"
    kotlin("plugin.spring") version "2.4.0"
    id("org.springframework.boot") version "4.1.1"
}
kotlin { jvmToolchain(25) }"#;
const POM_SPRING: &str = r#"<project><parent><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-parent</artifactId>
<version>3.5.2</version></parent><properties><java.version>17</java.version></properties></project>"#;

const DETECT_ROWS: [DetectRow; 10] = [
    DetectRow { note: "아무 파일도 없음 → 감지된 것 없음", files: &[], want: "-|-|-", build: "", sources: &[] },
    DetectRow { note: "Gradle Groovy Spring Boot + application.yml 의 postgresql + compose 이미지 판", files: &[("build.gradle", GRADLE_SPRING), ("src/main/resources/application.yml", "spring:\n  datasource:\n    url: jdbc:postgresql://localhost:5432/todo\n"), ("docker-compose.yml", "services:\n  db:\n    image: postgres:17\n")], want: "-|spring-java 4.0.8/Java 21|postgresql 17", build: "Gradle Groovy", sources: &["build.gradle", "src/main/resources/application.yml"] },
    DetectRow { note: "Gradle Kotlin(kts) + Kotlin 플러그인 → Kotlin · Spring Boot", files: &[("build.gradle.kts", KTS_SPRING)], want: "-|spring-kotlin 4.1.1/Kotlin 25|-", build: "Gradle Kotlin", sources: &["build.gradle.kts"] },
    DetectRow { note: "Maven 부모 판과 java.version", files: &[("pom.xml", POM_SPRING)], want: "-|spring-java 3.5.2/Java 17|-", build: "Maven", sources: &["pom.xml"] },
    DetectRow { note: "NestJS(engines.node) + pg", files: &[("package.json", r#"{"dependencies":{"@nestjs/core":"^11.1.0","pg":"^8"},"engines":{"node":">=22"}}"#)], want: "-|nest 11.1.0/Node.js 22|postgresql", build: "npm", sources: &["package.json"] },
    DetectRow { note: "뿌리 package.json 에 React+Vite, 백엔드 폴더에 FastAPI(pyproject requires-python·psycopg)", files: &[("package.json", r#"{"dependencies":{"react":"^19.3.0"},"devDependencies":{"vite":"^8.3.0"}}"#), ("backend/pyproject.toml", "[project]\nrequires-python = \">=3.13\"\ndependencies = [\"fastapi[standard]>=0.160.0\", \"psycopg[binary]\"]\n")], want: "react 19.3.0|fastapi 0.160.0/Python 3.13|postgresql", build: "pip-or-pyproject", sources: &[] },
    DetectRow { note: "frontend 폴더의 Next.js 와 backend 폴더의 Spring", files: &[("frontend/package.json", r#"{"dependencies":{"next":"16.4.1","react":"19.3.1"}}"#), ("backend/build.gradle", GRADLE_SPRING)], want: "next 16.4.1|spring-java 4.0.8/Java 21|-", build: "Gradle Groovy", sources: &["backend/build.gradle"] },
    DetectRow { note: "prisma provider mysql", files: &[("package.json", r#"{"dependencies":{"express":"^5.1.0"}}"#), ("prisma/schema.prisma", "datasource db {\n  provider = \"mysql\"\n}\n")], want: "-|Node.js · Express 5.1.0|mysql", build: "npm", sources: &["package.json"] },
    DetectRow { note: "Go 모듈과 Gin", files: &[("go.mod", "module example.com/todo\n\ngo 1.24\n\nrequire github.com/gin-gonic/gin v1.10.0\n")], want: "-|Go · Gin/Go 1.24|-", build: "go", sources: &["go.mod"] },
    DetectRow { note: "react-native 는 웹 프론트로 보지 않는다", files: &[("package.json", r#"{"dependencies":{"react":"19.0.0","react-native":"0.80.0"}}"#)], want: "-|-|-", build: "", sources: &[] },
];

#[test]
fn detect_표() {
    for r in DETECT_ROWS.iter() {
        let s = detect_stack(&map(r.files));
        assert_eq!(show(&s), r.want, "{}", r.note);
        if !r.build.is_empty() && r.build != "pip-or-pyproject" {
            assert_eq!(s.back.as_ref().map(|b| b.build.as_str()), Some(r.build), "{}", r.note);
        }
        let found: Vec<String> = s.back.iter().chain(s.db.iter()).flat_map(|x| x.sources.clone()).collect();
        if !r.sources.is_empty() {
            let head: Vec<&str> = found.iter().take(r.sources.len()).map(String::as_str).collect();
            assert_eq!(head, r.sources, "{}", r.note);
        }
        for slot in [&s.front, &s.back, &s.db].into_iter().flatten() {
            assert_eq!(slot.origin, "detected", "{}", r.note);
        }
    }
}

// ---------- 아래는 Swift 시험에 없는 시험 ----------

fn detect(files: &[(&str, &str)]) -> StackSet {
    detect_stack(&map(files))
}

fn back(files: &[(&str, &str)]) -> StackSlot {
    detect(files).back.expect("back")
}

#[test]
fn extra_candidates_are_dirs_times_names_in_order() {
    let c = stack_candidates();
    assert_eq!(c.len(), STACK_DIRS.len() * STACK_FILE_NAMES.len());
    assert_eq!(c.len(), 120);
    assert_eq!(c[0], "build.gradle");
    assert_eq!(c[14], "prisma/schema.prisma");
    assert_eq!(c[15], "backend/build.gradle");
    assert_eq!(c[119], "app/prisma/schema.prisma");
    assert!(c.iter().all(|p| !p.contains("..") && !p.starts_with('/')));
}

#[test]
fn extra_slot_json_is_camel_case_and_set_omits_empty_slots() {
    assert_eq!(serde_json::to_string(&StackSet::default()).unwrap(), "{}");
    assert!(StackSet::default().is_empty());
    let s = detect(&[("build.gradle", GRADLE_SPRING)]);
    assert!(!s.is_empty());
    let v = serde_json::to_value(&s).unwrap();
    assert!(v.get("front").is_none() && v.get("db").is_none());
    assert_eq!(v["back"]["langVersion"], "21");
    assert_eq!(v["back"]["frameworkVersion"], "4.0.8");
    assert_eq!(v["back"]["sources"][0], "build.gradle");
    let back: StackSet = serde_json::from_value(v).unwrap();
    assert_eq!(back, s);
    // 키가 모자란 칸은 읽지 않는다(Swift 합성 Decodable 과 같다)
    assert!(serde_json::from_str::<StackSlot>(r#"{"key":"x","name":"y"}"#).is_err());
}

#[test]
fn extra_gradle_java_version_sources_in_order() {
    let g = |extra: &str| format!("id 'org.springframework.boot' version '3.2.0'\n{extra}");
    let java = |extra: &str| back(&[("build.gradle", &g(extra))]).lang_version;
    assert_eq!(java("java { toolchain { languageVersion = JavaLanguageVersion.of( 17 ) } }"), "17");
    assert_eq!(java("sourceCompatibility = JavaVersion.VERSION_17"), "17");
    assert_eq!(java("sourceCompatibility = JavaVersion.VERSION_1_8"), "1.8");
    assert_eq!(java("sourceCompatibility = '11'"), "11");
    assert_eq!(java("sourceCompatibility=1.8"), "1.8");
    assert_eq!(java("kotlin { jvmToolchain(21) }"), "21");
    assert_eq!(java("// nothing"), "");
    // 앞 것이 이긴다: JavaLanguageVersion > JavaVersion > sourceCompatibility > jvmToolchain
    assert_eq!(java("jvmToolchain(8)\nsourceCompatibility = '11'\nJavaVersion.VERSION_17\nJavaLanguageVersion.of(21)"), "21");
    assert_eq!(java("jvmToolchain(8)\nsourceCompatibility = '11'\nJavaVersion.VERSION_17"), "17");
    assert_eq!(java("jvmToolchain(8)\nsourceCompatibility = '11'"), "11");
}

#[test]
fn extra_gradle_boot_version_forms_and_kotlin_marks() {
    let boot = |g: &str| back(&[("build.gradle", g)]).framework_version;
    assert_eq!(boot("id 'org.springframework.boot' version '3.2.0'"), "3.2.0");
    assert_eq!(boot("id \"org.springframework.boot\" version \"3.5.2-SNAPSHOT\""), "3.5.2");
    assert_eq!(boot("id(\"org.springframework.boot\") version \"4.1.1\""), "4.1.1");
    // 감지는 소문자 글(contains)이 있어야 하고, 판을 찾는 정규식은 대소문자를 가리지 않는다
    assert_eq!(boot("// org.springframework.boot\nid 'ORG.SPRINGFRAMEWORK.BOOT' VERSION '3.0'"), "3.0");
    assert_eq!(boot("id 'org.springframework.boot'\n  version\n'3.3.1'"), "3.3.1");
    assert_eq!(boot("id 'org.springframework.boot' version '3.2.0"), "");
    assert_eq!(boot("id 'org.springframework.boot' version 'x3.2.0'"), "");
    assert_eq!(boot("id 'org.springframework.boot'"), "");
    // 닫는 따옴표 없는 첫 번째는 건너뛰고 다음 자리를 본다
    assert_eq!(boot("org.springframework.boot version\n'3.1' org.springframework.boot' version '3.9.0'"), "3.1");
    let kotlin = |g: &str| back(&[("build.gradle", g)]).key;
    assert_eq!(kotlin("id 'org.springframework.boot' version '3.2.0'\nid 'org.jetbrains.kotlin.jvm' version '2.0'"), "spring-kotlin");
    assert_eq!(kotlin("id 'org.springframework.boot'\nkotlin(\"jvm\")"), "spring-kotlin");
    assert_eq!(kotlin("id 'org.springframework.boot'\nkotlin(\"plugin.spring\")"), "spring-kotlin");
    assert_eq!(kotlin("id 'org.springframework.boot'\nKOTLIN(\"jvm\")"), "spring-java");
    // build.gradle.kts 가 있으나 스프링이 없으면 build.gradle 로 넘어가지 않는다(원본 `??` 의 모양)
    let s = detect(&[("build.gradle.kts", "plugins { }"), ("build.gradle", GRADLE_SPRING)]);
    assert!(s.back.is_none());
}

#[test]
fn extra_maven_forms() {
    let pom = |p: &str| back(&[("pom.xml", p)]);
    let b = pom("<project><parent><artifactId>spring-boot-starter-parent</artifactId>\n  <version>3.4.1</version></parent><properties><maven.compiler.release>21</maven.compiler.release></properties></project>");
    assert_eq!((b.framework_version.as_str(), b.lang_version.as_str(), b.build.as_str()), ("3.4.1", "21", "Maven"));
    let b = pom("<ARTIFACTID>SPRING-BOOT-STARTER-PARENT</ARTIFACTID><VERSION>3.0.0</VERSION> spring-boot <java.version>17</java.version>");
    assert_eq!((b.framework_version.as_str(), b.lang_version.as_str()), ("3.0.0", "17"));
    // 버전 태그가 바로 따라오지 않으면 판 없음
    assert_eq!(pom("<artifactId>spring-boot-starter-parent</artifactId><name>x</name><version>3.4.1</version>").framework_version, "");
    assert_eq!(pom("spring-boot <artifactId>spring-boot-starter-parent</artifactId><version>3.4.1<x></version>").framework_version, "");
    assert_eq!(pom("spring-boot <java.version></java.version>").lang_version, "");
    assert_eq!(pom("spring-boot <java.version>17").lang_version, "");
    assert_eq!(pom("spring-boot kotlin-maven-plugin").key, "spring-kotlin");
}

#[test]
fn extra_backend_order_and_fastapi_forms() {
    // pom.xml 은 build.gradle 이 스프링이 아닐 때, 같은 폴더에서 앞 갈래가 이긴다
    let s = back(&[("build.gradle", GRADLE_SPRING), ("pom.xml", "spring-boot"), ("go.mod", "go 1.20")]);
    assert_eq!(s.build, "Gradle Groovy");
    // 뿌리가 폴더보다 먼저
    let s = back(&[("backend/go.mod", "go 1.20"), ("go.mod", "go 1.22")]);
    assert_eq!(s.sources, vec!["go.mod".to_string()]);
    let py = |f: &[(&str, &str)]| back(f);
    let b = py(&[("requirements.txt", "FastAPI==0.110.0\nuvicorn\n")]);
    assert_eq!((b.framework_version.as_str(), b.build.as_str(), b.sources[0].as_str(), b.lang_version.as_str()), ("0.110.0", "pip", "requirements.txt", ""));
    assert_eq!(py(&[("requirements.txt", "fastapi[all]>=0.1\n")]).framework_version, "0.1");
    assert_eq!(py(&[("requirements.txt", "fastapi~=0.2.1")]).framework_version, "0.2.1");
    assert_eq!(py(&[("requirements.txt", "fastapi ^ \"0.3\"")]).framework_version, "0.3");
    assert_eq!(py(&[("requirements.txt", "fastapi\n0.9")]).framework_version, "0.9");
    assert_eq!(py(&[("requirements.txt", "fastapi\nflask==1")]).framework_version, "");
    assert_eq!(py(&[("requirements.txt", "fastapi-utils fastapi==0.5")]).framework_version, "0.5");
    assert_eq!(py(&[("requirements.txt", "fastapi[x")]).framework_version, "");
    // pyproject 가 있으면 requirements 는 보지 않는다
    assert!(detect(&[("pyproject.toml", "[project]\n"), ("requirements.txt", "fastapi")]).back.is_none());
    let b = py(&[("pyproject.toml", "requires-python = \">=3.12,<4\"\ndependencies=[\"fastapi\"]")]);
    assert_eq!((b.lang_version.as_str(), b.build.as_str(), b.framework_version.as_str()), ("3.12", "pyproject", ""));
    assert_eq!(py(&[("pyproject.toml", "fastapi\nrequires-python = '>=3.12'")]).lang_version, "");
    assert_eq!(py(&[("pyproject.toml", "fastapi\nREQUIRES-PYTHON  =  \"~=3.11.2\"")]).lang_version, "3.11.2");
    assert_eq!(py(&[("pyproject.toml", "fastapi\nrequires-python = \"x\"")]).lang_version, "");
}

#[test]
fn extra_go_and_express_and_node_engine() {
    let b = back(&[("go.mod", "module x\r\n\r\ngo 1.23.1\r\nrequire github.com/labstack/echo/v4 v4.1\r\n")]);
    assert_eq!((b.name.as_str(), b.lang_version.as_str(), b.framework.as_str(), b.key.as_str()), ("Go · Echo", "1.23.1", "Echo", ""));
    let b = back(&[("go.mod", "module x\ngo\n1.21\nrequire github.com/gofiber/fiber/v2 v2.0\n")]);
    assert_eq!((b.name.as_str(), b.lang_version.as_str()), ("Go · Fiber", "1.21"));
    let b = back(&[("go.mod", "module github.com/foo/gox\n  go 1.20\nGO 1.19\n")]);
    assert_eq!((b.name.as_str(), b.lang_version.as_str()), ("Go", "1.19"));
    assert_eq!(back(&[("go.mod", "module x\ngolang 1.20\n")]).lang_version, "");
    let b = back(&[("package.json", r#"{"dependencies":{"express":"~4.19.2"},"engines":{"node":18}}"#)]);
    assert_eq!((b.name.as_str(), b.lang_version.as_str(), b.framework_version.as_str()), ("Node.js · Express", "18", "4.19.2"));
    // 의존성 판 순서: dependencies > devDependencies > peerDependencies
    let b = back(&[("package.json", r#"{"peerDependencies":{"@nestjs/core":"^9"},"devDependencies":{"@nestjs/core":"^10"},"dependencies":{"@nestjs/core":">= 11.2"}}"#)]);
    assert_eq!((b.key.as_str(), b.framework_version.as_str()), ("nest", "11.2"));
    // JSON 이 아니거나 모양이 다르면 감지하지 않는다
    assert!(detect(&[("package.json", "{not json")]).is_empty());
    assert!(detect(&[("package.json", r#"[{"dependencies":{"next":"1"}}]"#)]).is_empty());
    assert!(detect(&[("package.json", r#"{"dependencies":["next"]}"#)]).is_empty());
    // 판 값이 글이 아니어도 감지는 한다(판은 빈 글)
    let f = detect(&[("package.json", r#"{"dependencies":{"next":{"x":1}}}"#)]).front.expect("front");
    assert_eq!((f.key.as_str(), f.framework_version.as_str()), ("next", ""));
}

#[test]
fn extra_front_forms() {
    let front = |j: &str| detect(&[("package.json", j)]).front;
    let v = front(r#"{"dependencies":{"vue":"^3.5.0","vite":"^7"}}"#).expect("vue");
    assert_eq!((v.name.as_str(), v.build.as_str(), v.framework_version.as_str()), ("Vue", "Vite", "3.5.0"));
    let v = front(r#"{"dependencies":{"vue":"3"}}"#).expect("vue");
    assert_eq!((v.build.as_str(), v.framework_version.as_str()), ("npm", "3"));
    let r = front(r#"{"dependencies":{"react":"v19.1","vite":"1"}}"#).expect("react");
    assert_eq!((r.name.as_str(), r.build.as_str(), r.framework_version.as_str(), r.key.as_str()), ("React (Vite)", "Vite", "19.1", "react"));
    let r = front(r#"{"dependencies":{"react":"~ 18"}}"#).expect("react");
    assert_eq!((r.name.as_str(), r.build.as_str(), r.framework_version.as_str()), ("React", "npm", "18"));
    assert!(r.lang.is_empty() && r.lang_version.is_empty());
    // next > vue > react
    assert_eq!(front(r#"{"dependencies":{"react":"1","vue":"1","next":"1"}}"#).expect("next").key, "next");
    assert_eq!(front(r#"{"dependencies":{"react":"1","vue":"1"}}"#).expect("vue").key, "vue");
    // 프론트는 뿌리에 있어도 앞 폴더에서 먼저 찾은 것
    let s = detect(&[("package.json", r#"{"dependencies":{"vue":"1"}}"#), ("frontend/package.json", r#"{"dependencies":{"next":"1"}}"#)]);
    assert_eq!(s.front.expect("front").sources, vec!["package.json".to_string()]);
}

#[test]
fn extra_db_forms() {
    let db = |f: &[(&str, &str)]| detect(f).db.expect("db");
    let key_ver = |d: &StackSlot| format!("{} {} {}", d.key, d.framework_version, d.sources.join(","));
    // compose: 이미지 경로 앞부분·따옴표·대소문자·판
    assert_eq!(key_ver(&db(&[("compose.yaml", "services:\n  w:\n    image: nginx\n  d:\n    image: \"docker.io/library/postgres:16.3\"\n")])), "postgresql 16.3 compose.yaml");
    assert_eq!(key_ver(&db(&[("docker-compose.yml", "IMAGE: Mysql:8.4\n")])), "mysql 8.4 docker-compose.yml");
    assert_eq!(key_ver(&db(&[("docker-compose.yml", "image: bitnami/postgresql:15\n")])), "postgresql 15 docker-compose.yml");
    // 판 없는 이미지: 소문자 `image:` 만
    assert_eq!(key_ver(&db(&[("compose.yml", "image: postgres\n")])), "postgresql  compose.yml");
    assert_eq!(key_ver(&db(&[("compose.yml", "image: ghcr.io/x/postgres-alpine:latest\n")])), "postgresql  compose.yml");
    assert_eq!(key_ver(&db(&[("compose.yml", "image: 'mysql'\n")])), "mysql  compose.yml");
    assert!(detect(&[("compose.yml", "IMAGE: postgres\n")]).db.is_none());
    assert!(detect(&[("compose.yml", "image: Postgres\n")]).db.is_none());
    assert!(detect(&[("compose.yml", "image: mypostgres:17\n")]).db.is_none());
    assert!(detect(&[("compose.yml", "image: a/b-mysql:8\n")]).db.is_none());
    // 설정 파일이 compose 보다 먼저 정하고, compose 가 같은 DB 면 판만 더한다(출처는 설정 파일)
    let d = db(&[("src/main/resources/application.properties", "spring.datasource.url=jdbc:mariadb://x"), ("compose.yml", "image: mysql:8.0.1\n")]);
    assert_eq!(key_ver(&d), "mysql 8.0.1 src/main/resources/application.properties");
    // 다른 DB 면 compose 는 덮지 못한다
    let d = db(&[("src/main/resources/application.yaml", "jdbc:postgresql://x"), ("compose.yml", "image: mysql:8\n")]);
    assert_eq!(key_ver(&d), "postgresql  src/main/resources/application.yaml");
    let d = db(&[("src/main/resources/application.yml", "url: jdbc:mysql://x"), ("compose.yml", "image: postgres:15\n")]);
    assert_eq!(key_ver(&d), "mysql  src/main/resources/application.yml");
    // 의존성
    assert_eq!(key_ver(&db(&[("package.json", r#"{"dependencies":{"mysql2":"3"}}"#)])), "mysql  package.json");
    assert_eq!(key_ver(&db(&[("package.json", r#"{"dependencies":{"mysql":"2","pg":"8"}}"#)])), "postgresql  package.json");
    assert_eq!(key_ver(&db(&[("requirements.txt", "AsyncPG==0.29")])), "postgresql  requirements.txt");
    assert_eq!(key_ver(&db(&[("pyproject.toml", "dependencies=['PyMySQL']")])), "mysql  pyproject.toml");
    assert_eq!(key_ver(&db(&[("prisma/schema.prisma", "datasource db {\n PROVIDER = \"PostgreSQL\"\n}")])), "postgresql  prisma/schema.prisma");
    assert!(detect(&[("prisma/schema.prisma", "provider = \"sqlite\"")]).db.is_none());
    assert!(detect(&[("prisma/schema.prisma", "provider = \"mysql")]).db.is_none());
    // 폴더 안 출처는 폴더 이름이 앞에 붙는다
    assert_eq!(key_ver(&db(&[("server/prisma/schema.prisma", "provider = \"mysql\"")])), "mysql  server/prisma/schema.prisma");
    let d = db(&[("api/compose.yml", "image: postgres:17-alpine\n")]);
    assert_eq!((d.name.as_str(), d.framework.as_str(), d.framework_version.as_str(), d.build.as_str()), ("PostgreSQL", "PostgreSQL", "17", ""));
}

#[test]
fn extra_clean_strips_range_marks_and_keeps_at_most_three_parts() {
    let ver = |v: &str| back(&[("package.json", &format!(r#"{{"dependencies":{{"express":"{v}"}}}}"#))]).framework_version;
    assert_eq!(ver("^4.19.2"), "4.19.2");
    assert_eq!(ver("~4.1"), "4.1");
    assert_eq!(ver(">=5"), "5");
    assert_eq!(ver("  >= v 6.0.1.9"), "6.0.1");
    assert_eq!(ver("<7.2."), "7.2");
    assert_eq!(ver("latest"), "");
    assert_eq!(ver("*"), "");
    assert_eq!(ver("x.1"), "");
    assert_eq!(ver("12abc"), "12");
}
