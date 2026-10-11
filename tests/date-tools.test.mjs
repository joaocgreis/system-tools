import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { chmodSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, symlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const environment = { ...process.env, TZ: 'UTC', LC_ALL: 'C' };
const alphabet = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';

function run(command, args = [], overrides = {}) {
  return spawnSync(join(root, command), args, {
    encoding: 'utf8', timeout: 7000, env: environment, ...overrides,
  });
}
function success(command, args, expected, overrides) {
  const result = run(command, args, overrides);
  assert.equal(result.error, undefined);
  assert.equal(result.status, 0, `${command} ${args.join(' ')}: ${result.stderr}`);
  assert.equal(result.stderr, '');
  assert.equal(result.stdout, expected);
}
function failure(command, args, pattern, overrides) {
  const result = run(command, args, overrides);
  assert.equal(result.error, undefined);
  assert.equal(result.status, 1, `${command} ${args.join(' ')}`);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, pattern);
}
function systemDate(input, flags = [], env = environment) {
  return execFileSync('date', [`--date=${input}`, ...flags], { encoding: 'utf8', timeout: 5000, env });
}
function expectedCode(iso) {
  const [year, month, day] = iso.trim().split('-').map(Number);
  return alphabet[year - 2000] + alphabet[month] + alphabet[day];
}
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

test('the requested inputs and English/RFC/JavaScript forms encode A1B', () => {
  const examples = [
    ['2010-01-11'], ['20100111'], ['January', '11,', '2010'], ['2010-01-11T13:41:54Z'],
    ['2010-01-11T13:41:54.123456789Z'], ['2010-01-11T13:41:54,123Z'],
    ['Mon Jan 11 13:41:54 UTC 2010'], ['Mon, 11 Jan 2010 13:41:54 +0000'],
    ['Mon Jan 11 2010 13:41:54 GMT+0000 (Coordinated Universal Time)'],
    ['Mon Jan 11 2010'], ['11 Jan 2010'], ['1/11/2010, 1:41:54 PM'], ['11/01/2010', '-i', 'dmy'],
    ['01112010', '-i', 'mdy'], ['11012010', '-i', 'dmy'], ['2010/01/11', '-i', 'ymd'],
    ['01/11/10'], ['2010-01-11T13Z'], ['2010-01-11T13:41Z'],
    ['2010-01-11T13:41:54+05:30'], ['@1263217314'], ['1263217314'], ['1263217314000'],
    ['1263217314', '-i', 'unix'], ['1263217314000', '-i', 'unix-ms'],
    ['--date', 'January 11, 2010'], ['2010-01-11 13:41:54 UTC', '-i', 'date'],
    ['2010-01-11T13:41:54Z', '-i', 'javascript'],
  ];
  for (const args of examples) success('cdate', args, 'A1B\n');
});

test('date built-in output modes and JavaScript Date methods are accepted', () => {
  const input = '2010-01-11T13:41:54Z';
  const modes = [[], ['-I'], ['-Ihours'], ['-Iminutes'], ['-Iseconds'], ['-Ins'], ['-R'],
    ['--rfc-3339=date'], ['--rfc-3339=seconds'], ['--rfc-3339=ns']];
  const formats = modes.map((flags) => systemDate(input, flags).trim());
  const date = new Date(input);
  for (const method of ['toString', 'toUTCString', 'toISOString', 'toJSON', 'toDateString', 'toLocaleString', 'toLocaleDateString']) {
    formats.push(date[method]());
  }
  // Time-only output carries no historical calendar. It uses today, as date does.
  const timeOnly = date.toTimeString();
  success('cdate', [timeOnly], `${expectedCode(systemDate(timeOnly, ['+%F']))}\n`);
  success('cdate', [date.toLocaleTimeString('en-US')], `${expectedCode(systemDate('13:41:54', ['+%F']))}\n`);
  for (const input of formats) success('cdate', [input], 'A1B\n');
});

