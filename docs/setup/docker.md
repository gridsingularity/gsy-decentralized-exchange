# Docker

## Compose Topology

Contract deployment and local chain startup are handled by
`docker-compose.contracts.yml`. The application and e2e compose files consume
the generated contract addresses and connect to the already-running target EVM.

Recommended local sequence:

1. Run `./scripts/contracts.sh local deploy`.
2. Keep the local Anvil container running.
3. Start `docker-compose.yml` or `docker-compose.test.yml` with
   `--env-file contracts-output/addresses.env`.
4. `gsy-offchain-storage` subscribes to chain events and exposes APIs.

For remote Energy Web Chain or Volta deployments, see
[Contract Deployment and Gas Reports](contracts.md).

## Main Commands

```bash
# build all images
docker compose build

# deploy local contracts first
./scripts/contracts.sh local deploy

# run core stack against deployed local contracts
docker compose --env-file contracts-output/addresses.env up

# run and rebuild
docker compose --env-file contracts-output/addresses.env up --build

# stop
docker compose down
```

## EWDS Client Gateway Against EWF

Use `docker-compose.ewds.yml` to run only a local DDHub Client Gateway against EWF-hosted EWC Digital Spine services. This setup uses the shared EWF broker, public EWC RPC, and public identity cache; it does not run a local broker, IAM chain, SSI hub, MongoDB, or GSY DEX services.

```bash
cp .env.ewds.local.example .env.ewds.local
# Default local setup uses the bundled HashiCorp Vault service.
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml up --build
```

After the gateway starts, use the local proxy for the UI and API docs. Upload the EWF-provided mTLS client certificate and matching private key through `POST /api/v2/certificate`:

```bash
http://localhost:3009
http://localhost:3009/docs
```

Then configure the DID/EWC wallet private key through the UI. After mTLS and identity are stored, restart the compose stack without removing volumes so the API and scheduler reload the Vault-backed certificate/private-key material consistently:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml down --remove-orphans
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml up --build
```

Do not use `down -v` after mTLS/private-key setup unless you intentionally want to wipe the local Vault and Postgres state.

Expected healthy signals after restart:

- Dashboard shows `DDHub Message Broker` as `Online`.
- Dashboard shows `MTLS STATUS` as `Valid`.
- Scheduler logs include `connected to did registry, iam setup finalized`.
- Scheduler logs include `Login successful` and `Init ext channel successful`.
- Enrolment logs show synced roles for `dsmb.apps.ddhub.energyweb.auth.ewc` and `integration.apps.intelligent.auth.ewc`.

Runtime defaults in `.env.ewds.local.example` match the EWF-shared values:

- `EWDS_BROKER_BASE_URL=https://ddhub-ewc.energyweb.org`
- `EWDS_CACHE_SERVER_URL=https://identitycache.energyweb.org/v1`
- `EWDS_EVENT_SERVER_URL=https://identitycache.energyweb.org`
- `EWDS_RPC_URL=https://rpc.energyweb.org/`
- `EWDS_CHAIN_ID=246`
- `EWDS_CHAIN_NAME=EWC`
- `EWDS_PARENT_NAMESPACE=dsmb.apps.ddhub.energyweb.auth.ewc`
- `EWDS_APPLICATION_NAMESPACE_REGULAR_EXPRESSION=\w+\.apps\..*\.(iam|auth)\.ewc`
- `EWDS_DID_REGISTRY_ADDRESS=0xE29672f34e92b56C9169f9D485fFc8b9A136BCE4`
- `EWDS_MTLS_ENABLED=true`

### EWDS DEX Overlay

Once the local gateway can connect to the shared broker, create the GSY request/reply channels in the local Client Gateway and associate the required topics under `integration.apps.intelligent.auth.ewc`. The gateway API and scheduler must include `APPLICATION_NAMESPACE_REGULAR_EXPRESSION=\w+\.apps\..*\.(iam|auth)\.ewc`; without it, the gateway topic endpoints reject the Intelligent `.auth.ewc` owner as malformed. Then run the DEX stack with EWDS transport:

