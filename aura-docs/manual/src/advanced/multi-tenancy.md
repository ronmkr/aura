# Multi-Tenancy & Resource Isolation

Aura is designed to operate in shared environments where multiple users or applications share a single download daemon. To ensure fairness and security, Aura implements a robust **Multi-Tenancy** system (Decision 0032).

## Tenant Context (`TenantContext`)

Every download task in Aura belongs to a **Tenant**. By default, if no tenant is specified, tasks are assigned to the `Default` tenant.

### Key Isolation Features:

1.  **Bandwidth Throttling**: 
    - Each tenant has its own dedicated **Hierarchical Token Bucket**.
    - A user can be capped at 1MB/s without affecting the speeds of other tenants sharing the same daemon.
    - Limits are enforced at the actor level, preventing "noisy neighbor" scenarios.

2.  **Task Quotas**:
    - Limits the number of concurrent and active tasks per tenant.
    - Prevents a single user from exhausting the system's file descriptor or memory limits by spawning thousands of tasks.

3.  **Directory Isolation (`disk_path_root`)**:
    - Each tenant can be assigned a unique root directory on disk.
    - Aura's **Mapping Engine** is strictly relative to this root. It is physically impossible for a tenant to save a file outside of their assigned sandbox, even if they use malicious `../` path mapping rules.

4.  **Security Tokens**:
    - Tenants are identified by unique **X-Aura-Token** credentials.
    - The daemon automatically filters the task list (`tellActive`, `tellStatus`) based on the token provided, ensuring users can only see and manage their own downloads.

## Structured Audit Tracing

For multi-user systems, observability is critical. Aura includes high-fidelity tracing:

- **JSON Logs**: All internal actor transitions are logged in structured JSON format.
- **Request Tagging**: Every piece request and network handshake is tagged with the `TenantId`.
- **Performance Auditing**: Admins can parse the logs to determine exactly how much bandwidth each tenant has consumed over a specific period.

## Example Tenant Configuration

Tenants are configured via the daemon's internal state or a specialized configuration file (in multi-user mode):

```toml
# Tenant ID: "User_A"
[tenants.User_A]
max_tasks = 20
max_download_speed = 5242880 # 5MB/s
disk_path_root = "/home/user_a/downloads"
```
