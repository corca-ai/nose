import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSet;

final class GuavaImmutableCollectionsPositive {
    Object list() {
        return ImmutableList.of("alpha", "beta");
    }

    Object set() {
        return ImmutableSet.of("alpha", "beta");
    }
}
