"""Phase 2 Track 4: Response status and redirect state parity tests."""

import pytest

from eggfetch.compat.httpx import Response, Request, HTTPStatusError


class TestStatusPredicates:
    """4.1 Match status predicates."""

    def test_is_informational(self):
        assert Response(100).is_informational
        assert Response(101).is_informational
        assert Response(199).is_informational
        assert not Response(200).is_informational

    def test_is_success(self):
        assert Response(200).is_success
        assert Response(201).is_success
        assert Response(299).is_success
        assert not Response(300).is_success

    def test_is_redirect(self):
        assert Response(301).is_redirect
        assert Response(302).is_redirect
        assert Response(303).is_redirect
        assert Response(307).is_redirect
        assert Response(308).is_redirect
        assert not Response(200).is_redirect
        assert not Response(400).is_redirect

    def test_is_client_error(self):
        assert Response(400).is_client_error
        assert Response(404).is_client_error
        assert Response(499).is_client_error
        assert not Response(300).is_client_error
        assert not Response(500).is_client_error

    def test_is_server_error(self):
        assert Response(500).is_server_error
        assert Response(503).is_server_error
        assert Response(599).is_server_error
        assert not Response(400).is_server_error
        assert not Response(600).is_server_error

    def test_is_error(self):
        assert Response(400).is_error
        assert Response(500).is_error
        assert not Response(200).is_error
        assert not Response(300).is_error

    def test_has_redirect_location(self):
        resp = Response(
            301,
            headers={"Location": "https://example.com/new"},
        )
        assert resp.has_redirect_location

    def test_has_redirect_location_no_header(self):
        resp = Response(301)
        assert not resp.has_redirect_location

    def test_has_redirect_location_not_redirect(self):
        resp = Response(
            200,
            headers={"Location": "https://example.com/new"},
        )
        assert not resp.has_redirect_location


class TestRaiseForStatus:
    """4.2 Match raise_for_status()."""

    def test_success_returns_self(self):
        resp = Response(200, content=b"ok")
        result = resp.raise_for_status()
        assert result is resp

    def test_informational_raises(self):
        req = Request("GET", "https://example.com")
        resp = Response(100, request=req)
        with pytest.raises(HTTPStatusError) as exc_info:
            resp.raise_for_status()
        assert exc_info.value.response is resp
        assert exc_info.value.request is req

    def test_redirect_raises(self):
        req = Request("GET", "https://example.com")
        resp = Response(301, request=req, headers={"Location": "https://example.com/new"})
        with pytest.raises(HTTPStatusError) as exc_info:
            resp.raise_for_status()
        assert exc_info.value.response is resp
        assert "Location" in exc_info.value.message or "Redirect" in exc_info.value.message

    def test_4xx_raises(self):
        req = Request("GET", "https://example.com")
        resp = Response(404, request=req)
        with pytest.raises(HTTPStatusError) as exc_info:
            resp.raise_for_status()
        assert exc_info.value.response is resp
        assert exc_info.value.request is req

    def test_5xx_raises(self):
        req = Request("GET", "https://example.com")
        resp = Response(500, request=req)
        with pytest.raises(HTTPStatusError) as exc_info:
            resp.raise_for_status()
        assert exc_info.value.response is resp

    def test_raise_without_request(self):
        resp = Response(500)
        with pytest.raises(RuntimeError, match="No request"):
            resp.raise_for_status()


class TestNextRequest:
    """4.3 Add next_request."""

    def test_next_request_defaults_none(self):
        resp = Response(200, content=b"ok")
        assert resp.next_request is None

    def test_next_request_settable(self):
        resp = Response(200, content=b"ok")
        req = Request("GET", "https://example.com")
        resp.next_request = req
        assert resp.next_request is req


class TestHistoryMutability:
    """4.4 Preserve redirect history mutability semantics."""

    def test_history_copied_at_construction(self):
        h1 = Response(301)
        h2 = Response(302)
        history = [h1, h2]
        resp = Response(200, history=history)
        # Modifying the original list should not affect the response
        history.append(Response(303))
        assert len(resp.history) == 2

    def test_history_setter(self):
        resp = Response(200)
        h1 = Response(301)
        h2 = Response(302)
        resp.history = [h1, h2]
        assert len(resp.history) == 2

    def test_history_empty_default(self):
        resp = Response(200)
        assert resp.history == []
