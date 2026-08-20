# Changesets

Nine packages release in lockstep, and past two or three that is not
something to do by hand.

A change that users will notice gets a changeset:

```sh
pnpm changeset
```

It asks which packages moved and whether the move is major, minor or
patch, and writes a small markdown file here. Commit it with the change.
The description becomes the changelog entry, so write it for someone
reading the release notes rather than the diff.

`.changeset/config.json` has all nine as a `fixed` group: they take one
version number between them. Bumping any one bumps all of them. That is
deliberate — `@hozo/vite` and `@hozo/compiler` are halves of one compiler,
and a project holding two Hozo packages at different versions is a support
question nobody wants.

Under `0.x`, a breaking change is a **minor** bump: `0.1.x` → `0.2.0`.
`^0.1.0` does not allow `0.2.0`, so that is the signal semver gives you
before 1.0. Reserve `major` for the deliberate move to 1.0.

## Releasing

1. Merge the **Version Packages** pull request that the `release` workflow
   opens. It applies every pending changeset: nine versions bumped, nine
   changelogs written, the changeset files consumed.
2. Tag the resulting commit and push the tag.

```sh
git tag v0.2.0 && git push origin v0.2.0
```

The tag is what publishes. Nothing reaches npm without one, which for a
registry with no undo past 72 hours is worth the extra step.
