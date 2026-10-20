/** Link-shaped strings of every length the share dialog can draw, from a bare
 * page to a long IPv6 link, so the cases span versions 1 to 10. */

const hex = (n: number, seed: number) => Array.from({ length: n }, (_, i) => "0123456789abcdef"[(i * 7 + seed * 13) % 16]).join("");
const b64url = (n: number, seed: number) => Array.from({ length: n }, (_, i) => "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"[(i * 11 + seed * 5) % 64]).join("");
const key = (seed: number) => `${hex(32, seed)}.${b64url(43, seed)}`;

export const LINKS: readonly string[] = [
  "http://a.b/",
  "http://10.0.0.2:43821/c/",
  `http://192.168.1.23:43821/c/#${key(1)}`,
  `http://10.0.0.254:43822/c/#${key(2)}`,
  `http://172.16.200.199:43840/c/#${key(3)}`,
  `http://100.101.102.103:43821/c/#${key(4)}`,
  `http://[fd00::5]:43821/c/#${key(5)}`,
  `http://[fd7a:115c:a1e0:ab12:4843:cd96:6258:b240]:43821/c/#${key(6)}`,
  `http://[2001:db8:85a3:8d3:1319:8a2e:370:7348]:43821/c/#${key(7)}&n=${hex(40, 8)}`,
  `http://192.168.1.23:43821/c/#${key(9)}${b64url(90, 9)}`,
];
