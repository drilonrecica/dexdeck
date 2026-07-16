package dev.dexdeck.bridge;

import java.util.Map;

/** Version-specific, public-API Android Gradle Plugin model boundary. */
public interface ModelAdapter {
    Map<String, Object> modelVariant(Object variant);
}
