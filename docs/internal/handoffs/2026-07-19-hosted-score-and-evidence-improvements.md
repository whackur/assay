# Hosted score and evidence improvements handoff

## 결론

현재 hosted workflow는 GitHub 메타데이터를 근거로 AI 판단을 저장하지만, 결정론적 점수 컴파일러를 호출하지 않는다. 따라서 성공한 평가도 `validated_unpublished`와 `score_status: unavailable`로 끝난다.

다음 작업은 **점수 컴파일 연결**과 **제한된 공개 증거 확장**을 별도 단계로 구현해야 한다. AI 분석의 공개는 계속 사람의 명시적 승인을 요구해야 하며, 점수 정책 변경과 증거 수집 변경을 한 PR에 섞지 않는다.

## 현재 상태

| 항목 | 상태 | 근거 |
| --- | --- | --- |
| Hosted AI 입력 | GitHub 메타데이터만 사용 | `crates/assay-ai-evaluator/src/ollama/metadata.rs` |
| README, 설명, 소스, 트리 | AI 입력에 없음 | `crates/assay-github/src/hosted.rs`, `crates/assay-project-intelligence/src/hosted_workflow/mod.rs` |
| 결정론적 점수 컴파일러 | 구현됨 | `crates/assay-project-intelligence/src/scoring/` |
| Hosted worker의 점수 컴파일 호출 | 없음 | `apps/assay-worker/src/main.rs` |
| 성공한 hosted 평가의 점수 상태 | 명시적으로 `unavailable` | `crates/assay-storage/src/evaluation.rs` |
| AI 분석 공개 | 사람 승인 필요 | `docs/architecture/0017-public-ai-analysis-approval.md` |

Hermes (`nousresearch/hermes-agent`)는 이 경로를 통해 유효한 AI 판단을 얻었지만, 메타데이터 외의 근거가 없어 판단별 신뢰도가 낮았고 숫자 점수는 산출되지 않았다. 이는 프로젝트 품질의 결론이 아니라 현재 입력 범위와 orchestration의 한계다.

## 권장 작업 순서

### 1. Hosted score compilation을 연결한다

**목표:** 검증된 rubric judgment를 결정론적 컴파일러에 전달하고, 점수·confidence·limitations·provenance를 별도 파생 결과로 저장한다.

1. `HostedWorkflow`가 평가 저장 후 `ScoreCompiler`를 호출할 수 있는 의존성 경계를 추가한다.
2. 컴파일 결과를 원본 AI judgment와 분리된 파생 레코드로 저장한다.
3. `hosted_source_status.score_status`를 컴파일 결과에 따라 갱신한다. 증거가 부족하면 `insufficient` 또는 `unavailable`을 유지하며 0으로 바꾸지 않는다.
4. public project contract가 수치 점수, confidence, status, analysis version, rule-set hash, evidence provenance를 함께 노출하는지 확인한다.
5. 점수 공개 정책은 AI rationale 공개 승인과 분리한다. rationale은 계속 승인 전까지 비공개다.

### 2. 최소한의 구조·문서 증거를 추가한다

**목표:** 공개 저장소의 README와 트리에서 구조화된 사실만 만들고, 원본 소스나 raw diff를 PostgreSQL에 저장하지 않는다.

권장 순서:

1. README를 제한된 크기로 수집해 문서 존재·길이·섹션·지원 문서 링크 같은 추출 사실만 만든다.
2. 이미 존재하는 `GitHubCollector::stream_tree`를 제한된 깊이·파일 수·바이트 예산으로 hosted workflow에 연결한다.
3. 트리에서 언어 구성, 테스트/문서/CI/패키지 manifest 존재, 디렉터리 경계 같은 구조 사실을 추출한다.
4. 모든 사실에 source revision, 수집 한계, provenance, availability를 붙인다.
5. AI에는 검증 가능한 구조화된 사실만 전달한다. README 원문·소스 blob·raw diff의 장기 저장 또는 무제한 전송은 추가하지 않는다.

## 구현 경계

| 책임 | 위치 |
| --- | --- |
| GitHub README·트리 수집 | `crates/assay-github` |
| 공개 파일/구조 분류 | `crates/assay-classifier` |
| 원시 사실과 provenance | `crates/assay-project-intelligence` 또는 기존 domain contract |
| AI evidence bundle 구성과 검증 | `crates/assay-ai-evaluator` |
| 점수 계산 | `crates/assay-metrics` 또는 기존 `crates/assay-project-intelligence/src/scoring` 경계 검토 |
| 파생 결과 저장 | `crates/assay-storage` |
| orchestration | `apps/assay-worker`와 thin hosted workflow adapter |

`assay-domain`에는 데이터베이스, HTTP, GitHub, UI 의존성을 추가하지 않는다.

## 테스트와 검증

먼저 작은 redistributable fixture를 만든다. 최소 fixture는 README, 제한된 트리, manifest, CI 파일, 그리고 누락된 증거 상태를 포함해야 한다.

- [ ] 메타데이터만 있는 경우 숫자 점수를 만들지 않고 limitation을 유지한다.
- [ ] README·트리 증거가 있어 essential dimension이 충족되면 deterministic compiler가 provisional 또는 complete 결과를 만든다.
- [ ] 누락된 증거는 0이 아닌 `unavailable` 또는 `insufficient`로 남는다.
- [ ] AI가 인용하는 evidence ID는 실제 bundle에 존재한다.
- [ ] 원본 README·소스·diff가 PostgreSQL에 저장되지 않는다.
- [ ] 새 hosted 결과가 이전 AI rationale 승인을 자동으로 재사용하지 않는다.
- [ ] CLI/API JSON schema와 golden fixture를 의도적으로 갱신한다.

변경 후 다음을 실행한다.

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
pnpm --dir web lint
pnpm --dir web type-check
pnpm --dir web test
pnpm --dir web build
```

## 명시적 비범위

- 사람 단위 점수, leaderboard, 생산성 또는 보상 신호
- private source의 기본 수집 또는 외부 AI 제공자 전송
- raw source blob·README 전문·raw diff의 PostgreSQL 보관
- AI가 만든 prose를 점수 컴파일러에 직접 전달하는 것
- 승인되지 않은 AI rationale의 공개

## 다음 작업

첫 PR은 evidence 범위를 바꾸지 않고, 기존 validated judgment를 deterministic score compiler로 연결하는 수직 슬라이스로 제한한다. 두 번째 PR에서 bounded README·tree evidence와 해당 fixture를 추가한다.
