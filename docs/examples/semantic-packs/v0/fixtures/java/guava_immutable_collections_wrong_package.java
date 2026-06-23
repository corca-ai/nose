import java.util.List;

final class GuavaImmutableCollectionsWrongPackage {
    Object value() {
        return List.of("not", "guava");
    }
}
