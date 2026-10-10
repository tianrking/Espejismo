# Code tour documentation notes

## Change and rationale

Added `docs/CODE-TOUR.md` as a repository-path-oriented entry point for
contributors. Existing architecture documentation describes runtime and
protocol behavior, but does not give a quick module map. The tour maps the
four workspace crates, follows client and server connection handling, and
points to core module ownership and canonical references. Added it to the
development documentation index so contributors can discover it.

This is documentation-only: no Rust behavior, dependencies, protocol contract,
or product positioning changed. Expected runtime/performance improvement is
none; the expected benefit is less time locating the relevant implementation
when navigating a change.

## Validation and conclusion

Reviewed the workspace manifest, core public module list, client/server module
declarations, architecture guide, development index, and positioning document
against the tour. The documented paths and responsibilities match the current
tree. No executable code changed, so there is no behavior regression surface
and no compile or test result is claimed. No benchmark applies to this
documentation-only change.
