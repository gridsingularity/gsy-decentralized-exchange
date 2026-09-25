#!/usr/bin/env bash
# run-integration-tests.sh
set -uo pipefail   # no -e

COMPOSE_FILE="docker-compose.integration.yml"

SERVICES=(
  gsy-listener-test
  gsy-offchain-storage-integration-test
  gsy-analytics-engine-integration-test
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