test('calendar guards reject impossible days instead of JS rollover', () => {
  for (const input of ['2010-02-29', '2010-02-30', '20100230', '2010-04-31', '2010-00-11',
    '2010-13-11', '2010-01-00', '2010-01-32', 'February 30, 2010', 'Mon Feb 30 2010',
    '2010-01-11T25:00:00Z', '2010-01-11T24:00:01Z', '2010-01-11T24:00:00.001Z',
    '2010-01-11T13:60:00Z', '2010-01-11T13:00:60Z', '2010-01-11T13:00:00+24:00',
    '2010-01-11T13:00:00+05:60', 'not a date', 'Invalid Date', '', ' \n ']) {
    failure('cdate', [input], /Invalid|invalid|nonempty|could not parse/);
  }
  failure('cdate', ['2010-02-30', '-i', 'javascript'], /Invalid calendar/);
  failure('cdate', ['February 30, 2010', '-i', 'javascript'], /could not parse/);
  success('cdate', ['2012-02-29'], 'C2T\n');
  success('cdate', ['2010-01-11T24:00:00Z'], 'A1C\n');
  success('cdate', ['2010-01-11T24:00Z'], 'A1C\n');
});

test('range boundaries and year alphabet transitions are exact', () => {
  for (const [date, code] of [['2000-01-01', '011'], ['2009-12-31', '9CV'], ['2010-01-11', 'A1B'],
    ['2035-12-31', 'ZCV'], ['2036-01-01', 'a11'], ['2061-12-31', 'zCV']]) {
    success('cdate', [date], `${code}\n`);
    success('ddate', [code, '+%F'], `${date}\n`);
  }
  for (const input of ['1999-12-31', '2062-01-01', '2100-01-01', '1900-01-01', '2061-12-31T24:00Z']) {
    failure('cdate', [input], /cannot be represented/);
  }
  success('cdate', ['1999-12-31T23:30:00-01:00', '-u'], '011\n');
  success('cdate', ['2062-01-01T00:30:00+01:00', '-u'], 'zCV\n');
});

test('output zones independently select the calendar day; both defaults are UTC', () => {
  const west = { env: { ...environment, TZ: 'America/Los_Angeles' } };
  success('cdate', ['2010-01-11T00:30:00Z'], 'A1B\n', west);
  success('cdate', ['2010-01-11T00:30:00Z', '-l'], 'A1A\n', west);
  success('cdate', ['2010-01-11T00:30:00Z', '--local'], 'A1A\n', west);
  success('cdate', ['2010-01-11T00:30:00Z', '-u'], 'A1B\n', west);
  success('cdate', ['2010-01-11'], 'A1B\n', west);
  success('cdate', ['2010-01-11', '-l'], 'A1A\n', west);
  success('cdate', ['2010-01-11', '-i', 'javascript'], 'A1B\n', west);
  success('cdate', ['2010-01-11', '-i', 'javascript', '--local'], 'A1A\n', west);
  success('cdate', ['2010-01-11', '-i', 'javascript', '-u'], 'A1B\n', west);
  success('cdate', ['2010-01-11T00:30:00Z', '-z', 'America/Los_Angeles'], 'A1A\n');
  success('cdate', ['2010-01-11T23:30:00Z', '-z', 'Asia/Kolkata'], 'A1C\n');
  success('cdate', ['Mon Jan 11 2010 00:30:00 GMT+0500 (Pakistan Standard Time)'], 'A1A\n');
  success('cdate', [], `${expectedCode(systemDate('now', ['+%F']))}\n`, west);
  success('cdate', ['--local'], `${expectedCode(systemDate('now', ['+%F'], west.env))}\n`, west);
  success('cdate', ['2010-03-14 02:30:00'], 'A3E\n', west);
  const timestamp = String(Date.parse('2010-01-11T00:30:00Z') / 1000);
  success('cdate', [timestamp], 'A1B\n', west);
  success('cdate', [timestamp, '--local'], 'A1A\n', west);
  success('ddate', ['A1B', '+%F %T %z'], '2010-01-11 00:00:00 +0000\n', west);
  success('ddate', ['A1B', '-l', '+%F %T %z'], '2010-01-10 16:00:00 -0800\n', west);
  success('ddate', ['A1B', '--local', '+%F %T %z'], '2010-01-10 16:00:00 -0800\n', west);
  success('ddate', ['A1B', '-u', '+%F %T %z'], '2010-01-11 00:00:00 +0000\n', west);
  success('ddate', ['BCU', '-z', 'Pacific/Apia', '+%F %T %z'], '2011-12-29 14:00:00 -1000\n');
  success('ddate', ['BCU', '-u', '+%F'], '2011-12-30\n');
});

