#![allow(non_camel_case_types)]

pub mod networktables;
pub mod websocket;

use photon_targeting::PhotonPipelineResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataChangeSource {
    DCS_WEBSOCKET,
    DCS_NETWORKTABLES,
    DCS_VISIONMODULE,
    DCS_OTHER,
}

impl DataChangeSource {
    pub fn all() -> Vec<Self> {
        vec![
            Self::DCS_WEBSOCKET,
            Self::DCS_NETWORKTABLES,
            Self::DCS_VISIONMODULE,
            Self::DCS_OTHER,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataChangeDestination {
    DCD_GENSETTINGS,
    DCD_ACTIVEMODULE,
    DCD_ACTIVEPIPELINESETTINGS,
    DCD_UI,
    DCD_WEBSERVER,
    DCD_OTHER,
}

impl DataChangeDestination {
    pub fn all() -> Vec<Self> {
        vec![
            Self::DCD_GENSETTINGS,
            Self::DCD_ACTIVEMODULE,
            Self::DCD_ACTIVEPIPELINESETTINGS,
            Self::DCD_UI,
            Self::DCD_WEBSERVER,
            Self::DCD_OTHER,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataChangeEvent {
    pub source_type: DataChangeSource,
    pub dest_type: DataChangeDestination,
    pub property_name: String,
    pub data: Value,
    pub camera_unique_name: Option<String>,
    pub origin_client_id: Option<u64>,
}

impl DataChangeEvent {
    pub fn new(
        source_type: DataChangeSource,
        dest_type: DataChangeDestination,
        property_name: impl Into<String>,
        data: Value,
    ) -> Self {
        Self {
            source_type,
            dest_type,
            property_name: property_name.into(),
            data,
            camera_unique_name: None,
            origin_client_id: None,
        }
    }

    pub fn incoming_ws(
        dest_type: DataChangeDestination,
        property_name: impl Into<String>,
        data: Value,
        camera_unique_name: Option<String>,
        origin_client_id: Option<u64>,
    ) -> Self {
        Self {
            source_type: DataChangeSource::DCS_WEBSOCKET,
            dest_type,
            property_name: property_name.into(),
            data,
            camera_unique_name,
            origin_client_id,
        }
    }

    pub fn outgoing_ui(
        property_name: impl Into<String>,
        data: Value,
        origin_client_id: Option<u64>,
    ) -> Self {
        Self {
            source_type: DataChangeSource::DCS_OTHER,
            dest_type: DataChangeDestination::DCD_UI,
            property_name: property_name.into(),
            data,
            camera_unique_name: None,
            origin_client_id,
        }
    }
}

pub trait DataChangeSubscriber: Send + Sync {
    fn wanted_sources(&self) -> &[DataChangeSource];
    fn wanted_destinations(&self) -> &[DataChangeDestination];
    fn on_data_change_event(&self, event: &DataChangeEvent);
}

pub trait CVPipelineResultConsumer: Send + Sync {
    fn accept(&self, result: &PhotonPipelineResult, fps: f64);
}

type SubscriberList = Vec<Option<Arc<dyn DataChangeSubscriber>>>;

/// Thread-safe event bus matching `org.photonvision.common.dataflow.DataChangeService`.
#[derive(Clone)]
pub struct DataChangeService {
    subscribers: Arc<RwLock<SubscriberList>>,
    tx: broadcast::Sender<DataChangeEvent>,
}

impl Default for DataChangeService {
    fn default() -> Self {
        Self::new()
    }
}

impl DataChangeService {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
            tx,
        }
    }

    pub fn subscribe_channel(&self) -> broadcast::Receiver<DataChangeEvent> {
        self.tx.subscribe()
    }

    pub fn add_subscriber(&self, subscriber: Arc<dyn DataChangeSubscriber>) -> usize {
        if let Ok(mut guard) = self.subscribers.write() {
            let idx = guard.len();
            guard.push(Some(subscriber));
            idx
        } else {
            0
        }
    }

    pub fn remove_subscriber(&self, idx: usize) {
        if let Ok(mut guard) = self.subscribers.write() {
            if let Some(slot) = guard.get_mut(idx) {
                *slot = None;
            }
        }
    }

    pub fn publish_event(&self, event: DataChangeEvent) {
        if let Ok(guard) = self.subscribers.read() {
            for sub in guard.iter().flatten() {
                let src_ok = sub.wanted_sources().contains(&event.source_type);
                let dst_ok = sub.wanted_destinations().contains(&event.dest_type);
                if src_ok && dst_ok {
                    sub.on_data_change_event(&event);
                }
            }
        }
        let _ = self.tx.send(event);
    }

    pub fn publish_events(&self, events: impl IntoIterator<Item = DataChangeEvent>) {
        for ev in events {
            self.publish_event(ev);
        }
    }
}

/// Helper subscriber struct with configurable source/destination filters and closure callback.
pub struct ClosureSubscriber {
    sources: Vec<DataChangeSource>,
    destinations: Vec<DataChangeDestination>,
    callback: Box<dyn Fn(&DataChangeEvent) + Send + Sync>,
}

impl ClosureSubscriber {
    pub fn new<F>(
        sources: Vec<DataChangeSource>,
        destinations: Vec<DataChangeDestination>,
        callback: F,
    ) -> Self
    where
        F: Fn(&DataChangeEvent) + Send + Sync + 'static,
    {
        let src_set: HashSet<_> = sources.into_iter().collect();
        let dst_set: HashSet<_> = destinations.into_iter().collect();
        Self {
            sources: src_set.into_iter().collect(),
            destinations: dst_set.into_iter().collect(),
            callback: Box::new(callback),
        }
    }
}

impl DataChangeSubscriber for ClosureSubscriber {
    fn wanted_sources(&self) -> &[DataChangeSource] {
        &self.sources
    }

    fn wanted_destinations(&self) -> &[DataChangeDestination] {
        &self.destinations
    }

    fn on_data_change_event(&self, event: &DataChangeEvent) {
        (self.callback)(event);
    }
}
