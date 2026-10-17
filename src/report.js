// Progressive enhancement only. All metrics and plots are rendered in Rust.
(() => {
  for (const prefix of ['channel', 'channel-mux']) {
    const table = document.getElementById(`${prefix}-table`);
    const rows = Array.from(table.querySelectorAll('tbody tr[data-channel]'));
    if (!rows.length) continue;
    const body = table.tBodies[0];
    const search = document.getElementById(`${prefix}-search`);
    const sort = document.getElementById(`${prefix}-sort`);
    const size = document.getElementById(`${prefix}-page-size`);
    const previous = document.getElementById(`${prefix}-previous`);
    const next = document.getElementById(`${prefix}-next`);
    const status = document.getElementById(`${prefix}-status`);
    let page = 0;
    let matches = rows;
    function compare(a, b) {
      const key = sort.value;
      const av = a.dataset[key], bv = b.dataset[key];
      if (av === '' || bv === '') return av === bv ? 0 : av === '' ? 1 : -1;
      // Integer counts remain exact above Number.MAX_SAFE_INTEGER.
      const floating = ['qscore', 'meanlength', 'passedlength', 'medianlength', 'qmedian'].includes(key);
      const an = floating ? Number(av) : BigInt(av);
      const bn = floating ? Number(bv) : BigInt(bv);
      const order = an < bn ? -1 : an > bn ? 1 : 0;
      return (['channel', 'mux'].includes(key) ? order : -order) || Number(a.dataset.channel) - Number(b.dataset.channel) || Number(a.dataset.mux || 0) - Number(b.dataset.mux || 0);
    }
    function render() {
      const limit = Number(size.value);
      const pages = Math.max(1, Math.ceil(matches.length / limit));
      page = Math.max(0, Math.min(page, pages - 1));
      // The full collection stays detached; mount only the selected page.
      body.replaceChildren(...matches.slice(page * limit, (page + 1) * limit));
      previous.disabled = page === 0;
      next.disabled = page === pages - 1;
      status.textContent = matches.length
        ? `${page * limit + 1}–${Math.min((page + 1) * limit, matches.length)} of ${matches.length.toLocaleString('en-US')} ${prefix === 'channel' ? 'channels' : 'channel/mux pairs'}`
        : 'No matching channels';
    }
    function selectRows(sortChanged) {
      if (sortChanged) rows.sort(compare);
      const query = search.value.trim();
      matches = query ? rows.filter(row => row.dataset.channel.includes(query)) : rows;
      page = 0;
      render();
    }
    search.addEventListener('input', () => selectRows(false));
    sort.addEventListener('change', () => selectRows(true));
    size.addEventListener('change', () => { page = 0; render(); });
    previous.addEventListener('click', () => { page--; render(); });
    next.addEventListener('click', () => { page++; render(); });
    selectRows(true);
    document.getElementById(`${prefix}-controls`).hidden = false;
    document.getElementById(`${prefix}-pagination`).hidden = false;
  }
})();
