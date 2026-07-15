const assert = require('assert');
const net = require('net');
const { EggfetchClient } = require('./index.js');

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    const result = fn();
    if (result && typeof result.then === 'function') {
      return result
        .then(() => {
          passed++;
          console.log(`  ✓ ${name}`);
        })
        .catch((err) => {
          failed++;
          console.log(`  ✗ ${name}`);
          console.log(`    ${err.message}`);
        });
    }
    passed++;
    console.log(`  ✓ ${name}`);
  } catch (err) {
    failed++;
    console.log(`  ✗ ${name}`);
    console.log(`    ${err.message}`);
  }
}

function createEchoServer(responseBody) {
  return new Promise((resolve) => {
    const server = net.createServer((socket) => {
      let data = '';
      socket.on('data', (chunk) => {
        data += chunk.toString();
        const header = [
          'HTTP/1.1 200 OK',
          'Content-Type: text/plain',
          `Content-Length: ${responseBody.length}`,
          'Connection: close',
          '',
          '',
        ].join('\r\n');
        socket.write(header);
        socket.write(responseBody);
        socket.end();
      });
    });
    server.listen(0, '127.0.0.1', () => {
      resolve({
        addr: server.address(),
        close: () => server.close(),
      });
    });
  });
}

function createChunkedServer() {
  return new Promise((resolve) => {
    const server = net.createServer((socket) => {
      let data = '';
      socket.on('data', (chunk) => {
        data += chunk.toString();
        const response = [
          'HTTP/1.1 200 OK',
          'Content-Type: text/plain',
          'Transfer-Encoding: chunked',
          'Connection: close',
          '',
          '',
          '5\r\n',
          'Hello\r\n',
          '6\r\n',
          ', worl\r\n',
          '2\r\n',
          'd!\r\n',
          '0\r\n',
          '',
          '',
        ].join('\r\n');
        socket.write(response);
        socket.end();
      });
    });
    server.listen(0, '127.0.0.1', () => {
      resolve({
        addr: server.address(),
        close: () => server.close(),
      });
    });
  });
}

async function run() {
  console.log('eggfetch-node binding tests\n');

  // Client lifecycle
  await test('client creation and drop', async () => {
    const client = new EggfetchClient();
    assert.ok(client, 'client should be created');
  });

  // GET request
  await test('GET request with echo server', async () => {
    const server = await createEchoServer('Hello, Node!');
    try {
      const client = new EggfetchClient();
      const resp = await client.get(`http://127.0.0.1:${server.addr.port}/test`);
      assert.strictEqual(resp.status, 200);
      assert.strictEqual(resp.text, 'Hello, Node!');
      assert.strictEqual(resp.ok, true);
    } finally {
      server.close();
    }
  });

  // POST request with body
  await test('POST request with body', async () => {
    const server = await createEchoServer('echo');
    try {
      const client = new EggfetchClient();
      const resp = await client.post(`http://127.0.0.1:${server.addr.port}/post`, 'test body');
      assert.strictEqual(resp.status, 200);
    } finally {
      server.close();
    }
  });

  // Response URL
  await test('response URL is set', async () => {
    const server = await createEchoServer('ok');
    try {
      const client = new EggfetchClient();
      const resp = await client.get(`http://127.0.0.1:${server.addr.port}/path`);
      assert.ok(resp.url.includes(`127.0.0.1:${server.addr.port}`));
    } finally {
      server.close();
    }
  });

  // Response headers
  await test('response headers are available', async () => {
    const server = await createEchoServer('ok');
    try {
      const client = new EggfetchClient();
      const resp = await client.get(`http://127.0.0.1:${server.addr.port}/headers`);
      const headers = resp.headers;
      assert.ok(headers['content-type'], 'should have content-type header');
      assert.strictEqual(headers['content-type'], 'text/plain');
    } finally {
      server.close();
    }
  });

  // Error handling
  await test('error on invalid host', async () => {
    const client = new EggfetchClient();
    try {
      await client.get('http://invalid.example.test:19999/nope');
      assert.fail('should have thrown');
    } catch (err) {
      assert.ok(err.message.length > 0, 'error should have a message');
    }
  });

  // HEAD request
  await test('HEAD request', async () => {
    const server = await createEchoServer('should not see body');
    try {
      const client = new EggfetchClient();
      const resp = await client.head(`http://127.0.0.1:${server.addr.port}/head`);
      assert.strictEqual(resp.status, 200);
    } finally {
      server.close();
    }
  });

  // Multiple clients
  await test('multiple clients can coexist', async () => {
    const server = await createEchoServer('ok');
    try {
      const client1 = new EggfetchClient();
      const client2 = new EggfetchClient();
      const resp1 = await client1.get(`http://127.0.0.1:${server.addr.port}/a`);
      const resp2 = await client2.get(`http://127.0.0.1:${server.addr.port}/b`);
      assert.strictEqual(resp1.status, 200);
      assert.strictEqual(resp2.status, 200);
    } finally {
      server.close();
    }
  });

  // Request method
  await test('custom request method', async () => {
    const server = await createEchoServer('ok');
    try {
      const client = new EggfetchClient();
      const resp = await client.request('DELETE', `http://127.0.0.1:${server.addr.port}/del`, null);
      assert.strictEqual(resp.status, 200);
    } finally {
      server.close();
    }
  });

  // JSON body
  await test('JSON response parsing', async () => {
    const jsonData = JSON.stringify({ key: 'value', count: 42 });
    const server = await createEchoServer(jsonData);
    try {
      const client = new EggfetchClient();
      const resp = await client.get(`http://127.0.0.1:${server.addr.port}/json`);
      const json = resp.json;
      assert.deepStrictEqual(json, { key: 'value', count: 42 });
    } finally {
      server.close();
    }
  });

  console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed\n`);
  process.exit(failed > 0 ? 1 : 0);
}

run().catch((err) => {
  console.error(err);
  process.exit(1);
});
