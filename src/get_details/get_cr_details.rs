use std::sync::Arc;
use kube::ResourceExt;
use kube::{api::{ApiResource, DynamicObject, GroupVersionKind, ListParams}, Api, Client};

pub struct CustomResourceDetails {
    pub name: String,
}

pub async fn get_cr_details(
    client: Arc<Client>,
    name: String, // Имя экземпляра, например "my-cert"
    group: String,
    version: String,
    kind: String,
    plural: String,
    scope: String,
    namespace: Option<String>, // Namespace, где находится экземпляр
) -> Result<CustomResourceDetails, String> {
    let ar = ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk(&group, &version, &kind), &plural);

    println!("name: {}", &name);
    println!("group: {}", &group);
    println!("version: {}", &version);
    println!("scope: {}", &scope);
    println!("plural: {}", &plural);
    println!("namespace: {:?}", &namespace);

    let api: Api<DynamicObject> = if scope == "Namespaced" {
        Api::namespaced_with(client.as_ref().clone(), namespace.as_deref().unwrap_or("default"), &ar)
    } else {
        Api::all_with(client.as_ref().clone(), &ar)
    };

    let obj = api.get(&name).await.map_err(|e| e.to_string())?;

    let details = CustomResourceDetails {
        name: obj.metadata.name.clone().unwrap(),
    };
    Ok(details)
}
