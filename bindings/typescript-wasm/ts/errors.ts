export type StixErrorCode = "parse" | "model" | "match" | "validation";

export class StixError extends Error {
  readonly code: StixErrorCode;
  constructor(code: StixErrorCode, message: string) {
    super(message);
    this.code = code;
    this.name = new.target.name;
  }
}
export class ParseError extends StixError {
  constructor(message: string) { super("parse", message); }
}
export class ModelError extends StixError {
  constructor(message: string) { super("model", message); }
}
export class MatchError extends StixError {
  constructor(message: string) { super("match", message); }
}
export class ValidationError extends StixError {
  constructor(message: string) { super("validation", message); }
}

const CODE_RE = /^\[(parse|model|match|validation)\]\s?/;

/** Map a raw error (message "[code] msg") to the matching StixError subclass. */
export function toStixError(err: unknown): StixError {
  const raw = err instanceof Error ? err.message : String(err);
  const m = CODE_RE.exec(raw);
  const message = m ? raw.replace(CODE_RE, "") : raw;
  switch (m?.[1]) {
    case "parse": return new ParseError(message);
    case "match": return new MatchError(message);
    case "validation": return new ValidationError(message);
    default: return new ModelError(message);
  }
}
