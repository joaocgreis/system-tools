import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const alphabet = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';

async function internals(command, exports) {
  // Execute the actual pure helpers without the command's entry point. Node's
  // stripper is used only by this test harness; production runs native stdin TS.
  const source = readFileSync(join(root, command), 'utf8');
  const boundary = source.lastIndexOf('\ntry {\n  await main();');
  assert.ok(boundary > 0, 'The CLI entry point must be identifiable');
  const stripped = stripTypeScriptTypes(source.slice(0, boundary).split('\n').slice(4).join('\n'));
  return import(`data:text/javascript;base64,${Buffer.from(`${stripped}\nexport { ${exports.join(', ')} };`).toString('base64')}`);
}

test('every representable calendar day round-trips through the actual helpers', async () => {
  const { encode } = await internals('cdate', ['encode']);
  const { decode } = await internals('ddate', ['decode']);
  let count = 0;
  for (let year = 2000; year <= 2061; year++) {
    for (let month = 1; month <= 12; month++) {
      const last = new Date(Date.UTC(year, month, 0)).getUTCDate();
      for (let day = 1; day <= last; day++) {
        const calendar = { year, month, day };
        const code = encode(calendar);
        assert.equal(code, `${alphabet[year - 2000]}${alphabet[month]}${alphabet[day]}`);
        assert.deepEqual(decode(code), calendar);
        count++;
      }
      if (last < 31) assert.throws(() => decode(`${alphabet[year - 2000]}${alphabet[month]}${alphabet[last + 1]}`), /Invalid day/);
    }
  }
  assert.equal(count, 22646);
  assert.throws(() => encode({ year: 1999, month: 12, day: 31 }), /cannot be represented/);
  assert.throws(() => encode({ year: 2062, month: 1, day: 1 }), /cannot be represented/);
});

test('all three-character codes have exactly the intended domain', async () => {
  const { decode } = await internals('ddate', ['decode']);
  let accepted = 0;
  for (const year of alphabet) for (const month of alphabet) for (const day of alphabet) {
    const monthNumber = alphabet.indexOf(month);
    const dayNumber = alphabet.indexOf(day);
    const fullYear = 2000 + alphabet.indexOf(year);
    const valid = monthNumber >= 1 && monthNumber <= 12 && dayNumber >= 1
      && dayNumber <= new Date(Date.UTC(fullYear, monthNumber, 0)).getUTCDate();
    if (valid) { assert.deepEqual(decode(year + month + day), { year: fullYear, month: monthNumber, day: dayNumber }); accepted++; }
    else assert.throws(() => decode(year + month + day));
  }
  assert.equal(accepted, 22646);
});