test('input zones interpret unzoned input without changing the output zone', () => {
  const west = { env: { ...environment, TZ: 'America/Los_Angeles' } };
  success('cdate', ['2010-01-11', '--input-local'], 'A1B\n', west);
  success('cdate', ['2010-01-11', '--input-local', '--local'], 'A1B\n', west);
  success('cdate', ['2010-01-11 23:30:00', '--input-local'], 'A1C\n', west);
  success('cdate', ['2010-01-11 23:30:00', '--input-local', '--utc'], 'A1C\n', west);
  success('cdate', ['2010-01-11 23:30:00', '--input-local', '--local'], 'A1B\n', west);
  success('cdate', ['2010-01-11 00:30:00', '--input-utc', '--local'], 'A1A\n', west);
  success('cdate', ['2010-01-11 00:30:00', '--input-timezone', 'Asia/Tokyo'], 'A1A\n');
  success('cdate', ['2010-01-11 00:30:00', '--input-timezone', 'Asia/Tokyo', '-z', 'Asia/Tokyo'], 'A1B\n');
  success('cdate', ['2010-01-11 23:30:00', '--input-timezone', 'America/Los_Angeles', '-z', 'Asia/Tokyo'], 'A1C\n');
  for (const options of [['--input-local'], ['--input-timezone', 'Asia/Tokyo']]) {
    success('cdate', ['2010-01-11T00:30:00Z', ...options], 'A1B\n', west);
    success('cdate', ['2010-01-11T00:30:00Z', ...options, '--local'], 'A1A\n', west);
    success('cdate', ['1263169800', ...options], 'A1B\n', west);
  }
  failure('cdate', ['2010-03-14 02:30:00', '--input-local'], /could not parse|does not exist/, west);
  failure('ddate', ['BCU', '--input-timezone', 'Pacific/Apia', '+%F'], /could not interpret|does not exist/);
  success('ddate', ['A1B', '--input-local', '+%F %T %z'], '2010-01-11 08:00:00 +0000\n', west);
  success('ddate', ['A1B', '--input-local', '--local', '+%F %T %z'], '2010-01-11 00:00:00 -0800\n', west);
  success('ddate', ['A1B', '--input-utc', '--local', '+%F %T %z'], '2010-01-10 16:00:00 -0800\n', west);
  success('ddate', ['A1B', '--input-timezone', 'Asia/Tokyo', '+%F %T %z'], '2010-01-10 15:00:00 +0000\n');
  success('ddate', ['A1B', '--input-timezone', 'Asia/Tokyo', '-z', 'America/Los_Angeles', '+%F %T %z'], '2010-01-10 07:00:00 -0800\n');
  success('ddate', ['A1B', '+%s'], '1263168000\n', west);
  success('ddate', ['A1B', '--local', '+%s'], '1263168000\n', west);
  success('ddate', ['A1B', '--input-local', '+%s'], '1263196800\n', west);
});

test('JavaScript uses the input zone for unzoned date-times and restores the local output zone', () => {
  const west = { env: { ...environment, TZ: 'America/Los_Angeles' } };
  success('cdate', ['2010-01-11T00:30:00', '-i', 'javascript', '--local'], 'A1A\n', west);
  success('cdate', ['2010-01-11T23:30:00', '-i', 'javascript', '--input-local'], 'A1C\n', west);
  success('cdate', ['2010-01-11T23:30:00', '-i', 'javascript', '--input-local', '--local'], 'A1B\n', west);
  success('cdate', ['2010-01-11T00:30:00', '-i', 'javascript', '--input-timezone', 'Asia/Tokyo'], 'A1A\n', west);
  success('cdate', ['2010-01-11', '-i', 'javascript', '--input-local', '--local'], 'A1A\n', west);
  success('cdate', ['2011-12-30', '-i', 'javascript', '--input-timezone', 'Pacific/Apia'], 'BCU\n');
  success('cdate', ['2011-12-30T00:00:00Z', '--input-timezone', 'Pacific/Apia'], 'BCU\n');
  for (const input of ['Fri, 30 Dec 2011 00:00:00 +0000', 'Fri Dec 30 00:00:00 UTC 2011', '2011-12-30 00:00:00 UTC']) {
    success('cdate', [input, '--input-timezone', 'Pacific/Apia'], 'BCU\n');
  }
  failure('cdate', ['2010-03-14T02:30:00', '-i', 'javascript', '--input-local'], /could not parse|does not exist/, west);
});

