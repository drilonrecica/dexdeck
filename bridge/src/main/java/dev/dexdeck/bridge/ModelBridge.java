package dev.dexdeck.bridge;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

/** Java 17 entry point used by the Gradle init plugin. */
public final class ModelBridge {
    public static final int PROTOCOL_VERSION = 1;

    private ModelBridge() {}

    public static void writeFailure(Path output, String code, String message) throws IOException {
        String safeCode = escape(code);
        String safeMessage = escape(message);
        String error = "{\"protocolVersion\":1,\"type\":\"error\",\"code\":\"" + safeCode
                + "\",\"message\":\"" + safeMessage + "\"}\n";
        String complete = "{\"protocolVersion\":1,\"type\":\"complete\",\"durationMs\":0,"
                + "\"recordCount\":1,\"modelHash\":\"failure\"}\n";
        Files.writeString(output, error + complete, StandardCharsets.UTF_8);
    }

    private static String escape(String value) {
        return value.replace("\\", "\\\\").replace("\"", "\\\"")
                .replace("\r", "\\r").replace("\n", "\\n");
    }
}
