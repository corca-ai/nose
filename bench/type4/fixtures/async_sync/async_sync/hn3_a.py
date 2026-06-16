async def render(blocks, ctx):
    parts = []
    width = 0
    for blk in blocks:
        text = await format_block(blk, ctx)
        sized = await measure(text)
        if sized.fits:
            parts.append(text)
            width = width + sized.w
        else:
            await spill(blk)
    return join(parts, width)