test('ddate uses a supplied input zone unless an input flag explicitly reinterprets the day', () => {
  const west = { env: { ...environment, TZ: 'America/Los_Angeles' } };
  const format = '+%F %T %z';
  for (const input of [['A1B', 'Asia/Tokyo'], ['A1B Asia/Tokyo'], ['--date', 'A1B Asia/Tokyo']]) {
    success('ddate', [...input, format], '2010-01-10 15:00:00 +0000\n', west);
    success('ddate', [...input, '--local', format], '2010-01-10 07:00:00 -0800\n', west);
    success('ddate', [...input, '--input-utc', format], '2010-01-11 00:00:00 +0000\n', west);
    success('ddate', [...input, '--input-local', format], '2010-01-11 08:00:00 +0000\n', west);
    success('ddate', [...input, '--input-timezone', 'Europe/Berlin', format], '2010-01-10 23:00:00 +0000\n', west);
  }
  success('ddate', ['A1B America/Los_Angeles', format], '2010-01-11 08:00:00 +0000\n');
  for (const input of ['A1B UTC', 'A1B GMT', 'A1B Z', 'A1BZ', 'A1B+00:00', 'A1B-00']) {
    success('ddate', [input, format], '2010-01-11 00:00:00 +0000\n');
  }
  for (const input of [['A1B+05:30'], ['A1B+0530'], ['A1B +05:30'], ['A1B', '+05:30']]) {
    success('ddate', [...input, format], '2010-01-10 18:30:00 +0000\n');
    success('ddate', [...input, '--input-utc', format], '2010-01-11 00:00:00 +0000\n');
    success('ddate', [...input, '--input-local', '--local', format], '2010-01-11 00:00:00 -0800\n', west);
  }
  success('ddate', ['A1B-0800', format], '2010-01-11 08:00:00 +0000\n');
  success('ddate', ['--date', 'A1B+05:30', '+0500'], '0500\n');
  failure('ddate', ['BCU Pacific/Apia', format], /could not interpret|does not exist/);
  success('ddate', ['BCU Pacific/Apia', '--input-utc', '+%F'], '2011-12-30\n');
  for (const input of ['A1B Imaginary/Zone', 'A1B+24:00', 'A1B+05:60', 'A1B+5:30', 'A1B UTC extra']) {
    failure('ddate', [input], /Invalid|compact date/);
  }
});

test('epoch units use exact decimals and reject malformed values', () => {
  success('cdate', ['1263168000.000000001', '-i', 'unix'], 'A1B\n');
  success('cdate', ['1263167999.999999999', '-i', 'unix'], 'A1A\n');
  success('cdate', ['1263168000000.000001', '-i', 'unix-ms'], 'A1B\n');
  success('cdate', ['1263167999999.999999', '-i', 'unix-ms'], 'A1A\n');
  for (const value of ['NaN', 'Infinity', '1e12', '12x', '1.0000000001', '']) failure('cdate', [value, '-i', 'unix'], /Invalid|at most|nonempty/);
  failure('cdate', ['0', '-i', 'unix'], /cannot be represented/);
  failure('cdate', ['--', '-1'], /could not parse|cannot be represented/);
  failure('cdate', ['99999999999999999999999999999999', '-i', 'unix'], /could not parse|cannot be represented/);
});

test('date mode keeps the installed parser grammar and relative dates work', () => {
  success('cdate', ['2010'], 'A11\n');
  success('cdate', ['2010-01'], 'A11\n');
  success('cdate', ['2010', '-i', 'date'], `${expectedCode(systemDate('2010', ['+%F']))}\n`);
  success('cdate', ['2010-01-11 +1 day'], 'A1C\n');
  success('cdate', [], `${expectedCode(systemDate('now', ['+%F']))}\n`);
  for (const input of ['today', 'yesterday', 'tomorrow', 'next Friday']) {
    success('cdate', [input], `${expectedCode(systemDate(input, ['+%F']))}\n`);
  }
  success('cdate', ['13/01/2010'], 'A1D\n');
  success('cdate', ['11/01/2010'], 'AB1\n');
  success('cdate', ['11/01/2010', '-i', 'dmy'], 'A1B\n');
});

