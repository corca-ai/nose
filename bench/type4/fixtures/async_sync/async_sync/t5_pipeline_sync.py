def pipeline(stages, payload):
    current = payload
    trace = []
    for stage in stages:
        result = run_stage(stage, current)
        checked = validate(result)
        if checked.valid:
            current = checked.value
            trace.append(stage)
        else:
            break
    return current, trace
