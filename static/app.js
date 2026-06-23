// Opticrum Admin Console — REST API client

const API = '/api';

// ---- Tab navigation ----
document.querySelectorAll('.tab').forEach(btn => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('.tab').forEach(b => b.classList.remove('active'));
    document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
    btn.classList.add('active');
    document.getElementById('tab-' + btn.dataset.tab).classList.add('active');
  });
});

// ---- Dashboard ----
async function refreshStats() {
  const r = await fetch(API + '/admin/stats');
  const data = await r.json();
  document.getElementById('stat-orders').textContent =
    `Total: ${data.orders.total} | Live: ${data.orders.live} | Matched: ${data.orders.matched} | Cancelled: ${data.orders.cancelled}`;
  document.getElementById('stat-matches').textContent =
    `Total: ${data.matches.total} | Live: ${data.matches.live} | Exhausted: ${data.matches.exhausted} | Destroyed: ${data.matches.destroyed}`;
  document.getElementById('stat-extracted').textContent =
    (data.total_extracted_shannons / 100_000_000).toFixed(2) + ' CKB';
}
refreshStats();

// ---- Wallets ----
document.getElementById('wallet-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  const body = {
    label: fd.get('label'),
    private_key_hex: fd.get('private_key_hex'),
  };
  const r = await fetch(API + '/wallets', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (r.ok) {
    e.target.reset();
    loadWallets();
  } else {
    alert('Import failed: ' + (await r.json()).message);
  }
});

async function loadWallets() {
  const r = await fetch(API + '/wallets');
  const wallets = await r.json();
  const tbody = document.querySelector('#wallets-table tbody');
  tbody.innerHTML = wallets.map(w => `<tr>
    <td>${w.id}</td><td>${w.label}</td><td><code>${w.ckb_address}</code></td>
    <td>${w.created_at}</td>
    <td><button onclick="deleteWallet(${w.id})">Delete</button></td>
  </tr>`).join('');
}

async function deleteWallet(id) {
  if (!confirm('Delete wallet ' + id + '?')) return;
  await fetch(API + '/wallets/' + id, { method: 'DELETE' });
  loadWallets();
}
loadWallets();

// ---- On-Chain Orders ----
async function scanOrders() {
  document.getElementById('orders-result').textContent = 'Scanning...';
  const r = await fetch(API + '/orders/scan');
  const orders = await r.json();
  if (!orders.length) {
    document.getElementById('orders-result').innerHTML = '<p>No live orders found on chain.</p>';
    return;
  }
  document.getElementById('orders-result').innerHTML =
    '<table><thead><tr><th>Tx</th><th>Capacity</th><th>Escrow</th><th>Match</th></tr></thead><tbody>' +
    orders.map(o => `<tr>
      <td><code>${o.tx_hash.substring(0,16)}...</code></td>
      <td>${(o.channel_capacity / 100_000_000).toFixed(0)} CKB</td>
      <td>${o.escrow_blocks} blocks</td>
      <td><button onclick="matchOrderById('${o.tx_hash}', ${o.output_index})">Match</button></td>
    </tr>`).join('') + '</tbody></table>';
}

async function matchOrderById(txHash, outputIndex) {
  const channelTx = prompt('Channel outpoint tx_hash:');
  const channelIdx = prompt('Channel outpoint index:');
  if (!channelTx || !channelIdx) return;
  const seller = prompt('Seller address:');
  if (!seller) return;
  const r = await fetch(API + '/orders/1/match', { // TODO: order lookup by outpoint
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      seller_address: seller,
      channel_outpoint_tx_hash: channelTx,
      channel_outpoint_index: parseInt(channelIdx),
    }),
  });
  if (r.ok) {
    alert('Matched! ' + JSON.stringify(await r.json()));
  } else {
    alert('Match failed: ' + (await r.json()).message);
  }
}

// ---- Fiber Channels ----
async function scanChannels() {
  const owner = document.getElementById('channel-owner').value;
  document.getElementById('channels-result').textContent = 'Scanning...';
  const url = API + '/fiber/channels' + (owner ? '?owner=' + owner : '');
  const r = await fetch(url);
  const channels = await r.json();
  if (!channels.length) {
    document.getElementById('channels-result').innerHTML = '<p>No channels found.</p>';
    return;
  }
  document.getElementById('channels-result').innerHTML =
    '<table><thead><tr><th>Tx</th><th>Index</th><th>Capacity</th><th>Status</th></tr></thead><tbody>' +
    channels.map(c => `<tr>
      <td><code>${c.tx_hash.substring(0,16)}...</code></td>
      <td>${c.output_index}</td>
      <td>${(c.capacity / 100_000_000).toFixed(0)} CKB</td>
      <td>${c.status}</td>
    </tr>`).join('') + '</tbody></table>';
}