test('ddate preserves installed date formatting, including locale and directives', () => {
  const midnight = '2010-01-11 00:00:00';
  const choices = [[], ['-I'], ['-Ihours'], ['-Iminutes'], ['-Iseconds'], ['-Ins'], ['--iso-8601=ns'],
    ['-R'], ['--rfc-email'], ['--rfc-2822'], ['--rfc-3339', 'date'], ['--rfc-3339=seconds'],
    ['--rfc-3339=ns'], ['+%F'], ['+%A, %B %d, %Y'], ['+%Y %C %y %m %-d %_d %j %q'],
    ['+%G %g %V %U %W %u %w'], ['+%H %I %k %l %M %S %N %p %P %r %R %T'],
    ['+%s %z %:z %::z %:::z %Z'], ['+%c %x %X %a %b %h %D %e %% %t %n'], ['+']];
  for (const flags of choices) success('ddate', ['A1B', ...flags], systemDate(midnight, flags));
  success('ddate', ['--date', 'A1B', '+%F'], '2010-01-11\n');
  const env = { ...environment, TZ: 'Europe/Berlin', LC_ALL: 'C.UTF-8' };
  success('ddate', ['A1B', '--input-local', '--local', '+%c %Z'], systemDate(midnight, ['+%c %Z'], env), { env });
});

test('invalid compact codes, unknown flags and duplicate/conflicting options fail', () => {
  for (const code of ['', 'A1', 'A1BB', 'A0B', 'ADB', 'A1W', 'A1b', 'A10', 'A2T', 'A4V', 'ä1B', 'A1B ', ' A1B', 'A1\n']) {
    failure('ddate', [code], /compact date|Invalid/);
  }
  failure('ddate', [], /required/);
  for (const command of ['cdate', 'ddate']) {
    const input = command === 'cdate' ? '2010-01-11' : 'A1B';
    for (const options of [['--wat'], ['--utc', '-u'], ['--universal', '--utc'], ['--debug', '-d'], ['-u', '-l'],
      ['-u', '-z', 'UTC'], ['-z', 'Imaginary/Zone'], ['--date', input, input],
      ['--input-utc', '--input-local'], ['--input-local', '--input-timezone', 'UTC'],
      ['--input-utc', '--input-timezone', 'UTC'], ['--input-local', '--input-local'],
      ['--input-timezone', 'UTC', '--input-timezone', 'UTC'], ['--input-timezone', 'Imaginary/Zone']]) {
      failure(command, [input, ...options], /Unknown|more than once|Choose|Invalid|either|exactly/);
    }
    failure(command, [input, '--input-timezone'], /argument/);
  }
  failure('cdate', ['-i', 'unknown', '2010-01-11'], /Unknown input format/);
  failure('cdate', ['-i', 'ymd'], /requires a date/);
  failure('cdate', ['2010-01-11', '-r', '/tmp/file'], /either/);
  failure('cdate', ['January 11, 2010', '-i', 'ymd'], /does not match/);
  failure('cdate', ['-i'], /argument/);
  failure('ddate', ['A1B', 'A1C'], /Invalid.*time zone/);
  failure('ddate', ['A1B', '+%F', '+%Y'], /at most/);
  failure('ddate', ['A1B', '-I', '+%F'], /Choose/);
  failure('ddate', ['A1B', '-R', '--rfc-2822'], /more than once/);
  failure('ddate', ['A1B', '--iso-8601=bogus'], /Invalid ISO/);
  failure('ddate', ['A1B', '--rfc-3339=hours'], /Invalid RFC/);
});

