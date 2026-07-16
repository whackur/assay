# 첫 공개 MVP 통합 검증 상태

- 작성일: 2026-07-16
- 대상: 인터뷰 12장 "첫 공개 MVP" 11개 항목
- 성격: 신규 기능 구현이 아니라 지금까지 병합된 구성요소의 종단 간 통합 검증과
  gap 기록
- 관련 카드: MVP-001 (첫 공개 MVP 통합 검증)

## 1. 이 문서의 목적

지금까지 병합된 크레이트(`assay-domain`, `assay-git`, `assay-classifier`,
`assay-semantic-diff`, `assay-github`, `assay-ai-evaluator`,
`assay-project-intelligence`, `assay-identity`, `assay-local`, `assay-cli`),
`web/`, `schemas/`가 함께 정합하게 동작하는지 실제로 배선된 경로에서 확인하고,
인터뷰 12장의 MVP 11개 항목을 구현됨 / 부분 / 미배선으로 정직하게 판정한다.

핵심 결론: 기반 마일스톤은 **계약(스키마)·도메인 로직·로컬 CLI 수직 슬라이스·
로컬 대시보드**를 완성했다. 반면 **호스팅 표면**(장기 실행 API/워커, PostgreSQL,
실제 AI 공급자 자격 증명 연결, 실제 GitHub 수집·클론, 공개 웹 제출→분석
파이프라인)은 인수인계 문서 8장에 따라 **의도적으로 연기**되었다. 따라서 공개 웹
관련 항목 대부분은 "계약과 도메인 로직 및 UI 셸은 있으나 라이브 백엔드에
배선되지 않음"(부분) 상태다.

## 2. 통합 검증 방법과 결과

### 2.1 추가한 크로스 컴포넌트 통합 테스트

`crates/assay-cli/tests/cross_component_integration.rs` (신규). 기존 단위
테스트의 중복이 아니라 crate 경계를 실제로 넘는 배선 정합을 검증한다.

1. `fresh_cli_output_is_schema_valid_bundle_and_byte_deterministic`
   - `assay project analyze`(실제 바이너리)로 결정론적 픽스처
     (`RepositoryScenario::TypeScriptProject`)를 분석한다.
   - 고정 시계(`ASSAY_TEST_FIXED_TIME`)에서 두 번 실행해 stdout이 바이트 단위로
     동일함을 확인한다(git → classifier → project-intelligence → cli 결정성).
   - 산출 JSON을 `project-analysis` 스키마로, 중첩 `manifest`를
     `analysis-manifest` 스키마로, 각 evidence fact를 `project-evidence`
     스키마로 검증하고 `validate_project_bundle_consistency`로 교차 불변식을
     확인한다.
2. `cli_evidence_flows_through_evaluator_domain_and_score_compiler`
   - CLI가 실제로 방출한 evidence ID를 추출해 `EvidenceBundle`을 구성하고
     `assay-ai-evaluator`의 `DeterministicFakeProvider`로 평가한다.
   - `ValidatedJudgmentSet` → `to_rubric_judgment_set()` →
     `assay-domain::RubricJudgmentSet` → `ScoreCompilerInput::compile()`
     → `to_machine_value()`로 `project-evaluation` 스키마 유효 출력을 만든다.
   - 모든 판정 인용이 CLI가 만든 evidence ID 집합의 부분집합임을 확인한다
     (실제 증거가 AI·컴파일러 사슬을 통과함).
   - 컴파일러가 기록한 `compiler.judgment_bundle_hash`가 평가에 사용한 번들
     해시와 일치함을 확인한다.
3. `full_chain_evaluation_is_deterministic_and_matches_committed_fixture`
   - 기계 독립적인 리터럴 ID로 같은 사슬을 구동해 2회 바이트 동일 결정성을
     확인하고, 산출물을 `tests/integration/produced/project-evaluation.json`에
     결정론적으로 고정한다(`ASSAY_BLESS_PRODUCED=1`로 재생성, 평상시에는 stale
     가드로 비교).
4. `record_history_round_trips_the_local_report_contract_over_serve`
   - `assay project analyze --record-history` → `assay serve --once` 왕복.
   - `/api/history/rec-000001` 응답이 로컬 report 계약과 일치함을 확인한다:
     `schema_version` 1.0.0, `visibility`/`privacy.visibility` `private_local`,
     `privacy.catalog_eligible` false, `privacy.external_transmission`
     `consent_required`, 두 섹션(`ai_evaluation`, `competitor_discovery`)
     `state: disabled`, 그리고 CLI가 만든 분석이 그대로 임베드됨.

