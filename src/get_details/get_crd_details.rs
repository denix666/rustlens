use kube::{Api, Client};
use std::{collections::BTreeMap, sync::{Arc, Mutex}};
use kube::api::{DynamicObject, GroupVersionKind};
use kube::discovery;

#[derive(Default, Debug, Clone)]
pub struct CrdDetails {
    pub name: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
}

pub async fn get_crd_details(client: Arc<Client>, name: &str, details: Arc<Mutex<CrdDetails>>) -> Result<(), kube::Error> {
    let (ar, _caps) = discovery::pinned_kind(&client, &GroupVersionKind::gvk("apiextensions.k8s.io", "v1", "CustomResourceDefinition")).await.unwrap();
    let api: Api<DynamicObject> = Api::all_with(client.as_ref().clone(), &ar);

    let crd = api.get(name).await.unwrap();
    let mut details_items = details.lock().unwrap();
    let metadata = crd.metadata.clone();

    details_items.name = metadata.name;
    details_items.labels = metadata.labels.clone();
    details_items.annotations = metadata.annotations.clone();

    Ok(())
}
