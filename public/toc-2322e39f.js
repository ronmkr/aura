// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="introduction.html"><strong aria-hidden="true">1.</strong> Introduction</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="installation.html"><strong aria-hidden="true">2.</strong> Installation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="usage.html"><strong aria-hidden="true">3.</strong> Getting Started</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="configuration.html"><strong aria-hidden="true">4.</strong> Configuration Reference: Deep Dive</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="cli-reference.html"><strong aria-hidden="true">5.</strong> Cli Reference</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tui-guide.html"><strong aria-hidden="true">6.</strong> Tui Guide (pilot Dashboard)</a></span></li><li class="chapter-item expanded "><li class="spacer"></li></li><li class="chapter-item expanded "><li class="part-title">Advanced Features</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/aggregation.html"><strong aria-hidden="true">7.</strong> Multi-protocol Aggregation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/bittorrent-v2.html"><strong aria-hidden="true">8.</strong> Bittorrent V2 &amp; Merkle Trees</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/vpn-safety.html"><strong aria-hidden="true">9.</strong> Vpn Safety &amp; Privacy</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/rpc-daemon.html"><strong aria-hidden="true">10.</strong> Rpc &amp; Daemon Mode</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/task-chaining-mapping.html"><strong aria-hidden="true">11.</strong> Task Chaining &amp; Metadata Path Mapping</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/multi-tenancy.html"><strong aria-hidden="true">12.</strong> Multi-tenancy &amp; Resource Isolation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/resource-governor.html"><strong aria-hidden="true">13.</strong> Resource Governor &amp; Memory Backpressure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/network-filesystems.html"><strong aria-hidden="true">14.</strong> Network Filesystems (nfs &amp; Smb)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/examples.html"><strong aria-hidden="true">15.</strong> Real-world Applications: Resource Mapping &amp; Task Chaining</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/telemetry.html"><strong aria-hidden="true">16.</strong> Telemetry &amp; Metrics</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/web-ui.html"><strong aria-hidden="true">17.</strong> Built-in Web Ui</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/safety-integrity.html"><strong aria-hidden="true">18.</strong> Safety &amp; Data Integrity</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/download-history.html"><strong aria-hidden="true">19.</strong> Download History Log</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/resilience.html"><strong aria-hidden="true">20.</strong> Process Resilience &amp; Crash Recovery</a></span></li><li class="chapter-item expanded "><li class="spacer"></li></li><li class="chapter-item expanded "><li class="part-title">Project Management</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="project/ROADMAP.html"><strong aria-hidden="true">21.</strong> Aura: Project Tasks &amp; Roadmap</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="project/TASKS.html"><strong aria-hidden="true">22.</strong> Aura: Development Tasks</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="project/DEVELOPMENT.html"><strong aria-hidden="true">23.</strong> Aura: Developer Setup Guide</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="project/GEMINI.html"><strong aria-hidden="true">24.</strong> Project Instructions</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="project/CONTEXT.html"><strong aria-hidden="true">25.</strong> Context: Aura</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="project/DESIGN.html"><strong aria-hidden="true">26.</strong> Aura Design System</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="project/MANAGEMENT.html"><strong aria-hidden="true">27.</strong> Aura Project Management &amp; Issue Tracking</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="project/MIGRATION.html"><strong aria-hidden="true">28.</strong> Aura: Migration Guide</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="project/API.html"><strong aria-hidden="true">29.</strong> Aura: Public Rust API Documentation</a></span></li><li class="chapter-item expanded "><li class="spacer"></li></li><li class="chapter-item expanded "><li class="part-title">Developer Documentation</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/architecture.html"><strong aria-hidden="true">30.</strong> Aura Architectural Map</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/api-docs.html"><strong aria-hidden="true">31.</strong> API Documentation (rustdoc)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/testing.html"><strong aria-hidden="true">32.</strong> Testing &amp; Verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="advanced/adr-index.html"><strong aria-hidden="true">33.</strong> Design Decisions (decision)</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0001-orchestrated-pull-model.html"><strong aria-hidden="true">33.1.</strong> Decision 0001: Orchestrated Pull Model for Work Assignment</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0002-centralized-storage-writing.html"><strong aria-hidden="true">33.2.</strong> Decision 0002: Centralized Storage Writing and Ownership</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0003-atomic-completion-and-pre-allocation.html"><strong aria-hidden="true">33.3.</strong> Decision 0003: Atomic Completion and Pre-allocation Strategy</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0004-telemetry-and-event-bus.html"><strong aria-hidden="true">33.4.</strong> Decision 0004: Telemetry and Event Bus Architecture</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0005-racing-work-stealer.html"><strong aria-hidden="true">33.5.</strong> Decision 0005: Racing Work Stealer for Slow Stream Mitigation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0006-error-classification-and-self-healing.html"><strong aria-hidden="true">33.6.</strong> Decision 0006: Error Classification and Self-healing Strategy</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0007-protocol-encapsulation.html"><strong aria-hidden="true">33.7.</strong> Decision 0007: Protocol Encapsulation and Black-Box Workers</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0008-lifecycle-based-task-maturation.html"><strong aria-hidden="true">33.8.</strong> Decision 0008: Lifecycle-based Task Maturation (Magnet Links)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0009-global-token-bucket-throttling.html"><strong aria-hidden="true">33.9.</strong> Decision 0009: Global Token Bucket Throttling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0010-decoupled-peer-discovery.html"><strong aria-hidden="true">33.10.</strong> Decision 0010: Decoupled Peer Discovery and Registry</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0011-dynamic-configuration.html"><strong aria-hidden="true">33.11.</strong> Decision 0011: Dynamic Configuration and Hot-reloading</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0012-tui-theming-and-proxy.html"><strong aria-hidden="true">33.12.</strong> Decision 0012: Themeable TUI and Proxy Connector</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0013-cloud-and-metalink.html"><strong aria-hidden="true">33.13.</strong> Decision 0013: Cloud Storage and Metalink Integration</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0014-credential-and-security.html"><strong aria-hidden="true">33.14.</strong> Decision 0014: Credential and Security Abstraction</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0015-url-globbing.html"><strong aria-hidden="true">33.15.</strong> Decision 0015: URL Globbing and Batch Processing</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0016-rpc-and-interface.html"><strong aria-hidden="true">33.16.</strong> Decision 0016: RPC Server and Interface Binding</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0017-segmentation-and-persistence.html"><strong aria-hidden="true">33.17.</strong> Decision 0017: Segmentation and Discovery Persistence</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0018-hooks-hsts-ftp.html"><strong aria-hidden="true">33.18.</strong> Decision 0018: Hooks, HSTS, and Multi-Channel Protocols</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0019-buffer-pool-and-caching.html"><strong aria-hidden="true">33.19.</strong> Decision 0019: Buffer Pool and Write-Back Caching</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0020-engine-api.html"><strong aria-hidden="true">33.20.</strong> Decision 0020: Engine API and Library Embeddability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0021-network-filesystem-optimization.html"><strong aria-hidden="true">33.21.</strong> Decision 0021: Network Filesystem Optimization (NFS/SMB)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0022-disk-io-scheduling.html"><strong aria-hidden="true">33.22.</strong> Decision 0022: Advanced Disk I/O Scheduling and Kernel Hinting</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0023-adaptive-scaling-and-aggregation.html"><strong aria-hidden="true">33.23.</strong> Decision 0023: Adaptive Connection Scaling and Sourced Aggregation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0024-integrity-scrubbing.html"><strong aria-hidden="true">33.24.</strong> Decision 0024: Integrity Scrubbing and Torrent Refreshing</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0025-nat-traversal-and-lpd.html"><strong aria-hidden="true">33.25.</strong> Decision 0025: NAT Traversal and LAN Discovery</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0026-modern-networking.html"><strong aria-hidden="true">33.26.</strong> Decision 0026: Modern Networking (Happy Eyeballs, Alt-Svc, Streaming)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0027-power-management.html"><strong aria-hidden="true">33.27.</strong> Decision 0027: Power Management and Automated Lifecycle Actions</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0028-privacy-dns.html"><strong aria-hidden="true">33.28.</strong> Decision 0028: Privacy-Enhanced Resolution and Modern DNS</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0029-mapping-and-chaining.html"><strong aria-hidden="true">33.29.</strong> Decision 0029: Resource Mapping and Task Chaining</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0030-recursive-mirroring.html"><strong aria-hidden="true">33.30.</strong> Decision 0030: Recursive Mirroring and HTML Parsing</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0031-bittorrent-v2-merkle.html"><strong aria-hidden="true">33.31.</strong> Decision 0031: BitTorrent v2 and Merkle Tree Management</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0032-multi-tenancy-and-tracing.html"><strong aria-hidden="true">33.32.</strong> Decision 0032: Multi-Tenancy and Observability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0033-generation-writes-and-aggregation.html"><strong aria-hidden="true">33.33.</strong> Decision 0033: Generation-based Writes and Sequential Aggregation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0034-advanced-network-edge-cases.html"><strong aria-hidden="true">33.34.</strong> Decision 0034: Advanced Network Edge Cases (kTLS, Roaming, Captive Portals)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0035-advanced-filesystem-edge-cases.html"><strong aria-hidden="true">33.35.</strong> Decision 0035: Advanced Filesystem Edge Cases (COW, Long Paths, Endgame)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0036-bittorrent-core.html"><strong aria-hidden="true">33.36.</strong> Decision 0036: BitTorrent Core and Swarm Management</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0037-redirects-and-validation.html"><strong aria-hidden="true">33.37.</strong> Decision 0037: Redirect Handling and Content Validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0038-vpn-integration.html"><strong aria-hidden="true">33.38.</strong> Decision 0038: Native VPN Integration (OpenVPN, WireGuard)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0039-bittorrent-endgame-mode.html"><strong aria-hidden="true">33.39.</strong> Decision 0039: BitTorrent Endgame Mode</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0040-task-prioritization.html"><strong aria-hidden="true">33.40.</strong> Decision 0040: Task Prioritization and Dependency Chains</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0041-non-swarm-integrity.html"><strong aria-hidden="true">33.41.</strong> Decision 0041: Integrity Verification for Non-Swarm Protocols</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0042-i18n-architecture.html"><strong aria-hidden="true">33.42.</strong> Decision 0042: Internationalization (i18n) Architecture</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0043-unified-architecture.html"><strong aria-hidden="true">33.43.</strong> Decision 0043: Unified Architecture and CLI-Daemon-TUI Integration</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0044-bittorrent-choking-algorithm.html"><strong aria-hidden="true">33.44.</strong> Decision 0044: BitTorrent Choking Algorithm (Tit-for-Tat)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0045-peer-exchange-pex.html"><strong aria-hidden="true">33.45.</strong> Decision 0045: Peer Exchange (PEX) Implementation (BEP 11)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0046-peer-scoring-and-eviction.html"><strong aria-hidden="true">33.46.</strong> Decision 0046: Peer Registry Health Scoring &amp; Eviction</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0047-automated-release-pipeline.html"><strong aria-hidden="true">33.47.</strong> Decision 0047: Automated Release Pipeline</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0048-ftps-tls-support.html"><strong aria-hidden="true">33.48.</strong> Decision 0048: FTPS (TLS) Support and Retry Logic</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0049-browser-bridge.html"><strong aria-hidden="true">33.49.</strong> Decision 0049: Browser Bridge (Extension Support)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0050-integration-tests-suite.html"><strong aria-hidden="true">33.50.</strong> Decision 0050: Integration Tests Suite</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0051-docker-containerization.html"><strong aria-hidden="true">33.51.</strong> Decision 0051: Docker Containerization</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0052-allocation-prober.html"><strong aria-hidden="true">33.52.</strong> Decision 0052: Allocation Prober Diagnostic Tool</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0053-bep12-tracker-tiers.html"><strong aria-hidden="true">33.53.</strong> Decision 0053: BEP 12 Multitracker Compliance</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0054-sandbox-root-confinement.html"><strong aria-hidden="true">33.54.</strong> Decision 0054: SandboxRoot Confinement for Storage Engine</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0055-secret-scrubbing-log-sanitization.html"><strong aria-hidden="true">33.55.</strong> Decision 0055: SecretScrubber for Log Sanitization</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0056-rpc-security-hardening.html"><strong aria-hidden="true">33.56.</strong> Decision 0056: Daemon RPC Security Hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0057-resource-governor.html"><strong aria-hidden="true">33.57.</strong> Decision 0057: ResourceGovernor for Global Memory Backpressure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0058-graceful-shutdown-coordination.html"><strong aria-hidden="true">33.58.</strong> Decision 0058: Graceful Shutdown Coordination</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0059-uri-validation-ssrf-mitigation.html"><strong aria-hidden="true">33.59.</strong> Decision 0059: URI Validation and SSRF Mitigation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0060-pre-download-disk-space-verification.html"><strong aria-hidden="true">33.60.</strong> Decision 0060: Pre-Download Disk Space Verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0061-http-ftp-checksum-verification.html"><strong aria-hidden="true">33.61.</strong> Decision 0061: Checksum Verification for HTTP and FTP Downloads</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0062-download-history-and-aria2-compatibility.html"><strong aria-hidden="true">33.62.</strong> Decision 0062: Download History Log and aura Protocol Compatibility</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0063-bandwidth-time-scheduling.html"><strong aria-hidden="true">33.63.</strong> Decision 0063: Bandwidth Time Scheduling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0064-process-resilience-panic-fd-limits.html"><strong aria-hidden="true">33.64.</strong> Decision 0064: Process Resilience — Panic Recovery, Crash Reporting, and File Descriptor Management</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0065-interactive-tui-architecture.html"><strong aria-hidden="true">33.65.</strong> Decision 0065: Interactive TUI Architecture &amp; Selective Downloading</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0066-mse-pe-encryption.html"><strong aria-hidden="true">33.66.</strong> Decision 0066: MSE/PE Traffic Encryption</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0067-utp-ledbat.html"><strong aria-hidden="true">33.67.</strong> Decision 0067: μTP/LEDBAT Transport Layer</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0068-fast-resume.html"><strong aria-hidden="true">33.68.</strong> Decision 0068: Fast Resume and Piece Recheck</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0069-watch-folder.html"><strong aria-hidden="true">33.69.</strong> Decision 0069: Watch Folder Auto-ingestion</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0070-rss-subscriptions.html"><strong aria-hidden="true">33.70.</strong> Decision 0070: RSS/Atom Feed Subscriptions</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0071-system-service.html"><strong aria-hidden="true">33.71.</strong> Decision 0071: System Service Integration</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/0072-god-node-decoupling.html"><strong aria-hidden="true">33.72.</strong> Decision 0072: Architectural Decoupling of Engine God Nodes</a></span></li></ol><li class="chapter-item expanded "><li class="spacer"></li></li><li class="chapter-item expanded "><li class="part-title">Help &amp; Resources</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="troubleshooting.html"><strong aria-hidden="true">34.</strong> Troubleshooting &amp; Common Issues</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="faq.html"><strong aria-hidden="true">35.</strong> Faq: Frequently Asked Questions</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="glossary.html"><strong aria-hidden="true">36.</strong> Glossary</a></span></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split('#')[0].split('?')[0];
        if (current_page.endsWith('/')) {
            current_page += 'index.html';
        }
        const links = Array.prototype.slice.call(this.querySelectorAll('a'));
        const l = links.length;
        for (let i = 0; i < l; ++i) {
            const link = links[i];
            const href = link.getAttribute('href');
            if (href && !href.startsWith('#') && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The 'index' page is supposed to alias the first chapter in the book.
            // Check both with and without the '.html' suffix to be robust against pretty URLs
            if (link.href.replace(/\.html$/, '') === current_page.replace(/\.html$/, '')
                || i === 0
                && path_to_root === ''
                && current_page.endsWith('/index.html')) {
                link.classList.add('active');
                let parent = link.parentElement;
                while (parent) {
                    if (parent.tagName === 'LI' && parent.classList.contains('chapter-item')) {
                        parent.classList.add('expanded');
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', e => {
            if (e.target.tagName === 'A') {
                const clientRect = e.target.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                sessionStorage.setItem('sidebar-scroll-offset', clientRect.top - sidebarRect.top);
            }
        }, { passive: true });
        const sidebarScrollOffset = sessionStorage.getItem('sidebar-scroll-offset');
        sessionStorage.removeItem('sidebar-scroll-offset');
        if (sidebarScrollOffset !== null) {
            // preserve sidebar scroll position when navigating via links within sidebar
            const activeSection = this.querySelector('.active');
            if (activeSection) {
                const clientRect = activeSection.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                const currentOffset = clientRect.top - sidebarRect.top;
                this.scrollTop += currentOffset - parseFloat(sidebarScrollOffset);
            }
        } else {
            // scroll sidebar to current active section when navigating via
            // 'next/previous chapter' buttons
            const activeSection = document.querySelector('#mdbook-sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        const sidebarAnchorToggles = document.querySelectorAll('.chapter-fold-toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(el => {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define('mdbook-sidebar-scrollbox', MDBookSidebarScrollbox);


// ---------------------------------------------------------------------------
// Support for dynamically adding headers to the sidebar.

(function() {
    // This is used to detect which direction the page has scrolled since the
    // last scroll event.
    let lastKnownScrollPosition = 0;
    // This is the threshold in px from the top of the screen where it will
    // consider a header the "current" header when scrolling down.
    const defaultDownThreshold = 150;
    // Same as defaultDownThreshold, except when scrolling up.
    const defaultUpThreshold = 300;
    // The threshold is a virtual horizontal line on the screen where it
    // considers the "current" header to be above the line. The threshold is
    // modified dynamically to handle headers that are near the bottom of the
    // screen, and to slightly offset the behavior when scrolling up vs down.
    let threshold = defaultDownThreshold;
    // This is used to disable updates while scrolling. This is needed when
    // clicking the header in the sidebar, which triggers a scroll event. It
    // is somewhat finicky to detect when the scroll has finished, so this
    // uses a relatively dumb system of disabling scroll updates for a short
    // time after the click.
    let disableScroll = false;
    // Array of header elements on the page.
    let headers;
    // Array of li elements that are initially collapsed headers in the sidebar.
    // I'm not sure why eslint seems to have a false positive here.
    // eslint-disable-next-line prefer-const
    let headerToggles = [];
    // This is a debugging tool for the threshold which you can enable in the console.
    let thresholdDebug = false;

    // Updates the threshold based on the scroll position.
    function updateThreshold() {
        const scrollTop = window.pageYOffset || document.documentElement.scrollTop;
        const windowHeight = window.innerHeight;
        const documentHeight = document.documentElement.scrollHeight;

        // The number of pixels below the viewport, at most documentHeight.
        // This is used to push the threshold down to the bottom of the page
        // as the user scrolls towards the bottom.
        const pixelsBelow = Math.max(0, documentHeight - (scrollTop + windowHeight));
        // The number of pixels above the viewport, at least defaultDownThreshold.
        // Similar to pixelsBelow, this is used to push the threshold back towards
        // the top when reaching the top of the page.
        const pixelsAbove = Math.max(0, defaultDownThreshold - scrollTop);
        // How much the threshold should be offset once it gets close to the
        // bottom of the page.
        const bottomAdd = Math.max(0, windowHeight - pixelsBelow - defaultDownThreshold);
        let adjustedBottomAdd = bottomAdd;

        // Adjusts bottomAdd for a small document. The calculation above
        // assumes the document is at least twice the windowheight in size. If
        // it is less than that, then bottomAdd needs to be shrunk
        // proportional to the difference in size.
        if (documentHeight < windowHeight * 2) {
            const maxPixelsBelow = documentHeight - windowHeight;
            const t = 1 - pixelsBelow / Math.max(1, maxPixelsBelow);
            const clamp = Math.max(0, Math.min(1, t));
            adjustedBottomAdd *= clamp;
        }

        let scrollingDown = true;
        if (scrollTop < lastKnownScrollPosition) {
            scrollingDown = false;
        }

        if (scrollingDown) {
            // When scrolling down, move the threshold up towards the default
            // downwards threshold position. If near the bottom of the page,
            // adjustedBottomAdd will offset the threshold towards the bottom
            // of the page.
            const amountScrolledDown = scrollTop - lastKnownScrollPosition;
            const adjustedDefault = defaultDownThreshold + adjustedBottomAdd;
            threshold = Math.max(adjustedDefault, threshold - amountScrolledDown);
        } else {
            // When scrolling up, move the threshold down towards the default
            // upwards threshold position. If near the bottom of the page,
            // quickly transition the threshold back up where it normally
            // belongs.
            const amountScrolledUp = lastKnownScrollPosition - scrollTop;
            const adjustedDefault = defaultUpThreshold - pixelsAbove
                + Math.max(0, adjustedBottomAdd - defaultDownThreshold);
            threshold = Math.min(adjustedDefault, threshold + amountScrolledUp);
        }

        if (documentHeight <= windowHeight) {
            threshold = 0;
        }

        if (thresholdDebug) {
            const id = 'mdbook-threshold-debug-data';
            let data = document.getElementById(id);
            if (data === null) {
                data = document.createElement('div');
                data.id = id;
                data.style.cssText = `
                    position: fixed;
                    top: 50px;
                    right: 10px;
                    background-color: 0xeeeeee;
                    z-index: 9999;
                    pointer-events: none;
                `;
                document.body.appendChild(data);
            }
            data.innerHTML = `
                <table>
                  <tr><td>documentHeight</td><td>${documentHeight.toFixed(1)}</td></tr>
                  <tr><td>windowHeight</td><td>${windowHeight.toFixed(1)}</td></tr>
                  <tr><td>scrollTop</td><td>${scrollTop.toFixed(1)}</td></tr>
                  <tr><td>pixelsAbove</td><td>${pixelsAbove.toFixed(1)}</td></tr>
                  <tr><td>pixelsBelow</td><td>${pixelsBelow.toFixed(1)}</td></tr>
                  <tr><td>bottomAdd</td><td>${bottomAdd.toFixed(1)}</td></tr>
                  <tr><td>adjustedBottomAdd</td><td>${adjustedBottomAdd.toFixed(1)}</td></tr>
                  <tr><td>scrollingDown</td><td>${scrollingDown}</td></tr>
                  <tr><td>threshold</td><td>${threshold.toFixed(1)}</td></tr>
                </table>
            `;
            drawDebugLine();
        }

        lastKnownScrollPosition = scrollTop;
    }

    function drawDebugLine() {
        if (!document.body) {
            return;
        }
        const id = 'mdbook-threshold-debug-line';
        const existingLine = document.getElementById(id);
        if (existingLine) {
            existingLine.remove();
        }
        const line = document.createElement('div');
        line.id = id;
        line.style.cssText = `
            position: fixed;
            top: ${threshold}px;
            left: 0;
            width: 100vw;
            height: 2px;
            background-color: red;
            z-index: 9999;
            pointer-events: none;
        `;
        document.body.appendChild(line);
    }

    function mdbookEnableThresholdDebug() {
        thresholdDebug = true;
        updateThreshold();
        drawDebugLine();
    }

    window.mdbookEnableThresholdDebug = mdbookEnableThresholdDebug;

    // Updates which headers in the sidebar should be expanded. If the current
    // header is inside a collapsed group, then it, and all its parents should
    // be expanded.
    function updateHeaderExpanded(currentA) {
        // Add expanded to all header-item li ancestors.
        let current = currentA.parentElement;
        while (current) {
            if (current.tagName === 'LI' && current.classList.contains('header-item')) {
                current.classList.add('expanded');
            }
            current = current.parentElement;
        }
    }

    // Updates which header is marked as the "current" header in the sidebar.
    // This is done with a virtual Y threshold, where headers at or below
    // that line will be considered the current one.
    function updateCurrentHeader() {
        if (!headers || !headers.length) {
            return;
        }

        // Reset the classes, which will be rebuilt below.
        const els = document.getElementsByClassName('current-header');
        for (const el of els) {
            el.classList.remove('current-header');
        }
        for (const toggle of headerToggles) {
            toggle.classList.remove('expanded');
        }

        // Find the last header that is above the threshold.
        let lastHeader = null;
        for (const header of headers) {
            const rect = header.getBoundingClientRect();
            if (rect.top <= threshold) {
                lastHeader = header;
            } else {
                break;
            }
        }
        if (lastHeader === null) {
            lastHeader = headers[0];
            const rect = lastHeader.getBoundingClientRect();
            const windowHeight = window.innerHeight;
            if (rect.top >= windowHeight) {
                return;
            }
        }

        // Get the anchor in the summary.
        const href = '#' + lastHeader.id;
        const a = [...document.querySelectorAll('.header-in-summary')]
            .find(element => element.getAttribute('href') === href);
        if (!a) {
            return;
        }

        a.classList.add('current-header');

        updateHeaderExpanded(a);
    }

    // Updates which header is "current" based on the threshold line.
    function reloadCurrentHeader() {
        if (disableScroll) {
            return;
        }
        updateThreshold();
        updateCurrentHeader();
    }


    // When clicking on a header in the sidebar, this adjusts the threshold so
    // that it is located next to the header. This is so that header becomes
    // "current".
    function headerThresholdClick(event) {
        // See disableScroll description why this is done.
        disableScroll = true;
        setTimeout(() => {
            disableScroll = false;
        }, 100);
        // requestAnimationFrame is used to delay the update of the "current"
        // header until after the scroll is done, and the header is in the new
        // position.
        requestAnimationFrame(() => {
            requestAnimationFrame(() => {
                // Closest is needed because if it has child elements like <code>.
                const a = event.target.closest('a');
                const href = a.getAttribute('href');
                const targetId = href.substring(1);
                const targetElement = document.getElementById(targetId);
                if (targetElement) {
                    threshold = targetElement.getBoundingClientRect().bottom;
                    updateCurrentHeader();
                }
            });
        });
    }

    // Takes the nodes from the given head and copies them over to the
    // destination, along with some filtering.
    function filterHeader(source, dest) {
        const clone = source.cloneNode(true);
        clone.querySelectorAll('mark').forEach(mark => {
            mark.replaceWith(...mark.childNodes);
        });
        dest.append(...clone.childNodes);
    }

    // Scans page for headers and adds them to the sidebar.
    document.addEventListener('DOMContentLoaded', function() {
        const activeSection = document.querySelector('#mdbook-sidebar .active');
        if (activeSection === null) {
            return;
        }

        const main = document.getElementsByTagName('main')[0];
        headers = Array.from(main.querySelectorAll('h2, h3, h4, h5, h6'))
            .filter(h => h.id !== '' && h.children.length && h.children[0].tagName === 'A');

        if (headers.length === 0) {
            return;
        }

        // Build a tree of headers in the sidebar.

        const stack = [];

        const firstLevel = parseInt(headers[0].tagName.charAt(1));
        for (let i = 1; i < firstLevel; i++) {
            const ol = document.createElement('ol');
            ol.classList.add('section');
            if (stack.length > 0) {
                stack[stack.length - 1].ol.appendChild(ol);
            }
            stack.push({level: i + 1, ol: ol});
        }

        // The level where it will start folding deeply nested headers.
        const foldLevel = 3;

        for (let i = 0; i < headers.length; i++) {
            const header = headers[i];
            const level = parseInt(header.tagName.charAt(1));

            const currentLevel = stack[stack.length - 1].level;
            if (level > currentLevel) {
                // Begin nesting to this level.
                for (let nextLevel = currentLevel + 1; nextLevel <= level; nextLevel++) {
                    const ol = document.createElement('ol');
                    ol.classList.add('section');
                    const last = stack[stack.length - 1];
                    const lastChild = last.ol.lastChild;
                    // Handle the case where jumping more than one nesting
                    // level, which doesn't have a list item to place this new
                    // list inside of.
                    if (lastChild) {
                        lastChild.appendChild(ol);
                    } else {
                        last.ol.appendChild(ol);
                    }
                    stack.push({level: nextLevel, ol: ol});
                }
            } else if (level < currentLevel) {
                while (stack.length > 1 && stack[stack.length - 1].level > level) {
                    stack.pop();
                }
            }

            const li = document.createElement('li');
            li.classList.add('header-item');
            li.classList.add('expanded');
            if (level < foldLevel) {
                li.classList.add('expanded');
            }
            const span = document.createElement('span');
            span.classList.add('chapter-link-wrapper');
            const a = document.createElement('a');
            span.appendChild(a);
            a.href = '#' + header.id;
            a.classList.add('header-in-summary');
            filterHeader(header.children[0], a);
            a.addEventListener('click', headerThresholdClick);
            const nextHeader = headers[i + 1];
            if (nextHeader !== undefined) {
                const nextLevel = parseInt(nextHeader.tagName.charAt(1));
                if (nextLevel > level && level >= foldLevel) {
                    const toggle = document.createElement('a');
                    toggle.classList.add('chapter-fold-toggle');
                    toggle.classList.add('header-toggle');
                    toggle.addEventListener('click', () => {
                        li.classList.toggle('expanded');
                    });
                    const toggleDiv = document.createElement('div');
                    toggleDiv.textContent = '❱';
                    toggle.appendChild(toggleDiv);
                    span.appendChild(toggle);
                    headerToggles.push(li);
                }
            }
            li.appendChild(span);

            const currentParent = stack[stack.length - 1];
            currentParent.ol.appendChild(li);
        }

        const onThisPage = document.createElement('div');
        onThisPage.classList.add('on-this-page');
        onThisPage.append(stack[0].ol);
        const activeItemSpan = activeSection.parentElement;
        activeItemSpan.after(onThisPage);
    });

    document.addEventListener('DOMContentLoaded', reloadCurrentHeader);
    document.addEventListener('scroll', reloadCurrentHeader, { passive: true });
})();

