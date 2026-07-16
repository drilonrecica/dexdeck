package dev.dexdeck.bridge.agp9;

import com.android.build.api.variant.ApplicationVariant;
import com.android.build.api.variant.Variant;
import dev.dexdeck.bridge.ModelAdapter;
import java.util.LinkedHashMap;
import java.util.Map;

/** Adapter compiled against the minimum supported AGP 9 public API. */
public final class Agp9ModelAdapter implements ModelAdapter {
    @Override
    public Map<String, Object> modelVariant(Object candidate) {
        Variant variant = (Variant) candidate;
        Map<String, Object> value = new LinkedHashMap<>();
        value.put("name", variant.getName());
        value.put("enabled", true);
        value.put("buildType", variant.getBuildType() == null ? "" : variant.getBuildType());
        value.put("debuggable", variant.getDebuggable());
        String namespace = variant.getNamespace().getOrNull();
        if (namespace != null) value.put("namespace", namespace);
        if (variant instanceof ApplicationVariant application) {
            String applicationId = application.getApplicationId().getOrNull();
            if (applicationId != null) value.put("applicationId", applicationId);
        }
        return value;
    }
}
