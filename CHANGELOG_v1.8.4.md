# Changelog v1.8.4

## Security Hardening

This release focuses on hardening the application security and fixing potential vulnerabilities.

### Critical Fixes

- **Backend Safety**: Fixed Undefined Behavior (UB) in audio stream shutdown logic. Replaced unsafe `std::mem::zeroed()` usage with safe `try_clone()` mechanism.

### Security Improvements

- **Content Security Policy**: Implemented strict CSP header preventing execution of unauthorized scripts and styles.
- **API Restriction**: Disabled `withGlobalTauri` to prevent global exposure of Tauri APIs to the frontend.
- **XSS Mitigation**: Refactored frontend logging and notifications to use safe DOM manipulation instead of `innerHTML`, preventing XSS attacks via crafted log messages.

### Other Changes

- Bumped dependencies for security audits.
