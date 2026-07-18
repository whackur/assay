# Hosted repository ingestion handoff

이 문서는 2026-07-18에 시작한 hosted repository ingestion 작업을 다른 세션에서 바로 이어가기 위한 인계 자료다. 구현과 자동 검증을 마치고 `develop` 전달 단위로 정리했다.

## 현재 결과

- PostgreSQL에 GitHub 관찰값, source snapshot, evaluation provenance, durable job 상태를 보존한다.
- `assay-api`가 제출 및 상태 조회 HTTP 경계를 제공하고, `assay-worker`가 공유 workflow policy를 실행한다.
- GitHub 수집은 `assay-github`, Ollama-compatible transport는 `assay-ai-evaluator`, hosted contract와 provider-independent workflow는 `assay-project-intelligence`가 소유한다.
- web은 fixture 추정값 대신 hosted API의 source-processing 상태를 조회하고 terminal state까지 polling한다.
- production `compose.yaml`은 PostgreSQL 17, API, worker, web을 실행하도록 확장됐다.
- NAS의 PostgreSQL 데이터는 Compose project/directory와 무관한 external Docker volume으로 유지된다.

## 보안 경계

- 실제 token, password, Ollama key, NAS host/IP/path/user 값은 저장소에 넣지 않는다.
- `.env.example`에는 변수 이름과 빈 placeholder만 둔다.
- 배포 값은 GitHub Actions secrets/variables와 NAS-local `.env`에서만 주입한다.
- 공개 제출 폼은 provider credential을 받지 않는다.
- provider 결과는 현재 `validated_unpublished`까지만 보존하며 score나 provider prose를 공개하지 않는다.

## 운영 경로

| 목적 | 파일 |
| --- | --- |
| production stack | `compose.yaml` |
| local hosted stack | `compose.hosted.yaml` |
| hosted image build | `Dockerfile.hosted` |
| NAS lifecycle helper | `scripts/nas-hosted.sh` |
| Synology runbook | `docs/deployment/synology.md` |
| local runbook | `docs/development/hosted-local.md` |
| persistence ADR | `docs/architecture/0013-hosted-postgresql-source-history.md` |

`scripts/nas-hosted.sh init`만 external volume을 명시적으로 생성한다. 일반 `deploy`는 설정한 volume이 없으면 종료 코드 `69`로 실패하며 새 빈 DB를 자동 생성하지 않는다. backup, restore-to-fresh-volume, PostgreSQL 17 image/volume major-version guard 및 상태 검증도 제공한다. 활성 DB volume을 직접 덮어쓰거나 자동 삭제하지 않는다.

production application image는 tag가 아니라 `repository@sha256:<digest>` 형식만 허용한다. SSH 배포는 fingerprint가 일치하는 host-key 행만 `known_hosts`에 저장한다. Ollama secret이 설정된 경우 endpoint는 HTTPS여야 한다.

## 검증 상태

통과한 항목:

- `cargo fmt --check`
- workspace Clippy with `-D warnings`
- hosted contract/workflow 및 관련 Rust package tests
- web contract generation, lint, type-check, 119 tests, production build
- production/local Compose configuration validation
- deployment workflow YAML parsing
- `bash -n scripts/nas-hosted.sh`
- Markdown lint, `git diff --check`, 정규식 기반 secret/private-infrastructure scan
- 최종 correction focused checks: Ollama 5 tests, GitHub hosted 2 tests, storage 8 tests, hosted workflow 2 tests
- fake-Docker checks: missing deploy volume exit `69`, deploy의 volume 미생성, PostgreSQL 18 override exit `78`

주의할 항목:

- 최종 검증에서 workspace format, Clippy, tests와 web 전체 suite가 통과했다. Linux CI 결과는 push 이후 별도로 확인해야 한다.
- Docker image build, 실제 PostgreSQL migration, GitHub collection, Ollama call은 실행하지 않았다.
- backup/restore는 disposable Docker volume이나 NAS에서 실제 rehearsal하지 않았다.
- GitHub Actions 및 SSH deployment는 syntax만 검증했다.
- 실제 NAS와 외부 provider를 사용하는 runtime rehearsal은 자동 검증 범위에 포함하지 않았다.

## 리뷰 상태

- `assay-hosted-foundation-v1`: terminal `escalated`.
- `assay-hosted-expanded-v2`: 수정 13개는 검증됐지만 production NAS integration과 project-intelligence ownership 4개가 범위 밖이어서 terminal `escalated`.
- `assay-hosted-final-v3`: severe finding 10개를 단일 correction batch로 수정했고 scoped validation과 independent final verification이 승인됐다.
- 이후 native gate가 재현 가능한 ledger artifact binding 부재로 receipt를 `invalidated` 처리했다. 이는 제품 코드 결함이 아니라 리뷰 메타데이터 문제다.
- receipt 재생성 목적의 V4는 사용자 요청으로 중단했다. 추가 리뷰 없이 검증된 제품 tree를 기능 단위 commit으로 `develop`에 전달한다.

## 다음 세션 체크리스트

- [x] 현재 전체 tree 대상으로 `assay-hosted-final-v3` bounded review lineage를 시작한다.
- [x] corroborated severe finding 10개를 단일 correction batch로 수정한다.
- [x] `assay-hosted-final-v3` scoped fix validation과 independent final verification을 완료한다.
- [ ] 가능하면 disposable Linux Docker 환경에서 image build와 PostgreSQL volume persistence/backup/restore를 rehearsal한다.
- [ ] 실제 GitHub collection과 Ollama-compatible endpoint 호출을 trusted environment에서 rehearsal한다.
- [ ] `origin/develop` CI 결과와 배포 환경의 secret 주입 상태를 확인한다.

## 권장 commit 경계

1. `949ddab feat(hosted): add durable repository ingestion workflow`
2. `793d698 feat(web): render live repository processing status`
3. `3a8c7cc feat(deploy): add persistent hosted NAS stack`
4. `docs(hosted): streamline project and session handoff`

각 commit에는 해당 동작의 테스트와 문서를 함께 포함한다. file type별로 나누지 않는다.
