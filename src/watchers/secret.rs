use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{core::v1::Secret}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher};
use kube::runtime::reflector::Lookup;

#[derive(Clone, Debug)]
pub struct SecretItem {
    pub name: String,
    pub labels: String,
    pub keys: String,
    pub type_: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

fn convert_secret(secret: Secret) -> Option<SecretItem> {
    let name = secret.name().unwrap().to_string();
    let namespace = secret.metadata.namespace.clone();
    Some(SecretItem {
        name,
        labels: secret.metadata.labels.unwrap_or_default().into_iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(", "),
        keys: secret.data.as_ref().map(|d| d.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_else(|| "-".into()),
        type_: secret.type_.unwrap_or_else(|| "-".into()),
        creation_timestamp: secret.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_secrets(client: Arc<Client>, secrets_list: Arc<Mutex<Vec<SecretItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Secret> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = secrets_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_secret).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                watcher::Event::Init => initial.clear(),
                watcher::Event::InitApply(secret) => {
                    if let Some(item) = convert_secret(secret) {
                        initial.push(item);
                    }
                }
                watcher::Event::InitDone => {
                    let mut list = secrets_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                watcher::Event::Apply(secret) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_secret(secret) {
                        let mut list = secrets_list.lock().unwrap();
                        list.push(item);
                    }
                }
                watcher::Event::Delete(secret) => {
                    if let Some(item) = secret.metadata.name {
                        let mut secrets_vec = secrets_list.lock().unwrap();
                        secrets_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Secret watch error: {:?}", e),
        }
    }
}
