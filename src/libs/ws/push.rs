use dashmap::DashMap;
use serde::Serialize;
use std::collections::HashMap;
use std::hash::Hash;

pub struct SubscriberContext {
    pub ctx: RequestContext,
    pub stream_seq: u32,
}

#[derive(Default)]
pub struct Subscribers {
    pub subscribers: HashMap<ConnectionId, SubscriberContext>,
}

pub struct SubscribeManager<Key: Eq + Hash> {
    pub topics: DashMap<Key, Subscribers>,
}

impl<Key: Hash + Eq + Into<u32>> Default for SubscribeManager<Key> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Key: Hash + Eq + Into<u32>> SubscribeManager<Key> {
    pub fn new() -> Self {
        Self {
            topics: DashMap::new(),
        }
    }

    pub fn add_topics(&self, topics: Vec<Key>) {
        for topic in topics {
            self.add_topic(topic);
        }
    }
    pub fn add_topic(&self, topic: Key) {
        self.topics.entry(topic).or_default();
    }
    pub fn subscribe_multi(&self, topics: Vec<Key>, ctx: RequestContext) {
        for topic in topics {
            self.subscribe(topic, ctx);
        }
    }
    pub fn subscribe(&self, topic: Key, ctx: RequestContext) {
        let mut subscribers = self.topics.entry(topic).or_default();
        subscribers
            .subscribers
            .insert(ctx.connection_id, SubscriberContext { ctx, stream_seq: 0 });
    }
    pub fn unsubscribe_multi(&self, topics: Vec<Key>, connection_id: ConnectionId) {
        for topic in topics {
            self.unsubscribe(topic, connection_id);
        }
    }
    pub fn unsubscribe(&self, topic: Key, connection_id: ConnectionId) {
        if let Some(mut subscribers) = self.topics.get_mut(&topic) {
            subscribers.subscribers.remove(&connection_id);
        }
    }
    pub fn publish_with_filter(
        &self,
        toolbox: &ArcToolbox,
        topic: Key,
        msg: &impl Serialize,
        filter: impl Fn(&RequestContext) -> bool,
    ) {
        if let Some(mut topic_2) = self.topics.get_mut(&topic) {
            let data = serde_json::to_value(msg).unwrap();
            let mut dead_connections = vec![];
            let stream_code = topic.into();
            for sub in topic_2.subscribers.values_mut() {
                if !filter(&sub.ctx) {
                    continue;
                }
                let msg = WsResponseGeneric::Stream(WsStreamResponseGeneric {
                    original_seq: sub.ctx.seq,
                    method: sub.ctx.method,
                    stream_seq: sub.stream_seq,
                    stream_code,
                    data: data.clone(),
                });
                sub.stream_seq += 1;
                if !toolbox.send(sub.ctx.connection_id, msg) {
                    dead_connections.push(sub.ctx.connection_id);
                }
            }
            for conn_id in dead_connections {
                topic_2.subscribers.remove(&conn_id);
            }
        }
    }
    pub fn publish_to_all(&self, toolbox: &ArcToolbox, topic: Key, msg: &impl Serialize) {
        self.publish_with_filter(toolbox, topic, msg, |_| true)
    }
}
