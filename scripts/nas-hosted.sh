#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ENV_FILE="$ROOT/.env.production"
WEB_IMAGE_OVERRIDE=${ASSAY_WEB_IMAGE_OVERRIDE:-}
API_IMAGE_OVERRIDE=${ASSAY_API_IMAGE_OVERRIDE:-}
WORKER_IMAGE_OVERRIDE=${ASSAY_WORKER_IMAGE_OVERRIDE:-}

if [ "${1:-}" = "--env-file" ]; then
  [ "$#" -ge 3 ] || {
    echo "usage: $0 [--env-file PATH] {init|backup|deploy|verify|restore} [args]" >&2
    exit 64
  }
  ENV_FILE=$2
  shift 2
fi

COMMAND=${1:-}
[ -n "$COMMAND" ] || {
  echo "usage: $0 [--env-file PATH] {init|backup|deploy|verify|restore} [args]" >&2
  exit 64
}
shift

[ -f "$ENV_FILE" ] || {
  echo "NAS-local environment file not found: $ENV_FILE" >&2
  exit 66
}
if find "$ENV_FILE" -prune -perm /077 -print -quit 2>/dev/null | grep -q .; then
  echo "refusing group/world-readable environment file; run chmod 600 '$ENV_FILE'" >&2
  exit 77
fi

set -a
# The environment file is operator-owned configuration and must contain shell-safe assignments.
# shellcheck disable=SC1090
. "$ENV_FILE"
set +a

[ -z "$WEB_IMAGE_OVERRIDE" ] || ASSAY_WEB_IMAGE=$WEB_IMAGE_OVERRIDE
[ -z "$API_IMAGE_OVERRIDE" ] || ASSAY_API_IMAGE=$API_IMAGE_OVERRIDE
[ -z "$WORKER_IMAGE_OVERRIDE" ] || ASSAY_WORKER_IMAGE=$WORKER_IMAGE_OVERRIDE
export ASSAY_WEB_IMAGE ASSAY_API_IMAGE ASSAY_WORKER_IMAGE

: "${ASSAY_POSTGRES_VOLUME:?set ASSAY_POSTGRES_VOLUME in the NAS-local environment file}"
: "${ASSAY_WEB_VOLUME:?set ASSAY_WEB_VOLUME in the NAS-local environment file}"
: "${ASSAY_WEB_IMAGE:?set ASSAY_WEB_IMAGE to an immutable digest reference}"
: "${ASSAY_API_IMAGE:?set ASSAY_API_IMAGE to an immutable digest reference}"
: "${ASSAY_WORKER_IMAGE:?set ASSAY_WORKER_IMAGE to an immutable digest reference}"
: "${POSTGRES_PASSWORD:?set POSTGRES_PASSWORD in the NAS-local environment file}"
: "${DATABASE_URL:?set DATABASE_URL in the NAS-local environment file}"

POSTGRES_DB=${POSTGRES_DB:-assay}
POSTGRES_USER=${POSTGRES_USER:-assay}
SUPPORTED_POSTGRES_IMAGE=postgres:18-alpine
ASSAY_POSTGRES_IMAGE=${ASSAY_POSTGRES_IMAGE:-$SUPPORTED_POSTGRES_IMAGE}
ASSAY_WEB_PORT=${ASSAY_WEB_PORT:-1019}
ASSAY_BACKUP_DIR=${ASSAY_BACKUP_DIR:-"$ROOT/backups/postgres"}
EXPECTED_POSTGRES_MAJOR=18

validate_name() {
  case "$1" in
    ""|*[!A-Za-z0-9_.-]*)
      echo "unsafe Docker resource name: $1" >&2
      exit 65
      ;;
  esac
}

validate_digest_reference() {
  reference=$1
  label=$2
  case "$reference" in
    *@sha256:*) digest=${reference##*@sha256:} ;;
    *)
      echo "$label must be an immutable registry digest reference" >&2
      exit 65
      ;;
  esac
  case "$digest" in
    *[!0-9a-f]*)
      echo "$label must contain a lowercase SHA-256 digest" >&2
      exit 65
      ;;
  esac
  [ "${#digest}" -eq 64 ] || {
    echo "$label must contain a complete SHA-256 digest" >&2
    exit 65
  }
}

