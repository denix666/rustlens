use chrono::{DateTime, Utc};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use serde_json::Value;
use crate::{functions::item_color, theme::ITEM_NAME_COLOR};

// Cluster secret store
/////////////////////////////////////////////
pub fn show_cluster_secret_store_header(ui: &mut egui::Ui) {
    ui.label("Name");
    ui.label("Message");
    ui.label("Reason");
    ui.label("Type");
    ui.label("Status");
    ui.label("Capabilities");
    ui.label("Age");
    ui.end_row();
}

pub fn show_cluster_secret_store_details(name: &String, data: &Value, ui: &mut egui::Ui) {
    let status_obj = data.get("status");
    let metadata_obj = data.get("metadata");

    let message = status_obj
        .and_then(|s| s.get("conditions"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|cond| cond.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let reason = status_obj
        .and_then(|s| s.get("conditions"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|cond| cond.get("reason"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let ctype = status_obj
        .and_then(|s| s.get("conditions"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|cond| cond.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let cstatus = status_obj
        .and_then(|s| s.get("conditions"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|cond| cond.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let capabilities = status_obj
        .and_then(|s| s.get("capabilities"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let creation_timestamp = metadata_obj
        .and_then(|s| s.get("creationTimestamp"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let datetime: DateTime<Utc> = creation_timestamp.parse().unwrap_or_default();
    let creation_timestamp_k8s_time = Time(datetime);

    ui.label(egui::RichText::new(name).color(ITEM_NAME_COLOR));
    ui.label(message);
    ui.label(reason);
    ui.label(egui::RichText::new(ctype).color(item_color(ctype)));
    ui.label(cstatus);
    ui.label(capabilities);
    ui.label(crate::format_age(&creation_timestamp_k8s_time));
    ui.end_row();
}

// External secret
/////////////////////////////////////////////
pub fn show_external_secret_header(ui: &mut egui::Ui) {
    ui.label("Name");
    ui.label("Namespace");
    ui.label("StoreType");
    ui.label("Refresh interval");
    ui.label("Status");
    ui.label("Reason");
    ui.label("Age");
    ui.end_row();
}

pub fn show_external_secret_details(name: &String, data: &Value, ui: &mut egui::Ui) {
    let metadata_obj = data.get("metadata");
    let spec_obj = data.get("spec");
    let status_obj = data.get("status");

    let ctype = status_obj
        .and_then(|s| s.get("conditions"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|cond| cond.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let namespace = metadata_obj
        .and_then(|s| s.get("namespace"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let refresh_interval = spec_obj
        .and_then(|s| s.get("refreshInterval"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let store_type = spec_obj
        .and_then(|s| s.get("secretStoreRef"))
        .and_then(|cond| cond.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let creation_timestamp = metadata_obj
        .and_then(|s| s.get("creationTimestamp"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let datetime: DateTime<Utc> = creation_timestamp.parse().unwrap_or_default();
    let creation_timestamp_k8s_time = Time(datetime);

    let reason = status_obj
        .and_then(|s| s.get("conditions"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|cond| cond.get("reason"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    ui.label(egui::RichText::new(name).color(ITEM_NAME_COLOR));
    ui.label(namespace);
    ui.label(store_type);
    ui.label(refresh_interval);
    ui.label(egui::RichText::new(ctype).color(item_color(ctype)));
    ui.label(reason);
    ui.label(crate::format_age(&creation_timestamp_k8s_time));

    ui.end_row();
}

// Virtual service
/////////////////////////////////////////////
pub fn show_virtual_service_header(ui: &mut egui::Ui) {
    ui.label("Name");
    ui.label("Namespace");
    ui.label("Gateway");
    ui.label("Host");
    ui.label("Age");
    ui.end_row();
}

pub fn show_virtual_service_details(name: &String, data: &Value, ui: &mut egui::Ui) {
    let metadata_obj = data.get("metadata");
    let spec_obj = data.get("spec");

    let namespace = metadata_obj
        .and_then(|s| s.get("namespace"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let creation_timestamp = metadata_obj
        .and_then(|s| s.get("creationTimestamp"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let datetime: DateTime<Utc> = creation_timestamp.parse().unwrap_or_default();
    let creation_timestamp_k8s_time = Time(datetime);

    let gateways = spec_obj
        .and_then(|s| s.get("gateways"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let hosts = spec_obj
        .and_then(|s| s.get("hosts"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    ui.label(egui::RichText::new(name).color(ITEM_NAME_COLOR));
    ui.label(namespace);
    ui.label(gateways);
    ui.label(hosts);
    ui.label(crate::format_age(&creation_timestamp_k8s_time));

    ui.end_row();
}


// CiliumLoadBalancerIPPool
/////////////////////////////////////////////
pub fn show_cilium_load_balancer_ip_pool_header(ui: &mut egui::Ui) {
    ui.label("Name");
    ui.label("Age");
    ui.label("allowFirstLastIPs");
    ui.label("IP Pools");
    ui.label("Disabled");
    ui.end_row();
}

pub fn show_cilium_load_balancer_ip_pool_details(name: &String, data: &Value, ui: &mut egui::Ui) {
    let metadata_obj = data.get("metadata");
    let spec_obj = data.get("spec");

    let creation_timestamp = metadata_obj
        .and_then(|s| s.get("creationTimestamp"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let datetime: DateTime<Utc> = creation_timestamp.parse().unwrap_or_default();
    let creation_timestamp_k8s_time = Time(datetime);

    let allow_first_last_ips = spec_obj
        .and_then(|s| s.get("allowFirstLastIPs"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    let disabled = spec_obj
        .and_then(|s| s.get("disabled"))
        .and_then(|v| v.as_bool()).unwrap_or_default();

    let blocks_str = spec_obj
        .and_then(|s| s.get("blocks"))
        .and_then(|b| b.as_array())
        .map(|blocks_array| {
            blocks_array
                .iter()
                .map(|block_obj| {
                    if let Some(cidr) = block_obj.get("cidr").and_then(|v| v.as_str()) {
                        cidr.to_string()
                    } else if let (Some(start), Some(stop)) = (
                        block_obj.get("start").and_then(|v| v.as_str()),
                        block_obj.get("stop").and_then(|v| v.as_str())
                    ) {
                        format!("{} - {}", start, stop)
                    } else {
                        "?".to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "-".to_string());

    ui.label(egui::RichText::new(name).color(ITEM_NAME_COLOR));
    ui.label(crate::format_age(&creation_timestamp_k8s_time));
    ui.label(allow_first_last_ips);
    ui.label(blocks_str);
    if disabled {
        ui.label("Yes");
    } else {
        ui.label("No");
    }

    ui.end_row();
}
