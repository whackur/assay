# Windows 툴체인 구동과 테스트 이식성 인수인계

- 작성일: 2026-07-16
- 기준 브랜치: `main`
- 성격: Windows 개발 환경에서 Assay를 구동한 결과, 발견한 이식성 결함,
  적용한 수정, 그리고 `cargo test --workspace`를 Windows에서 무사 통과시키기
  위해 남은 작업의 기록
- 제품 정책의 최종 기준은 항상 `docs/specs/`다. 이 문서는 구현 시점 컨텍스트다.

## 1. 이번 세션 목표

- Docker로 웹 표면을 실행한다.
- 최신 Rust를 설치하고 실제 CLI 수직 슬라이스를 Windows에서 구동한다.
- outdated 인수인계 문서를 정리한다.
- Windows에서 발견한 실행/테스트 결함을 진단하고 수정한다.

## 2. 실행 환경 (Windows)

- OS: Windows 11 Pro, 셸: PowerShell + Git Bash.
- Docker: `29.6.1`, Compose `v5.3.0`. 데몬 작동 중.
- Rust: 기존에 표준 위치(`C:\Users\<user>\.rustup`, 기본 host
  `x86_64-pc-windows-msvc`)의 rustup이 있었다. 이번 세션에 scoop
  `rustup-gnu`(rustup 1.29.0, stable `1.97.1-gnu`)도 설치했으나, 저장소
  `rust-toolchain.toml`이 `1.97.0`으로 고정되어 있어 저장소 안에서는
  `1.97.0-x86_64-pc-windows-msvc`가 활성 툴체인이다.
- MSVC 링커: `cl.exe`는 PATH에 없지만 VS 2022 **BuildTools**
  (`...\2022\BuildTools\VC\Tools\MSVC\14.44.35207\...`)에 설치되어 있어
  cargo가 vswhere로 자동 탐지한다. 별도 PATH 설정 없이 링크된다.
- cargo 프록시 실제 위치:
  `C:\Users\<user>\scoop\persist\rustup-gnu\.cargo\bin`. 이 경로가 자동으로
  PATH에 잡히지 않아 Bash에서 명시적으로 앞에 추가해야 한다:
  `export PATH="$HOME/scoop/persist/rustup-gnu/.cargo/bin:$PATH"`.
- Git: `C:\Program Files\Git\cmd\git.exe` (`git version 2.50.1.windows.1`,
  어댑터 요구치 2.47+ 충족).

## 3. Docker 실행 결과

- `docker compose up --build -d` 성공. `assay-web-1`이 healthy 상태로
  `http://localhost:3000`에서 구동된다.
- 주의: `Dockerfile`/`compose.yaml`은 `web/`(Next.js) 셸만 빌드하며 in-repo
  픽스처로 구동된다. 라이브 Rust 백엔드에는 연결되어 있지 않다(기반
  마일스톤에서 의도적으로 연기됨, MVP 통합 상태 문서 참고).
- `compose.yaml`은 Compose Specification 표준 기본 파일명이다
  (`docker-compose.yml`은 하위 호환 이름).

## 4. Rust CLI 구동 결과

`cargo build -p assay-cli` 성공(MSVC 링크). 구동 검증:

- `assay capabilities --format json`: 정상. 구현 범위를 정직하게 보고한다
  (`file_classification`/`local_git_snapshot`/`loopback_dashboard` =
  `implemented`, `ai_evaluation`/`project_scores`/`github_collection` =
  `not_implemented`).
- `assay project analyze . --revision HEAD --evaluator deterministic
  --format json --output -`: 수정 후 `exit 0`, stderr 비어 있음, stdout에
  스키마 유효 `project-analysis` 번들(`schema_version 1.0.0`, `manifest` +
  `evidence`, 약 996KB)을 산출한다.
- `assay serve --history <dir> --port 0 --once`: 정상 바인딩
  (`listening on http://127.0.0.1:<port>`).

## 5. 발견 1 — `trusted_git()`가 Unix 전용 (수정 완료)

### 증상

Windows에서 `assay project analyze`가
`exit 10: collection_failed stage=configure_adapter kind=executable_missing`
으로 실패했다.

### 근본 원인

