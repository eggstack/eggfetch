"""Category: sync SDK/client behavior — exercises Client sync dispatch."""

import sys

sys.path.insert(0, "crates/eggfetch-python/python")

from eggfetch.compat.httpx import Client


def test_sync_get(http_server):
    with Client() as c:
        r = c.get(f"{http_server}/get")
        assert r.status_code == 200
        data = r.json()
        assert data["method"] == "GET"


def test_sync_post_json(http_server):
    with Client() as c:
        r = c.post(f"{http_server}/post", json={"key": "value"})
        assert r.status_code == 200
        data = r.json()
        assert data["data"] == {"key": "value"}


def test_sync_request_method(http_server):
    with Client() as c:
        r = c.request("GET", f"{http_server}/get")
        assert r.status_code == 200


def test_sync_put(http_server):
    with Client() as c:
        r = c.put(f"{http_server}/put", json={"updated": True})
        assert r.status_code == 200
        data = r.json()
        assert data["data"] == {"updated": True}


def test_sync_delete(http_server):
    with Client() as c:
        r = c.delete(f"{http_server}/delete")
        assert r.status_code == 200
