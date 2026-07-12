extension Array where Element == [Bool] {
    func flatMap<T>(_ transform: ([Bool]) -> [T]) -> [T] { [] }
}
