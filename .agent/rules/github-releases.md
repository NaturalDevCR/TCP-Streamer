---
trigger: always_on
---

Make sure you match the release with the current previously bumped version of the app, and make sure the release contains all the details of the changes before pushing the tag.

Also make sure github actions only builds the releases so it won't duplicate the builds when pushing the commits.
