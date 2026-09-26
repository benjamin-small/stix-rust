// Example patterns offered in the playground's picker. The first one loads on
// page load. Each one exercises a different part of the IR.
export const EXAMPLES = [
  { label: "Single comparison", pattern: "[file:size > 1024]" },
  {
    label: "AND / OR in one observation",
    pattern: "[file:name = 'invoice.exe' OR file:size > 1024 AND file:size < 4096]",
  },
  { label: "EXISTS", pattern: "[EXISTS file:hashes.'SHA-256']" },
  { label: "IN a set", pattern: "[ipv4-addr:value IN ('198.51.100.1', '203.0.113.7')]" },
  { label: "NOT with an operator", pattern: "[domain-name:value NOT LIKE '%.example.com']" },
  {
    label: "FOLLOWEDBY … WITHIN",
    pattern:
      "[ipv4-addr:value = '198.51.100.1'] FOLLOWEDBY [domain-name:value = 'evil.example'] WITHIN 300 SECONDS",
  },
  { label: "REPEATS", pattern: "[network-traffic:dst_port = 22] REPEATS 5 TIMES" },
  {
    label: "START … STOP",
    pattern: "[file:name = 'a.exe'] START t'2024-01-01T00:00:00Z' STOP t'2024-02-01T00:00:00Z'",
  },
  {
    label: "Observation-level AND / OR",
    pattern: "([ipv4-addr:value = '1.2.3.4'] AND [file:size > 0]) OR [process:name MATCHES '^cmd']",
  },
];
