# Release process

1. Confirm every P0/P1 ledger item and acceptance mapping is complete.
2. Run quality, platform, AGP, Logcat stress, reliability, privacy, scheduled
   Android, benchmark, bridge reproducibility, and package jobs on clean hosts.
3. Rebuild the bridge and Deckmark assets; require no tracked-file changes.
4. Build five target archives, checksums, installers, SBOM/provenance, and test
   archive contents and permissions.
5. Publish a release candidate and test clean installation plus Homebrew formula.
6. Sign and push the version-matching `vX.Y.Z` tag only after every gate succeeds.

Do not create the final tag from a local implementation session. Release jobs
must operate from an immutable reviewed commit. Windows remains explicitly
experimental in release notes and artifact metadata.