// ---- Matches ----
async function listMatches() {
  const r = await fetch(API + '/matches');
  const matches = await r.json();
  document.getElementById('matches-result').innerHTML = matches.length
    ? '<table><thead><tr><th>ID</th><th>Tx</th><th>Seller</th><th>Rent/Block</th><th>Status</th><th>Actions</th></tr></thead><tbody>' +
      matches.map(m => `<tr>
        <td>${m.id}</td><td><code>${m.tx_hash.substring(0,16)}...</code></td>
        <td>${m.seller_address}</td>
        <td>${m.rent_per_block.toFixed(2)}</td>
        <td>${m.status}</td>
        <td>
          ${m.status === 'live' ? `<button onclick="extractRent(${m.id})">Extract</button>` : ''}
          ${m.status === 'exhausted' ? `<button onclick="destroyMatch(${m.id})">Destroy</button>` : ''}
        </td>
      </tr>`).join('') + '</tbody></table>'
    : '<p>No tracked matches.</p>';
}
listMatches();

async function extractRent(id) {
  const r = await fetch(API + '/matches/' + id + '/extract', { method: 'POST' });
  if (r.ok) {
    alert('Extracted: ' + JSON.stringify(await r.json()));
    listMatches();
  }
}

async function destroyMatch(id) {
  if (!confirm('Destroy exhausted match ' + id + '?')) return;
  const r = await fetch(API + '/matches/' + id + '/destroy', { method: 'POST' });
  if (r.ok) listMatches();
}

// ---- Auto-Match Config ----
async function loadAutoMatchConfig() {
  const r = await fetch(API + '/admin/auto-match/config');
  const cfg = await r.json();
  document.getElementById('automatch-config').textContent = JSON.stringify(cfg, null, 2);
  const form = document.getElementById('automatch-form');
  form.style.display = 'block';
  form.enabled.checked = cfg.enabled;
  form.min_capacity_shannons.value = cfg.min_capacity_shannons;
  form.max_escrow_blocks.value = cfg.max_escrow_blocks;
  form.interval_secs.value = cfg.interval_secs;
}

document.getElementById('automatch-form').addEventListener('submit', async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  const body = {
    enabled: fd.get('enabled') === 'on',
    min_capacity_shannons: parseInt(fd.get('min_capacity_shannons')),
    max_escrow_blocks: parseInt(fd.get('max_escrow_blocks')),
    interval_secs: parseInt(fd.get('interval_secs')),
  };
  const r = await fetch(API + '/admin/auto-match/config', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  alert(JSON.stringify(await r.json(), null, 2));
});

// ---- External Signing ----
async function listUnsignedTxs() {
  const r = await fetch(API + '/transactions/unsigned');
  const txs = await r.json();
  document.getElementById('unsigned-result').innerHTML = txs.length
    ? '<table><thead><tr><th>ID</th><th>Operation</th><th>Status</th><th>Created</th><th>Actions</th></tr></thead><tbody>' +
      txs.map(tx => `<tr>
        <td><code>${tx.id}</code></td><td>${tx.operation}</td><td>${tx.status}</td>
        <td>${tx.created_at}</td>
        <td>
          ${tx.status === 'pending' ? `<button onclick="viewUnsignedTx('${tx.id}')">View</button>` : ''}
          ${tx.status === 'signed' ? `<button onclick="submitTx('${tx.id}')">Broadcast</button>` : ''}
        </td>
      </tr>`).join('') + '</tbody></table>'
    : '<p>No pending unsigned transactions.</p>';
}

async function viewUnsignedTx(id) {
  const r = await fetch(API + '/transactions/unsigned/' + id);
  const tx = await r.json();
  const witnesses = prompt('Paste signed witnesses (JSON):');
  if (!witnesses) return;
  const sr = await fetch(API + '/transactions/unsigned/' + id + '/witnesses', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ witnesses: JSON.parse(witnesses) }),
  });
  if (sr.ok) { alert('Witnesses submitted!'); listUnsignedTxs(); }
}

async function submitTx(id) {
  const r = await fetch(API + '/transactions/unsigned/' + id + '/submit', { method: 'POST' });
  if (r.ok) { alert('Broadcast!'); listUnsignedTxs(); }
}
