#!/bin/sh
#
# Create the INTELLIGENT EWDS topics and configure the GSY channels on the
# DDHub client gateway, then read every channel back and verify it.
#
# Usage:
#   ./ewds_channel_topic_handler.sh                  create missing topics, configure all channels
#   ./ewds_channel_topic_handler.sh --check          read-only: report drift, exit 1 if any
#   ./ewds_channel_topic_handler.sh --channel FQCN   limit to one channel (repeatable)
#
# Environment:
#   BASE_URL             gateway API base (default http://localhost:3009/api/v2)
#   OWNER                topic owner / role namespace (default integration.apps.intelligent.auth.ewc)
#   MAX_ATTEMPTS         attempts per API call on 429/5xx (default 6)
#   CHANNEL_PAUSE_SECS   pause between channel updates (default 5)
#
# Requires curl and python3 (python3 is only used to parse and compare JSON).

set -eu

BASE_URL="${BASE_URL:-http://localhost:3009/api/v2}"
OWNER="${OWNER:-integration.apps.intelligent.auth.ewc}"
MAX_ATTEMPTS="${MAX_ATTEMPTS:-6}"
CHANNEL_PAUSE_SECS="${CHANNEL_PAUSE_SECS:-5}"
ROLE="user.roles.$OWNER"

REQUEST_TOPICS="
ordersQuery
tradesQuery
measurementsQuery
clearingResultsQuery
marketsQuery
facilitiesQuery
idsQuery
communityUpsert
communitiesQuery
ordersQueryTest
tradesQueryTest
measurementsQueryTest
clearingResultsQueryTest
marketsQueryTest
facilitiesQueryTest
idsQueryTest
communityUpsertTest
communitiesQueryTest
"

CHANNELS="
gsy.intelligent.requests.pub
gsy.intelligent.requests.sub
gsy.intelligent.responses.pub
gsy.intelligent.responses.sub
"

CHECK_ONLY=false
SELECTED_CHANNELS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --check) CHECK_ONLY=true ;;
    --channel)
      [ $# -ge 2 ] || { echo "--channel needs an FQCN" >&2; exit 2; }
      SELECTED_CHANNELS="$SELECTED_CHANNELS $2"
      shift
      ;;
    -h|--help) sed -n '2,19p' "$0"; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

if [ -n "$SELECTED_CHANNELS" ]; then
  for CHANNEL in $SELECTED_CHANNELS; do
    case " $(echo $CHANNELS) " in
      *" $CHANNEL "*) ;;
      *) echo "Unknown channel: $CHANNEL" >&2; exit 2 ;;
    esac
  done
  CHANNELS="$SELECTED_CHANNELS"
fi

# Response topics are the request topics with a "Response" suffix
RESPONSE_TOPICS=""
for TOPIC in $REQUEST_TOPICS; do
  RESPONSE_TOPICS="$RESPONSE_TOPICS
${TOPIC}Response"
done

RESP=$(mktemp)
trap 'rm -f "$RESP"' EXIT

# Helper: call the gateway API. The response body is left in $RESP.
# Retries 429s (including the message broker's 429 wrapped in a 400), 5xx and
# connection errors with backoff; any other non-2xx status aborts the script.
api() {
  METHOD=$1
  API_PATH=$2
  BODY=${3:-}
  ATTEMPT=1
  DELAY=2
  while :; do
    if [ -n "$BODY" ]; then
      STATUS=$(curl -sS -o "$RESP" -w '%{http_code}' -X "$METHOD" "$BASE_URL$API_PATH" \
        -H 'accept: application/json' -H 'Content-Type: application/json' -d "$BODY") || STATUS=000
    else
      STATUS=$(curl -sS -o "$RESP" -w '%{http_code}' -X "$METHOD" "$BASE_URL$API_PATH" \
        -H 'accept: application/json') || STATUS=000
    fi
    case "$STATUS" in
      2??) return 0 ;;
    esac
    RETRYABLE=false
    case "$STATUS" in
      000|429|5??) RETRYABLE=true ;;
    esac
    if grep -qi 'status code 429' "$RESP" 2>/dev/null; then
      RETRYABLE=true
    fi
    if [ "$RETRYABLE" = true ] && [ "$ATTEMPT" -lt "$MAX_ATTEMPTS" ]; then
      echo "  $METHOD $API_PATH -> HTTP $STATUS, retry $ATTEMPT/$((MAX_ATTEMPTS - 1)) in ${DELAY}s" >&2
      sleep "$DELAY"
      ATTEMPT=$((ATTEMPT + 1))
      DELAY=$((DELAY * 2))
      [ "$DELAY" -gt 30 ] && DELAY=30
      continue
    fi
    echo "ERROR: $METHOD $API_PATH -> HTTP $STATUS after $ATTEMPT attempt(s):" >&2
    cat "$RESP" >&2
    echo "" >&2
    exit 1
  done
}

