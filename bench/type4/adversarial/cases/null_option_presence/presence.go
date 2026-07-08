package p

func GoMissing(value any, other any) bool {
    return value == nil
}

func GoPresent(value any, other any) bool {
    return value != nil
}
