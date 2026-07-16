# Claude goal 재개 인수인계

- 작성일: 2026-07-16
- 저장소: `whackur/assay`
- 기준 브랜치: `main`
- 구현 기준 커밋: `7798927dfc07984e6424b171b387a7da315625f9`
- 구현 기준 tree: `0c3398d6accfd543b6d5a575931c11d956a18aad`
- Kanban board: `assay`
- 현재 상태: 완료 12, 준비 3, 대기 9, 실행 0, 차단 0
- 공개 PR: 생성하지 않음

이 문서는 Claude에서 장기 goal을 다시 시작하기 위한 시점별 구현 인수인계다.
제품 정책과 공개 계약의 최종 기준은 항상 `docs/specs/`다.

## 1. 재개 목표

Kanban board `assay`의 남은 12개 카드를 선행 의존성 순서대로 모두 구현한다.
각 카드는 격리 worktree에서 구현하고, 구현자와 분리된 독립 리뷰를 통과한 뒤
`main`에 fast-forward 병합한다. 최종 상태는 board 전부 `done`, 전체 검증 통과,
깨끗한 `main`, 남은 worktree 없음이다.

Contribution Intelligence와 Project Intelligence의 경계를 유지한다. 개인 단위
합성 점수나 순위는 만들지 않고, 프로젝트 점수는 차원·버전·신뢰도·근거를 가진
설명 가능한 계약으로만 구현한다.

## 2. Claude goal에 그대로 사용할 프롬프트

아래 블록을 새 goal의 첫 지시로 사용할 수 있다.

```text
/opt/data/workspace/projects/assay에서 Assay 구현 goal을 재개한다.

먼저 다음 파일을 끝까지 읽는다.
1. AGENTS.md
2. docs/specs/functional-development-specification.md
3. docs/specs/open-source-project-intelligence-specification.md
4. docs/specs/identity-private-workspace-and-entitlements-specification.md
5. docs/internal/product-decisions/2026-07-16-project-intelligence-interview.md
6. docs/internal/handoffs/next-implementation-handoff.md
7. docs/internal/handoffs/2026-07-16-claude-goal-handoff.md

Hermes Kanban의 assay board를 유일한 작업 큐로 사용한다. 모든 명령에
--board assay를 명시한다. 현재 done 12, ready 3, todo 9이며 running과 blocked는
0이어야 한다. diagnostics를 먼저 확인한다.

남은 12개 카드를 선행 의존성 순서대로 모두 완료한다. 동시에 최대 3개의 서로
독립적인 ready 카드만 진행한다. 각 카드 시작 시 TTL 7200으로 claim하고, 카드가
지정한 격리 worktree와 branch를 사용한다. 구현 에이전트는 TDD로 작업하고 영어
commit을 만들되 rebase, main 병합, Kanban complete를 하지 않는다.

각 구현 commit은 별도의 독립 리뷰어가 전체 delta와 관련 mutation을 검토한다.
리뷰가 정확히 No findings.가 될 때까지 같은 branch에서 수정과 재리뷰를 반복한다.
그 뒤 메인 담당만 최신 main에 rebase하고 rebase 전후 tree 동일성을 확인하며,
필수 검증을 직접 재실행한 뒤 main을 fast-forward한다. 완료 증거를 Kanban comment와
structured complete metadata에 남기고 worktree와 merged local branch를 제거한다.

세 ready 카드에는 이미 원격 checkpoint가 있다.
- ADR-002: origin/wt/assay-adr-002 @ 6600ecf20240d42c047648c74273c5aa0d604e92
- AIE-001: origin/wt/assay-aie-001 @ 59593694388c88f95a35d36bd63785da366d49e8
- GH-001: origin/wt/assay-gh-001 @ b5f76f2c6bdcc20557e9b363a422d56b8a1190fe

새로 구현하지 말고 반드시 해당 checkpoint에서 이어간다. 같은 workspace라면 남아
있는 local branch를 사용한다. 다른 clone이라면 git fetch origin 후 해당 remote
branch를 local tracking branch로 만든다. Hermes claim이 main 기준 branch를 새로
만들었다면 checkpoint commit을 정확히 cherry-pick하고 tree를 대조한다.

모든 카드에서 source code, token, raw diff, credential payload, machine absolute
path를 출력하거나 저장하지 않는다. 분석 대상 repository의 코드를 install, import,
build, test, execute하지 않는다. unavailable/unsupported/partial/pending/insufficient를
0으로 바꾸지 않는다. public artifact는 영어, docs/internal prose는 한글이다.

매 병합 전 cargo fmt --check, full clippy -D warnings, cargo test --workspace와 범위별
contract/schema/golden/security/markdown/web gate를 실제 실행한다. assay-git의
ProbeCapabilities/Io가 간헐적으로 한 번 실패한 이력이 있으므로 실패를 숨기지 말고
4/1 thread 보안 테스트와 plain workspace 재실행으로 delta 회귀 여부를 분리한다.

최종 goal 완료 조건:
- board 24/24 done, running/ready/todo/blocked/diagnostics 0
- main에 모든 독립 리뷰 완료 commit이 fast-forward 병합됨
- cargo fmt, clippy, workspace test 및 모든 범위별 gate 통과
- git status clean, 임시 worktree/branch 정리
- 사람 점수나 프로젝트/개인 도메인 혼합 없음
- 실제 검증, 미실행 항목, 알려진 제한을 한글로 최종 보고

추가 push, PR, deploy는 사용자가 다시 명시하지 않는 한 하지 않는다.
```

