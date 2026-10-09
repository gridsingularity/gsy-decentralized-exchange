//! Polls a whole EWDS subscribe channel and routes its messages to per-topic queues.
//!
//! The gateway returns the messages of every topic attached to a channel, and a poll
//! acknowledges all of them for its `clientId`. So exactly one [`EwdsChannelPoller`] per
//! service and channel reads the channel, and hands every message to the receiver registered
//! for its topic.

use super::dto::{EwdsEventEnvelope, EwdsInboundMessage, EwdsRequestEnvelope};
use super::{
    format_response_body, next_poll_delay_ms, parse_inbound_messages, EwdsEventTopicConfig,
    EwdsTopicConfig,
};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

/// Finds the topic of a message that the gateway returned without a `topicName`, from its
/// payload. Returns `None` when the message belongs to no known topic.
pub type EwdsFallbackRouter = Box<dyn Fn(&EwdsInboundMessage) -> Option<String> + Send + Sync>;

#[derive(Debug, Clone)]
pub struct EwdsChannelPollerConfig {
    pub gateway_base: String,
    /// The subscribe channel to poll.
    pub fqcn: String,
    /// The gateway cursor. No other poller may use it, or the two split the messages.
    pub client_id: String,
    /// Messages of any other topic owner are dropped.
    pub topic_owner: String,
    /// How many messages one poll fetches at most, over all topics of the channel.
    pub batch_size: u32,
    /// The pause between two polls. A poll that returns a full batch is followed at once.
    pub poll_interval_ms: u64,
}

/// What [`EwdsChannelPoller::dispatch`] did with a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EwdsDispatch {
    Routed(String),
    ForeignOwner(String),
    Unrouted(Option<String>),
    ReceiverClosed(String),
}

pub struct EwdsChannelPoller {
    client: reqwest::Client,
    config: EwdsChannelPollerConfig,
    routes: HashMap<String, UnboundedSender<EwdsInboundMessage>>,
    fallback_router: Option<EwdsFallbackRouter>,
}

