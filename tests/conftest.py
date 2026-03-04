import os
import time
import pytest
import requests
from trueflow import TrueFlowClient, AsyncClient

@pytest.fixture(autouse=True)
def _cleanup_clients(monkeypatch):
    created_clients = []
    original_init = TrueFlowClient.__init__
    
    def new_init(self, *args, **kwargs):
        original_init(self, *args, **kwargs)
        created_clients.append(self)
        
    monkeypatch.setattr(TrueFlowClient, "__init__", new_init)
    yield
    for client in created_clients:
        try:
            client.close()
        except Exception:
            pass
 
# ── Global Config ─────────────────────────────────────────────────────────────
GATEWAY_URL = os.getenv("GATEWAY_URL", "http://127.0.0.1:8443")
ADMIN_KEY = os.getenv("ADMIN_KEY", "trueflow-admin-test")
MOCK_UPSTREAM_URL = os.getenv("MOCK_UPSTREAM_URL", "http://mock-upstream:80")
PROJECT_ID = "00000000-0000-0000-0000-000000000001"


@pytest.fixture(scope="session")
def gateway_url():
    """Return the base URL of the running gateway."""
    return GATEWAY_URL


@pytest.fixture(scope="session")
def admin_key():
    """Return the admin authentication key."""
    return ADMIN_KEY


@pytest.fixture(scope="session")
def mock_upstream_url():
    """Return the URL of the mock upstream service (httpbin)."""
    return MOCK_UPSTREAM_URL


@pytest.fixture(scope="session")
def project_id():
    """Return the default project ID."""
    return PROJECT_ID


@pytest.fixture(scope="session")
def gateway_up(gateway_url):
    """
    Block until the gateway is reachable at /healthz.
    Returns False if gateway is not reachable (allows unit tests to still run).
    """
    for _ in range(15):
        try:
            r = requests.get(f"{gateway_url}/healthz", timeout=2)
            if r.status_code == 200:
                return True
        except requests.ConnectionError:
            pass
        time.sleep(1)

    return False


@pytest.fixture(scope="session")
def admin_client(gateway_up, gateway_url, admin_key):
    """
    Return a configured TrueFlowClient with admin privileges.
    Uses the TrueFlowClient.admin() factory — the intended way to create admin clients.
    """
    if not gateway_up:
        pytest.skip(f"Gateway not reachable at {gateway_url}")
    return TrueFlowClient.admin(admin_key=admin_key, gateway_url=gateway_url)


@pytest.fixture(scope="session")
def async_admin_client(gateway_up, gateway_url, admin_key):
    """
    Return a configured AsyncClient with admin privileges.
    Skip if gateway is not up.
    """
    if not gateway_up:
        pytest.skip(f"Gateway not reachable at {gateway_url}")
    return AsyncClient(api_key=admin_key, gateway_url=gateway_url)
