// minimal RFC-1321 MD5 implementation in TypeScript.
//
// Used by `gravatar.ts` to hash author emails — Gravatar URLs require
// the MD5 of the lower-cased trimmed email. Inlined so we don't pull
// in a 6KB dep for one helper. Web Crypto's `subtle.digest` does not
// expose MD5, so we go the long way.
//
// Algorithm: based on Joseph Myers' public-domain JavaScript MD5
// (https://www.myersdaily.org/joseph/javascript/md5.js). Adapted to
// TypeScript with explicit types and UTF-8 input.

function safeAdd(x: number, y: number): number {
  const lsw = (x & 0xffff) + (y & 0xffff);
  const msw = (x >> 16) + (y >> 16) + (lsw >> 16);
  return (msw << 16) | (lsw & 0xffff);
}

function rol(num: number, cnt: number): number {
  return (num << cnt) | (num >>> (32 - cnt));
}

function cmn(
  q: number,
  a: number,
  b: number,
  x: number,
  s: number,
  t: number,
): number {
  return safeAdd(rol(safeAdd(safeAdd(a, q), safeAdd(x, t)), s), b);
}

function ff(
  a: number,
  b: number,
  c: number,
  d: number,
  x: number,
  s: number,
  t: number,
): number {
  return cmn((b & c) | (~b & d), a, b, x, s, t);
}
function gg(
  a: number,
  b: number,
  c: number,
  d: number,
  x: number,
  s: number,
  t: number,
): number {
  return cmn((b & d) | (c & ~d), a, b, x, s, t);
}
function hh(
  a: number,
  b: number,
  c: number,
  d: number,
  x: number,
  s: number,
  t: number,
): number {
  return cmn(b ^ c ^ d, a, b, x, s, t);
}
function ii(
  a: number,
  b: number,
  c: number,
  d: number,
  x: number,
  s: number,
  t: number,
): number {
  return cmn(c ^ (b | ~d), a, b, x, s, t);
}

