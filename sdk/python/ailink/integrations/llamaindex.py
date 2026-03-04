"""
LlamaIndex integration for TrueFlow Gateway.

Provides a factory function that returns a LlamaIndex-native LLM object
pre-configured to route through the TrueFlow gateway.

Usage:
    from trueflow import TrueFlowClient
    from trueflow.integrations import llamaindex_llm

    client = TrueFlowClient(api_key="tf_v1_...")

    # Create a LlamaIndex-compatible LLM
    llm = llamaindex_llm(client, model="gpt-4o")

    # Use with LlamaIndex
    response = llm.complete("What is TrueFlow?")

    # Use with LlamaIndex query engine
    from llama_index.core import Settings
    Settings.llm = llm
"""

from __future__ import annotations
from typing import Optional, Any, TYPE_CHECKING

if TYPE_CHECKING:
    from trueflow.client import TrueFlowClient


def llamaindex_llm(
    client: "TrueFlowClient",
    model: str = "gpt-4o",
    *,
    temperature: Optional[float] = None,
    max_tokens: Optional[int] = None,
    is_chat_model: bool = True,
    is_function_calling_model: bool = True,
    context_window: int = 128000,
    **kwargs: Any,
):
    """
    Create a LlamaIndex OpenAILike LLM instance routed through TrueFlow.

    Uses ``llama_index.llms.openai_like.OpenAILike`` which is designed for
    OpenAI-compatible third-party endpoints. This routes all LLM calls
    through the TrueFlow gateway.

    Args:
        client:         An initialized TrueFlowClient instance.
        model:          Model name (e.g. "gpt-4o", "gpt-4o-mini").
        temperature:    Sampling temperature.
        max_tokens:     Maximum tokens in the response.
        is_chat_model:  Whether the model supports chat API (default: True).
        is_function_calling_model: Whether the model supports tool/function
                        calling (default: True).
        context_window: Maximum context window size (default: 128000 for GPT-4o).
        **kwargs:       Passed through to OpenAILike constructor.

    Returns:
        A ``llama_index.llms.openai_like.OpenAILike`` instance.

    Raises:
        ImportError: If llama-index-llms-openai-like is not installed.

    Example::

        from trueflow import TrueFlowClient
        from trueflow.integrations import llamaindex_llm
        from llama_index.core import VectorStoreIndex, SimpleDirectoryReader, Settings

        client = TrueFlowClient(api_key="tf_v1_...")

        # Set as the global LLM
        Settings.llm = llamaindex_llm(client, model="gpt-4o", temperature=0)

        # Build and query an index
        documents = SimpleDirectoryReader("data").load_data()
        index = VectorStoreIndex.from_documents(documents)
        query_engine = index.as_query_engine()
        response = query_engine.query("What is the main topic?")
    """
    try:
        from llama_index.llms.openai_like import OpenAILike
    except ImportError:
        raise ImportError(
            "LlamaIndex integration requires 'llama-index-llms-openai-like'.\n"
            "Install it with: pip install trueflow[llamaindex]\n"
            "Or standalone:   pip install llama-index-llms-openai-like"
        ) from None

    additional_kwargs = kwargs.pop("additional_kwargs", {})
    if client._agent_name:
        additional_kwargs.setdefault("headers", {})
        additional_kwargs["headers"]["X-TrueFlow-Agent-Name"] = client._agent_name

    init_kwargs: dict[str, Any] = {
        "model": model,
        "api_base": client.gateway_url,
        "api_key": client.api_key,
        "is_chat_model": is_chat_model,
        "is_function_calling_model": is_function_calling_model,
        "context_window": context_window,
        "additional_kwargs": additional_kwargs,
        **kwargs,
    }
    if temperature is not None:
        init_kwargs["temperature"] = temperature
    if max_tokens is not None:
        init_kwargs["max_tokens"] = max_tokens

    return OpenAILike(**init_kwargs)
