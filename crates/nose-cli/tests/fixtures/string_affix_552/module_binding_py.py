PREFIX = "pre"


def module_binding_prefix(subject: str) -> bool:
    return subject.startswith(PREFIX)