`web/src/lib/contract/producer-output.test.ts` (신규). 위 3번이 고정한 **신선한
산출물**(골든이 아님)을 `web`의 계약 파서 `parseEvaluation`이 파싱할 수 있는지
Node 테스트로 확인한다. `evaluation_version`, `evaluator.rubric_version`,
Assay Score의 insufficient 상태와 Potential의 분리(`forecast_horizon`)를
검증한다.

### 2.2 실행한 검증과 결과

- `cargo fmt --check`: 통과.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: 통과.
- `cargo test --workspace`: `assay-git`의
  `rejects_malformed_or_multiline_commit_time`가 4-스레드 부하에서 간헐적으로
  `ProbeCapabilities/Io`로 실패(기존에 알려진 flaky, 이번 변경과 무관).
  `cargo test -p assay-git -- --test-threads=1` 재실행 시 14/14 통과. 나머지
  모든 크레이트는 통과.
- `RUSTFLAGS='-D unnameable-types' cargo check --workspace --all-targets
  --all-features`: 통과.
- `web`: `npm ci` 후 `type-check`, `lint`, `test`(58/58), `build` 모두 통과.

### 2.3 발견한 배선 상태

통합 검증에서 **수정이 필요한 배선 결함은 발견되지 않았다.** 코드 변경은 신규
통합 테스트, `assay-cli`의 dev-dependency 추가(`assay-ai-evaluator`,
`assay-domain`), 고정된 산출 픽스처뿐이다.

대신 아래의 **의도된 미배선 지점**을 확인했다. 이들은 결함이 아니라 기반
마일스톤에서 연기된 wiring이다.

- **evidence-manifest → evaluation-bundle 어댑터 부재.** CLI가 만드는
  `ProjectEvidenceManifest`(그리고 공개 `project-analysis` 번들)와 AI 평가기의
  입력 `EvidenceBundle` 사이를 잇는 변환이 워크스페이스에 없다. 위 통합 테스트는
  CLI가 실제로 방출한 evidence ID로 번들을 손수 조립해 사슬 정합을 증명하되,
  이 어댑터가 아직 없다는 사실을 명시적으로 남긴다.
- **AI 평가기·점수 컴파일러가 CLI에 배선되지 않음.** `assay capabilities`는
  `ai_evaluation`, `project_scores`를 `not_implemented`로 정직하게 보고한다.
  두 기능은 크레이트 수준 계약·로직으로 존재하고 통합 테스트로 조합 가능함이
  증명되었으나, `assay project analyze`는 증거 매니페스트까지만 산출한다.
- **호스팅 표면 부재.** 장기 실행 API/워커, PostgreSQL, 실제 GitHub 수집·클론,
  실제 AI 공급자 자격 증명 연결이 없다. `web`의 UI는 in-repo 계약 픽스처로
  구동되며 라이브 Rust API에 연결되어 있지 않다.

## 3. MVP 11개 항목 판정

| # | 항목 | 판정 |
| --- | --- | --- |
| 1 | GitHub 공개 저장소 웹 분석 | 부분 |
| 2 | 경과 시간·단계 진행 화면 | 부분 |
| 3 | Anonymous Engine 점수·차원·증거·상위 5 유사 프로젝트 | 부분 |
| 4 | 자동 공개 카탈로그·결과 URL·README 배지 | 부분 |
| 5 | hakhub.net 로그인·Authenticated Engine 비공개 미리보기 | 부분 |
| 6 | 엔진별 결과와 분석 이력 | 부분 |
| 7 | 로컬 GitHub 공개·비공개 저장소 분석 | 구현됨 |
| 8 | PAT 환경 변수·이미 clone한 저장소 지원 | 구현됨 |
| 9 | 로컬 브라우저 대시보드와 기록 보존 | 구현됨 |
| 10 | 비공개 저장소 AI 처리의 명시적 opt-in | 부분 |
| 11 | 관리자 삭제·복구·실패 단계 재실행·감사 로그 | 부분 |

### 3.1 항목별 상세

