package dev.dexdeck.bridge;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.List;

/** Java 17 output boundary used by the Gradle init plugin. */
public final class ModelBridge {
    public static final int PROTOCOL_VERSION = 1;

    private ModelBridge() {}

    public static void writeModel(Path output, List<String> records, String canonicalModelJson)
            throws IOException {
        StringBuilder stream = new StringBuilder();
        for (String record : records) {
            stream.append(record).append('\n');
        }
        stream.append("{\"protocolVersion\":1,\"type\":\"complete\",\"durationMs\":0,")
                .append("\"recordCount\":").append(records.size())
                .append(",\"modelHash\":\"").append(sha256(canonicalModelJson)).append("\"}\n");
        writeAtomic(output, stream.toString());
    }

    public static void writeFailure(Path output, String code, String message) throws IOException {
        String error = "{\"protocolVersion\":1,\"type\":\"error\",\"code\":\""
                + escape(code) + "\",\"message\":\"" + escape(message) + "\"}";
        writeModel(output, List.of(error), "failure");
    }

    private static void writeAtomic(Path output, String value) throws IOException {
        Path parent = output.toAbsolutePath().getParent();
        if (parent == null) {
            throw new IOException("bridge output has no parent directory");
        }
        Files.createDirectories(parent);
        Path temporary = Files.createTempFile(parent, ".dexdeck-model-", ".tmp");
        try {
            Files.writeString(temporary, value, StandardCharsets.UTF_8);
            try {
                Files.move(temporary, output, StandardCopyOption.ATOMIC_MOVE,
                        StandardCopyOption.REPLACE_EXISTING);
            } catch (AtomicMoveNotSupportedException ignored) {
                Files.move(temporary, output, StandardCopyOption.REPLACE_EXISTING);
            }
        } finally {
            Files.deleteIfExists(temporary);
        }
    }

    private static String sha256(String value) {
        try {
            byte[] digest = MessageDigest.getInstance("SHA-256")
                    .digest(value.getBytes(StandardCharsets.UTF_8));
            StringBuilder result = new StringBuilder(digest.length * 2);
            for (byte item : digest) {
                result.append(String.format("%02x", item & 0xff));
            }
            return result.toString();
        } catch (NoSuchAlgorithmException impossible) {
            throw new IllegalStateException("SHA-256 is unavailable", impossible);
        }
    }

    private static String escape(String value) {
        return value.replace("\\", "\\\\").replace("\"", "\\\"")
                .replace("\r", "\\r").replace("\n", "\\n");
    }
}
