# Kora Helm Chart

Deploy [Kora](https://github.com/Romderful/kora), a Confluent-compatible Schema Registry backed by PostgreSQL, on Kubernetes.

## Prerequisites

- Kubernetes 1.24+
- Helm 3.8+
- An existing PostgreSQL database

## Quick Start

```bash
helm install kora oci://ghcr.io/romderful/kora/charts/kora \
  --set database.host=my-postgres.example.com \
  --set database.password=my-secret
```

## Database Configuration

Three configuration modes:

### Host and password

```bash
helm install kora oci://ghcr.io/romderful/kora/charts/kora \
  --set database.host=postgres.example.com \
  --set database.password=secret
```

### Full URL (for SSL, custom params)

```bash
helm install kora oci://ghcr.io/romderful/kora/charts/kora \
  --set database.url="postgres://kora:secret@postgres:5432/kora?sslmode=require"
```

### Existing secret (recommended for production)

```bash
kubectl create secret generic kora-db --from-literal=DATABASE_URL="postgres://..."
helm install kora oci://ghcr.io/romderful/kora/charts/kora \
  --set database.existingSecret=kora-db
```

## Parameters

### Global

| Parameter | Description | Default |
|-----------|-------------|---------|
| `global.imageRegistry` | Global Docker image registry | `""` |
| `global.imagePullSecrets` | Global Docker registry secret names | `[]` |

### Common

| Parameter | Description | Default |
|-----------|-------------|---------|
| `nameOverride` | Partial name override | `""` |
| `fullnameOverride` | Full name override | `""` |
| `namespaceOverride` | Namespace override | `""` |
| `commonLabels` | Labels for all resources | `{}` |
| `commonAnnotations` | Annotations for all resources | `{}` |
| `extraDeploy` | Extra objects to deploy | `[]` |

### Image

| Parameter | Description | Default |
|-----------|-------------|---------|
| `image.registry` | Image registry | `ghcr.io` |
| `image.repository` | Image repository | `romderful/kora` |
| `image.tag` | Image tag | `v{appVersion}` |
| `image.digest` | Image digest (overrides tag) | `""` |
| `image.pullPolicy` | Pull policy | `IfNotPresent` |
| `image.pullSecrets` | Image pull secrets | `[]` |

### Kora Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `kora.port` | Listen port | `8080` |
| `kora.logLevel` | Log level (RUST_LOG) | `info` |
| `kora.dbPoolMax` | Max DB connections | `20` |
| `kora.maxBodySize` | Max request body (bytes) | `16777216` |
| `kora.extraEnvVars` | Extra env vars | `[]` |
| `kora.extraEnvVarsCM` | ConfigMap with extra env vars | `""` |
| `kora.extraEnvVarsSecret` | Secret with extra env vars | `""` |

### Database

| Parameter | Description | Default |
|-----------|-------------|---------|
| `database.host` | PostgreSQL host | `""` |
| `database.port` | PostgreSQL port | `5432` |
| `database.user` | PostgreSQL user | `kora` |
| `database.password` | PostgreSQL password | `""` |
| `database.name` | PostgreSQL database | `kora` |
| `database.url` | Full DATABASE_URL override | `""` |
| `database.existingSecret` | Existing secret name | `""` |
| `database.existingSecretKey` | Key in existing secret | `DATABASE_URL` |

### Deployment

| Parameter | Description | Default |
|-----------|-------------|---------|
| `replicaCount` | Number of replicas | `1` |
| `updateStrategy.type` | Deployment strategy | `RollingUpdate` |
| `deploymentAnnotations` | Deployment annotations | `{}` |
| `podAnnotations` | Pod annotations | `{}` |
| `podLabels` | Extra pod labels | `{}` |

### Pod

| Parameter | Description | Default |
|-----------|-------------|---------|
| `terminationGracePeriodSeconds` | Grace period | `30` |
| `schedulerName` | Scheduler name | `""` |
| `priorityClassName` | Priority class | `""` |
| `automountServiceAccountToken` | Mount SA token | `false` |
| `initContainers` | Init containers | `[]` |
| `sidecars` | Sidecar containers | `[]` |

### Security

| Parameter | Description | Default |
|-----------|-------------|---------|
| `podSecurityContext.enabled` | Enable pod security context | `true` |
| `podSecurityContext.runAsNonRoot` | Force non-root | `true` |
| `podSecurityContext.fsGroup` | Set fsGroup | `65534` |
| `podSecurityContext.seccompProfile.type` | Seccomp profile | `RuntimeDefault` |
| `containerSecurityContext.enabled` | Enable container security context | `true` |
| `containerSecurityContext.runAsUser` | Run as user | `65534` |
| `containerSecurityContext.runAsGroup` | Run as group | `65534` |
| `containerSecurityContext.runAsNonRoot` | Force non-root | `true` |
| `containerSecurityContext.readOnlyRootFilesystem` | Read-only root FS | `true` |
| `containerSecurityContext.allowPrivilegeEscalation` | Allow privilege escalation | `false` |
| `containerSecurityContext.capabilities.drop` | Capabilities to drop | `["ALL"]` |
| `containerSecurityContext.seccompProfile.type` | Container seccomp | `RuntimeDefault` |

### Resources

| Parameter | Description | Default |
|-----------|-------------|---------|
| `resources.requests.cpu` | CPU request | `100m` |
| `resources.requests.memory` | Memory request | `64Mi` |
| `resources.limits.cpu` | CPU limit | `500m` |
| `resources.limits.memory` | Memory limit | `256Mi` |

### Probes

All probes have an `enabled` flag and can be fully overridden via `custom*Probe`.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `startupProbe.enabled` | Enable startup probe | `true` |
| `startupProbe.periodSeconds` | Period | `2` |
| `startupProbe.failureThreshold` | Failure threshold | `30` |
| `startupProbe.timeoutSeconds` | Timeout | `3` |
| `readinessProbe.enabled` | Enable readiness probe | `true` |
| `readinessProbe.initialDelaySeconds` | Initial delay | `5` |
| `readinessProbe.periodSeconds` | Period | `5` |
| `readinessProbe.timeoutSeconds` | Timeout | `3` |
| `readinessProbe.failureThreshold` | Failure threshold | `3` |
| `livenessProbe.enabled` | Enable liveness probe (TCP) | `true` |
| `livenessProbe.initialDelaySeconds` | Initial delay | `10` |
| `livenessProbe.periodSeconds` | Period | `10` |
| `livenessProbe.timeoutSeconds` | Timeout | `3` |
| `livenessProbe.failureThreshold` | Failure threshold | `6` |
| `customStartupProbe` | Custom startup probe | `{}` |
| `customReadinessProbe` | Custom readiness probe | `{}` |
| `customLivenessProbe` | Custom liveness probe | `{}` |
| `lifecycleHooks` | Container lifecycle hooks | preStop sleep 3s |

### Service

| Parameter | Description | Default |
|-----------|-------------|---------|
| `service.type` | Service type | `ClusterIP` |
| `service.ports.http` | HTTP port | `8080` |
| `service.nodePorts.http` | NodePort for HTTP | `""` |
| `service.clusterIP` | Static ClusterIP | `""` |
| `service.loadBalancerIP` | Static LB IP | `""` |
| `service.loadBalancerSourceRanges` | LB source ranges | `[]` |
| `service.externalTrafficPolicy` | External traffic policy | `Cluster` |
| `service.annotations` | Service annotations | `{}` |

### Service Account

| Parameter | Description | Default |
|-----------|-------------|---------|
| `serviceAccount.create` | Create ServiceAccount | `true` |
| `serviceAccount.name` | SA name override | `""` |
| `serviceAccount.annotations` | SA annotations (IRSA, Workload Identity) | `{}` |
| `serviceAccount.automountServiceAccountToken` | Mount token | `false` |

### Ingress

| Parameter | Description | Default |
|-----------|-------------|---------|
| `ingress.enabled` | Enable ingress | `false` |
| `ingress.pathType` | Path type | `ImplementationSpecific` |
| `ingress.hostname` | Hostname | `kora.local` |
| `ingress.path` | Path | `/` |
| `ingress.ingressClassName` | Ingress class | `""` |
| `ingress.annotations` | Ingress annotations | `{}` |
| `ingress.tls` | Enable TLS | `false` |
| `ingress.selfSigned` | Self-signed TLS cert | `false` |
| `ingress.extraHosts` | Additional hosts | `[]` |
| `ingress.extraTls` | Additional TLS config | `[]` |
| `ingress.secrets` | Custom TLS secrets | `[]` |
| `ingress.extraRules` | Additional ingress rules | `[]` |

### Metrics

| Parameter | Description | Default |
|-----------|-------------|---------|
| `metrics.enabled` | Enable metrics | `false` |
| `metrics.serviceMonitor.enabled` | Enable ServiceMonitor | `false` |
| `metrics.serviceMonitor.namespace` | ServiceMonitor namespace | `""` |
| `metrics.serviceMonitor.interval` | Scrape interval | `30s` |
| `metrics.serviceMonitor.scrapeTimeout` | Scrape timeout | `10s` |
| `metrics.serviceMonitor.labels` | Extra labels | `{}` |
| `metrics.serviceMonitor.annotations` | Annotations | `{}` |
| `metrics.serviceMonitor.honorLabels` | Honor labels | `false` |
| `metrics.serviceMonitor.metricRelabelings` | Metric relabelings | `[]` |
| `metrics.serviceMonitor.relabelings` | Relabelings | `[]` |

### Autoscaling

| Parameter | Description | Default |
|-----------|-------------|---------|
| `autoscaling.enabled` | Enable HPA | `false` |
| `autoscaling.minReplicas` | Min replicas | `2` |
| `autoscaling.maxReplicas` | Max replicas | `10` |
| `autoscaling.targetCPU` | Target CPU % | `80` |
| `autoscaling.targetMemory` | Target memory % | `""` |

### PDB

| Parameter | Description | Default |
|-----------|-------------|---------|
| `pdb.create` | Enable PDB | `false` |
| `pdb.minAvailable` | Min available | `""` |
| `pdb.maxUnavailable` | Max unavailable | `1` |

### Network Policy

| Parameter | Description | Default |
|-----------|-------------|---------|
| `networkPolicy.enabled` | Enable NetworkPolicy | `false` |
| `networkPolicy.allowExternal` | Allow external ingress | `true` |
| `networkPolicy.allowExternalEgress` | Allow external egress | `true` |
| `networkPolicy.extraIngress` | Extra ingress rules | `[]` |
| `networkPolicy.extraEgress` | Extra egress rules | `[]` |

### Scheduling

| Parameter | Description | Default |
|-----------|-------------|---------|
| `nodeSelector` | Node labels for pod assignment | `{}` |
| `tolerations` | Tolerations | `[]` |
| `affinity` | Affinity rules | `{}` |
| `topologySpreadConstraints` | Topology spread | `[]` |

## Production Example

```yaml
replicaCount: 3

database:
  existingSecret: kora-db-credentials

ingress:
  enabled: true
  hostname: schema-registry.example.com
  tls: true
  ingressClassName: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt

autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10

pdb:
  create: true

metrics:
  enabled: true
  serviceMonitor:
    enabled: true

networkPolicy:
  enabled: true

resources:
  requests:
    cpu: 250m
    memory: 128Mi
  limits:
    cpu: "1"
    memory: 512Mi
```

## Security

The chart enforces [Pod Security Standards "restricted"](https://kubernetes.io/docs/concepts/security/pod-security-standards/) by default:

- Non-root user (UID 65534)
- Read-only root filesystem
- No privilege escalation
- All capabilities dropped
- Seccomp RuntimeDefault profile

## Monitoring

Kora exposes Prometheus metrics at `/metrics`. Enable the ServiceMonitor for automatic scraping with Prometheus Operator:

```yaml
metrics:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 15s
```

## Uninstalling

```bash
helm uninstall kora
```
