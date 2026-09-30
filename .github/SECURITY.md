# Security

## Reporting a problem

Please report security problems privately, not in a public issue: use
**[Report a vulnerability](https://github.com/joshgallantt/steamcards/security/advisories/new)**
on the repository's Security tab. steamcards is a one-person project, so a
reply can take a few days. A fix goes out in a new release as soon as it's
ready, crediting you unless you'd rather it didn't.

It helps to include what someone could do with the problem, how to make it
happen, and your version (`steamcards --version`).

## What's covered

- **steamcards itself**, including how it keeps your sign-in: the config
  file holds a Steam refresh token, readable by your user only.
- **What you download and run to install it**: the install scripts, the
  release archives, and the Homebrew formula.

Problems with Steam's own services are Valve's to fix: report them to
Valve.

## Supported versions

Fixes go into the latest release. To update, run the command you installed
with again (see the [README](../README.md#install)).

## Checking a download

Each release lists every archive's SHA-256 in `SHA256SUMS`, and the install
scripts check it before installing. GitHub also signs a statement of where
each archive was built, which you can check with the
[GitHub CLI](https://cli.github.com):

```sh
gh attestation verify steamcards-x86_64-unknown-linux-musl.tar.gz --repo joshgallantt/steamcards
```
