// Word cues: `word(phrase, nth)` is when the nth time the narration says
// `phrase` starts, `wordEnd` when it ends, in scene seconds (narration lead
// included). Each call is handed its own scene's words as JSON (set in
// entry.js), parsed on first use. Words match after normalising: lowercase,
// letters, digits and apostrophes only. A hyphen or dash splits a word into
// parts, in the phrase and in the narration alike, so "ninety one" and
// "ninety-one" match each other; a phrase is consecutive parts that start
// and end on whole narrated words, since only words have times.
// `pause(nth)` is when the nth `[pause]` of the scene's script starts.
let cueJson = null;
let cueData = null;

const normWord = (w) => String(w).replace(/[‘’]/g, "'").toLowerCase().replace(/[^\p{L}\p{N}']/gu, '');
const partsOf = (w) => String(w).split(/[-‐‑‒–—]/u).map(normWord).filter((p) => p !== '');
const phraseParts = (s) => s.split(/\s+/).flatMap(partsOf);

const cues = () => {
  if (cueData === null && cueJson !== null) {
    const d = JSON.parse(cueJson);
    if (typeof d.unavailable === 'string') {
      cueData = { unavailable: d.unavailable };
    } else {
      const words = d.words
        .map(([text, start, end]) => ({ parts: partsOf(text), start, end }))
        .filter((w) => w.parts.length > 0);
      // Every part in order, with its word and whether it opens or closes it.
      const seq = words.flatMap((w, i) =>
        w.parts.map((norm, j) => ({ norm, word: i, first: j === 0, last: j === w.parts.length - 1 })),
      );
      cueData = { scene: d.scene, words, seq, pauses: d.pauses ?? [] };
    }
  }
  return cueData;
};

// The narrated phrase nearest to `want` (its parts): every run of up to one
// word more than it has parts, spelt as the narration spells it.
const nearest = (want, words) => {
  const spelt = new Map();
  const longest = Math.min(want.length + 1, words.length);
  for (let n = 1; n <= longest; n++) {
    for (let i = 0; i + n <= words.length; i++) {
      const run = words.slice(i, i + n);
      const key = run.flatMap((w) => w.parts).join(' ');
      if (!spelt.has(key)) spelt.set(key, run.map((w) => w.parts.join('-')).join(' '));
    }
  }
  const near = __studioClosest(want.join(' '), [...spelt.keys()]);
  return near === null || near === undefined ? null : spelt.get(near);
};

// This call's cue data, or a throw naming `call` when there is none.
const narrated = (call) => {
  const c = cues();
  if (c === null) {
    throw new Error(`${call}: this scene is not narrated, so it has no words to cue; give it an id and a \`## <id>\` section in script.md, then run \`oculus studio narrate\``);
  }
  if (c.unavailable !== undefined) throw new Error(`${call}: ${c.unavailable}`);
  return c;
};

// The first and last word of the nth occurrence of `phrase`.
const findPhrase = (fn, phrase, nth) => {
  const call = `\`${fn}(${JSON.stringify(phrase)}${nth === 1 ? '' : `, ${nth}`})\``;
  if (typeof phrase !== 'string') throw new TypeError(`${call}: the phrase must be a string`);
  if (!(Number.isInteger(nth) && nth >= 1)) throw new RangeError(`${call}: nth must be a whole number from 1`);
  const want = phraseParts(phrase);
  if (want.length === 0) throw new TypeError(`${call}: the phrase has no words (letters or digits)`);
  const { seq, words, scene } = narrated(call);
  const n = want.length;
  let found = 0;
  for (let i = 0; i + n <= seq.length; i++) {
    if (!seq[i].first || !seq[i + n - 1].last) continue;
    let j = 0;
    while (j < n && seq[i + j].norm === want[j]) j++;
    if (j === n && ++found === nth) return [words[seq[i].word], words[seq[i + n - 1].word]];
  }
  const said = want.join(' ');
  if (found > 0) {
    const times = found === 1 ? 'once' : `${found} times`;
    throw new RangeError(`${call}: scene \`${scene}\` says "${said}" ${times}, so there is no occurrence ${nth}`);
  }
  const near = nearest(want, words);
  const closest = near === null ? '' : `; closest: ${near}`;
  throw new Error(`${call}: no such word in scene \`${scene}\`${closest}`);
};

const findPause = (nth) => {
  const call = `\`pause(${nth === 1 ? '' : nth})\``;
  if (!(Number.isInteger(nth) && nth >= 1)) throw new RangeError(`${call}: nth must be a whole number from 1`);
  const { scene, pauses } = narrated(call);
  if (nth <= pauses.length) return pauses[nth - 1][0];
  if (pauses.length === 0) {
    throw new Error(`${call}: scene \`${scene}\` has no \`[pause]\` in script.md`);
  }
  const n = pauses.length === 1 ? 'one pause' : `${pauses.length} pauses`;
  throw new RangeError(`${call}: scene \`${scene}\` has ${n}, so there is no pause ${nth}`);
};

define('word', (phrase, nth = 1) => findPhrase('word', phrase, nth)[0].start);
define('wordEnd', (phrase, nth = 1) => findPhrase('wordEnd', phrase, nth)[1].end);
define('pause', (nth = 1) => findPause(nth));
