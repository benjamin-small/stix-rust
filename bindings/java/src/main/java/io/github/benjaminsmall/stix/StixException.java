package io.github.benjaminsmall.stix;

/** Base class for all stix errors (unchecked). */
public class StixException extends RuntimeException {
    private final String code;

    public StixException(String code, String message) {
        super(message);
        this.code = code;
    }

    /** One of "parse", "model", "match", "validation". */
    public String code() {
        return code;
    }
}
