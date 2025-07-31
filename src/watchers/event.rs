use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{apimachinery::pkg::apis::meta::v1::Time};
use kube::{Api, runtime::watcher};
use kube::Client;

#[derive(Clone)]
pub struct EventItem {
    pub message: String,
    pub reason: String,
    pub involved_object: String,
    pub event_type: String,
    pub timestamp: String,
    pub namespace: String,
    pub creation_timestamp: Option<Time>,
}

pub fn convert_event(ev: k8s_openapi::api::core::v1::Event) -> Option<EventItem> {
    let involved_object = format!(
        "{}/{}",
        ev.involved_object.kind.clone().unwrap_or_else(|| "Unknown".to_string()),
        ev.involved_object.name.clone().unwrap_or_else(|| "Unknown".to_string())
    );
    Some(EventItem {
        creation_timestamp: ev.metadata.creation_timestamp,
        message: ev.message.clone().unwrap_or_else(|| "Empty".to_string()),
        reason: ev.reason.clone().unwrap_or_else(|| "Unknown".to_string()),
        involved_object,
        event_type: ev.type_.clone().unwrap_or_else(|| "Normal".to_string()),
        timestamp: ev.event_time.as_ref().map(|t| t.0.to_rfc3339()).or_else(|| ev.last_timestamp.as_ref().map(|t| t.0.to_rfc3339()))
            .unwrap_or_else(|| "N/A".to_string()),
        namespace: ev.involved_object.namespace.clone().unwrap_or_else(|| "default".to_string()),
    })
}

pub async fn watch_events(client: Arc<Client>, events_list: Arc<Mutex<Vec<EventItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<k8s_openapi::api::core::v1::Event> = Api::all(client.as_ref().clone());
    let mut event_stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(ev) => {
                    if let Some(item) = convert_event(ev) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = events_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(ev) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_event(ev) {
                        let mut list = events_list.lock().unwrap();
                        list.push(item);
                    }
                }
                watcher::Event::Delete(_) => {} // Events should not be deleted
            },
            Err(e) => {
                eprintln!("Event watch error: {:?}", e);
            }
        }
    }
}
