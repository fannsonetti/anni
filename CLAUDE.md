# Working on Anni

Two people work on this repo. To avoid overwriting each other:

1. **Sync before starting.** Run `git fetch` and make sure your branch is up to date with `origin/main` before making any changes. Pull if behind.
2. **One feature = one PR.** Each new feature gets its own branch off the latest `origin/main` and its own pull request. Never commit features straight to `main`, and never bundle multiple features in one PR.
3. **Wait for CI and a review before merging.** `main` is protected: a PR needs the `build` check to pass on a branch that is up to date with `main`, plus one approval from the other person. Never merge while CI is still running.
4. **Releases only from `main`.** Version bumps go in their own PR. Create `v*` tags only on `main` after that PR is merged — pushing a tag publishes a GitHub release.