test('reference input uses modification time and fails on missing files', () => {
  const dir = mkdtempSync(join(tmpdir(), 'date-tools-reference-'));
  try {
    const path = join(dir, 'file with spaces');
    writeFileSync(path, 'fixture\n');
    const timestamp = new Date('2010-01-11T00:30:00Z');
    utimesSync(path, timestamp, timestamp);
    success('cdate', ['-r', path], 'A1B\n');
    const west = { env: { ...environment, TZ: 'America/Los_Angeles' } };
    success('cdate', ['-r', path], 'A1B\n', west);
    success('cdate', ['-r', path, '--local'], 'A1A\n', west);
    success('cdate', ['-r', path, '--input-local'], 'A1B\n', west);
    failure('cdate', ['-r', join(dir, 'missing')], /ENOENT/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('help and version have no operational effects and work without date', () => {
  const dir = mkdtempSync(join(tmpdir(), 'date-tools-help-'));
  try {
    symlinkSync(process.execPath, join(dir, 'node'));
    const env = { ...environment, PATH: dir };
    for (const command of ['cdate', 'ddate']) {
      for (const flag of ['-h', '--help']) {
        const result = run(command, [flag, '--debug', '--date', 'invalid'], { cwd: dir, env });
        assert.equal(result.status, 0, result.stderr);
        assert.match(result.stdout, new RegExp(`Usage: ./` + command));
        assert.match(result.stdout, /--debug/);
        assert.equal(result.stderr, '');
      }
      success(command, ['--version', '--debug'], `${command} 1.0.0\n`, { cwd: dir, env });
      failure(command, [command === 'cdate' ? '2010-01-11' : 'A1B'], /ENOENT/, { cwd: dir, env });
    }
    assert.deepEqual(readdirSync(dir), ['node']);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('private debug logs record success and failures; collisions never overwrite', () => {
  for (const command of ['cdate', 'ddate']) {
    const dir = mkdtempSync(join(tmpdir(), 'date-tools-debug-'));
    try {
      const args = command === 'cdate' ? ['2010-01-11'] : ['A1B', '+%F'];
      const result = run(command, ['--debug', ...args], { cwd: dir });
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.stdout, command === 'cdate' ? 'A1B\n' : '2010-01-11\n');
      assert.match(result.stderr, /debug log:/);
      const logs = readdirSync(dir);
      assert.equal(logs.length, 1);
      const name = logs[0];
      assert.match(name, new RegExp(`^_\\d{8}_\\d{6}_${command}\\.log$`));
      const path = join(dir, name);
      assert.equal(statSync(path).mode & 0o777, 0o600);
      const log = readFileSync(path, 'utf8');
      for (const step of ['Environment and invocation', 'Execution plan', 'Completed']) assert.ok(log.includes(step));
      assert.ok(log.includes('"reason"'));
      assert.ok(!result.stderr.includes('Execution plan'));

      // Precreate all plausible next-second names; do not rely on test timing.
      const base = new Date();
      for (let offset = -2; offset <= 5; offset++) {
        const now = new Date(base.getTime() + offset * 1000);
        const pad = (n) => String(n).padStart(2, '0');
        const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
        const candidate = join(dir, `_${stamp}_${command}.log`);
        if (candidate !== path) writeFileSync(candidate, 'keep me\n', { flag: 'wx', mode: 0o600 });
      }
      failure(command, ['--debug', ...args], /EEXIST/, { cwd: dir });
      assert.equal(readFileSync(path, 'utf8'), log);
    } finally { rmSync(dir, { recursive: true, force: true }); }

    const failureDir = mkdtempSync(join(tmpdir(), 'date-tools-debug-failure-'));
    try {
      failure(command, ['--debug', command === 'cdate' ? '2010-02-30' : 'A2T'], /Invalid/, { cwd: failureDir });
      const logs = readdirSync(failureDir);
      assert.equal(logs.length, 1);
      assert.match(readFileSync(join(failureDir, logs[0]), 'utf8'), /Failed[\s\S]*"stack"/);
    } finally { rmSync(failureDir, { recursive: true, force: true }); }
  }
});

test('executable paths with spaces preserve arguments', () => {
  const dir = mkdtempSync(join(tmpdir(), 'date tools path '));
  try {
    for (const command of ['cdate', 'ddate']) {
      const path = join(dir, command);
      writeFileSync(path, readFileSync(join(root, command)));
      chmodSync(path, 0o755);
      const args = command === 'cdate' ? ['January', '11,', '2010'] : ['A1B', '+%A %F'];
      const result = spawnSync(path, args, { encoding: 'utf8', timeout: 7000, env: environment });
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.stdout, command === 'cdate' ? 'A1B\n' : 'Monday 2010-01-11\n');
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