Channel/topic setup checklist:

1. Open `http://localhost:3009`.
2. In `Topic Management`, select the `Intelligent Integration Service` application (`integration.apps.intelligent.auth.ewc`).
3. Confirm the required topic versions exist: `ordersQuery`, `ordersQueryResponse`, `tradesQuery`, `tradesQueryResponse`, `measurementsQuery`, and `measurementsQueryResponse`.
4. If a topic is missing, request or use the `topiccreator` role before creating the topic schema version.
5. In `Channel Management`, create the four local messaging channels listed below.
6. Add the `user.roles.integration.apps.intelligent.auth.ewc` role restriction to each channel so DDHub can resolve recipients.
7. Keep payload encryption disabled for the first smoke test; enable it later only if required.

| Local channel FQCN | Gateway type | Attached topics | Used by |
|---|---|---|---|
| `gsy.intelligent.requests.pub` | Publish | `ordersQuery`, `tradesQuery`, `measurementsQuery` | matching/execution engines publish requests |
| `gsy.intelligent.requests.sub` | Subscribe | `ordersQuery`, `tradesQuery`, `measurementsQuery` | off-chain storage service polls requests |
| `gsy.intelligent.responses.pub` | Publish | `ordersQueryResponse`, `tradesQueryResponse`, `measurementsQueryResponse` | off-chain storage service publishes responses |
| `gsy.intelligent.responses.sub` | Subscribe | `ordersQueryResponse`, `tradesQueryResponse`, `measurementsQueryResponse` | matching/execution engines poll responses |

DDHub Client Gateway requires unique internal channel names, so publish and subscribe records cannot reuse the same FQCN. The topic owner and topic names remain the same across channels; only the local channel FQCN changes by direction.

DDHub topic names must not contain dots. Use camelCase topic names in the gateway and keep dotted names only inside the JSON payload `operation` field, for example topic `ordersQuery` with payload operation `orders.query`.

Gateway messaging details confirmed during the smoke test:

- `POST /api/v2/messages` expects `payload` to be a JSON-encoded string.
- `POST /api/v2/messages` must include `topicVersion`; the default is `EWDS_TOPIC_VERSION=1.0.0`.
- `POST /api/v2/messages` must include `transactionId` and `anonymousRecipient`; use an empty `anonymousRecipient` array for role-based recipient resolution.
- `GET /api/v2/messages` must include a stable `clientId` receive cursor. Client IDs must be alphanumeric; the gateway rejects hyphens, dots, and other punctuation.

Default channel/runtime mapping:

- `EWDS_REQUEST_PUBLISH_FQCN=gsy.intelligent.requests.pub`
- `EWDS_REQUEST_SUBSCRIBE_FQCN=gsy.intelligent.requests.sub`
- `EWDS_RESPONSE_PUBLISH_FQCN=gsy.intelligent.responses.pub`
- `EWDS_RESPONSE_SUBSCRIBE_FQCN=gsy.intelligent.responses.sub`

```bash
docker compose --env-file .env.ewds.local -f docker-compose.yml up --build
```

GSY topic/channel registration is intentionally separate from the local compose file. EWF confirmed that we manage channels in our local Client Gateway, multiple topics can be associated with one channel, and Intelligent topics should use the `integration.apps.intelligent.auth.ewc` owner namespace. The gateway must be started first with `docker-compose.ewds.yml`; the GSY services read the Docker-internal gateway URL from `.env.ewds.local`.

Run the gateway and GSY commands from the repository root without changing the Compose project name. This keeps both stacks on the same default Docker network so `http://ddhub-gateway-api:3333` resolves from the GSY service containers.

Useful runtime overrides:

