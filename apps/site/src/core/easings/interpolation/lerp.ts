/** The default speed value */
const DEFAULT_SPEED: number = 0.1;
/** The default limit value */
const DEFAULT_LIMIT: number = 0.1;

export function lerp(
  current: number,
  target: number,
  speed: number = DEFAULT_SPEED,
  limit: number = DEFAULT_LIMIT,
) {
  let change = (target - current) * speed;

  if (Math.abs(change) < limit) {
    change = target - current;
  }

  return change;
}
