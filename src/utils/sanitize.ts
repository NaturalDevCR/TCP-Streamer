/** Redacts known secrets/endpoints and literal IP addresses from support text. */
export function sanitizeSupport(text: string, sensitiveValues: string[] = []): string {
  let output = text;
  for (const value of [...new Set(sensitiveValues.filter(Boolean))].sort(
    (a, b) => b.length - a.length,
  )) {
    output = output.split(value).join("[redacted]");
  }
  return output
    .replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g, "[address]")
    .replace(/\[[0-9a-fA-F:]+\]/g, "[address]")
    .replace(/\b(?:[0-9a-fA-F]{0,4}:){3,}[0-9a-fA-F]{0,4}\b/g, "[address]")
    .replace(/(?:\b[0-9a-fA-F]{1,4})?::[0-9a-fA-F:]+/g, "[address]")
    .replace(/\b(psk|password|secret)\s*[:=]\s*\S+/gi, "$1=[redacted]");
}
