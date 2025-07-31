use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{storage::v1::StorageClass}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct StorageClassItem {
    pub name: String,
    //pub labels: BTreeMap<String, String>,
    pub provisioner: String,
    pub reclaim_policy: String,
    pub volume_binding_mode: String,
    pub is_default: String,
    pub creation_timestamp: Option<Time>,
}

pub fn convert_storage_class(sc: StorageClass) -> Option<StorageClassItem> {
    Some(StorageClassItem {
        name: sc.metadata.name.clone()?,
        //labels: sc.metadata.labels.clone().unwrap_or_default(),
        provisioner: sc.provisioner.clone(),
        reclaim_policy: sc
            .reclaim_policy
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        volume_binding_mode: sc
            .volume_binding_mode
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        is_default: match sc.metadata.annotations {
            Some(ann) => {
                if let Some(val) = ann.get("storageclass.kubernetes.io/is-default-class") {
                    if val == "true" {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    }
                } else {
                    "no".to_string()
                }
            }
            None => "no".to_string(),
        },
        creation_timestamp: sc.metadata.creation_timestamp,
    })
}

pub async fn watch_storage_classes(client: Arc<Client>, sc_list: Arc<Mutex<Vec<StorageClassItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<StorageClass> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(sc) => {
                    if let Some(item) = convert_storage_class(sc) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = sc_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(sc) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_storage_class(sc) {
                        let mut list = sc_list.lock().unwrap();
                        list.push(item);
                    }
                }
                Event::Delete(_) => {}
            },
            Err(e) => eprintln!("StorageClass watch error: {:?}", e),
        }
    }
}
