"""This fixture must fail for unsupported inputs and return contracts."""

import ssl
from typing import cast

import eggfetch


def invalid_verify_and_tls_result() -> None:
    # The runtime accepts a concrete list of DER bytes, not arbitrary
    # sequences such as tuples.
    eggfetch.Client(verify=(b"not-a-list",))

    stream = cast(eggfetch.NetworkStream, object())
    result: None = stream.start_tls(ssl.create_default_context(), "example.com")
    assert result is None
