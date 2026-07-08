class NullPresence {
    static boolean javaMissing(Object value, Object other) {
        return value == null;
    }

    static boolean javaPresent(Object value, Object other) {
        return value != null;
    }
}
