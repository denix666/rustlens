use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use futures_util::StreamExt;
use k8s_openapi::{api::{batch::v1::Job}, apimachinery::pkg::apis::meta::v1::Time};
use kube::{api::ListParams, Client};
use kube::{Api, runtime::watcher, runtime::watcher::Event};

//#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct JobItem {
    pub name: String,
    //pub labels: BTreeMap<String, String>,
    pub completions: i32,
    pub condition: String,
    pub creation_timestamp: Option<Time>,
    pub namespace: Option<String>,
}

pub fn convert_job(job: Job) -> Option<JobItem> {
    let condition = job
        .status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .map(|conds| {
            if conds.iter().any(|c| c.type_ == "Complete" && c.status == "True") {
                "Complete".to_string()
            } else if conds.iter().any(|c| c.type_ == "Failed" && c.status == "True") {
                "Failed".to_string()
            } else {
                "Running".to_string()
            }
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let namespace = job.metadata.namespace.clone();

    Some(JobItem {
        name: job.metadata.name.clone()?,
        //labels: job.metadata.labels.unwrap_or_default(),
        completions: job
            .status
            .as_ref()
            .and_then(|s| s.succeeded)
            .unwrap_or(0),
        condition,
        creation_timestamp: job.metadata.creation_timestamp,
        namespace,
    })
}

pub async fn watch_jobs(client: Arc<Client>, jobs_list: Arc<Mutex<Vec<JobItem>>>, load_status: Arc<AtomicBool>) {
    let api: Api<Job> = Api::all(client.as_ref().clone());

    load_status.store(true, Ordering::Relaxed);

    // first-fast load
    if let Ok(ol) = api.list(&ListParams::default()).await {
        let mut items = jobs_list.lock().unwrap();
        *items = ol.into_iter().filter_map(convert_job).collect();
    }

    let mut stream = watcher(api, watcher::Config::default()).boxed();
    let mut initial = vec![];
    let mut initialized = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => match ev {
                Event::Init => initial.clear(),
                Event::InitApply(job) => {
                    if let Some(item) = convert_job(job) {
                        initial.push(item);
                    }
                }
                Event::InitDone => {
                    let mut list = jobs_list.lock().unwrap();
                    *list = initial.clone();
                    initialized = true;

                    load_status.store(false, Ordering::Relaxed);
                }
                Event::Apply(job) => {
                    if !initialized {
                        continue;
                    }
                    if let Some(item) = convert_job(job) {
                        let mut list = jobs_list.lock().unwrap();
                        if let Some(existing) = list.iter_mut().find(|f| f.name == item.name && f.namespace == item.namespace) {
                            *existing = item; // renew
                        } else {
                            list.push(item); // add new
                        }
                    }
                }
                Event::Delete(job) => {
                    if let Some(item) = job.metadata.name {
                        let mut job_vec = jobs_list.lock().unwrap();
                        job_vec.retain(|p| p.name != item);
                    }
                }
            },
            Err(e) => eprintln!("Job watch error: {:?}", e),
        }
    }
}
