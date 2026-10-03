// Damped step response using the macOS source response/damping parameters.
// CSS linear() interpolates the sampled curve; physical equivalence still
// requires recording comparison against SwiftUI's implementation.
export function spring(response: number, damping: number): string {
  const omega = 2 * Math.PI / response;
  const duration = Math.log(1000) / (damping * omega);
  const wd = omega * Math.sqrt(1 - damping * damping);
  const samples = Array.from({ length: 81 }, (_, index) => {
    const t = duration * index / 80;
    const y = index === 80 ? 1 : 1 - Math.exp(-damping * omega * t) * (Math.cos(wd * t) + damping * omega / wd * Math.sin(wd * t));
    return `${y.toFixed(5)} ${(index * 100 / 80).toFixed(2)}%`;
  });
  return `${duration.toFixed(4)}s linear(${samples.join(",")})`;
}
export function installMotionTokens() {
  const tokens = { "segment-slide": [0.28, 0.72], "segment-hover": [0.24, 0.74], "segment-press": [0.22, 0.68], "tool-hover": [0.22, 0.7], "tool-press": [0.18, 0.65], "icon-hover": [0.2, 0.66], "icon-press": [0.16, 0.62] };
  for (const [name, [response, damping]] of Object.entries(tokens)) document.documentElement.style.setProperty(`--motion-${name}`, spring(response, damping));
}
