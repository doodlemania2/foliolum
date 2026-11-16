# CLAUDE.md

Foliolum: a Publisher-familiar page layout app in the browser, for parishes, schools, nonprofits and
small groups, built on the open-source DesignCraft engine. This repo is PUBLIC. Its marketing and guide
sites live in the private repo `doodlemania2/foliolum-site`.

## Rules

- **Trunk is `main`.** One work stream gets one branch and one PR. Commit locally in small steps and push
  once, when the work is done.
- **CI runs only on our self-hosted runners, never on GitHub-hosted ones, not even as a fallback**, even
  though the repo is public. Use `runs-on: ${{ vars.RUNNER_LINUX || 'tdp-linux' }}` (the Linux tier on
  doodlemanage). Never target a `--docker` runner from this repo.
- **Fork PRs never reach a self-hosted runner with anything privileged.** Use `pull_request` (never
  `pull_request_target`) only for unprivileged checks, with the Dependabot guard
  `if: github.event.pull_request.user.login != 'dependabot[bot]'` on every job, and rely on the repo
  setting "require approval for all external contributors". Anything that holds a secret or writes
  somewhere runs on push to `main` or `workflow_dispatch` only.
- **Dependabot PRs are notices, never CI runs.** Push triggers are limited to `main`.
- **Secrets never go in git.** They live in GitHub Actions secrets or OpenBao; reference them by name.
- **No "Claude Code"** text, footer or session link in any PR, issue or comment.
- **Maintain `presence/features.json`** (fleet presence standard; schema in the public repo
  `doodlemania2/fleet-presence-dispatch`, `schemas/features.schema.json`). Add a feature as `building`
  when work starts, list the files it lives in under `sources`, and flip it to `ready` only when it works
  on `main`. Only `ready` features appear on the marketing site. `.github/workflows/presence.yml` tells
  foliolum-site to rebuild on every push to `main`.
- No license is chosen yet. Do not add one.

## Worktrees

Follow `/Volumes/Data/repos/CLAUDE.md`: `/Volumes/Data/repos/_tools/wt new foliolum <issue#>-<slug>`, and
`wt done foliolum <issue#>-<slug>` once the branch has landed on `main`. Leave the primary checkout on
`main` and clean. No `git stash`.
