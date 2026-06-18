function squarePlusOne(value: number): number {
  return value * value + 1;
}

function axisCase(x: number, y: number): number {
  const total = x + y;
  return squarePlusOne(total);
}
