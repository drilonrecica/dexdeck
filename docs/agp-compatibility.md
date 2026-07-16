# AGP compatibility

| Android Gradle Plugin | Adapter | Status |
| --- | --- | --- |
| 8.0.2 | AGP 8 | Minimum tested |
| 8.13.0 | AGP 8 | Tested |
| 9.0.1 | AGP 9 | Tested |
| 9.3.0 | AGP 9 | Current tested |
| Older than 8 | None | Degraded task mode |
| 10 or newer | None | Degraded until reviewed |

The executable matrix pairs these versions with Gradle 8.0, 8.13, 9.1, and
9.5 respectively. Patch versions are pinned in the test-support crate and CI.
Support means project modeling is exercised against real generated projects;
it does not override the compatibility requirements of a project’s own Java,
Gradle, Kotlin, SDK, or third-party plugins.
