package io.github.benjaminsmall.stix;

public class ParseException extends StixException {
    public ParseException(String message) { super("parse", message); }
}