- `EWDS_GATEWAY_BACKEND_IMAGE`
- `EWDS_GATEWAY_FRONTEND_IMAGE`
- `EWDS_GATEWAY_SCHEDULER_IMAGE`
- `EWDS_GATEWAY_PLATFORM`
- `EWDS_BROKER_BASE_URL`
- `EWDS_CACHE_SERVER_URL`
- `EWDS_EVENT_SERVER_URL`
- `EWDS_RPC_URL` / `EWDS_ENS_URL`
- `EWDS_CHAIN_ID` / `EWDS_CHAIN_NAME`
- `EWDS_PARENT_NAMESPACE`
- `EWDS_APPLICATION_NAMESPACE_REGULAR_EXPRESSION`
- `EWDS_DID_REGISTRY_ADDRESS`
- `EWDS_MTLS_ENABLED`
- `EWDS_OFFCHAIN_STORAGE_URL`
- `OFFCHAIN_STORAGE_TRANSPORT` (`http` or `ewds`)
- `EWDS_GATEWAY_URL` (Docker-internal API URL, defaults to `http://ddhub-gateway-api:3333`)
- `EWDS_GATEWAY_PROXY_PORT` (browser-facing proxy port, defaults to `3009`)
- `EWDS_TOPIC_OWNER=integration.apps.intelligent.auth.ewc`
- `EWDS_TOPIC_VERSION=1.0.0`
- `EWDS_REQUEST_PUBLISH_FQCN` / `EWDS_REQUEST_SUBSCRIBE_FQCN`
- `EWDS_RESPONSE_PUBLISH_FQCN` / `EWDS_RESPONSE_SUBSCRIBE_FQCN`
- `EWDS_OFFCHAIN_STORAGE_CLIENT_ID` / `EWDS_MATCHING_ENGINE_CLIENT_ID` / `EWDS_EXECUTION_ENGINE_CLIENT_ID`
- `EWDS_ORDERS_REQUEST_TOPIC` / `EWDS_ORDERS_RESPONSE_TOPIC`
- `EWDS_TRADES_REQUEST_TOPIC` / `EWDS_TRADES_RESPONSE_TOPIC`
- `EWDS_MEASUREMENTS_REQUEST_TOPIC` / `EWDS_MEASUREMENTS_RESPONSE_TOPIC`
- `EWDS_ENABLE_HANDLER=true` (enables EWDS query responder in `gsy-offchain-storage`)
- `EWDS_HANDLER_POLL_INTERVAL_MS` / `EWDS_HANDLER_BATCH_SIZE`
- `EWDS_RESPONSE_TIMEOUT_MS` / `EWDS_RESPONSE_POLL_INTERVAL_MS`

### EWDS Smoke Test

Validate the gateway before starting the GSY services.

1. Start or restart only the gateway stack without deleting volumes:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml up --build
```

2. Confirm the Intelligent topic namespace works:

```bash
curl -G -i 'http://localhost:3009/api/v2/topics/count' \
  --data-urlencode 'owner[]=integration.apps.intelligent.auth.ewc'
```

3. Prime the receive cursor for an existing smoke-test subscribe channel and topic:

```bash
curl -G -i 'http://localhost:3009/api/v2/messages' \
  --data-urlencode 'fqcn=gsy.intelligent.hello.sub' \
  --data-urlencode 'amount=10' \
  --data-urlencode 'topicName=helloWorld' \
  --data-urlencode 'topicOwner=integration.apps.intelligent.auth.ewc' \
  --data-urlencode 'clientId=gsysmoketest'
```

4. Publish a `helloWorld` smoke message:

```bash
curl -i -X POST 'http://localhost:3009/api/v2/messages' \
  -H 'Content-Type: application/json' \
  -d '{
    "fqcn": "gsy.intelligent.hello",
    "topicName": "helloWorld",
    "topicOwner": "integration.apps.intelligent.auth.ewc",
    "topicVersion": "1.0.0",
    "transactionId": "gsy-smoke-hello-001",
    "payload": "{\"vendorName\":\"Grid Singularity\",\"email\":\"test@example.com\"}",
    "anonymousRecipient": []
  }'
