function dynamicParamPrefix(subject: string, prefix: string): boolean {
  const normalized = prefix.trim();
  return subject.startsWith(normalized);
}
