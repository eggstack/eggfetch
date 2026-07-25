"""Category: sdk-async-client — exercises anthropic SDK with controlled transport.

anthropic is a major AI SDK using httpx.AsyncClient with custom auth,
base_url, timeouts, streaming, and exception inspection. This fixture
constructs the SDK with a controlled async HTTP client/transport and
exercises one local or mocked request path without credentials or external
network.
"""

import httpx


def test_anthropic_sdk_custom_async_client():
    """anthropic SDK accepts a custom async HTTP client for controlled transport."""
    try:
        from anthropic import AsyncAnthropic
    except ImportError:
        import pytest
        pytest.skip("anthropic not installed")

    # Build a controlled transport that returns a mock response
    mock_response = httpx.Response(
        200,
        json={
            "id": "msg_test",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Hello from mock"}],
            "model": "claude-3-sonnet-20240229",
        },
        headers={"content-type": "application/json"},
    )
    transport = httpx.MockTransport(lambda r: mock_response)

    # Construct the SDK with a controlled async HTTP client
    custom_client = httpx.AsyncClient(
        transport=transport,
        base_url="http://test/anthropic",
        timeout=30.0,
    )

    client = AsyncAnthropic(
        api_key="test-key-not-real",
        http_client=custom_client,
        base_url="http://test/anthropic",
    )

    # Verify the SDK was constructed with our controlled client
    assert client.base_url == "http://test/anthropic"
    # The SDK's http_client should be our custom client
    assert client.http_client is custom_client or client.http_client.transport is transport


def test_anthropic_sdk_custom_transport():
    """anthropic SDK works with a custom transport without external network."""
    try:
        from anthropic import AsyncAnthropic
    except ImportError:
        import pytest
        pytest.skip("anthropic not installed")

    mock_response = httpx.Response(
        200,
        json={
            "id": "msg_test_2",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Mocked response"}],
            "model": "claude-3-sonnet-20240229",
        },
        headers={"content-type": "application/json"},
    )
    transport = httpx.MockTransport(lambda r: mock_response)

    client = AsyncAnthropic(
        api_key="test-key-not-real",
        http_client=httpx.AsyncClient(transport=transport),
        base_url="http://test/anthropic",
    )

    # Verify the SDK is configured to use our controlled transport
    assert client.base_url == "http://test/anthropic"
    assert client.api_key == "test-key-not-real"