impl EwdsChannelPoller {
    pub fn new(config: EwdsChannelPollerConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
            routes: HashMap::new(),
            fallback_router: None,
        }
    }

    /// Registers `topic_name` and returns the queue its messages arrive on, in channel order.
    /// The queue is unbounded, so a topic whose consumer waits for a retry never stalls the
    /// poller or the other topics.
    pub fn route(&mut self, topic_name: &str) -> UnboundedReceiver<EwdsInboundMessage> {
        let (sender, receiver) = unbounded_channel();
        if self.routes.insert(topic_name.to_string(), sender).is_some() {
            warn!(
                "EWDS topic '{}' on channel '{}' was routed twice; only the last route receives it",
                topic_name, self.config.fqcn
            );
        }
        receiver
    }

    /// Sets how messages without a `topicName` are routed.
    pub fn with_fallback_router(mut self, router: EwdsFallbackRouter) -> Self {
        self.fallback_router = Some(router);
        self
    }

    /// Polls the channel and dispatches its messages until every route's receiver is dropped.
    pub async fn run(self) {
        let mut rate_limit_attempt = 0u32;

        loop {
            let result = self.poll_once().await;
            let batch_full = match &result {
                Ok(messages) => messages.len() >= self.config.batch_size.max(1) as usize,
                Err(error) => {
                    warn!(
                        "EWDS poll of channel '{}' failed: {:#}",
                        self.config.fqcn, error
                    );
                    false
                }
            };
            let result = result.map(|messages| self.dispatch_all(messages));

            if self.all_receivers_closed() {
                info!(
                    "Stopping the EWDS poller of channel '{}': no topic is consumed any more",
                    self.config.fqcn
                );
                return;
            }

            let delay_ms = next_poll_delay_ms(
                &result,
                self.config.poll_interval_ms,
                &mut rate_limit_attempt,
            );
            // A full batch means more messages are waiting; fetch them at once instead of
            // making every topic of the channel wait a whole interval.
            if batch_full && rate_limit_attempt == 0 {
                continue;
            }
            sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    /// One `GET /api/v2/messages` of the whole channel: no `topicName` and no `topicOwner`.
    pub async fn poll_once(&self) -> Result<Vec<EwdsInboundMessage>> {
        let get_url = format!(
            "{}/api/v2/messages",
            self.config.gateway_base.trim_end_matches('/')
        );
        let amount = self.config.batch_size.to_string();
        let response = self
            .client
            .get(get_url.as_str())
            .query(&[
                ("fqcn", self.config.fqcn.as_str()),
                ("amount", amount.as_str()),
                ("clientId", self.config.client_id.as_str()),
            ])
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "EWDS poll failed for fqcn='{}': HTTP {}{}",
                self.config.fqcn,
                status,
                format_response_body(&body)
            ));
        }

        parse_inbound_messages(&response.text().await?)
            .with_context(|| format!("EWDS poll of fqcn='{}'", self.config.fqcn))
    }

    /// Hands one message to the queue of its topic.
    pub fn dispatch(&self, message: EwdsInboundMessage) -> EwdsDispatch {
        if let Some(owner) = message.topic_owner.as_deref() {
            if owner != self.config.topic_owner {
                return EwdsDispatch::ForeignOwner(owner.to_string());
            }
        }

        let topic_name = match &message.topic_name {
            Some(topic_name) => Some(topic_name.clone()),
            None => self
                .fallback_router
                .as_ref()
                .and_then(|router| router(&message)),
        };
        let Some(topic_name) = topic_name else {
            return EwdsDispatch::Unrouted(None);
        };
        let Some(sender) = self.routes.get(&topic_name) else {
            return EwdsDispatch::Unrouted(Some(topic_name));
        };

        match sender.send(message) {
            Ok(()) => EwdsDispatch::Routed(topic_name),
            Err(_) => EwdsDispatch::ReceiverClosed(topic_name),
        }
    }

    fn dispatch_all(&self, messages: Vec<EwdsInboundMessage>) {
        let mut dropped = 0usize;
        for message in messages {
            let message_id = message.id.clone().unwrap_or_default();
            match self.dispatch(message) {
                EwdsDispatch::Routed(_) => {}
                EwdsDispatch::ForeignOwner(owner) => {
                    dropped += 1;
                    debug!(
                        "Dropping EWDS message '{}' on channel '{}' of foreign topic owner '{}'",
                        message_id, self.config.fqcn, owner
                    );
                }
                EwdsDispatch::Unrouted(topic_name) => {
                    dropped += 1;
                    debug!(
                        "Dropping EWDS message '{}' on channel '{}' of unrouted topic {:?}",
                        message_id, self.config.fqcn, topic_name
                    );
                }
                EwdsDispatch::ReceiverClosed(topic_name) => {
                    dropped += 1;
                    warn!(
                        "Dropping EWDS message '{}' on channel '{}': nobody consumes topic '{}' any more",
                        message_id, self.config.fqcn, topic_name
                    );
                }
            }
        }
        if dropped > 0 {
            // Expected: a service receives every topic of the channel, including the ones meant
            // for other services. A sudden rise points at a missing route.
            info!(
                "Dropped {} EWDS messages on channel '{}' that no topic of this service consumes",
                dropped, self.config.fqcn
            );
        }
    }

    fn all_receivers_closed(&self) -> bool {
        !self.routes.is_empty() && self.routes.values().all(|sender| sender.is_closed())
    }
}

/// Routes an event without a `topicName` to the topic of its `eventType`.
pub fn route_event_by_type(event_topics: EwdsEventTopicConfig) -> EwdsFallbackRouter {
    Box::new(move |message| {
        let envelope = serde_json::from_str::<EwdsEventEnvelope<Value>>(&message.payload).ok()?;
        Some(event_topics.for_event_type(envelope.event_type).to_string())
    })
}

/// Routes a request without a `topicName` to the request topic of its `operation`.
pub fn route_request_by_operation(topics: EwdsTopicConfig) -> EwdsFallbackRouter {
    Box::new(move |message| {
        let envelope = serde_json::from_str::<EwdsRequestEnvelope>(&message.payload).ok()?;
        Some(topics.for_operation(envelope.operation).request.clone())
    })
}
