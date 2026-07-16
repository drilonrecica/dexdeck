package org.gradle.api.provider;

/** Minimal compile-only view of the stable Gradle Provider API. */
public interface Provider<T> {
    T getOrNull();
}