function md5cycle(state: number[], block: number[]): void {
  let [a, b, c, d] = state;

  a = ff(a, b, c, d, block[0]!, 7, -680876936);
  d = ff(d, a, b, c, block[1]!, 12, -389564586);
  c = ff(c, d, a, b, block[2]!, 17, 606105819);
  b = ff(b, c, d, a, block[3]!, 22, -1044525330);
  a = ff(a, b, c, d, block[4]!, 7, -176418897);
  d = ff(d, a, b, c, block[5]!, 12, 1200080426);
  c = ff(c, d, a, b, block[6]!, 17, -1473231341);
  b = ff(b, c, d, a, block[7]!, 22, -45705983);
  a = ff(a, b, c, d, block[8]!, 7, 1770035416);
  d = ff(d, a, b, c, block[9]!, 12, -1958414417);
  c = ff(c, d, a, b, block[10]!, 17, -42063);
  b = ff(b, c, d, a, block[11]!, 22, -1990404162);
  a = ff(a, b, c, d, block[12]!, 7, 1804603682);
  d = ff(d, a, b, c, block[13]!, 12, -40341101);
  c = ff(c, d, a, b, block[14]!, 17, -1502002290);
  b = ff(b, c, d, a, block[15]!, 22, 1236535329);

  a = gg(a, b, c, d, block[1]!, 5, -165796510);
  d = gg(d, a, b, c, block[6]!, 9, -1069501632);
  c = gg(c, d, a, b, block[11]!, 14, 643717713);
  b = gg(b, c, d, a, block[0]!, 20, -373897302);
  a = gg(a, b, c, d, block[5]!, 5, -701558691);
  d = gg(d, a, b, c, block[10]!, 9, 38016083);
  c = gg(c, d, a, b, block[15]!, 14, -660478335);
  b = gg(b, c, d, a, block[4]!, 20, -405537848);
  a = gg(a, b, c, d, block[9]!, 5, 568446438);
  d = gg(d, a, b, c, block[14]!, 9, -1019803690);
  c = gg(c, d, a, b, block[3]!, 14, -187363961);
  b = gg(b, c, d, a, block[8]!, 20, 1163531501);
  a = gg(a, b, c, d, block[13]!, 5, -1444681467);
  d = gg(d, a, b, c, block[2]!, 9, -51403784);
  c = gg(c, d, a, b, block[7]!, 14, 1735328473);
  b = gg(b, c, d, a, block[12]!, 20, -1926607734);

  a = hh(a, b, c, d, block[5]!, 4, -378558);
  d = hh(d, a, b, c, block[8]!, 11, -2022574463);
  c = hh(c, d, a, b, block[11]!, 16, 1839030562);
  b = hh(b, c, d, a, block[14]!, 23, -35309556);
  a = hh(a, b, c, d, block[1]!, 4, -1530992060);
  d = hh(d, a, b, c, block[4]!, 11, 1272893353);
  c = hh(c, d, a, b, block[7]!, 16, -155497632);
  b = hh(b, c, d, a, block[10]!, 23, -1094730640);
  a = hh(a, b, c, d, block[13]!, 4, 681279174);
  d = hh(d, a, b, c, block[0]!, 11, -358537222);
  c = hh(c, d, a, b, block[3]!, 16, -722521979);
  b = hh(b, c, d, a, block[6]!, 23, 76029189);
  a = hh(a, b, c, d, block[9]!, 4, -640364487);
  d = hh(d, a, b, c, block[12]!, 11, -421815835);
  c = hh(c, d, a, b, block[15]!, 16, 530742520);
  b = hh(b, c, d, a, block[2]!, 23, -995338651);

  a = ii(a, b, c, d, block[0]!, 6, -198630844);
  d = ii(d, a, b, c, block[7]!, 10, 1126891415);
  c = ii(c, d, a, b, block[14]!, 15, -1416354905);
  b = ii(b, c, d, a, block[5]!, 21, -57434055);
  a = ii(a, b, c, d, block[12]!, 6, 1700485571);
  d = ii(d, a, b, c, block[3]!, 10, -1894986606);
  c = ii(c, d, a, b, block[10]!, 15, -1051523);
  b = ii(b, c, d, a, block[1]!, 21, -2054922799);
  a = ii(a, b, c, d, block[8]!, 6, 1873313359);
  d = ii(d, a, b, c, block[15]!, 10, -30611744);
  c = ii(c, d, a, b, block[6]!, 15, -1560198380);
  b = ii(b, c, d, a, block[13]!, 21, 1309151649);
  a = ii(a, b, c, d, block[4]!, 6, -145523070);
  d = ii(d, a, b, c, block[11]!, 10, -1120210379);
  c = ii(c, d, a, b, block[2]!, 15, 718787259);
  b = ii(b, c, d, a, block[9]!, 21, -343485551);

  state[0] = safeAdd(state[0]!, a);
  state[1] = safeAdd(state[1]!, b);
  state[2] = safeAdd(state[2]!, c);
  state[3] = safeAdd(state[3]!, d);
}

function md51(bytes: Uint8Array): number[] {
  const state = [1732584193, -271733879, -1732584194, 271733878];
  const len = bytes.length;
  let i: number;
  for (i = 64; i <= len; i += 64) {
    md5cycle(state, md5blk(bytes.subarray(i - 64, i)));
  }
  const tail = bytes.subarray(i - 64);
  const block = new Array<number>(16).fill(0);
  let j: number;
  for (j = 0; j < tail.length; j += 1) {
    block[j >> 2]! |= tail[j]! << ((j % 4) << 3);
  }
  block[j >> 2]! |= 0x80 << ((j % 4) << 3);
  if (j > 55) {
    md5cycle(state, block);
    block.fill(0);
  }
  // Bit length: low 32 bits then high 32 (we only support up to 2^32).
  block[14] = len * 8;
  md5cycle(state, block);
  return state;
}

function md5blk(block: Uint8Array): number[] {
  const out = new Array<number>(16);
  for (let i = 0; i < 16; i += 1) {
    const o = i << 2;
    out[i] =
      (block[o]! ?? 0) |
      ((block[o + 1]! ?? 0) << 8) |
      ((block[o + 2]! ?? 0) << 16) |
      ((block[o + 3]! ?? 0) << 24);
  }
  return out;
}

function rhex(n: number): string {
  const hex = "0123456789abcdef";
  let s = "";
  for (let i = 0; i < 4; i += 1) {
    s += hex[(n >> (i * 8 + 4)) & 0x0f]! + hex[(n >> (i * 8)) & 0x0f]!;
  }
  return s;
}

function hex(state: number[]): string {
  return state.map((n) => rhex(n)).join("");
}

/** RFC-1321 MD5 of a UTF-8 string. Returns the lower-case hex digest. */
export function md5(input: string): string {
  const bytes = new TextEncoder().encode(input);
  return hex(md51(bytes));
}
