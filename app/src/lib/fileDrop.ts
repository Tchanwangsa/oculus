/** Wry reports macOS drops in logical points, Windows/Linux in physical
 * pixels. Scale from the host's coordinate space into the zoomed CSS viewport. */
export function fileDropRatio(viewportWidth: number, physicalWidth: number, scale: number, mac: boolean): number {
  const width = mac ? physicalWidth / (scale || 1) : physicalWidth;
  return width > 0 && viewportWidth > 0 ? viewportWidth / width : 1;
}
