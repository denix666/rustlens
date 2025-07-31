use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{storage::v1::CSIDriver}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{Client, Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct CSIDriverItem {
    pub name: String,
    pub attach_required: String,
    pub pod_info_on_mount: String,
    pub storage_capacity: String,
    pub fs_group_policy: String,
    pub creation_timestamp: Option<Time>,
}

pub fn convert_csi_driver(driver: CSIDriver) -> Option<CSIDriverItem> {
    Some(CSIDriverItem {
        name: driver.metadata.name.clone()?,
        attach_required: driver
            .spec
            .attach_required
            .map_or("Unknown".to_string(), |b| if b { "Yes" } else { "No" }.to_string()),
        pod_info_on_mount: driver
            .spec
            .pod_info_on_mount
            .map_or("Unknown".to_string(), |b| if b { "Yes" } else { "No" }.to_string()),
        storage_capacity: driver
            .spec
            .storage_capacity
            .map_or("Unknown".to_string(), |b| if b { "Yes" } else { "No" }.to_string()),
        fs_group_policy: driver
            .spec
            .fs_group_policy
            .as_ref()
            .map_or("Unknown".to_string(), |s| s.clone()),
        creation_timestamp: driver.metadata.creation_timestamp,
    })
}

pub async fn watch_csi_drivers(client: Arc<Client>, csi_list: Arc<Mutex<Vec<CSIDriverItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<CSIDriver> = Api::all(client.as_ref().clone());
    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    load_status.store(true, Ordering::Relaxed);

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(driver) => {
                    if let Some(item) = convert_csi_driver(driver) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = csi_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(driver) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_csi_driver(driver) {
                        let mut list = csi_list.lock().unwrap();
                        list.push(item);
                    }
                }
                Event::Delete(driver) => {
                    if let Some(item) = driver.metadata.name {
                        let mut drivers_vec = csi_list.lock().unwrap();
                        drivers_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("CSIDriver watch error: {:?}", e),
        }
    }
}
