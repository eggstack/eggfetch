import eggfetch
from collections.abc import AsyncIterator
import ssl
from typing import cast


def sync_use() -> bytes:
    client = eggfetch.Client()
    response: eggfetch.Response = client.get("http://127.0.0.1/")
    return response.content


async def async_use() -> None:
    async def body() -> AsyncIterator[bytes]:
        yield b"chunk"

    async with eggfetch.AsyncClient() as client:
        response: eggfetch.Response = await client.post(
            "http://127.0.0.1/", content=body()
        )
        assert response.status_code >= 0


def corrected_sync_contracts() -> None:
    context = ssl.create_default_context()
    der_certs: list[bytes] = []
    client = eggfetch.Client(
        verify=der_certs,
        http1=True,
        limits=eggfetch.Limits(max_connections=2),
    )
    closed: bool = client.is_closed
    cookies: eggfetch.Cookies = client.cookies
    assert not closed and isinstance(cookies, eggfetch.Cookies)

    stream = cast(eggfetch.NetworkStream, object())
    upgraded: bool = stream.is_upgraded
    stream2: eggfetch.NetworkStream = stream.start_tls(context, "example.com")
    assert isinstance(upgraded, bool) and isinstance(stream2, eggfetch.NetworkStream)
    entered: eggfetch.Client
    with client as entered:
        assert isinstance(entered.cookies, eggfetch.Cookies)
    exit_value: bool = client.__exit__(None, None, None)
    assert exit_value is False


async def corrected_async_contracts() -> None:
    client = eggfetch.AsyncClient(
        verify=[b"der-certificate"],
        http2=True,
        trust_env=False,
    )
    closed: bool = client.is_closed
    cookies: eggfetch.Cookies = client.cookies
    assert not closed and isinstance(cookies, eggfetch.Cookies)

    stream = cast(eggfetch.AsyncNetworkStream, object())
    upgraded: bool = stream.is_upgraded
    stream2: eggfetch.AsyncNetworkStream = await stream.start_tls(
        ssl_context=None, server_hostname="example.com"
    )
    assert isinstance(upgraded, bool) and isinstance(stream2, eggfetch.AsyncNetworkStream)
    entered: eggfetch.AsyncClient
    async with client as entered:
        assert isinstance(entered.cookies, eggfetch.Cookies)
    exit_value: bool = await client.__aexit__(None, None, None)
    assert exit_value is False


def streaming_context_contracts() -> None:
    response = cast(eggfetch.StreamingResponse, object())
    entered: eggfetch.StreamingResponse
    with response as entered:
        assert isinstance(entered, eggfetch.StreamingResponse)
    exit_value: bool = response.__exit__(None, None, None)
    assert exit_value is False


async def async_streaming_context_contracts() -> None:
    response = cast(eggfetch.StreamingResponse, object())
    entered: eggfetch.StreamingResponse
    async with response as entered:
        assert isinstance(entered, eggfetch.StreamingResponse)
    exit_value: bool = await response.__aexit__(None, None, None)
    assert exit_value is False
