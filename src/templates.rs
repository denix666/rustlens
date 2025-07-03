pub const NAMESPACE_TEMPLATE: &'static str = r#"apiVersion: v1
kind: Namespace
metadata:
  name: namespace_name
"#;

pub const PVC_TEMPLATE: &'static str = r#"apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: pvc_name
  namespace: default
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 1Gi
  storageClassName: default
"#;

pub const POD_TEMPLATE: &'static str = r#"apiVersion: v1
kind: Pod
metadata:
  name: pod_name
  namespace: default
spec:
  containers:
    - name: pod_container
      image: pod_image
"#;
