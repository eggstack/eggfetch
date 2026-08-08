"""Track 5: Protocol validation and transport option acceptance.

Tests that protocol combinations are validated and transport options
are now accepted (previously raised NotImplementedError).
"""

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, Timeout
from eggfetch.compat.httpx._transports import HTTPTransport, AsyncHTTPTransport
from eggfetch.compat.httpx._client import (
    _validate_protocol_options,
    _validate_transport_options,
)


# ---------------------------------------------------------------------------
# Protocol validation
# ---------------------------------------------------------------------------

class TestProtocolValidation:
    def test_http1_true_http2_false_ok(self):
        client = Client(http1=True, http2=False)
        assert client._http1 is True
        assert client._http2 is False

    def test_http1_true_http2_true_ok(self):
        client = Client(http1=True, http2=True)
        assert client._http1 is True
        assert client._http2 is True

    def test_http1_false_http2_false_raises(self):
        with pytest.raises(ValueError, match="At least one of http1 or http2"):
            Client(http1=False, http2=False)

    def test_http1_false_http2_true_raises(self):
        with pytest.raises(NotImplementedError, match="H2-only"):
            Client(http1=False, http2=True)

    def test_async_client_protocol_validation(self):
        with pytest.raises(ValueError, match="At least one of http1 or http2"):
            AsyncClient(http1=False, http2=False)

    def test_async_client_h2_only_raises(self):
        with pytest.raises(NotImplementedError, match="H2-only"):
            AsyncClient(http1=False, http2=True)

    def test_validate_protocol_direct(self):
        _validate_protocol_options(True, False)  # should not raise
        _validate_protocol_options(True, True)   # should not raise

    def test_validate_protocol_both_false(self):
        with pytest.raises(ValueError):
            _validate_protocol_options(False, False)

    def test_validate_protocol_h2_only(self):
        with pytest.raises(NotImplementedError):
            _validate_protocol_options(False, True)


# ---------------------------------------------------------------------------
# Transport option acceptance (formerly rejected)
# ---------------------------------------------------------------------------

class TestTransportOptionsAccepted:
    def test_uds_accepted(self):
        """UDS is now accepted in the transport constructor."""
        transport = HTTPTransport(uds="/tmp/test.sock")
        assert transport._uds == "/tmp/test.sock"

    def test_local_address_accepted(self):
        """local_address is now accepted in the transport constructor."""
        transport = HTTPTransport(local_address="127.0.0.1:0")
        assert transport._local_address == "127.0.0.1:0"

    def test_socket_options_accepted(self):
        """socket_options is now accepted in the transport constructor."""
        opts = [(6, 1, b"\x01\x00\x00\x00")]  # TCP_NODELAY
        transport = HTTPTransport(socket_options=opts)
        assert transport._socket_options == opts

    def test_async_uds_accepted(self):
        """UDS is now accepted in the async transport constructor."""
        transport = AsyncHTTPTransport(uds="/tmp/test.sock")
        assert transport._uds == "/tmp/test.sock"

    def test_async_local_address_accepted(self):
        """local_address is now accepted in the async transport constructor."""
        transport = AsyncHTTPTransport(local_address="127.0.0.1:0")
        assert transport._local_address == "127.0.0.1:0"

    def test_async_socket_options_accepted(self):
        """socket_options is now accepted in the async transport constructor."""
        opts = [(6, 1, b"\x01\x00\x00\x00")]  # TCP_NODELAY
        transport = AsyncHTTPTransport(socket_options=opts)
        assert transport._socket_options == opts

    def test_default_none_values_accepted(self):
        """Default None values should be accepted for signature compatibility."""
        transport = HTTPTransport(
            uds=None,
            local_address=None,
            socket_options=None,
        )
        assert transport._uds is None
        assert transport._local_address is None
        assert transport._socket_options is None

    def test_async_default_none_values_accepted(self):
        """Default None values should be accepted for signature compatibility."""
        transport = AsyncHTTPTransport(
            uds=None,
            local_address=None,
            socket_options=None,
        )
        assert transport._uds is None
        assert transport._local_address is None
        assert transport._socket_options is None

    def test_validate_transport_options_direct(self):
        _validate_transport_options()  # should not raise
        _validate_transport_options(uds=None, local_address=None,
                                   socket_options=None)  # should not raise

    def test_validate_transport_options_uds(self):
        _validate_transport_options(uds="/tmp/test.sock")  # should not raise

    def test_validate_transport_options_local_address(self):
        _validate_transport_options(local_address="127.0.0.1:0")  # should not raise

    def test_validate_transport_options_socket_options(self):
        _validate_transport_options(socket_options=[(6, 1, b"\x01")])  # should not raise

    def test_validate_transport_options_invalid_local_address(self):
        with pytest.raises(ValueError, match="host:port"):
            _validate_transport_options(local_address="bad-format")

    def test_validate_transport_options_invalid_socket_options_type(self):
        with pytest.raises(TypeError, match="list of tuples"):
            _validate_transport_options(socket_options="not-a-list")

    def test_validate_transport_options_invalid_socket_option_triple(self):
        with pytest.raises(ValueError, match="triples"):
            _validate_transport_options(socket_options=[(1, 2)])  # only 2 elements
