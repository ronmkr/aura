# Security Policy

## Supported Versions

We provide security updates for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

We take the security of Aura seriously. If you believe you have found a security vulnerability, please do **not** report it via a public issue.

Instead, please report it through one of the following channels:
1. **GitHub Security Advisory**: Use the "Report a vulnerability" button on the [Security tab](https://github.com/ronmkr/aura/security/advisories/new) of this repository.
2. **Email**: Send a detailed report to raunak.jyotishi@gmail.com.

Please include the following in your report:
- A description of the vulnerability.
- Steps to reproduce the issue.
- Potential impact.
- Any suggested fixes or mitigations.

We will acknowledge receipt of your report within 48 hours and provide a timeline for resolution.

## Security Mandates

Aura adheres to strict security standards:
- **Zero-Unwrap Policy**: Library code must never panic.
- **Dependency Auditing**: Automated Dependabot scans and periodic `cargo audit` runs.
- **Sandboxing**: All file I/O is restricted to authorized directories.
- **VPN Kill-switch**: Traffic is blocked if the authorized network interface is unavailable.