```

5. Poll the subscribe channel with the same `clientId`:

```bash
curl -G -i 'http://localhost:3009/api/v2/messages' \
  --data-urlencode 'fqcn=gsy.intelligent.hello.sub' \
  --data-urlencode 'amount=10' \
  --data-urlencode 'topicName=helloWorld' \
  --data-urlencode 'topicOwner=integration.apps.intelligent.auth.ewc' \
  --data-urlencode 'clientId=gsysmoketest'
```

Only after this gateway smoke test succeeds, create the GSY request/reply topics and channels from the checklist above, then run the GSY DEX stack with EWDS mode enabled.

### Full EWDS Validation

After the gateway smoke test succeeds and the GSY request/reply topics/channels exist, start the DEX services with EWDS transport:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.yml up --build
```

In a second terminal, follow gateway logs:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml logs -f \
  ddhub-gateway-api \
  ddhub-gateway-scheduler
```

Follow GSY service logs separately:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.yml logs -f \
  gsy-offchain-storage \
  gsy-matching-engine \
  gsy-execution-engine
```

Expected healthy signals:

- `ddhub-gateway-scheduler` no longer logs `Timeout has occurred` for `https://ddhub-ewc.energyweb.org/auth/login`.
- `ddhub-gateway-scheduler` refreshes applications and topics for `integration.apps.intelligent.auth.ewc`.
- `gsy-offchain-storage` logs `Starting EWDS request handler` with `request_fqcn=gsy.intelligent.requests.sub` and `response_fqcn=gsy.intelligent.responses.pub`.
- `gsy-matching-engine` logs `Fetching orders via EWDS transport`.
- `gsy-execution-engine` sends `tradesQuery` and `measurementsQuery` through EWDS when the execution cycle reaches those reads.

Errors that indicate channel/topic setup is still incomplete:

- `CHANNEL::NOT_FOUND`: the queried FQCN was not created in Channel Management, or the service is using the wrong publish/subscribe env var.
- `MESSAGING::RECIPIENTS_NOT_PRESENT`: the publish/subscribe counterpart or role restriction is missing.
- `VALIDATION::FAILED` with `Malformed owner name`: `EWDS_APPLICATION_NAMESPACE_REGULAR_EXPRESSION` is missing from the API or scheduler container env.
- `topicVersion should not be empty`: `EWDS_TOPIC_VERSION` is missing.
- `PAYLOAD JSON PARSE FAILED`: the DDHub payload was sent as an object instead of a JSON-encoded string.
- `clientId contains invalid characters`: the service client ID contains punctuation; use only alphanumeric values.

## Test Compose

Use the test compose file to run e2e and integration scenarios:

```bash
./scripts/contracts.sh local deploy

docker compose --env-file contracts-output/addresses.env \
  -f docker-compose.test.yml \
  up --build --force-recreate \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

EWDS-enabled test execution:

```bash
docker compose --env-file .env.ewds.local \
  -f docker-compose.ewds.yml \
  up --build
```

After the gateway is configured, healthy, and still running, deploy local
contracts in another shell:

```bash
./scripts/contracts.sh local deploy
```

Then run the GSY e2e stack with the EWDS and contract address env files:

```bash
docker compose --env-file .env.ewds.local \
  --env-file contracts-output/addresses.env \
  -f docker-compose.test.yml \
  up --build \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

If the local DDHub Client Gateway is already configured and running, do not use
`down` on `docker-compose.ewds.yml` unless you intentionally want to stop the
gateway, Vault, and Postgres containers too. For the final
validated e2e workflow and the GSY-only reset commands, see
`docs/setup/test.md`.

## Important Environment Contracts

The services expect these deployed addresses:

- `MARKET_CONTROLLER_ADDRESS`
- `ORDER_REGISTRY_ADDRESS`
- `TRADE_SETTLEMENT_ADDRESS`
- `ACTOR_REGISTRY_ADDRESS`

In default local setup, they are provisioned by `./scripts/contracts.sh local
deploy` and injected with `--env-file contracts-output/addresses.env`. These
are proxy addresses. The deployment script also writes implementation and
`ProxyAdmin` addresses for inspection and upgrade operations.
