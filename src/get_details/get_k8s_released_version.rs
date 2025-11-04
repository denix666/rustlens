use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::sleep;

const K8S_RELEASE_URL: &str = "https://api.github.com/repos/kubernetes/kubernetes/releases/latest";

pub fn parse_k8s_minor(version: &str) -> Option<u32> {
    let s = version.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let mut parts = s.split('.');

    let _major = parts.next()?;                 // "1"
    let minor_str = parts.next()?;              // "34"
    minor_str.parse::<u32>().ok()
}

pub struct KubernetesVersionFetcher {
    version: Arc<Mutex<Option<String>>>,
}

impl KubernetesVersionFetcher {
    /// Создает новый fetcher и сразу запускает фоновую задачу.
    pub fn new() -> Self {
        let version = Arc::new(Mutex::new(None));
        Self::spawn_fetcher(version.clone());
        Self { version }
    }

    /// Возвращает последнее значение версии (или None, если ещё не загружено).
    pub fn get_version(&self) -> Option<String> {
        self.version.lock().unwrap().clone()
    }

    fn spawn_fetcher(shared: Arc<Mutex<Option<String>>>) {
        tokio::spawn(async move {
            loop {
                match reqwest::Client::new()
                    .get(K8S_RELEASE_URL)
                    .header("User-Agent", "rust-egui-client")
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(tag) = json["tag_name"].as_str() {
                                let mut lock = shared.lock().unwrap();
                                *lock = Some(tag.to_string());
                                if let Some(major) = parse_k8s_minor(tag) {
                                    let _ = crate::ACTUAL_K8S_MINOR_VERSION.set(major);
                                }
                                break; // нашли версию – прекращаем попытки
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("Error getting k8s version: {err}");
                    }
                }
                // Ждём 3 минуты перед повтором
                sleep(Duration::from_secs(60)).await;
            }
        });
    }
}
