pub const NAMESPACE_TEMPLATE: &'static str = r#"apiVersion: v1
kind: Namespace
metadata:
  name: namespace-name
"#;

pub const CONFIGMAP_TEMPLATE: &'static str = r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: configmap-name
  namespace: namespace-name
data:
  key: "value"
"#;

pub const PVC_TEMPLATE: &'static str = r#"apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: pvc-name
  namespace: namespace-name
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 1Gi
  storageClassName: storage-class-name
"#;

pub const POD_TEMPLATE: &'static str = r#"apiVersion: v1
kind: Pod
metadata:
  name: pod-name
  namespace: namespace-name
spec:
  containers:
    - name: pod-container
      image: nginx
"#;

pub const POD_WITH_PVC_TEMPLATE: &'static str = r#"apiVersion: v1
kind: Pod
metadata:
  name: pod-name
  namespace: namespace-name
spec:
  containers:
    - name: pod-container
      image: nginx
      volumeMounts:
        - name: volume-name
          mountPath: /testdata
  volumes:
    - name: volume-name
      persistentVolumeClaim:
        claimName: pvc-name
"#;

pub const SECRET_TEMPLATE: &'static str = r#"apiVersion: v1
kind: Secret
metadata:
  name: secret-name
  namespace: namespace-name
data:
  key: cXFx
type: Opaque
"#;

pub const SERVICE_ACCOUNT_TEMPLATE: &'static str = r#"apiVersion: v1
kind: ServiceAccount
metadata:
  name: service-account-name
  namespace: namespace-name
"#;

pub const ROLE_TEMPLATE: &'static str = r#"apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: role-name
  namespace: namespace-name
"#;

pub const ROLE_BINDING_TEMPLATE: &'static str = r#"apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: role-binding-name
  namespace: namespace-name
"#;

pub const CLUSTER_ROLE_TEMPLATE: &'static str = r#"apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: cluster-role-name
"#;

pub const CLUSTER_ROLE_BINDING_TEMPLATE: &'static str = r#"apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: cluster-role-binding-name
"#;

pub const INGRESS_TEMPLATE: &'static str = r#"apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: ingress-name
  namespace: namespace-name
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
spec:
  ingressClassName: nginx
  rules:
  - http:
      paths:
      - path: /testpath
        pathType: Prefix
        backend:
          service:
            name: service-name
            port:
              number: 80
"#;

pub const SERVICE_TEMPLATE: &'static str = r#"apiVersion: v1
kind: Service
metadata:
  name: service-name
  namespace: namespace-name
  labels:
    app: app-name
spec:
  selector:
    app.kubernetes.io/name: app-name
  ports:
    - port: 80
"#;

pub const DAEMONSET_TEMPLATE: &'static str = r#"apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: daemonset-name
  namespace: namespace-name
"#;

pub const REPLICASET_TEMPLATE: &'static str = r#"apiVersion: apps/v1
kind: ReplicaSet
metadata:
  name: replicaset-name
  namespace: namespace-name
  labels:
    app: app-name
spec:
  replicas: 3
  selector:
    matchLabels:
      app: app-name
  template:
    metadata:
      labels:
        app: app-name
    spec:
      containers:
      - name: nginx
        image: nginx
        ports:
        - containerPort: 80
"#;

pub const DEPLOYMENT_TEMPLATE: &'static str = r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: deployment-name
  namespace: namespace-name
  labels:
    app: app-name
spec:
  replicas: 3
  selector:
    matchLabels:
      app: app-name
  template:
    metadata:
      labels:
        app: app-name
    spec:
      containers:
      - name: nginx
        image: nginx
        ports:
        - containerPort: 80
"#;

pub const EXTERNAL_SECRET_TEMPLATE: &'static str = r#"apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: secret-name
  namespace: namespace-name
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
