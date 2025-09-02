use std::sync::Arc;
use kube::{api::{ApiResource, DynamicObject, GroupVersionKind, ListParams}, Api, Client};
use kube::ResourceExt;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct CrdInstance {
    pub name: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub data: Value,
}

pub async fn get_cr_instances(
    client: Arc<Client>,
    group: String,
    version: String,
    kind: String,
    plural: String,
    scope: String,
) -> Result<Vec<CrdInstance>, String> {
    let ar = ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk(&group, &version, &kind), &plural);

    let api: Api<DynamicObject> = if scope == "Namespaced" {
        Api::namespaced_with(client.as_ref().clone(), "", &ar)
    } else {
        Api::all_with(client.as_ref().clone(), &ar)
    };

    let list = api
        .list(&ListParams::default())
        .await
        .map_err(|e| e.to_string())?;

    let instances = list
        .iter()
        .map(|item| {
            let mut obj = serde_json::to_value(&item.data).unwrap_or(Value::Null);

            if let Value::Object(ref mut map) = obj {
                map.insert("metadata".into(), serde_json::to_value(&item.metadata).unwrap());
            }

            CrdInstance {
                name: item.name_any(),
                kind: kind.clone(),
                namespace: item.namespace(),
                data: obj,
            }
        })
        .collect();

    Ok(instances)
}