`crates/assay-cli/src/lib.rs`의 `trusted_git()`가
`["/usr/bin/git", "/usr/local/bin/git"]`만 `.is_file()`로 검사했다. `assay`는
네이티브 `x86_64-pc-windows-msvc` 바이너리라 이 Unix 경로가 존재하지 않아
executable을 찾지 못했다. `GitCliAdapter::from_trusted_executable`는 보안상
절대 경로만 받고 PATH를 탐색하지 않는다(ADR 0002, 의도된 설계).

### 수정

ADR 0002 규칙 1("trusted deployment configuration 또는 trusted startup
environment에서 executable을 해석하고 repository content에서는 절대 도출하지
않는다")에 맞춰 다음을 구현했다.

- `ASSAY_GIT_EXECUTABLE` 환경 변수(운영자 지정 절대 경로)를 최우선으로 사용.
- 플랫폼별 기본 절대 경로: Unix는 기존 두 경로, Windows는
  `ProgramW6432`/`ProgramFiles`/`ProgramFiles(x86)` 기반
  `Git\cmd\git.exe`·`Git\bin\git.exe` + `C:\Program Files\Git` 리터럴 폴백.
- 순수 함수 `resolve_trusted_git(Option<OsString>)`로 분리해 전역 환경을
  오염시키지 않고 우선순위·절대성 계약을 단위 테스트한다(Rust 2024의
  `set_var` unsafe 회피).

파일: `crates/assay-cli/src/lib.rs` (함수 + 단위 테스트 3개),
`docs/architecture/0002-git-adapter.md` (Consequences 노트 추가).

검증: `cargo fmt --check` 통과, `cargo clippy -p assay-cli --all-targets
--all-features -- -D warnings` 경고 0, `cargo test -p assay-cli --lib` 3/3
통과, 실제 `project analyze` `exit 0`.

## 6. 발견 2 — Windows 전체 테스트 실패 진단

`cargo test --workspace --no-fail-fast` 결과, 6개 테스트 바이너리에서 약 37개
테스트가 실패했다. **모든 크레이트는 컴파일에 성공하며**(Unix 전용 구성은
`#[cfg(unix)]`로 적절히 게이트됨), 실패는 전부 런타임이다. 세 부류로
분류했고, **셋 다 테스트 하네스의 이식성 결함이며 제품 코드 결함이 아니다.**
제품 CLI는 5절/4절대로 Windows에서 정상 동작한다.

### 부류 A — 테스트의 Git executable 해석 (약 32건, 테스트 전용)

테스트 헬퍼들이 수정 전 `trusted_git()`와 같은 Unix 하드코딩을 자체 복제한다.
panic 메시지: "tests require a deployment-trusted Git executable" /
"the Git adapter integration tests require a trusted absolute Git executable"
/ `Command::new("/usr/bin/git")`의 Windows `NotFound`(code 3).

해당 위치:

- `crates/assay-git/tests/snapshot_adapter.rs:25` (헬퍼, 12건)
- `crates/assay-project-intelligence/tests/evidence_manifest.rs:30` (헬퍼, 15건)
- `crates/assay-project-intelligence/tests/machine_contract.rs:308` (헬퍼, 5건)
- `crates/assay-cli/tests/cli_contract.rs:353,538`
  (`Command::new("/usr/bin/git")` 직접 호출, 2건)

수정 방향: 5절의 `resolve_trusted_git`와 동일한 크로스플랫폼 해석을 테스트
헬퍼에 적용한다(또는 픽스처 빌더처럼 `PathBuf::from("git")` PATH 기반 사용 —
`tests/support/assay-fixtures/src/lib.rs:264`는 이미 이식성 있음). 공유
헬퍼로 묶는 것을 권장한다.

### 부류 B — 테스트가 Unix 스타일 절대 경로 사용 (1건, 테스트 전용)

`crates/assay-git/tests/snapshot_adapter.rs:614`
(`reports_missing_and_incompatible_git_without_executable_paths`)는
`PathBuf::from("/definitely/missing/assay-git")`를 넘기고 실패 stage가
`ProbeCapabilities`이길 기대한다. Windows에서 `/definitely/...`는 절대 경로가
아니라 `from_trusted_executable`의 `is_absolute()` 검사에서 먼저 걸려
`ConfigureAdapter`(`UntrustedExecutable`)로 실패한다(assertion left/right
불일치).

수정 방향: 플랫폼별로 실재하지 않는 **절대** 경로를 사용한다(예: Windows에서
`C:\definitely\missing\assay-git`). `#[cfg(windows)]`로 분기.

### 부류 C — 테스트의 `env_clear()`가 Winsock을 깨뜨림 (2건, 테스트 전용)

`crates/assay-cli/tests/local_dashboard.rs`와
`crates/assay-cli/tests/cross_component_integration.rs`의 loopback 왕복
테스트가 `serve_bind_failed`로 실패했다. cross_component는 이후 파싱에서
"invalid port value"로 나타나지만 같은 근본 원인이다.

근본 원인(검증 완료): 테스트가 `Command::env_clear()`로 환경을 비운 뒤
`ASSAY_TEST_FIXED_TIME`만 설정한다. Windows에서 환경을 완전히 비우면
`SystemRoot`가 사라지고 `TcpListener::bind`(Winsock 초기화)가 실패한다. 실제
운영자는 정상 환경이라 `assay serve`는 문제없이 바인딩된다(4절에서 실증).
따라서 제품이 아닌 테스트 결함이다. (참고: MSYS `env -i`는 Windows 환경 블록을
완전히 비우지 않아 재현되지 않는다. Rust `env_clear()`는 완전히 비운다.)

수정: 환경을 비운 뒤 Windows에서 `SystemRoot`를 보존한다.

- `local_dashboard.rs`의 `fixed_command()`에 `#[cfg(windows)]`로 `SystemRoot`
  재설정을 추가함 — **적용 완료**. 해당 테스트 격리 재실행 시 통과 확인.
- `cross_component_integration.rs`의 serve 스폰 헬퍼에도 같은 보존을 적용해야
  함 — **미적용**.

## 7. 현재 변경 상태 (미커밋)

이번 세션 변경은 아직 커밋하지 않았다.

- `crates/assay-cli/src/lib.rs` — `trusted_git()` 크로스플랫폼화 + 단위 테스트.
- `docs/architecture/0002-git-adapter.md` — executable 해석 Consequences 노트.
- `crates/assay-cli/tests/local_dashboard.rs` — Windows `SystemRoot` 보존.
- `docs/internal/handoffs/` — outdated 문서 2건 삭제
  (`next-implementation-handoff.md`, `2026-07-16-claude-goal-handoff.md`),
  README와 `product-decisions/2026-07-16-project-intelligence-interview.md`의
  참조 갱신.

## 8. Windows에서 무사 통과시키기 위해 남은 작업

1. 부류 A: 3개 테스트 헬퍼와 `cli_contract.rs`의 Git 해석을 크로스플랫폼화.
2. 부류 B: `snapshot_adapter.rs:614`의 missing 경로를 플랫폼별 절대 경로로.
3. 부류 C: `cross_component_integration.rs`에 `SystemRoot` 보존 적용.
4. 전체 재검증: `cargo fmt --check`, `cargo clippy --workspace --all-targets
   --all-features -- -D warnings`, `cargo test --workspace`.
5. 알려진 flaky: `assay-git`의 `security_boundaries` 일부가 부하 시 같은
   `ProbeCapabilities/Io`로 간헐 실패한다(이번 변경과 무관, 기존 이슈). 단일
   스레드 재실행으로 안정 통과. Windows에서도 별도 추적 대상.

## 9. 재현/검증 명령 (Bash)

```sh
export PATH="$HOME/scoop/persist/rustup-gnu/.cargo/bin:$PATH"
cd /c/Users/<user>/coding/github/whackur/assay

# 빌드 및 CLI 구동
cargo build -p assay-cli
target/debug/assay.exe capabilities --format json
target/debug/assay.exe project analyze . --revision HEAD \
  --evaluator deterministic --format json --output -

# 전체 테스트 실패 인벤토리
cargo test --workspace --no-fail-fast

# Docker 웹
docker compose up --build -d   # http://localhost:3000
```

## 10. CI 관점 메모

`.github/workflows/ci.yml`은 현재 리눅스 러너를 전제로 한다. Windows 지원을
릴리스 게이트로 삼으려면(ADR 0002의 "self-contained Windows binaries become a
release gate" 재검토 조건) 위 부류 A/B/C 수정 후 Windows job 추가를 검토한다.
