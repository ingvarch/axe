# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |
| Nightly builds | Best effort |

Only the latest release in the 0.1.x series receives security fixes. Nightly builds are unsupported but reported issues will be evaluated.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, use [GitHub Security Advisories](https://github.com/ingvarch/axe/security/advisories/new) to report the issue privately. Include the following information:

- A description of the vulnerability
- Steps to reproduce the issue
- The affected version(s)
- The potential impact (e.g., arbitrary code execution, data loss, path traversal)
- Any suggested fix, if you have one

## Response Timeline

- **Acknowledgment**: within 72 hours of receiving the report
- **Initial assessment**: within 7 days
- **Fix or mitigation**: depends on severity, but the goal is to release a patch within 30 days for critical issues

You will be kept informed of progress throughout the process.

## What Qualifies as a Security Issue

The following are considered security issues and should be reported privately:

- Path traversal or arbitrary file access outside the project root
- Arbitrary code execution through crafted input (e.g., malicious config files, LSP responses, or terminal escape sequences)
- Privilege escalation or sandbox escape in the embedded terminal
- Vulnerabilities in dependency handling that affect Axe users
- Memory safety issues (e.g., buffer overflows in unsafe code)

## What Is a Regular Bug

The following should be filed as regular GitHub issues:

- Application crashes or panics that do not involve untrusted input exploitation
- Rendering glitches or incorrect terminal output
- Configuration parsing errors for malformed but non-malicious input
- Performance issues
- Feature requests

## Disclosure Policy

Once a fix is released, the vulnerability will be disclosed publicly with credit given to the reporter (unless anonymity is requested). We follow a coordinated disclosure approach -- please allow a reasonable window for a fix before any public disclosure.

## Acknowledgments

We appreciate the security research community and anyone who takes the time to report vulnerabilities responsibly. Contributors who report valid security issues will be credited in the release notes.
