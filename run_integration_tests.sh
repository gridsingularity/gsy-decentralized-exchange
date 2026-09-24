#!/usr/bin/env bash
# run_integration_tests.sh
set -uo pipefail   # no -e

COMPOSE_FILE="docker-compose.integration.yml"
BASE_IMAGE="ghcr.io/gridsingularity/gsy-rust-base:latest"

# Every Rust service builds FROM the base image: use a local copy if present,
# otherwise pull it (CI), otherwise build it locally (no GHCR access).
if ! docker image inspect "$BASE_IMAGE" >/dev/null 2>&1; then
  echo "==> $BASE_IMAGE not found locally, trying to pull"
  if ! docker pull "$BASE_IMAGE"; then
    echo "==> Pull failed, building $BASE_IMAGE from Dockerfile.base"
    docker build -f Dockerfile.base -t "$BASE_IMAGE" . || exit 1
  fi
fi

SERVICES=(
  gsy-listener-test
  gsy-offchain-storage-integration-test
  gsy-market-orchestrator-integration-test
  gsy-matching-engine-integration-test
  gsy-execution-engine-integration-test
  gsy-community-client-integration-test
  gsy-primitives-integration-test
  gsy-contracts-tests
)

cleanup() {
  docker compose -f "$COMPOSE_FILE" down -v
}
trap cleanup EXIT

passed=()
failed=()

for svc in "${SERVICES[@]}"; do
  echo "==> $svc"
  start=$SECONDS
  if docker compose -f "$COMPOSE_FILE" run --rm --build "$svc"; then
    passed+=("$svc $((SECONDS - start))s")
  else
    failed+=("$svc $((SECONDS - start))s")
  fi
done

echo
echo "===================== Summary ====================="

if [ ${#passed[@]} -ne 0 ]; then
  echo "Passed (${#passed[@]}):"
  printf '  ✓ %s\n' "${passed[@]}"
fi

if [ ${#failed[@]} -ne 0 ]; then
  echo "Failed (${#failed[@]}):"
  printf '  ✗ %s\n' "${failed[@]}"
fi
echo "==================================================="

if [ -n "${GITHUB_ACTIONS:-}" ]; then
  {
    echo "### Integration test results"
    printf -- '- ✓ %s\n' "${passed[@]}"
    [ ${#failed[@]} -ne 0 ] && printf -- '- ✗ %s\n' "${failed[@]}"
  } >> "$GITHUB_STEP_SUMMARY"
fi

[ ${#failed[@]} -eq 0 ] || exit 1