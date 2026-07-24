"""Auth replay rejection tests for one-shot bodies.

Track 4.4: Verify that sync and async iterators used as request bodies
are correctly handled when auth requires replaying the body.

In the native transport path, the body is consumed on first dispatch and
buffered into request._content. On replay, the buffered content is used.
Mock transports that don't read the body leave the iterator unconsumed,
which is correct behavior (the request object handles buffering).
"""

import pytest
from eggfetch.compat.httpx import (
    Auth,
    AsyncClient,
    Client,
    MockTransport,
    Request,
    Response,
)


class ReplayRequiredAuth(Auth):
    """Auth that always requires a second request (replaying the body)."""

    def auth_flow(self, request):
        request.headers["x-attempt"] = "first"
        response = yield request
        if response.status_code == 401:
            request.headers["x-attempt"] = "second"
            response = yield request


class TestSyncReplayBehavior:
    def test_bytes_body_replay_succeeds(self):
        """Bytes bodies are replayable and should succeed through auth."""
        def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt == "first":
                return Response(401)
            return Response(200, text="ok")

        with Client(
            auth=ReplayRequiredAuth(),
            transport=MockTransport(handler),
        ) as client:
            resp = client.post("http://testserver/", content=b"payload")
            assert resp.status_code == 200

    def test_string_body_replay_succeeds(self):
        """String bodies are converted to bytes and replayable."""
        def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt == "first":
                return Response(401)
            return Response(200, text="ok")

        with Client(
            auth=ReplayRequiredAuth(),
            transport=MockTransport(handler),
        ) as client:
            resp = client.post("http://testserver/", content="payload")
            assert resp.status_code == 200

    def test_iterator_body_buffered_on_read(self):
        """Iterator body is buffered into request.content after first read."""
        def body_gen():
            yield b"chunk1"
            yield b"chunk2"

        def handler(request):
            # Read the body (simulating real transport behavior)
            body = request.read()
            attempt = request.headers.get("x-attempt", "")
            if attempt == "first":
                return Response(401)
            return Response(200, text=f"received={body}")

        with Client(
            auth=ReplayRequiredAuth(),
            transport=MockTransport(handler),
        ) as client:
            resp = client.post("http://testserver/", content=body_gen())
            assert resp.status_code == 200
            assert "received=b" in resp.text

    def test_iterator_body_replay_uses_buffered_content(self):
        """On replay, the buffered content is used instead of the exhausted iterator."""
        read_count = [0]

        def body_gen():
            yield b"data"

        def handler(request):
            read_count[0] += 1
            body = request.read()
            if read_count[0] == 1:
                return Response(401)
            return Response(200, text=f"replay={body}")

        with Client(
            auth=ReplayRequiredAuth(),
            transport=MockTransport(handler),
        ) as client:
            resp = client.post("http://testserver/", content=body_gen())
            assert resp.status_code == 200
            assert "replay=b'data'" in resp.text
            assert read_count[0] == 2

    def test_empty_body_replay_succeeds(self):
        """Empty body is replayable."""
        def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt == "first":
                return Response(401)
            return Response(200, text="ok")

        with Client(
            auth=ReplayRequiredAuth(),
            transport=MockTransport(handler),
        ) as client:
            resp = client.post("http://testserver/", content=b"")
            assert resp.status_code == 200

    def test_no_body_request_replay_succeeds(self):
        """GET request with no body is replayable."""
        def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt == "first":
                return Response(401)
            return Response(200, text="ok")

        with Client(
            auth=ReplayRequiredAuth(),
            transport=MockTransport(handler),
        ) as client:
            resp = client.get("http://testserver/")
            assert resp.status_code == 200


class TestAsyncReplayBehavior:
    @pytest.mark.asyncio
    async def test_async_bytes_body_replay_succeeds(self):
        """Async bytes body is replayable through auth."""
        async def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt == "first":
                return Response(401)
            return Response(200, text="ok")

        async with AsyncClient(
            auth=ReplayRequiredAuth(),
            async_transport=MockTransport(handler),
        ) as client:
            resp = await client.post("http://testserver/", content=b"payload")
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_string_body_replay_succeeds(self):
        """Async string body is converted and replayable."""
        async def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt == "first":
                return Response(401)
            return Response(200, text="ok")

        async with AsyncClient(
            auth=ReplayRequiredAuth(),
            async_transport=MockTransport(handler),
        ) as client:
            resp = await client.post("http://testserver/", content="payload")
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_empty_body_replay_succeeds(self):
        """Async empty body is replayable."""
        async def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt == "first":
                return Response(401)
            return Response(200, text="ok")

        async with AsyncClient(
            auth=ReplayRequiredAuth(),
            async_transport=MockTransport(handler),
        ) as client:
            resp = await client.post("http://testserver/", content=b"")
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_no_body_request_replay_succeeds(self):
        """Async GET request with no body is replayable."""
        async def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt == "first":
                return Response(401)
            return Response(200, text="ok")

        async with AsyncClient(
            auth=ReplayRequiredAuth(),
            async_transport=MockTransport(handler),
        ) as client:
            resp = await client.get("http://testserver/")
            assert resp.status_code == 200
