async def pipeline(stages, payload):
    current = payload
    trace = []
    for stage in stages:
        result = await run_stage(stage, current)
        checked = await validate(result)
        if checked.valid:
            current = checked.value
            trace.append(stage)
        else:
            break
    return current, trace
