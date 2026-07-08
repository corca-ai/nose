#include <stddef.h>

int c_missing(void *value, void *other) {
    return value == NULL;
}

int c_present(void *value, void *other) {
    return value != NULL;
}
