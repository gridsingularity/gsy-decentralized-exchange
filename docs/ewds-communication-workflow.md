# EWDS Communication Workflow

This page shows how a message travels between EWDS (DDHub) and the GSY services.
The example is an `orders.query` request that a third party sends to the off-chain
storage. Inbound events take the same path up to the worker. For the channel and
topic setup, see [EWDS Integration](platform/ewds-integration.md).

## Overview

```
 3rd party                 DDHub broker             our gateway               gsy-offchain-storage
 ─────────                 ────────────             ───────────               ────────────────────
 1 POST /messages ───────▶ 2 deliver to the
   topic ordersQuery         recipients of the
                             channel ──────────────▶ queued on
                                                    requests.sub
                                                          ▲
                                                          │ 3 GET (whole channel)
                                                          └──────────────────── EwdsChannelPoller
                                                                                  │ 4 dispatch_all
                                                                                  ▼
                                                                                ordersQuery queue
                                                                                  │ 5 orders worker
                                                                                  ▼
                                                                                handle_request
                                                                                  │ 6 MongoDB
                                                                                  ▼
                                       ◀───────────── 7 POST /messages ◀──────── send_success_response
                                                      responses.pub,
                                                      topic ordersQueryResponse
 8 GET responses ◀──────── deliver
   match requestId
```

Only the poller and the response sender make HTTP calls to the gateway. Everything
in between happens in memory, inside the off-chain storage process.

## Step by Step

1. **The third party publishes the request.** It calls `POST /api/v2/messages`
   through its own gateway, on its publish channel, with:
   - topic `ordersQuery`, version `1.0.0`, owner
     `integration.apps.intelligent.auth.ewc`;
   - `transactionId` set to the request ID;
   - a payload such as
     `{"requestId":"r-1","operation":"orders.query","payload":{"marketId":"m1"}}`.

   Its gateway validates the payload against the `ordersQuery` schema.
2. **DDHub delivers the request** to `gsy.intelligent.requests.sub` on our gateway.
   It stays there for up to 24 hours until our `clientId` reads it.
3. **The poller reads the whole channel.** Every `EWDS_HANDLER_POLL_INTERVAL_MS`,
   the `EwdsChannelPoller` sends
   `GET /api/v2/messages?fqcn=gsy.intelligent.requests.sub&amount=100&clientId=gsyoffchainstoragerequests`.
   - The request has no `topicName` and no `topicOwner`, so the reply contains the
     messages of every topic on the channel.
   - Each message carries `topicName`, `topicOwner`, `topicVersion`, `id`,
     `transactionId`, `sender` and `payload`.
   - The gateway acknowledges the previous poll's messages when this poll starts.
   - If the reply is a full batch, the poller polls again straight away.
4. **`dispatch_all` routes each message** (see [Routing](#routing)). This request
   goes into the `ordersQuery` queue.
5. **The `orders.query` worker** takes the message from its queue. It:
   - parses the envelope, skipping a malformed one;
   - skips a request published longer ago than `EWDS_REQUEST_MAX_AGE_MS`
     (default: `EWDS_RESPONSE_TIMEOUT_MS`), because its sender has stopped
     waiting;
   - skips a `requestId` it has already answered;
   - checks that `operation` belongs to the topic. If it doesn't, it replies with
     an `OPERATION_TOPIC_MISMATCH` error.
6. **`handle_request` runs the query.** It parses the query payload, runs
   `filter_orders` in MongoDB and converts the orders to `EwdsOrderDto`s.
7. **The response is published.** `POST /api/v2/messages` on
   `gsy.intelligent.responses.pub` sends:
   - topic `ordersQueryResponse`, with `transactionId` set to the request ID;
   - the payload `{"requestId":"r-1","success":true,"data":[...]}`.

   On rate limits, transient errors or "delivered to no recipients", the send is
   retried for up to `EWDS_RESPONSE_TIMEOUT_MS` (60 s).
8. **The third party receives the response.** It polls `ordersQueryResponse` on its
   subscribe channel and picks out the response whose `requestId` matches its
   request.

## Routing

For every message of a poll, `dispatch()` does the following:

1. **Owner check.** If the message has a `topicOwner` other than
   `EWDS_TOPIC_OWNER`, it is dropped.
2. **Topic.** The topic is the message's `topicName`. If that's missing, a
   fallback reads it from the payload: `eventType` for events, `operation` for
   requests.
3. **Queue.** The message goes to the queue registered for that topic. A topic with
   no queue is dropped. This is expected: every service receives the topics meant
   for other services too.

`dispatch_all()` logs each dropped message at debug level and writes one info line
per poll with the number dropped. Dropped messages are gone only for this
`clientId`; other services read the channel with their own `clientId`s.

## Poller, Queues and Workers

- Each queue is an in-memory `tokio::sync::mpsc` channel. The poller moves the
  message into the queue, and the worker takes ownership of it from there. Nothing
  is copied or serialized again.
- Each topic has its own queue and worker: 8 operation workers in the request
  handler, and one per event type in an event subscriber. A slow request, or a
  response that is waiting for a retry, only delays later messages of its own
  topic.
- Messages of one topic are handled in the order they arrived.
- The poller and the workers are started together:

  ```rust
  join(poller.run(), join_all(workers)).await;
  ```

  - Calling an `async fn` only creates a future; nothing runs yet.
  - `join_all` combines the worker futures into one, and `join` adds the poller.
  - `.await` drives all of them on **one task**. Each runs until it has to wait (on
    HTTP, MongoDB, a timer or its queue), then hands control to the others.
  - This is concurrency, not parallelism. It allows plain borrows of `db`,
    `client` and `config`, which `tokio::spawn` would not.
  - The combined future runs for the service's lifetime. Dropping it stops all of
    the parts at once.
  - Because everything shares one task, a blocking or CPU-heavy call in one worker
    stalls all of them, and a panic in one stops the whole handler.

## Client IDs

The `clientId` is the gateway's read position for a consumer. Each one has its own
position, so two pollers must never share a `clientId`, or they split the messages
between them.

| Channel | Polled by | `clientId` |
|---|---|---|
| `gsy.intelligent.requests.sub` | off-chain storage | `<EWDS_REQUEST_CLIENT_ID>requests` |
| `gsy.intelligent.events.sub` | off-chain storage | `<EWDS_REQUEST_CLIENT_ID>events` |
| `gsy.intelligent.events.sub` | community client | `<EWDS_COMMUNITY_CLIENT_ID>events` |
| `gsy.intelligent.responses.sub` | every query caller | `<client ID><responseTopic>` |

Query responses are still polled per response topic. The caller matches them by
`requestId`, so this works whether or not the gateway filters by topic.

## Gateway Behaviour

- A new `clientId` receives everything the broker still holds for the channel, up
  to 24 hours. The first poll can take more than 30 seconds.
- Every requester's cursor sees every response on a response topic. A flood of
  responses, e.g. answers to a backlog of stale requests, delays the fresh
  responses for everyone. That is why stale requests are skipped.
- A poll's messages are acknowledged at the next poll of the same `clientId`. A
  message is never delivered twice, even if handling it fails, so failed events are
  retried from memory and failed requests stay unanswered.
- For a request to work, the third party's publish channel must resolve to the
  off-chain storage only. Our `responses.pub` channel must also include the third
  party as a recipient.