## 3. 현재 `main` 완료 상태

`main`의 구현 기준은 `7798927`이다. 이 시점까지 12개 카드가 완료됐다. 마지막으로
완료한 `VER-001`은 기반 수직 슬라이스를 실제 CLI로 종단 간 검증하고 다음을 추가했다.

- pull request와 `main` push에서 실행되는 read-only GitHub Actions CI
- exact workflow structure, immutable action SHA, Rust 1.97, Git 2.47 이상 계약
- 고정 revision/time의 21-file 합성 저장소를 두 번 분석하는 CLI vertical slice
- schema, reviewed digest, provenance, data sufficiency 및 상태 분리 검증
- PATH shim과 Git/package/JS/Python sentinel을 사용한 no-execution 검증
- citation closure와 잘못된 payload/diagnostic citation production negative tests
- `git ls-files --stage -z` fail-closed parser와 mode `160000` gitlink 거부
- credential, token, secret, auth 디렉터리 및 build/cache 산출물 추적 방지

최종 독립 리뷰는 `No findings.`였다. 구현자, 리뷰어, 메인 담당이 각각 전체 Rust
게이트를 실행했다. 메인 담당의 첫 `cargo test --workspace`에서 기존
`assay-git` process-wrapper의 `ProbeCapabilities/Io` 간헐 실패가 1회 있었으나,
해당 카드에는 `assay-git` 변경이 없었고 다음 검증은 모두 통과했다.

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace` 재실행
- `RUSTFLAGS='-D unnameable-types' cargo check --workspace --all-targets --all-features`
- `security_boundaries` 4 thread와 1 thread 각각 15/15
- `snapshot_adapter` 1 thread 14/14
- Markdown 26개, 오류 0
- workflow parse, `git diff --check`, gitlink 및 공개 한글 scan

기반 카드 완료 시에는 push 금지 범위 때문에 hosted GitHub Actions를 실행하지
않았다. 인수인계 push 뒤 실행된 CI run `29464612278`은 1분 5초에 성공했고 format,
Clippy, workspace tests와 public schema/golden 검증이 모두 통과했다.

비차단 annotation이 1건 있다. pinned `actions/checkout`과 `actions/cache`가 Node.js
20을 대상으로 하므로 GitHub runner가 Node.js 24로 강제 실행했다. 다음 goal에서
upstream의 현재 immutable SHA와 runtime 변경을 공식 자료로 검토하되, exact CI
contract와 tests를 함께 갱신하지 않고 action pin만 임의로 바꾸지 않는다.

## 4. 준비 상태의 checkpoint 세 개

세 카드는 완료가 아니다. checkpoint는 중단 시점의 작업 손실을 막기 위해 만들고
원격에 push한 것이다. 전체 workspace 검증과 독립 리뷰를 거쳐야 병합할 수 있다.

### 4.1 ADR-002 — 의미론적 diff 엔진 스파이크

- 카드: `t_efa0205f`
- branch: `wt/assay-adr-002`
- checkpoint: `6600ecf20240d42c047648c74273c5aa0d604e92`
- 원격: `origin/wt/assay-adr-002`
- 변경: `assay-semantic-diff` crate, native tree-sitter adapter, JS/TS/Python fixture,
  reviewed fixture contract, spike example
- checkpoint 검증: `git diff --check`, crate tests 3/3 통과

현재 fixture는 각 언어의 format-only, body modification, top-level move, symbol
rename을 표현한다. parse error는 명시 상태로 유지하고 engine metadata는 버전이
있으며 사람 가치를 주장하지 않는다.

남은 작업:

1. 공식 1차 자료를 사용해 difftastic과 GumTree의 현재 버전, 라이선스, 배포 및
   Rust 통합 조건을 다시 확인한다.
2. 같은 reviewed fixture에서 native tree-sitter, difftastic, GumTree를 실제로
   실행해 정확성, move/rename, format-only 동작을 비교한다.
3. 재현 가능한 cold/warm latency와 peak RSS 측정 환경 및 원시 요약을 기록한다.
4. 결과와 선택/보류 결정을 영문 `docs/architecture/` ADR로 남긴다.
5. 외부 engine binary나 clone을 repository에 넣지 않고 임시 경로만 사용한다.
6. 전체 gate와 Markdown lint를 실행하고 독립 리뷰한다.

의도적으로 제외한 범위는 범용 AST matcher 완성, malformed-source fallback,
public schema/CLI 변경, 외부 engine production 포함이다.

### 4.2 AIE-001 — 가짜 공급자를 사용한 AI 평가기 계약

- 카드: `t_dc507dd8`
- branch: `wt/assay-aie-001`
- checkpoint: `59593694388c88f95a35d36bd63785da366d49e8`
- 원격: `origin/wt/assay-aie-001`
- 변경: `assay-ai-evaluator` crate와 README, rubric/bundle/error/evaluator 계약,
  deterministic fake provider, `schemas/README.md` 연결 설명
- checkpoint 검증: `git diff --check`, evaluator contract 16/16 통과

현재 테스트는 unknown criterion, 범위 밖 rating, 누락·조작·중복 citation,
schema-invalid output, prompt injection, 민감 evidence, private transmission policy,
bundle/privacy forgery, 전체 rubric coverage, canonical hash, provider prose 격리를
검증한다. 기존 `schemas/ai-judgment/v1.json`을 사용하며 새 schema는 만들지 않았다.

남은 작업:

1. 공개 API와 error/debug 출력이 provider prose, evidence content, secret, absolute
   path를 어떤 경우에도 노출하지 않는지 mutation 중심으로 독립 리뷰한다.
2. prompt-injection 판정이 정상 기술 문장을 과대 차단하지 않는지 positive/negative
   경계를 검토한다.
3. fake provider의 결정성, citation closure, rubric version/hash binding 및
   privacy/transmission policy를 별도 reviewer가 재계산한다.
4. full fmt/clippy/workspace/unnameable/schema/Markdown gate를 실행한다.
5. findings를 같은 branch에서 TDD로 닫은 뒤 main에만 병합한다.

실제 네트워크 공급자, 자격 증명, OpenAI/Codex adapter, 점수 계산, HTTP/DB/CLI
연결은 의도적으로 제외했다.

### 4.3 GH-001 — GitHub URL 및 증분 캐시

- 카드: `t_d14c2b49`
- branch: `wt/assay-gh-001`
- checkpoint: `b5f76f2c6bdcc20557e9b363a422d56b8a1190fe`
- 원격: `origin/wt/assay-gh-001`
- 변경: `assay-github` crate의 source/cache/http/collection/tree 계약과 tests
- checkpoint 검증: `git diff --check`, crate tests 18/18 통과

현재 구현은 public GitHub URL strict normalization, GET-only fake transport,
symbolic ref의 immutable SHA 해석, explicit rate state, versioned evaluation/blob
cache key, bounded streaming tree sink, monorepo boundary와 partial reason을 포함한다.

남은 작업:

1. HTTP origin/path/percent-encoding, redirect, rate-limit header와 response byte
   bound, pagination/truncation 및 error redaction을 adversarial mutation으로 독립
   리뷰한다.
2. cache key가 repository/provider/revision/evidence/evaluation/rubric/profile 및
   blob/analyzer/rule version의 모든 의미 변화를 구분하는지 재검증한다.
3. large tree streaming이 전체 payload나 source body를 보관하지 않고 local/provider
   bounds에서 explicit partial로 끝나는지 확인한다.
4. 새 GitHub/cache boundary 결정을 영문 `docs/architecture/` ADR로 기록한다.
5. full fmt/clippy/workspace/unnameable/Markdown gate 후 독립 리뷰한다.

live network/token/private repository, 실제 HTTP client, persistent storage,
quota/cooldown, API/CLI 연결과 score는 의도적으로 제외했다.

## 5. 이후 대기 카드

세 ready 카드가 완료되면 부모 의존성에 따라 다음 카드가 자동으로 `ready`가 된다.
사람이 수동으로 unblock하지 않는다.

| 카드 | 제목 | 현재 상태 |
| --- | --- | --- |
| `t_73cf0fcf` | AIE-002 — OpenAI API 어댑터 | todo |
| `t_f1fca5fa` | SCR-001 — 결정론적 점수 컴파일러 | todo |
| `t_15bcd430` | CMP-001 — 유형 판정 및 자동 경쟁 프로젝트 비교 | todo |
| `t_a7b9839b` | WEB-001 — 최소 GitHub URL 프런트엔드 | todo |
| `t_09089efd` | CAT-001 — 공개 카탈로그와 공유 | todo |
| `t_7e83d0bf` | IAM-001 — hakhub.net 인증 배포 | todo |
| `t_20599b9d` | LOC-001 — 로컬 비공개 GitHub 분석과 대시보드 | todo |
| `t_987c8e74` | OPS-001 — 부분 실패와 관리자 작업 | todo |
| `t_294cbbdc` | MVP-001 — 첫 공개 MVP 통합 검증 | todo |

## 6. Kanban 운용 계약

모든 명령에는 board를 명시한다.

```sh
hermes kanban --board assay stats
hermes kanban --board assay diagnostics
hermes kanban --board assay list --status ready --json
hermes kanban --board assay claim <task-id> --ttl 7200
```

카드 시작 comment에는 구현 에이전트, worktree, branch, 기준 commit, TTL을 남긴다.
구현 에이전트는 commit까지만 수행한다. 메인 담당은 독립 reviewer, findings 폐쇄,
rebase, tree 대조, 검증, fast-forward, Kanban complete와 worktree 제거를 소유한다.

claim heartbeat는 TTL을 연장하지 않는다. 2시간에 가까워지면 card run을 확인하고,
만료됐다면 안전하게 reclaim/claim한다. 실행 중 claim을 임의로 중복하지 않는다.

## 7. branch 재개와 병합 절차

같은 local workspace에서는 checkpoint local branch가 남아 있을 수 있다.

```sh
git fetch origin
git branch --list 'wt/assay-*'
git ls-remote --heads origin \
  wt/assay-adr-002 wt/assay-aie-001 wt/assay-gh-001
