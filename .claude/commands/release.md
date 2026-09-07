Perform a release of jcard.

Optional override: $ARGUMENTS (format: vX.Y.Z). If provided, use that version.

## Version determination

1. Find the last release tag (`git tag --sort=-v:refname | head -1`).
2. Examine commits since that tag to classify the release type (0.x semver:
   the minor is the breaking axis):
   - **Patch** (0.Y.z+1): bug fixes, additive public API, dependency bumps,
     build changes, docs.
   - **Breaking** (0.Y+1.0): changed/removed public items, incompatible
     behavior changes. Stop and confirm before proceeding.

## Steps

1. Pre-release checks — stop and report on any failure:

```sh
scripts/release-check.sh
```

2. Draft a changelog from `git log --oneline <last-tag>..HEAD` and write it to
   `scratch/changelog-vX.Y.Z.txt` (gitignored; not part of the published
   package).

   **Rules:**
   - Group under: `New features:`, `Bug fixes:`, `Build:`, `Refactoring:` — omit empty sections.
   - Describe user-visible behavior, not implementation details.
   - Merge related commits for the same feature into one bullet.
   - No git hashes, no raw commit subjects, no co-author lines.

   File format (becomes the tag annotation verbatim):
   ```
   vX.Y.Z

   New features:
   - what changed

   Bug fixes:
   - what was fixed

   Build:
   - what changed
   ```

3. Bump, commit, and tag:

```sh
scripts/release-tag.sh vX.Y.Z scratch/changelog-vX.Y.Z.txt
```

   Bumps `Cargo.toml`, commits `release: vX.Y.Z`, detaches HEAD, pins
   `Cargo.lock` on that detached commit (`build: pin Cargo.lock for vX.Y.Z`),
   signs the tag from the changelog file, and returns to the branch. Refuses
   to run on a dirty tree, off master, or if the tag already exists. Nothing
   is pushed yet.

4. Push master, wait for CI green:

```sh
git push
gh run watch "$(gh run list --workflow=ci.yml -b master -L1 --json databaseId --jq '.[0].databaseId')" --exit-status
```

   No run within a couple of minutes: check the `Actions` component at
   `https://www.githubstatus.com/api/v2/components.json` — during an outage no
   run is created and missed events are never backfilled. Stop and report.

   Red: fix on master, delete the local tag (`git tag -d vX.Y.Z`), rebuild it
   with `scripts/release-tag.sh` onto the new head, restart this step.

5. Push the tag:

```sh
git push origin vX.Y.Z
```

   The tag is IMMUTABLE once pushed — never retag. Wrong? Make a new patch
   release.

6. Publish — `cargo publish` is definitive and irrevocable, so it is never
   wrapped in a script; run each command directly and review the dry-run
   output before the real one:

```sh
git checkout vX.Y.Z
cargo publish --dry-run
cargo publish
git switch master
```

   `git switch master` drops the working-tree `Cargo.lock` back to untracked;
   the next cargo command regenerates it.

7. Report the tag, the changelog, the CI run that gated the publish, and the
   crates.io version (`curl https://index.crates.io/jc/ar/jcard`).

## Important

- **Never publish a commit CI has not run on.** The tag's pin commit differs
  from the CI-green master tip only by `Cargo.lock`. If anything else changed
  after the checks — a rebase, a hand-resolved conflict — the earlier green
  run does not cover it. Re-run the checks and go back to step 4.
- **Ask before publishing when anything deviated from these steps.** An outage,
  a rebase, a skipped step, a red-then-fixed run: report the state and let me
  decide.
- **Cargo.lock never reaches master** — library crate, stays gitignored there.
  It exists only on the tag's own commit, so a release build is reproducible.
  `pre-commit` rejects a staged lock, `pre-push` rejects a branch tip that
  tracks it, and CI fails if it is tracked at all.
