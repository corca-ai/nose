function sumFor(items: number[]): number {
  let total = 0;
  for (let i = 0; i < items.length; i++) {
    total += items[i];
  }
  return total;
}

const sumOf = (numbers: number[]) => {
  let acc = 0;
  for (const n of numbers) {
    acc = acc + n;
  }
  return acc;
};

class Calc {
  add(a: number, b: number): number {
    if (a > b) {
      return a + b;
    } else {
      return b + a;
    }
  }
}