**1. GitHub 공개 저장소 웹 분석 — 부분.**
최소 GitHub URL 프런트엔드(commit `fe2f360`)와 공개 결과로의 라우트 게이팅
(`52afe9e`), 제출 쿨다운·다음 가능 시각 표시(`624fa41`)가 `web`에 구현되어
있으나 in-repo 계약 픽스처로 구동된다. GitHub 수집·증분 캐시 경계는
`assay-github`와 ADR 0004로 정의되어 있으나(`d34f2f5`, `aa12f16`, `6bab3ba`)
라이브 fetch에도, CLI/웹에도 배선되지 않았다. 제출→분석 라이브 파이프라인
(API/워커/DB)이 없다. 남은 것: GH-001, WEB-001의 라이브 배선.

**2. 경과 시간·단계 진행 화면 — 부분.**
run-state 계약과 `RunLifecycle`/`Stage` 상태 기계, 제한 재시도·관리자 복구가
`assay-project-intelligence`와 ADR 0011로 구현되어 있고(`feab7fc`, `b8f3160`,
테스트 `crates/assay-project-intelligence/tests/run_state.rs`), `web`에도 단계
렌더링(`src/lib/state/stages.test.ts`)이 있다. 진행 상태를 방출하는 라이브
워커가 없다. 남은 것: OPS-001의 라이브 배선.

**3. Anonymous Engine 점수·차원·증거·상위 5 유사 — 부분.**
결정론적 점수 컴파일러와 `project-evaluation` 계약(ADR 0007, `8ae1a54`,
`score_compiler.rs`, 본 카드의 통합 테스트), 1-depth 코호트 탐색과
`project-comparison` 계약(ADR 0008, `d843bce`, `a2edf4e`, `261658a`,
`comparison.rs`), 증거 매니페스트(`project-evidence`)가 구현되어 있다. 공개
숫자형 Assay Score는 충분성·보정 게이트 뒤에 있어 기본적으로 insufficient로
유지된다(`score_release_gate_not_met`). Anonymous/Authenticated 두 엔진 프로필은
`EvaluatorDescriptor`로 표현되나 두 개의 라이브 스냅샷으로 제공되지는 않는다.

**4. 자동 공개 카탈로그·결과 URL·README 배지 — 부분.**
카탈로그·유사 프로젝트 비교·README SVG 배지 UI가 구현되어 있고(`8bd499a`,
`web/src/lib/badge/badge.test.ts`) 공개 라우트 게이팅(`52afe9e`)이 있으나 모두
픽스처 기반이다. 자동 공개 발행·영속화·14일 재분석 쿨다운(CAT-001)은
배선되지 않았다.

**5. hakhub.net 로그인·Authenticated Engine 비공개 미리보기 — 부분.**
공급자 비종속 신원 경계 크레이트가 ADR 0009로 구현되어 있다(`3218761`,
`0648ebb`, 테스트 `single_issuer`, `admin_mapping`, `session`,
`token_validation`, `entitlements`). hakhub.net 단일 issuer 배포 바인딩
(IAM-001)과 Authenticated Engine 비공개 미리보기는 세션을 발급하는 라이브 API가
없어 배선되지 않았다.

**6. 엔진별 결과와 분석 이력 — 부분.**
로컬 불변 이력 저장소와 loopback `serve`가 ADR 0010으로 구현·검증되어 있고
(`9c9135e`, `crates/assay-local/tests/loopback_dashboard.rs`,
`crates/assay-cli/tests/local_dashboard.rs`, 본 카드의 왕복 테스트) 평가에
evaluator/engine 서술자가 기록된다. 호스팅 엔진별 이력과 계정·저장소별 쿨다운은
연기되었다.

**7. 로컬 GitHub 공개·비공개 저장소 분석 — 구현됨.**
`assay project analyze <local-repository>` 결정론적 수직 슬라이스가 구현되어
있다(`crates/assay-cli/tests/foundation_vertical_slice.rs`, `cli_contract.rs`).
이미 clone한 공개·비공개 저장소를 네트워크 없이 분석하며, 비공개 소스의 AI
평가와 공개 경쟁 프로젝트 탐색은 기본 비활성이다.

**8. PAT 환경 변수·이미 clone한 저장소 지원 — 구현됨.**
`--github-token-env VAR`는 변수 **이름**만 받고 값을 인수·로그·결과·기록에
넣지 않는다(`crates/assay-local/tests/token_non_exposure.rs`, `local_dashboard`
의 planted-token 검증). 잘못된 변수명은 분석 전에 exit code 2로 거부된다.

