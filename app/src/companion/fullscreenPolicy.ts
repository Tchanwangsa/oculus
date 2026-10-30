type BrowserDevice = Pick<Navigator, 'userAgent' | 'platform' | 'maxTouchPoints'>;

/** iPadOS also sends a Mac desktop identity; touch points distinguish it.
 * Apple's native fullscreen dismissal gesture competes with canvas strokes.
 */
export function shouldUseNativeCanvasFullscreen(device: BrowserDevice): boolean {
  const appleMobile = /iPad|iPhone|iPod/i.test(device.userAgent);
  const desktopIdentityOnIpad = /Mac/i.test(device.platform) && device.maxTouchPoints > 1;
  return !appleMobile && !desktopIdentityOnIpad;
}
