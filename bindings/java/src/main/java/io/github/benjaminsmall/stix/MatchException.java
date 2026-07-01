package io.github.benjaminsmall.stix;

public class MatchException extends StixException {
    public MatchException(String message) { super("match", message); }
}
