export function roundToSigFigs(num: number, sigFigs: number) {
  if (num === 0) return 0;
  const exponent = Math.floor(Math.log10(Math.abs(num)));
  const decimalsToKeep = sigFigs - 1 - exponent;
  const shifted = Math.round(Number(num + 'e' + decimalsToKeep));
  const result = Number(shifted + 'e-' + decimalsToKeep);
  return result;
}
