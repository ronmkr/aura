# ADR 0037: Redirect Handling and Content Validation

## Status
Accepted

## Context
When a user provides a URI, it may not be a direct link to a file. It could be a 3xx redirect chain, or a landing page containing a download button. Naively following these can lead to "poisoning" where the engine saves HTML landing pages as binary data.

## Decision
1. **Redirect Manager**: The **HttpWorker** will use a dedicated manager to follow redirects. It will:
    - Limit the chain length (default: 10).
    - Prevent "Protocol Downgrade" (HTTPS -> HTTP).
    - Propagate cookies and referrers across the chain.
2. **MIME Validator**: Before writing data to the **Storage Engine**, the worker must validate the `Content-Type` header. If the user expects a binary file but receives `text/html`, the engine must pause the task and trigger the **Landing Page Resolver**.
3. **Headless Pre-flight**: For complex sites, the engine will perform a `HEAD` request or a "pre-flight" `GET` (reading only the first few bytes) to verify the resource before allocating disk space.

## Alternatives Considered
- **Blind Following**: Always follow and save whatever the server sends. *Rejected:* Leads to corrupted downloads and wasted bandwidth.
- **Manual Mirroring Only**: Force users to provide direct links. *Rejected:* Poor UX; modern tools like `curl` and `aria2` handle redirects automatically.

## Consequences
- **Pros**: Robustness against complex web hosting setups, prevention of "HTML-as-Binary" corruption, and improved security.
- **Cons**: Adds latency due to pre-flight requests and header validation.
