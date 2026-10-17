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
    function compare(a, b) {
      const key = sort.value;
      const av = a.dataset[key], bv = b.dataset[key];
      if (av === '' || bv === '') return av === bv ? 0 : av === '' ? 1 : -1;
      // Integer counts remain exact above Number.MAX_SAFE_INTEGER.
      const floating = ['qscore', 'meanlength', 'passedlength'].includes(key);
      const an = floating ? Number(av) : BigInt(av);
      const bn = floating ? Number(bv) : BigInt(bv);
      const order = an < bn ? -1 : an > bn ? 1 : 0;
      return (['channel', 'mux'].includes(key) ? order : -order) || Number(a.dataset.channel) - Number(b.dataset.channel) || Number(a.dataset.mux || 0) - Number(b.dataset.mux || 0);
    }
    function update() {
      rows.sort(compare);
      const matches = rows.filter(row => row.dataset.channel.includes(search.value.trim()));
      const limit = Number(size.value);
      const pages = Math.max(1, Math.ceil(matches.length / limit));
      page = Math.min(page, pages - 1);
      rows.forEach(row => { row.hidden = true; });
      matches.slice(page * limit, (page + 1) * limit).forEach(row => { row.hidden = false; });
      body.replaceChildren(...rows);
      previous.disabled = page === 0;
      next.disabled = page === pages - 1;
      status.textContent = matches.length
        ? `${page * limit + 1}–${Math.min((page + 1) * limit, matches.length)} of ${matches.length.toLocaleString('en-US')} ${prefix === 'channel' ? 'channels' : 'channel/mux pairs'}`
        : 'No matching channels';
    }
    search.addEventListener('input', () => { page = 0; update(); });
    sort.addEventListener('change', () => { page = 0; update(); });
    size.addEventListener('change', () => { page = 0; update(); });
    previous.addEventListener('click', () => { page--; update(); });
    next.addEventListener('click', () => { page++; update(); });
    update();
    document.getElementById(`${prefix}-controls`).hidden = false;
    document.getElementById(`${prefix}-pagination`).hidden = false;
  }
})();
