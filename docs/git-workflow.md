# Git Workflow

This project uses a `dev -> main` workflow.

## Branch Roles

* `main` is the stable/release branch.
* `dev` is the active integration branch.
* Feature, fix, docs, chore, and security branches should be created from `dev`.

## Creating a Working Branch

Before starting new work, update `dev` and create a new branch from it:

```bash
git checkout dev
git pull --ff-only origin dev
git checkout -b fix/example-change
```

Use short, descriptive branch names. Examples:

* `fix/notification-dedup-flow`
* `fix/event-upsert-status`
* `feat/writeup-search`
* `docs/git-workflow`
* `chore/update-dependencies`
* `security/harden-external-fetch`

## Opening Pull Requests

Most pull requests should target:

```text
your-branch -> dev
```

Only release pull requests should target:

```text
dev -> main
```

Before opening a PR, check the base branch on GitHub. If the PR accidentally targets `main`, change the base branch to `dev` before merging.

## After a PR Is Squash-Merged

This repository commonly uses squash merges. After a squash merge, GitHub creates a new commit on the target branch. The original branch commits may still appear in Git graph tools until the branch is deleted.

After the PR is merged, update `dev` and delete the local branch:

```bash
git checkout dev
git pull --ff-only origin dev
git branch -D your-branch
git fetch --prune
```

If the remote branch was not deleted automatically, delete it manually:

```bash
git push origin --delete your-branch
git fetch --prune
```

Using `-D` is normal after squash merges because Git may not see the original branch commit as merged into `dev`, even though the code is already present through the squash commit.

## Handling Accidental PRs to `main`

If a PR is opened against `main` by mistake and has not been merged yet, change the PR base branch from `main` to `dev` in the GitHub UI.

If a PR is accidentally merged into `main` but the change should be kept, merge `main` back into `dev`:

```bash
git checkout dev
git pull --ff-only origin dev
git fetch origin
git merge origin/main
git push origin dev
```

If the change should not be kept on `main`, revert it on `main` instead.

For a normal commit or squash merge:

```bash
git checkout main
git pull --ff-only origin main
git revert <commit_sha>
git push origin main
```

For a merge commit:

```bash
git checkout main
git pull --ff-only origin main
git revert -m 1 <merge_commit_sha>
git push origin main
```

## Releasing

To release changes, open a pull request from:

```text
dev -> main
```

After the release PR is merged, tag the release from `main` if needed.

Example:

```bash
git checkout main
git pull --ff-only origin main
git tag v0.2.0
git push origin v0.2.0
```

## Useful Commands

View local branches:

```bash
git branch
```

View local and remote branches:

```bash
git branch -a
```

View the commit graph:

```bash
git log --oneline --graph --decorate --all
```

Prune deleted remote branches:

```bash
git fetch --prune
```

Check which branch you are currently on:

```bash
git status
```

## Recommended Contributor Flow

For normal development:

```bash
git checkout dev
git pull --ff-only origin dev
git checkout -b fix/something
# make changes
git add .
git commit -m "fix: describe the change"
git push -u origin fix/something
```

Then open a PR:

```text
fix/something -> dev
```

After the PR is squash-merged:

```bash
git checkout dev
git pull --ff-only origin dev
git branch -D fix/something
git fetch --pr
```
