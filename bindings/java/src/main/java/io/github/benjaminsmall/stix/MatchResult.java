package io.github.benjaminsmall.stix;

import java.util.List;

/** The outcome of a match. */
public final class MatchResult {
    private final boolean matched;
    private final List<Long> observations;

    MatchResult(boolean matched, List<Long> observations) {
        this.matched = matched;
        this.observations = observations;
    }

    public boolean matched() { return matched; }

    public List<Long> observations() { return observations; }
}
