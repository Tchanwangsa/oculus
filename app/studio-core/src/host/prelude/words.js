// Word cues: `word(phrase, nth)` is when the nth time the narration says
// `phrase` starts, `wordEnd` when it ends, in scene seconds (narration lead
// included). Each call is handed its own scene's words as JSON (set in
// entry.js), parsed on first use. Words match after normalising: lowercase,
// letters, digits and apostrophes only; a phrase is consecutive words.
let cueJson = null;
let cueData = null;

const normWord = (w) => String(w).replace(/[‘’]/g, "'").toLowerCase().replace(/[^\p{L}\p{N}']/gu, '');
const phraseWords = (s) => s.split(/\s+/).map(normWord).filter((w) => w !== '');

const cues = () => {
  if (cueData === null && cueJson !== null) {
    const d = JSON.parse(cueJson);
    const words = d.words.map(([text, start, end]) => ({ norm: normWord(text), start, end }));
    cueData = { scene: d.scene, words: words.filter((w) => w.norm !== '') };
  }
  return cueData;
};

// The first and last word of the nth occurrence of `phrase`.
const findPhrase = (fn, phrase, nth) => {
  const call = `\`${fn}(${JSON.stringify(phrase)}${nth === 1 ? '' : `, ${nth}`})\``;
  if (typeof phrase !== 'string') throw new TypeError(`${call}: the phrase must be a string`);
  if (!(Number.isInteger(nth) && nth >= 1)) throw new RangeError(`${call}: nth must be a whole number from 1`);
  const want = phraseWords(phrase);
  if (want.length === 0) throw new TypeError(`${call}: the phrase has no words (letters or digits)`);
  const c = cues();
  if (c === null) {
    throw new Error(`${call}: this scene is not narrated, so it has no words to cue; give it an id and a \`## <id>\` section in script.md, then run \`oculus studio narrate\``);
  }
  const ws = c.words;
  const n = want.length;
  let found = 0;
  for (let i = 0; i + n <= ws.length; i++) {
    let j = 0;
    while (j < n && ws[i + j].norm === want[j]) j++;
    if (j === n && ++found === nth) return [ws[i], ws[i + n - 1]];
  }
  const said = want.join(' ');
  if (found > 0) {
    const times = found === 1 ? 'once' : `${found} times`;
    throw new RangeError(`${call}: scene \`${c.scene}\` says "${said}" ${times}, so there is no occurrence ${nth}`);
  }
  const windows = [];
  for (let i = 0; i + n <= ws.length; i++) windows.push(ws.slice(i, i + n).map((w) => w.norm).join(' '));
  const near = __studioClosest(said, windows);
  const closest = near === null || near === undefined ? '' : `; closest: ${near}`;
  throw new Error(`${call}: no such word in scene \`${c.scene}\`${closest}`);
};

define('word', (phrase, nth = 1) => findPhrase('word', phrase, nth)[0].start);
define('wordEnd', (phrase, nth = 1) => findPhrase('wordEnd', phrase, nth)[1].end);
