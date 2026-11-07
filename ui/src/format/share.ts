export function formatShare(share: number): string {
  if (!Number.isFinite(share) || share < 0) {
    throw new Error("share must be a finite, non-negative number");
  }
  if (share > 0 && share < 0.01) {
    return "<1%";
  }
  return `${Math.round(share * 100)}%`;
}
