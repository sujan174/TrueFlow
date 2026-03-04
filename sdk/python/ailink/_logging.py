import logging

logger = logging.getLogger("trueflow")

def log_request(method: str, url: str):
    logger.debug("TrueFlow SDK → %s %s", method, url)

def log_response(status: int, url: str, elapsed_ms: float):
    logger.debug("TrueFlow SDK ← %d %s (%.0fms)", status, url, elapsed_ms)
