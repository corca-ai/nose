import static WrongTables.LOOKUP;

class JavaImportedWrongMap {
  static int lookup(String key, String other) {
    return LOOKUP.getOrDefault(key, 0);
  }
}
