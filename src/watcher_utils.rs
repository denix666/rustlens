use kube::{Api, Client, Resource, runtime::watcher};
use kube::api::ListParams;
use futures::StreamExt;
use serde::de::DeserializeOwned;
use std::sync::{Arc, Mutex};

pub async fn init_and_watch<T, Item>(
    client: Arc<Client>,
    namespace: String,
    list: Arc<Mutex<Vec<Item>>>,
    api_fn: fn(Arc<Client>, &str) -> Api<T>,
    convert_fn: fn(T) -> Option<Item>,
    get_name_fn: fn(&Item) -> String,
)
where
    T: Clone + Resource + DeserializeOwned + std::fmt::Debug + Send + Sync + 'static,
    Item: Send + Sync + Clone + 'static,
{
    let api = api_fn(client.clone(), &namespace);

    // 🟢 Инициализация
    match api.list(&ListParams::default()).await {
        Ok(obj_list) => {
            let initial: Vec<Item> = obj_list.into_iter().filter_map(convert_fn).collect();
            let mut locked = list.lock().unwrap();
            *locked = initial;
        }
        Err(err) => {
            eprintln!("Failed to list resources in init_and_watch: {:?}", err);
        }
    }

    // 🔄 Watcher
    let mut stream = watcher(api_fn(client.clone(), &namespace), watcher::Config::default()).boxed();
    let list_clone = Arc::clone(&list);

    tokio::spawn(async move {
        let mut initialized = false;
        let mut init_data = vec![];

        while let Some(event) = stream.next().await {
            match event {
                Ok(ev) => match ev {
                    watcher::Event::Init => init_data.clear(),
                    watcher::Event::InitApply(obj) => {
                        if let Some(item) = convert_fn(obj) {
                            init_data.push(item);
                        }
                    }
                    watcher::Event::InitDone => {
                        let mut lock = list_clone.lock().unwrap();
                        *lock = init_data.clone();
                        initialized = true;
                    }
                    watcher::Event::Apply(obj) => {
                        if !initialized {
                            continue;
                        }
                        if let Some(new_item) = convert_fn(obj) {
                            let mut lock = list_clone.lock().unwrap();
                            let name = get_name_fn(&new_item);
                            if let Some(pos) = lock.iter().position(|i| get_name_fn(i) == name) {
                                lock[pos] = new_item;
                            } else {
                                lock.push(new_item);
                            }
                        }
                    }
                    watcher::Event::Delete(obj) => {
                        if let Some(name) = obj.meta().name.clone() {
                            let mut lock = list_clone.lock().unwrap();
                            lock.retain(|i| get_name_fn(i) != name);
                        }
                    }
                },
                Err(err) => {
                    eprintln!("Watcher error: {:?}", err);
                }
            }
        }
    });
}