```

다른 clone에서 직접 복구할 때는 다음과 같이 tracking branch를 만든다.

```sh
git switch --create wt/assay-aie-001 --track origin/wt/assay-aie-001
```

Hermes가 격리 worktree를 생성해야 한다면 branch와 worktree 소유권 충돌을 먼저
확인한다. checkpoint branch를 새 main 기반 branch에 합칠 때는 merge commit을
만들지 말고 정확한 checkpoint commit을 cherry-pick한 뒤 전체 delta를 리뷰한다.

최종 병합 전 절차:

1. 구현 worktree clean과 commit hash 기록
2. fresh detached worktree에서 독립 reviewer 실행
3. `No findings.`까지 TDD 수정과 fresh review 반복
4. 최신 `main`에 rebase
5. rebase 전후 tree hash와 diff 동일성 확인
6. 메인 담당의 전체 gate 재실행
7. `main`에서 `git merge --ff-only <branch>`
8. 상세 Kanban comment와 structured completion metadata 기록
9. worktree 제거, merged local branch 삭제, `git worktree prune`

## 8. 보안과 제품 경계 재확인

- target repository code를 install, import, build, test, execute하지 않는다.
- Git과 GitHub 수집은 기본 read-only다.
- token, private source, raw diff, credential payload, absolute machine path를 로그,
  error, JSON, DB에 넣지 않는다.
- cache에는 full source blob을 기본 저장하지 않는다.
- AI provider prose와 prompt는 검증되지 않은 입력이다.
- AI 판정은 citation이 검증된 bounded judgment만 score compiler에 전달한다.
- Project Intelligence score에는 개인 단위 관측을 넣지 않는다.
- Contribution Intelligence에는 개인 합성 점수나 leaderboard를 만들지 않는다.
- unavailable, unsupported, partial, pending, insufficient를 0으로 치환하지 않는다.

## 9. 현재 원격과 정리 상태

이 handoff 작성 시점에 다음 branch를 push했다.

- `main`: 완료된 기반 12개 카드와 이 handoff
- `wt/assay-adr-002`: `6600ecf20240d42c047648c74273c5aa0d604e92`
- `wt/assay-aie-001`: `59593694388c88f95a35d36bd63785da366d49e8`
- `wt/assay-gh-001`: `b5f76f2c6bdcc20557e9b363a422d56b8a1190fe`

PR과 deploy는 수행하지 않았다. 세 카드의 active claim은 해제했으며 board에는
`running`과 `blocked`가 없다. local worktree는 checkpoint push 후 제거한다.
