use serde::Serialize;
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::libs::toolbox::{ArcToolbox, RequestContext};

use super::{ConnectionId, WsResponseGeneric, WsStreamResponseGeneric};

pub struct SubscribeContext<S> {
    pub ctx: RequestContext,
    pub stream_seq: AtomicU32,
    pub settings: S,
}

pub struct SubscriptionManager<S, Key = ()> {
    pub stream_code: u32,
    pub subscribes: HashMap<ConnectionId, SubscribeContext<S>>,
    pub mappings: HashMap<Key, HashSet<ConnectionId>>,
}

impl<S, Key: Eq + Hash> SubscriptionManager<S, Key> {
    pub fn new(stream_code: u32) -> Self {
        Self {
            stream_code,
            subscribes: Default::default(),
            mappings: Default::default(),
        }
    }

    pub fn subscribe(&mut self, ctx: RequestContext, setting: S, modify: impl FnOnce(&mut SubscribeContext<S>)) {
        self.subscribe_with(ctx, vec![], || setting, modify)
    }
    pub fn subscribe_with_keys(
        &mut self,
        ctx: RequestContext,
        keys: Vec<Key>,
        setting: S,
        modify: impl FnOnce(&mut SubscribeContext<S>),
    ) {
        self.subscribe_with(ctx, keys, || setting, modify)
    }
    pub fn subscribe_with(
        &mut self,
        ctx: RequestContext,
        keys: Vec<Key>,
        new: impl FnOnce() -> S,
        modify: impl FnOnce(&mut SubscribeContext<S>),
    ) {
        self.subscribes
            .entry(ctx.connection_id)
            .and_modify(modify)
            .or_insert_with(|| SubscribeContext {
                ctx,
                stream_seq: AtomicU32::new(0),
                settings: new(),
            });

        for key in keys {
            self.mappings.entry(key).or_default().insert(ctx.connection_id);
        }
    }

    pub fn unsubscribe(&mut self, connection_id: ConnectionId) {
        self.subscribes.remove(&connection_id);
        for pair in self.mappings.values_mut() {
            pair.remove(&connection_id);
        }
    }
    pub fn unsubscribe_with(
        &mut self,
        connection_id: ConnectionId,
        remove: impl Fn(&mut SubscribeContext<S>) -> (bool, Vec<Key>),
    ) {
        let Some((remove1, keys)) = self.subscribes.get_mut(&connection_id).map(remove) else {
            return;
        };
        if remove1 {
            self.subscribes.remove(&connection_id);
        }
        for key in keys {
            let remove = self
                .mappings
                .get_mut(&key)
                .map(|set| {
                    set.remove(&connection_id);
                    set.is_empty()
                })
                .unwrap_or_default();
            if remove {
                self.mappings.remove(&key);
            }
        }
    }

    pub fn publish_to(&mut self, toolbox: &ArcToolbox, connection_id: ConnectionId, msg: &impl Serialize) {
        let mut dead_connection = None;

        let Some(sub) = self.subscribes.get(&connection_id) else {
            return;
        };

        let data = serde_json::to_value(msg).unwrap();

        let msg = WsResponseGeneric::Stream(WsStreamResponseGeneric {
            original_seq: sub.ctx.seq,
            method: sub.ctx.method,
            stream_seq: sub.stream_seq.fetch_add(1, Ordering::SeqCst),
            stream_code: self.stream_code,
            data: data.clone(),
        });

        if !toolbox.send(sub.ctx.connection_id, msg) {
            dead_connection = Some(sub.ctx.connection_id);
        }

        if let Some(conn_id) = dead_connection {
            self.unsubscribe(conn_id)
        }
    }
    pub fn publish_to_key<Q>(&mut self, toolbox: &ArcToolbox, key: &Q, msg: &impl Serialize)
    where
        Key: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        let Some(conn_ids) = self.mappings.get(key).cloned() else {
            return;
        };

        for conn_id in conn_ids {
            self.publish_to(toolbox, conn_id, msg);
        }
    }
    pub fn publish_to_keys<Q>(&mut self, toolbox: &ArcToolbox, keys: &[&Q], msg: &impl Serialize)
    where
        Key: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        let mut published = HashSet::new();
        for key in keys {
            let conn_ids = self.mappings.get(key).cloned();
            if let Some(conn_ids) = conn_ids {
                for conn_id in conn_ids.iter() {
                    // if newly inserted
                    if published.insert(*conn_id) {
                        self.publish_to(toolbox, *conn_id, msg);
                    }
                }
            }
        }
    }
    pub fn publish_with_filter<M: Serialize>(
        &mut self,
        toolbox: &ArcToolbox,
        filter: impl Fn(&SubscribeContext<S>) -> Option<M>,
    ) {
        let mut dead_connections = vec![];

        for sub in self.subscribes.values() {
            let Some(data) = filter(sub) else {
                continue;
            };
            let data = serde_json::to_value(&data).unwrap();
            let msg = WsResponseGeneric::Stream(WsStreamResponseGeneric {
                original_seq: sub.ctx.seq,
                method: sub.ctx.method,
                stream_seq: sub.stream_seq.fetch_add(1, Ordering::SeqCst),
                stream_code: self.stream_code,
                data,
            });

            if !toolbox.send(sub.ctx.connection_id, msg) {
                dead_connections.push(sub.ctx.connection_id);
            }
        }
        for conn_id in dead_connections {
            self.unsubscribe(conn_id);
        }
    }
    pub fn publish_to_all(&mut self, toolbox: &ArcToolbox, msg: &impl Serialize) {
        self.publish_with_filter(toolbox, |_| Some(msg))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;


    use crate::libs::toolbox::{RequestContext, Toolbox};

    pub(super) use super::*;

    #[tokio::test]
    async fn test_subscribe() {
        let mut manager: SubscriptionManager<(), ()> = SubscriptionManager::new(0);

        let ctx = RequestContext {
            connection_id: 1,
            ..RequestContext::empty()
        };
        manager.subscribe(ctx, (), |_| {});
        assert_eq!(manager.subscribes.len(), 1);
        assert_eq!(manager.mappings.len(), 0);
        let toolbox = Arc::new(Toolbox::new());
        manager.publish_to_all(&toolbox, &());
        manager.publish_to_key(&toolbox, &(), &());
        manager.publish_to_keys(&toolbox, &[], &());
        manager.unsubscribe(ctx.connection_id);
        assert_eq!(manager.subscribes.len(), 0);
        assert_eq!(manager.mappings.len(), 0);
    }
}
