# Gradle bridge protocol version 1

The Java bridge writes JSON Lines to an explicit temporary output file. Every
record includes protocolVersion and a type. Gradle stdout and stderr are not
parsed as bridge records.

The stream must end with exactly one complete record. Its recordCount equals
the number of preceding records. Missing, duplicate, incompatible, malformed,
or trailing records invalidate the complete output; callers retain the previous
valid project model.

The complete record's modelHash is the lowercase hexadecimal SHA-256 of the
exact UTF-8 bytes of every preceding JSON record followed by one LF byte. The
complete record is excluded. Hashing the emitted stream avoids relying on
serializer-specific JSON escaping while still detecting altered output.

Record families are build, module, dimension, flavor, variant, task,
diagnostic, error, and complete.
