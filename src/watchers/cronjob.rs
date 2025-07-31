use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::batch::v1::CronJob, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};

#[derive(Debug, Clone)]
pub struct CronJobItem {
    pub name: String,
    pub schedule: String,
    pub suspend: String,
    pub active: usize,
    pub last_schedule: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_cronjob(cj: CronJob) -> Option<CronJobItem> {
    let metadata = &cj.metadata;
    let name = metadata.name.clone()?;
    let creation_timestamp =  metadata.creation_timestamp.clone();
    let namespace = cj.metadata.namespace.clone();
    let spec = cj.spec?;
    let schedule = spec.schedule;
    let suspend = spec.suspend.unwrap_or(false);

    let active = cj.status
        .as_ref()
        .and_then(|s| s.active.as_ref()
        .map(|a| a.len())).unwrap_or(0);

    let last_schedule = cj.status
        .as_ref()
        .and_then(|s| s.last_schedule_time.as_ref())
        .map(|t| t.0.to_rfc3339())
        .unwrap_or_else(|| "-".to_string());

    Some(CronJobItem {
        name,
        schedule,
        suspend: if suspend { "true".into() } else { "false".into() },
        active,
        last_schedule,
        creation_timestamp,
        namespace,
    })
}

pub async fn watch_cronjobs(client: Arc<Client>, cronjob_list: Arc<Mutex<Vec<CronJobItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<CronJob> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = cronjob_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_cronjob).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();

    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(cronjob) => {
                    if let Some(item) = convert_cronjob(cronjob) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list_guard = cronjob_list.lock().unwrap();
                    *list_guard = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(cronjob) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_cronjob(cronjob) {
                        let mut list = cronjob_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(cronjob) => {
                    if let Some(item) = cronjob.metadata.name {
                        let mut cronjobs_vec = cronjob_list.lock().unwrap();
                        cronjobs_vec.retain(|n| n.name != item);
                    }
                }
            },
            Err(e) => eprintln!("CronJob watch error: {:?}", e),
        }
    }
}