# Helper: build a JSON topics array from a topic list
build_topics_json() {
  RESULT=""
  for TOPIC in $1; do
    [ -n "$RESULT" ] && RESULT="$RESULT,"
    RESULT="$RESULT
         {
           \"topicName\": \"$TOPIC\",
           \"owner\": \"$OWNER\"
         }"
  done
  echo "$RESULT"
}

# Helper: compare the channel JSON in $RESP with the expected type, role and
# topics. Prints OK/DRIFT and returns 1 on drift.
verify_channel() {
  python3 - "$RESP" "$1" "$2" "$ROLE" $3 <<'EOF'
import json, sys

path, fqcn, expected_type, role, *topics = sys.argv[1:]
with open(path) as fh:
    channel = json.load(fh)
conditions = channel.get("conditions") or {}
roles = conditions.get("roles") or []
actual_topics = {t.get("topicName") for t in conditions.get("topics") or []}
dids = conditions.get("qualifiedDids") or []

problems = []
if channel.get("type") != expected_type:
    problems.append(f"type {channel.get('type')!r} (expected {expected_type!r})")
if roles != [role]:
    problems.append(f"roles {roles} (expected [{role!r}])")
missing = sorted(set(topics) - actual_topics)
extra = sorted(actual_topics - set(topics))
if missing:
    problems.append(f"missing topics {missing}")
if extra:
    problems.append(f"extra topics {extra}")
if not dids:
    problems.append("no qualified DIDs (nobody can receive on this channel)")

if problems:
    print(f"DRIFT {fqcn}:")
    for problem in problems:
        print(f"  - {problem}")
    sys.exit(1)
print(f"OK    {fqcn} (roles={roles}, topics={len(actual_topics)}, qualifiedDids={len(dids)})")
EOF
}

# 1. Create missing topics (request + response)
api GET "/topics?owner=$OWNER&limit=200"
EXISTING_TOPICS=$(python3 -c '
import json, sys
data = json.load(open(sys.argv[1]))
records = data.get("records", []) if isinstance(data, dict) else data
print("\n".join(r.get("name", "") for r in records))
' "$RESP")

MISSING_TOPICS=""
for TOPIC in $REQUEST_TOPICS $RESPONSE_TOPICS; do
  if ! echo "$EXISTING_TOPICS" | grep -qx "$TOPIC"; then
    MISSING_TOPICS="$MISSING_TOPICS $TOPIC"
  fi
done

DRIFT=false
if [ -z "$MISSING_TOPICS" ]; then
  echo "All topics exist for owner $OWNER"
elif [ "$CHECK_ONLY" = true ]; then
  echo "DRIFT topics missing:$MISSING_TOPICS"
  DRIFT=true
else
  for TOPIC in $MISSING_TOPICS; do
    echo "Adding topic: $TOPIC"
    api POST "/topics" "{
  \"name\": \"$TOPIC\",
  \"schemaType\": \"JSD7\",
  \"schema\": \"{}\",
  \"version\": \"1.0.0\",
  \"owner\": \"$OWNER\",
  \"tags\": []
}"
  done
fi

REQUEST_TOPICS_JSON=$(build_topics_json "$REQUEST_TOPICS")
RESPONSE_TOPICS_JSON=$(build_topics_json "$RESPONSE_TOPICS")

# 2. Configure (or, with --check, only verify) each channel
FIRST=true
for CHANNEL in $CHANNELS; do
  TYPE="${CHANNEL##*.}"

  # 3rd part of the name decides request vs response channel
  KIND=$(echo "$CHANNEL" | cut -d. -f3)
  case "$KIND" in
    request*) TOPICS="$REQUEST_TOPICS"; TOPICS_JSON="$REQUEST_TOPICS_JSON" ;;
    response*) TOPICS="$RESPONSE_TOPICS"; TOPICS_JSON="$RESPONSE_TOPICS_JSON" ;;
    *) echo "Unknown channel kind for $CHANNEL, skipping"; continue ;;
  esac

  if [ "$CHECK_ONLY" = false ]; then
    [ "$FIRST" = true ] || sleep "$CHANNEL_PAUSE_SECS"
    FIRST=false
    echo "Configuring channel: $CHANNEL (type: $TYPE, kind: $KIND)"
    api PUT "/channels/$CHANNEL" "{
    \"type\": \"$TYPE\",
    \"payloadEncryption\": false,
    \"conditions\": {
    \"roles\": [
      \"$ROLE\"
    ],
    \"topics\": [$TOPICS_JSON
       ],
       \"responseTopics\": [
       ]
  }
}"
  fi

  api GET "/channels/$CHANNEL"
  verify_channel "$CHANNEL" "$TYPE" "$TOPICS" || DRIFT=true
done

if [ "$DRIFT" = true ]; then
  echo "Channel configuration does not match the expected state" >&2
  exit 1
fi
echo "Channel configuration verified"
