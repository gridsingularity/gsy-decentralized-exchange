#!/usr/bin/env bash
set -euo pipefail

target="${1:-local}"
action="${2:-deploy}"
compose_file="docker-compose.contracts.yml"
profile=""
service=""

case "$target" in
  local)
    profile="local-contracts"
    case "$action" in
      deploy) service="gsy-contracts-deploy-local" ;;
      gas-report) service="gsy-contracts-gas-report-local" ;;
      *) echo "Unknown action '$action'. Use deploy or gas-report." >&2; exit 2 ;;
    esac
    ;;
  volta)
    export CONTRACTS_HARDHAT_NETWORK="volta"
    profile="remote-contracts"
    case "$action" in
      deploy) service="gsy-contracts-deploy-remote" ;;
      gas-report) service="gsy-contracts-gas-report-remote" ;;
      *) echo "Unknown action '$action'. Use deploy or gas-report." >&2; exit 2 ;;
    esac
    ;;
  ewc)
    export CONTRACTS_HARDHAT_NETWORK="ewc"
    profile="remote-contracts"
    case "$action" in
      deploy) service="gsy-contracts-deploy-remote" ;;
      gas-report) service="gsy-contracts-gas-report-remote" ;;
      *) echo "Unknown action '$action'. Use deploy or gas-report." >&2; exit 2 ;;
    esac
    ;;
  *)
    echo "Unknown target '$target'. Use local, volta, or ewc." >&2
    exit 2
    ;;
esac

if [ "$target" = "local" ]; then
  docker compose -f "$compose_file" --profile "$profile" up -d --build anvil
fi

docker compose -f "$compose_file" --profile "$profile" build "$service"
docker compose -f "$compose_file" --profile "$profile" run --rm "$service"
