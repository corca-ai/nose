import static Tables.LOOKUP;

class JavaImported {
  static int lookup(String key, String other) {
    return LOOKUP.getOrDefault(key, 0);
  }
}
