import { expect, test } from 'bun:test';
import { shouldUseNativeCanvasFullscreen } from '@/companion/fullscreenPolicy';

test('Apple touch devices keep drawing out of the native fullscreen dismissal gesture', () => {
  for (const device of [
    { platform: 'iPad', userAgent: 'Mozilla/5.0 (iPad; CPU OS 26_0 like Mac OS X) AppleWebKit/605.1.15 Version/26.0 Mobile/15E148 Safari/604.1', maxTouchPoints: 5 },
    { platform: 'MacIntel', userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) AppleWebKit/605.1.15 Version/26.0 Safari/605.1.15', maxTouchPoints: 5 },
    { platform: 'MacIntel', userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) AppleWebKit/605.1.15 CriOS/140.0 Safari/605.1.15', maxTouchPoints: 5 },
    { platform: 'iPhone', userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148 Safari/604.1', maxTouchPoints: 5 },
  ]) expect(shouldUseNativeCanvasFullscreen(device)).toBe(false);
});

test('desktop Safari and other touch platforms can still request native fullscreen', () => {
  for (const device of [
    { platform: 'MacIntel', userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) AppleWebKit/605.1.15 Version/26.0 Safari/605.1.15', maxTouchPoints: 0 },
    { platform: 'Linux armv8l', userAgent: 'Mozilla/5.0 (Linux; Android 16) AppleWebKit/537.36 Chrome/140.0 Safari/537.36', maxTouchPoints: 5 },
    { platform: 'Win32', userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/140.0 Safari/537.36', maxTouchPoints: 10 },
  ]) expect(shouldUseNativeCanvasFullscreen(device)).toBe(true);
});
