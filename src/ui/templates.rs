pub const NAMESPACE_TEMPLATE: &'static str = r#"apiVersion: v1
kind: Namespace
metadata:
  name: namespace-name
"#;

pub const PVC_TEMPLATE: &'static str = r#"apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: pvc-name
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
  name: pod-name
  namespace: default
spec:
  containers:
    - name: pod-container
      image: pod-image
"#;

pub const SECRET_TEMPLATE: &'static str = r#"apiVersion: v1
kind: Secret
metadata:
  name: secret-name
  namespace: default
data:
  key: cXFx
type: Opaque
"#;

pub const SERVICE_ACCOUNT_TEMPLATE: &'static str = r#"apiVersion: v1
kind: ServiceAccount
metadata:
  name: service-account-name
  namespace: default
"#;

pub const ROLE_TEMPLATE: &'static str = r#"apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: role-name
  namespace: default
"#;

pub const CLUSTER_ROLE_TEMPLATE: &'static str = r#"apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: role-name
"#;

pub const EXTERNAL_SECRET_TEMPLATE: &'static str = r#"apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: secret-name
  namespace: default
spec:
  dataFrom:
    - extract:
        conversionStrategy: Default
        decodingStrategy: None
        key: path/to/hashicorp/vault
  refreshInterval: 1h
  secretStoreRef:
    kind: ClusterSecretStore
    name: external-secrets-secret-store
  target:
    creationPolicy: Owner
    deletionPolicy: Retain
    name: secret-name
    template:
      engineVersion: v2
      mergePolicy: Replace
      type: Opaque
"#;
