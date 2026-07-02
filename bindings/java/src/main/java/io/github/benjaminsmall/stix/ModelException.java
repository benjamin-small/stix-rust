package io.github.benjaminsmall.stix;

public class ModelException extends StixException {
    public ModelException(String message) { super("model", message); }
}