[ "$ASSAY_POSTGRES_IMAGE" = "$SUPPORTED_POSTGRES_IMAGE" ] || {
  echo "unsupported PostgreSQL image; expected $SUPPORTED_POSTGRES_IMAGE" >&2
  exit 78
}

validate_name "$ASSAY_POSTGRES_VOLUME"
validate_name "$ASSAY_WEB_VOLUME"
validate_digest_reference "$ASSAY_WEB_IMAGE" ASSAY_WEB_IMAGE
validate_digest_reference "$ASSAY_API_IMAGE" ASSAY_API_IMAGE
validate_digest_reference "$ASSAY_WORKER_IMAGE" ASSAY_WORKER_IMAGE

compose() {
  docker compose --project-name assay-prod --env-file "$ENV_FILE" -f "$ROOT/compose.yaml" "$@"
}

postgres_major() {
  docker run --rm --volume "$ASSAY_POSTGRES_VOLUME:/var/lib/postgresql:ro" \
    "$ASSAY_POSTGRES_IMAGE" \
    sh -c 'if [ -f /var/lib/postgresql/data/PG_VERSION ]; then cat /var/lib/postgresql/data/PG_VERSION; fi'
}

validate_external_volumes() {
  for volume in "$ASSAY_POSTGRES_VOLUME" "$ASSAY_WEB_VOLUME"; do
    docker volume inspect "$volume" >/dev/null 2>&1 || {
      echo "required external volume is missing: $volume" >&2
      echo "run the explicit init command for a first installation" >&2
      exit 69
    }
  done
}

validate_database_major() {
  actual_major=$(postgres_major)
  if [ -n "$actual_major" ] && [ "$actual_major" != "$EXPECTED_POSTGRES_MAJOR" ]; then
    echo "PostgreSQL volume major is $actual_major; expected $EXPECTED_POSTGRES_MAJOR" >&2
    echo "restore into a fresh volume or perform an explicit PostgreSQL major upgrade" >&2
    exit 78
  fi
}

initialize() {
  for volume in "$ASSAY_POSTGRES_VOLUME" "$ASSAY_WEB_VOLUME"; do
    if ! docker volume inspect "$volume" >/dev/null 2>&1; then
      docker volume create "$volume" >/dev/null
      echo "created external volume $volume"
    fi
  done
  validate_external_volumes
  validate_database_major
}

backup() {
  validate_external_volumes
  validate_database_major
  container=$(compose ps -q db)
  [ -n "$container" ] && [ "$(docker inspect -f '{{.State.Running}}' "$container")" = "true" ] || {
    echo "database service is not running; no backup was created" >&2
    exit 69
  }

  umask 077
  mkdir -p "$ASSAY_BACKUP_DIR"
  stamp=$(date -u +%Y%m%dT%H%M%SZ)
  final="$ASSAY_BACKUP_DIR/assay-$stamp.dump"
  temporary="$final.partial"
  [ ! -e "$final" ] && [ ! -e "$temporary" ] || {
    echo "backup target already exists: $final" >&2
    exit 73
  }
  trap 'rm -f "$temporary"' 0
  trap 'exit 129' 1
  trap 'exit 130' 2
  trap 'exit 143' 15

  compose exec -T db pg_dump \
    --format=custom --compress=9 --no-owner --no-acl \
    --username "$POSTGRES_USER" "$POSTGRES_DB" >"$temporary"
  [ -s "$temporary" ] || {
    echo "backup is empty" >&2
    exit 74
  }
  compose exec -T db pg_restore --list <"$temporary" >/dev/null
  mv "$temporary" "$final"
  trap - 0 1 2 15
  printf '%s\n' "$final"
}

