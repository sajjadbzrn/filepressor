# Security Policy

## Supported Versions

Only the latest released version of FilePressor receives security updates.
Older versions should be upgraded through the in-app updater.

| Version | Supported |
| ------- | --------- |
| latest `v*` release | ✅ |
| older releases | ❌ |

## Reporting a Vulnerability

If you discover a security vulnerability, please **do not open a public issue**.
Instead, report it privately so we can investigate and release a fix before
details become public.

- **GitHub Private Advisory:** use
  [Security → Advisories → Report a vulnerability](https://github.com/sajjadbzrn/filepressor/security/advisories/new)
  on the repository.
- **Email:** sajjadbzrn@users.noreply.github.com

Please include:

- A description of the vulnerability and its impact.
- Steps to reproduce (or a proof of concept).
- Affected version(s) and platform(s).

We aim to acknowledge reports within **72 hours** and will keep you informed as
we work on a fix. Once a fix is released, we are happy to credit you (unless you
prefer to remain anonymous).

## Signing & Supply Chain

FilePressor updates are cryptographically signed. The public key ships inside
`src-tauri/tauri.conf.json`; the private key is held only by maintainers and used
by the release pipeline. Update bundles are verified before installation, so a
compromised mirror or CDN cannot push untrusted code to users.
