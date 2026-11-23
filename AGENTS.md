# Working on Anni

Two people work on this repo. To avoid overwriting each other:

1. **Sync before starting.** Run `git fetch`, then bring your branch up to date with `origin/main` (`git rebase origin/main`, or `git merge origin/main` if the branch is already shared) before making any changes. `git pull` alone only syncs your branch with its own remote copy, not with `main`.
2. **One feature = one PR.** Each new feature gets its own branch off the latest `origin/main` and its own pull request. Never commit features straight to `main`, and never bundle multiple features in one PR.
3. **Wait for CI and a review before merging.** `main` is protected: a PR needs the `build` check to pass on a branch that is up to date with `main`, and it needs to be reviewed and approved. Give every PR a clear title and a description of what changed and why. Never merge while CI is still running.
4. **Releases only from `main`.** Put each version bump in its own PR. Update `version` in `crates/inna-app/Cargo.toml` and the `AppVersion` default in `installer/anni.iss`, then refresh `Cargo.lock` for the new app version. Keep the other crate versions as they are. After that PR is merged, tag the merge commit on `main` as `v<that version>` (e.g. `v0.3.2`) and push the tag to publish a GitHub release. The release job refuses tags that aren't on `main` or don't match the `inna-app` version.
