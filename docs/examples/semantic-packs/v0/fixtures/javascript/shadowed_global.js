export function fromLocalPromise(value) {
  const Promise = {
    resolve(input) {
      return input;
    },
  };
  return Promise.resolve(value);
}
