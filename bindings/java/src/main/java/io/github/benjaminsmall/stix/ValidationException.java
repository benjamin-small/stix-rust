package io.github.benjaminsmall.stix;

public class ValidationException extends StixException {
    public ValidationException(String message) { super("validation", message); }
}