verify() {
  validate_external_volumes
  validate_database_major
  for service in db api worker web; do
    container=$(compose ps -q "$service")
    [ -n "$container" ] || {
      echo "$service container is missing" >&2
      exit 69
    }
    [ "$(docker inspect -f '{{.State.Running}}' "$container")" = "true" ] || {
      echo "$service container is not running" >&2
      exit 69
    }
  done

  compose exec -T db pg_isready --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" >/dev/null
  compose exec -T db psql --no-psqlrc --set ON_ERROR_STOP=1 \
    --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
    --command 'SELECT count(*) FROM _sqlx_migrations;' >/dev/null
  compose exec -T api curl --fail --silent http://127.0.0.1:8080/health/ready >/dev/null
  compose exec -T web node -e \
    "fetch('http://api:8080/health/ready').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
  curl --fail --show-error --silent "http://127.0.0.1:$ASSAY_WEB_PORT/" >/dev/null
  echo "production stack verification passed"
}

deploy() {
  validate_external_volumes
  validate_database_major
  existing=$(compose ps -q db)
  if [ -n "$(postgres_major)" ] && {
    [ -z "$existing" ] || [ "$(docker inspect -f '{{.State.Running}}' "$existing")" != "true" ];
  }; then
    compose up -d --no-deps --wait db
    existing=$(compose ps -q db)
  fi
  if [ -n "$existing" ] && [ "$(docker inspect -f '{{.State.Running}}' "$existing")" = "true" ]; then
    backup_path=$(backup)
    echo "pre-deploy backup verified: $backup_path"
  fi
  compose pull
  compose up -d --remove-orphans --wait
  verify
}

restore() {
  [ "$#" -eq 2 ] || {
    echo "usage: $0 [--env-file PATH] restore BACKUP NEW_VOLUME" >&2
    exit 64
  }
  backup_file=$1
  new_volume=$2
  [ -f "$backup_file" ] && [ -s "$backup_file" ] || {
    echo "backup file is missing or empty" >&2
    exit 66
  }
  validate_name "$new_volume"
  [ "$new_volume" != "$ASSAY_POSTGRES_VOLUME" ] || {
    echo "restore must target a new volume, never the active database volume" >&2
    exit 65
  }
  docker run --rm -i "$ASSAY_POSTGRES_IMAGE" pg_restore --list <"$backup_file" >/dev/null

  if docker volume inspect "$new_volume" >/dev/null 2>&1; then
    populated=$(docker run --rm --volume "$new_volume:/restore:ro" \
      "$ASSAY_POSTGRES_IMAGE" \
      sh -c 'find /restore -mindepth 1 -maxdepth 1 -print -quit')
    [ -z "$populated" ] || {
      echo "restore target volume is not empty: $new_volume" >&2
      exit 73
    }
  else
    docker volume create "$new_volume" >/dev/null
  fi

  restore_name="assay-restore-$(date -u +%Y%m%d%H%M%S)-$$"
  cleanup_restore() {
    docker rm -f "$restore_name" >/dev/null 2>&1 || true
  }
  trap cleanup_restore 0
  trap 'exit 129' 1
  trap 'exit 130' 2
  trap 'exit 143' 15
  docker run -d --name "$restore_name" \
    --env POSTGRES_DB --env POSTGRES_USER --env POSTGRES_PASSWORD \
    --volume "$new_volume:/var/lib/postgresql" \
    "$ASSAY_POSTGRES_IMAGE" >/dev/null

  attempt=0
  until docker exec "$restore_name" pg_isready \
    --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 60 ] || {
      echo "restored PostgreSQL instance did not become ready" >&2
      exit 70
    }
    sleep 2
  done

  docker exec -i "$restore_name" pg_restore \
    --exit-on-error --no-owner --no-acl \
    --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <"$backup_file"
  docker exec "$restore_name" psql --no-psqlrc --set ON_ERROR_STOP=1 \
    --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
    --command 'SELECT count(*) FROM _sqlx_migrations;' >/dev/null

  cleanup_restore
  trap - 0 1 2 15
  echo "restore verified in external volume $new_volume"
  echo "set ASSAY_POSTGRES_VOLUME=$new_volume, then run deploy"
}

case "$COMMAND" in
  init) [ "$#" -eq 0 ] || exit 64; initialize ;;
  backup) [ "$#" -eq 0 ] || exit 64; backup ;;
  deploy) [ "$#" -eq 0 ] || exit 64; deploy ;;
  verify) [ "$#" -eq 0 ] || exit 64; verify ;;
  restore) restore "$@" ;;
  *)
    echo "unknown command: $COMMAND" >&2
    exit 64
    ;;
esac
