#!/bin/sh

BASE_URL="http://localhost:3009/api/v2"
OWNER="integration.apps.intelligent.auth.ewc"

REQUEST_TOPICS="
ordersQuery
tradesQuery
measurementsQuery
clearingResultsQuery
marketsQuery
facilitiesQuery
ordersQueryTest
tradesQueryTest
measurementsQueryTest
clearingResultsQueryTest
marketsQueryTest
facilitiesQueryTest
"

CHANNELS="
gsy.intelligent.requests.pub
gsy.intelligent.requests.sub
gsy.intelligent.responses.pub
gsy.intelligent.responses.sub
"

# Response topics are the request topics with a "Response" suffix
RESPONSE_TOPICS=""
for TOPIC in $REQUEST_TOPICS; do
  RESPONSE_TOPICS="$RESPONSE_TOPICS
${TOPIC}Response"
done

# 1. Add all topics (request + response)
for TOPIC in $REQUEST_TOPICS $RESPONSE_TOPICS; do
  echo "Adding topic: $TOPIC"
  curl -X 'POST' \
    "$BASE_URL/topics" \
    -H 'accept: application/json' \
    -H 'Content-Type: application/json' \
    -d "{
  \"name\": \"$TOPIC\",
  \"schemaType\": \"JSD7\",
  \"schema\": \"{}\",
  \"version\": \"1.0.0\",
  \"owner\": \"$OWNER\",
  \"tags\": []
}"
  echo ""
done

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

REQUEST_TOPICS_JSON=$(build_topics_json "$REQUEST_TOPICS")
RESPONSE_TOPICS_JSON=$(build_topics_json "$RESPONSE_TOPICS")

# 2. Configure each channel
for CHANNEL in $CHANNELS; do
  TYPE="${CHANNEL##*.}"

  # 3rd part of the name decides request vs response channel
  KIND=$(echo "$CHANNEL" | cut -d. -f3)
  case "$KIND" in
    request*) TOPICS_JSON="$REQUEST_TOPICS_JSON" ;;
    response*) TOPICS_JSON="$RESPONSE_TOPICS_JSON" ;;
    *) echo "Unknown channel kind for $CHANNEL, skipping"; continue ;;
  esac

  echo "Configuring channel: $CHANNEL (type: $TYPE, kind: $KIND)"
  curl -X 'PUT' \
    "$BASE_URL/channels/$CHANNEL" \
    -H 'accept: application/json' \
    -H 'Content-Type: application/json' \
    -d "{
    \"type\": \"$TYPE\",
    \"payloadEncryption\": false,
    \"conditions\": {
    \"roles\": [
      \"user.roles.$OWNER\"
    ],
    \"topics\": [$TOPICS_JSON
       ],
       \"responseTopics\": [
       ]
  }
}"
  echo ""
done