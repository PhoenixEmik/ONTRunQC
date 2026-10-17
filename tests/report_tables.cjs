// Optional dependency-free behavioral test: node tests/report_tables.cjs
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

class Element {
  constructor(value = '') { this.value = value; this.hidden = true; this.listeners = {}; }
  addEventListener(name, handler) { this.listeners[name] = handler; }
  fire(name) { if (!this.disabled) this.listeners[name](); }
}

function exercise(empty = false) {
  const elements = new Map();
  const collections = new Map();
  for (const prefix of ['channel', 'channel-mux']) {
    const rows = [];
    if (!empty) for (let channel = 1; channel <= 127; channel++) {
      for (const mux of prefix === 'channel' ? [0] : [1, 7]) {
        rows.push({ dataset: {
          channel: String(channel), mux: String(mux),
          yield: String(9007199254740992n + BigInt(channel % 7)),
          reads: String(channel % 9), qscore: channel % 11 ? String(channel % 5 + 0.5) : '',
          meanlength: channel % 11 ? String(channel % 13 + 0.25) : '',
          passedlength: channel % 11 ? String(channel % 17 + 0.75) : '',
        } });
      }
    }
    const body = { children: rows.slice(), replaceChildren(...page) { this.children = page; } };
    elements.set(`${prefix}-table`, { querySelectorAll: () => body.children, tBodies: [body] });
    for (const [id, value] of Object.entries({ search: '', sort: 'channel', 'page-size': '25', previous: '', next: '', status: '', controls: '', pagination: '' })) {
      elements.set(`${prefix}-${id}`, new Element(value));
    }
    collections.set(prefix, { rows, body });
  }
  vm.runInNewContext(fs.readFileSync(`${__dirname}/../src/report.js`, 'utf8'), {
    document: { getElementById: id => elements.get(id) },
  });
  for (const [prefix, { rows, body }] of collections) {
    const get = id => elements.get(`${prefix}-${id}`);
    if (empty) {
      assert.equal(body.children.length, 0);
      assert.equal(get('controls').hidden, true);
      continue;
    }
    assert.equal(get('controls').hidden, false);
    assert.equal(get('pagination').hidden, false);
    const key = row => `${row.dataset.channel}/${row.dataset.mux}`;
    let expected = rows.slice();
    const check = (page = 0) => {
      const limit = Number(get('page-size').value);
      assert.deepEqual(body.children.map(key), expected.slice(page * limit, (page + 1) * limit).map(key));
      assert.ok(body.children.length <= limit);
      assert.equal(get('previous').disabled, page === 0);
      assert.equal(get('next').disabled, (page + 1) * limit >= expected.length);
      const range = `${page * limit + 1}–${Math.min((page + 1) * limit, expected.length)}`;
      assert.equal(get('status').textContent, expected.length
        ? `${range} of ${expected.length.toLocaleString('en-US')} ${prefix === 'channel' ? 'channels' : 'channel/mux pairs'}`
        : 'No matching channels');
    };
    check();
    for (const limit of [50, 100, 25]) {
      get('page-size').value = String(limit); get('page-size').fire('change'); check();
      let page = 0;
      while (!get('next').disabled) { get('next').fire('click'); check(++page); }
      if (page) { get('previous').fire('click'); check(page - 1); }
    }
    const sortKeys = prefix === 'channel' ? ['yield', 'reads', 'qscore', 'meanlength', 'passedlength', 'channel'] : ['yield', 'reads', 'mux', 'channel'];
    for (const sort of sortKeys) {
      // Independent reference ordering, including integers differing by one above 2^53.
      expected.sort((a, b) => {
        const av = a.dataset[sort], bv = b.dataset[sort];
        if (av === '' || bv === '') return av === bv ? 0 : av === '' ? 1 : -1;
        const floating = ['qscore', 'meanlength', 'passedlength'].includes(sort);
        const left = floating ? Number(av) : BigInt(av), right = floating ? Number(bv) : BigInt(bv);
        const difference = left === right ? 0 : left > right ? 1 : -1;
        return difference * (['channel', 'mux'].includes(sort) ? 1 : -1)
          || Number(a.dataset.channel) - Number(b.dataset.channel) || Number(a.dataset.mux) - Number(b.dataset.mux);
      });
      get('sort').value = sort; get('sort').fire('change'); check();
      const ordered = expected.slice();
      for (const query of [' 12 ', 'not-a-channel', '']) {
        expected = ordered.filter(row => row.dataset.channel.includes(query.trim()));
        get('search').value = query; get('search').fire('input'); check();
      }
    }
    assert.ok(rows.every(row => !('hidden' in row)), 'detached data rows must not need hidden toggles');
  }
}
exercise();
exercise(true);
console.log('Table behavior passed: both tables, all page sizes, last pages, search/empty/reset, every sort, nulls, BigInt and stable ties.');
