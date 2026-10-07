#!/bin/sh
#
# Create the INTELLIGENT EWDS topics and create or update the GSY channels on
# the DDHub client gateway, so that every channel carries exactly the topics
# configured below.
#
# Usage:
#   ./ewds_channel_topic_handler.sh                  create missing topics, create or update all channels
#   ./ewds_channel_topic_handler.sh --channel FQCN   limit to one channel (repeatable)
#
# Environment:
#   BASE_URL             gateway API base (default http://localhost:3009/api/v2)
#   OWNER                topic owner / role namespace (default integration.apps.intelligent.auth.ewc)
#   MAX_ATTEMPTS         attempts per API call on 429/5xx (default 6)
#   CHANNEL_PAUSE_SECS   pause between channel updates (default 5)
#
# Requires curl and python3 (python3 is only used to parse JSON).

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

EVENT_TOPICS="
trade
clearingResult
market
tradeTest
clearingResultTest
marketTest
"

CHANNELS="
gsy.intelligent.requests.pub
gsy.intelligent.requests.sub
gsy.intelligent.responses.pub
gsy.intelligent.responses.sub
gsy.intelligent.events.pub
gsy.intelligent.events.sub
"

SELECTED_CHANNELS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --channel)
      [ $# -ge 2 ] || { echo "--channel needs an FQCN" >&2; exit 2; }
      SELECTED_CHANNELS="$SELECTED_CHANNELS $2"
      shift
      ;;
    -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
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
# connection errors with backoff; any other non-2xx status aborts the script,
# except that with API_MISSING_OK=true a CHANNEL::NOT_FOUND answer returns 1.
API_MISSING_OK=false
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
    if [ "$API_MISSING_OK" = true ] && grep -q 'CHANNEL::NOT_FOUND' "$RESP" 2>/dev/null; then
      return 1
    fi
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

# Helper: succeed if the channel exists on the gateway, which answers a
# missing channel with HTTP 400 CHANNEL::NOT_FOUND.
channel_exists() {
  API_MISSING_OK=true
  if api GET "/channels/$1"; then EXISTS=0; else EXISTS=1; fi
  API_MISSING_OK=false
  return $EXISTS
}

# 1. Create missing topics (request + response + event)
api GET "/topics?owner=$OWNER&limit=200"
EXISTING_TOPICS=$(python3 -c '
import json, sys
data = json.load(open(sys.argv[1]))
records = data.get("records", []) if isinstance(data, dict) else data
print("\n".join(r.get("name", "") for r in records))
' "$RESP")

MISSING_TOPICS=""
for TOPIC in $REQUEST_TOPICS $RESPONSE_TOPICS $EVENT_TOPICS; do
  if ! echo "$EXISTING_TOPICS" | grep -qx "$TOPIC"; then
    MISSING_TOPICS="$MISSING_TOPICS $TOPIC"
  fi
done

if [ -z "$MISSING_TOPICS" ]; then
  echo "All topics exist for owner $OWNER"
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
EVENT_TOPICS_JSON=$(build_topics_json "$EVENT_TOPICS")

# 2. Create or update each channel
FIRST=true
for CHANNEL in $CHANNELS; do
  TYPE="${CHANNEL##*.}"

  # 3rd part of the name decides request vs response channel
  KIND=$(echo "$CHANNEL" | cut -d. -f3)
  case "$KIND" in
    request*) TOPICS_JSON="$REQUEST_TOPICS_JSON" ;;
    response*) TOPICS_JSON="$RESPONSE_TOPICS_JSON" ;;
    event*) TOPICS_JSON="$EVENT_TOPICS_JSON" ;;
    *) echo "Unknown channel kind for $CHANNEL, skipping"; continue ;;
  esac

  [ "$FIRST" = true ] || sleep "$CHANNEL_PAUSE_SECS"
  FIRST=false
  CHANNEL_SETTINGS="\"type\": \"$TYPE\",
  \"payloadEncryption\": false,
  \"conditions\": {
  \"roles\": [
    \"$ROLE\"
  ],
  \"topics\": [$TOPICS_JSON
     ],
     \"responseTopics\": [
     ]
}"
  if channel_exists "$CHANNEL"; then
    echo "Updating channel: $CHANNEL (type: $TYPE, kind: $KIND)"
    api PUT "/channels/$CHANNEL" "{
  $CHANNEL_SETTINGS
}"
  else
    echo "Creating channel: $CHANNEL (type: $TYPE, kind: $KIND)"
    api POST "/channels" "{
  \"fqcn\": \"$CHANNEL\",
  $CHANNEL_SETTINGS
}"
  fi
done
echo "All channels configured"
