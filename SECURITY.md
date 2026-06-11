# Security Policy

Deck is a desktop-app **starter** — meant to be forked, renamed, and shipped as
your own app. This policy covers the upstream starter repo
(`github.com/hellno/deck`). **Your fork owns its own security posture:** once you
add a model client, network calls, a data store, or any other domain logic, the
threat model is yours, not the starter's.

## Supported versions

There are no release branches. The starter is shipped from `main`, and only the
**latest `main`** receives security fixes. Pull the latest commit before
reporting.

## Reporting a vulnerability

**Please report privately — do not open a public issue for a security bug.**

Preferred: use GitHub's **Private Vulnerability Reporting** for this repo
(Security tab → *Report a vulnerability*). It gives us a private thread and a
coordinated-disclosure workflow.

> Maintainer: enable Private Vulnerability Reporting under
> *Settings → Code security and analysis* so the link above works.

If that is unavailable, reach the maintainer (**@hellno**) through their
[GitHub profile](https://github.com/hellno) and ask for a private channel before
sharing any details.

Please include:

- the affected commit (the `gpui`/`deck` commit you're on),
- OS and feature flags in play (`default` / `tray` / `overlay`),
- a description of the issue and, ideally, a minimal reproduction.

This is a single-maintainer project, so there is no formal SLA — expect a
best-effort acknowledgement and a fix or mitigation on `main` once confirmed.