**9. 로컬 브라우저 대시보드와 기록 보존 — 구현됨.**
`assay serve`가 `127.0.0.1`에만 바인딩하고 불변 `LocalHistoryStore`를
렌더링한다(ADR 0010, `9c9135e`, `fa31432`). 요청 라인 크기·읽기 타임아웃 경계가
설정되어 있고, 왕복이 테스트로 확인된다.

**10. 비공개 저장소 AI 처리의 명시적 opt-in — 부분.**
동의 모델(`ConsentState`/`SectionReport`, `external_transmission`
`consent_required`, AI·경쟁 탐색 기본 disabled — `crates/assay-local/src/report.rs`,
본 카드의 왕복 테스트)과 AI 평가기 프라이버시 경계(증거 scope·transmission,
동의 강제, PublicOnly 코퍼스 제외 — `evaluator_contract.rs`)가 구현·강제된다.
다만 opt-in이 게이팅하는 AI 평가 자체가 CLI/로컬 흐름에 배선되지 않았고
(`ai_evaluation: not_implemented`), OpenAI 어댑터는 자격 증명·전송 포트 뒤에
존재하나(ADR 0006, `a41d3ca`) 라이브 서비스에서 자격 증명으로 구동되지 않는다.

**11. 관리자 삭제·복구·실패 단계 재실행·감사 로그 — 부분.**
로컬 `assay history delete|restore|purge`(`LocalAdministrator`)가 구현·검증되어
있다(`local_dashboard.rs`). 실패 단계 재실행과 관리자 감사(`AdminAction`,
`AdminAuditEvent`)는 ADR 0011의 run-stage 모델로 결정론적으로 표현되나
(`run_state.rs`) 라이브 워커/오케스트레이터로 구동되지 않는다. 실패 단계 재실행이
라이브 파이프라인으로 노출되지 않았다.

## 4. 기반 마일스톤에서 의도적으로 연기한 사실

다음은 인수인계 문서 8장에 따라 기반 마일스톤에서 시작하지 않은 항목이며, 위
"부분"·"미배선" 판정의 원인이다. 이는 증거·스키마 기반을 먼저 굳히기 위한
의도적 연기다.

- PostgreSQL 영속성과 마이그레이션.
- 장기 실행 API 및 워커 배포와 백그라운드 작업 큐.
- 공개 숫자형 Assay Score 또는 Potential의 무조건 공개(충분성·보정 게이트 전).
- 실제 AI 공급자(OpenAI API 등) 자격 증명 연결과 라이브 호출.
- 실제 GitHub 공개·비공개 저장소 수집과 클론.
- hakhub.net OIDC 멤버십과 비공개 미리보기의 라이브 세션 발급.
- 공개 카탈로그 자동 발행·영속화, 예약 재스캔.

첫 공개 MVP 범위 자체(공개 웹 분석, 두 엔진 프로필, 카탈로그, 자동 비교,
README 배지, hakhub.net 인증, 로컬 비공개 GitHub 분석, 로컬 대시보드)는 여전히
목표이며, 위 연기 항목이 배선될 때 "부분" 항목들이 "구현됨"으로 승격된다.

## 5. 남은 경고·미결

- `assay-git`의 `rejects_malformed_or_multiline_commit_time`는 4-스레드 부하에서
  `ProbeCapabilities/Io`로 간헐 실패한다(이번 변경과 무관). 단일 스레드
  재실행으로 안정 통과. 근본 원인(부하 시 capability probe I/O)은 별도 추적
  대상이다.
- `tests/integration/produced/project-evaluation.json`은 리뷰된 골든이 아니라
  결정론적 **산출 픽스처**다. 컴파일러 산출이 바뀌면 stale 가드가 실패하며,
  의미론을 검토한 뒤에만 `ASSAY_BLESS_PRODUCED=1`로 재생성한다.
- `assay-cli`에 dev-dependency로 `assay-ai-evaluator`, `assay-domain`을 추가했다
  (프로덕션 의존성 방향에는 영향 없음, 통합 테스트 전용).
- `web`의 계약 파서는 `project-evaluation`, `project-evidence`,
  `project-comparison`만 지원한다. `project-analysis`, `run-state`,
  `analysis-manifest`, `capabilities`, `ai-judgment`는 TS 타입·파서가 없어 현재
  Rust 측 스키마 검증으로만 다룬다.
