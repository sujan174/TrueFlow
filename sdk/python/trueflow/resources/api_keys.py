from typing import List, Optional, Dict, Any
from ..types import Response
from ..exceptions import raise_for_status

class ApiKeysResource:
    def __init__(self, client):
        self._client = client

    def create(
        self,
        name: str,
        role: str,
        scopes: List[str],
        key_prefix: Optional[str] = None,
        org_id: Optional[str] = None,
        user_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Create a new API Key.
        
        Args:
            name: Human-readable name for the key.
            role: Role (admin, member, readonly).
            scopes: List of permission scopes.
            key_prefix: Optional prefix for the key (default: "ak_live").
            org_id: Organization ID (if creating for a specific org as superadmin).
            user_id: User ID (optional).
            
        Returns:
            Dict containing the new key (secret is only returned once).
        """
        payload = {
            "name": name,
            "role": role,
            "scopes": scopes,
        }
        if key_prefix:
            payload["key_prefix"] = key_prefix
        if org_id:
            payload["org_id"] = org_id
        if user_id:
            payload["user_id"] = user_id

        resp = self._client._http.post("/api/v1/auth/keys", json=payload)
        raise_for_status(resp)
        return resp.json()

    def list(self, limit: int = 50, offset: int = 0) -> List[Dict[str, Any]]:
        """List API Keys."""
        params = {"limit": limit, "offset": offset}
        resp = self._client._http.get("/api/v1/auth/keys", params=params)
        raise_for_status(resp)
        return resp.json()

    def revoke(self, key_id: str) -> Dict[str, Any]:
        """Revoke an API Key."""
        resp = self._client._http.delete(f"/api/v1/auth/keys/{key_id}")
        raise_for_status(resp)
        return resp.json()

    def update(
        self,
        key_id: str,
        name: Optional[str] = None,
        scopes: Optional[List[str]] = None,
    ) -> Dict[str, Any]:
        """
        Update an API Key's name and/or scopes.

        Args:
            key_id: The ID of the API key to update.
            name: Optional new name for the key.
            scopes: Optional new list of permission scopes.

        Returns:
            Dict containing the updated key details.
        """
        payload = {}
        if name is not None:
            payload["name"] = name
        if scopes is not None:
            payload["scopes"] = scopes

        resp = self._client._http.put(f"/api/v1/auth/keys/{key_id}", json=payload)
        raise_for_status(resp)
        return resp.json()

    def whoami(self) -> Dict[str, Any]:
        """Get information about the current authentication context."""
        resp = self._client._http.get("/api/v1/auth/whoami")
        raise_for_status(resp)
        return resp.json()


class AsyncApiKeysResource:
    def __init__(self, client):
        self._client = client

    async def create(
        self,
        name: str,
        role: str,
        scopes: List[str],
        key_prefix: Optional[str] = None,
        org_id: Optional[str] = None,
        user_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        payload = {
            "name": name,
            "role": role,
            "scopes": scopes,
        }
        if key_prefix:
            payload["key_prefix"] = key_prefix
        if org_id:
            payload["org_id"] = org_id
        if user_id:
            payload["user_id"] = user_id

        response = await self._client._http.post("/api/v1/auth/keys", json=payload)
        raise_for_status(response)
        return response.json()

    async def list(self, limit: int = 50, offset: int = 0) -> List[Dict[str, Any]]:
        params = {"limit": limit, "offset": offset}
        response = await self._client._http.get("/api/v1/auth/keys", params=params)
        raise_for_status(response)
        return response.json()

    async def revoke(self, key_id: str) -> Dict[str, Any]:
        response = await self._client._http.delete(f"/api/v1/auth/keys/{key_id}")
        raise_for_status(response)
        return response.json()

    async def update(
        self,
        key_id: str,
        name: Optional[str] = None,
        scopes: Optional[List[str]] = None,
    ) -> Dict[str, Any]:
        """
        Update an API Key's name and/or scopes.

        Args:
            key_id: The ID of the API key to update.
            name: Optional new name for the key.
            scopes: Optional new list of permission scopes.

        Returns:
            Dict containing the updated key details.
        """
        payload = {}
        if name is not None:
            payload["name"] = name
        if scopes is not None:
            payload["scopes"] = scopes

        response = await self._client._http.put(f"/api/v1/auth/keys/{key_id}", json=payload)
        raise_for_status(response)
        return response.json()

    async def whoami(self) -> Dict[str, Any]:
        response = await self._client._http.get("/api/v1/auth/whoami")
        raise_for_status(response)
        return response.json()
