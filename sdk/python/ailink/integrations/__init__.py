"""
TrueFlow Framework Integrations.

Provides zero-config factory functions for popular AI frameworks:
- LangChain: langchain_chat(), langchain_embeddings(), TrueFlowCallbackHandler
- CrewAI:    crewai_llm()
- LlamaIndex: llamaindex_llm()

Each function returns a framework-native LLM object pre-configured
to route all requests through the TrueFlow gateway with full policy
enforcement, audit logging, spend tracking, and guardrails.
"""

from .langchain import langchain_chat, langchain_embeddings, TrueFlowCallbackHandler
from .crewai import crewai_llm
from .llamaindex import llamaindex_llm

__all__ = [
    "langchain_chat",
    "langchain_embeddings",
    "TrueFlowCallbackHandler",
    "crewai_llm",
    "llamaindex_llm",
]
