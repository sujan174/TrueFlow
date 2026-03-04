"""
LangChain integration for TrueFlow Gateway.

Provides factory functions that return LangChain-native LLM objects
pre-configured to route through the TrueFlow gateway.

Usage:
    from trueflow import TrueFlowClient
    from trueflow.integrations import langchain_chat

    client = TrueFlowClient(api_key="tf_v1_...")

    # Drop-in replacement for ChatOpenAI
    llm = langchain_chat(client, model="gpt-4o")

    # Use with any LangChain chain, agent, or tool
    from langchain_core.messages import HumanMessage
    response = llm.invoke([HumanMessage(content="Hello")])

    # Works with LangChain agents
    from langchain.agents import create_openai_tools_agent
    agent = create_openai_tools_agent(llm, tools, prompt)
"""

from __future__ import annotations
from typing import Optional, Any, TYPE_CHECKING

if TYPE_CHECKING:
    from trueflow.client import TrueFlowClient


def langchain_chat(
    client: "TrueFlowClient",
    model: str = "gpt-4o",
    *,
    temperature: Optional[float] = None,
    max_tokens: Optional[int] = None,
    streaming: bool = False,
    default_headers: Optional[dict[str, str]] = None,
    **kwargs: Any,
):
    """
    Create a LangChain ChatOpenAI instance routed through TrueFlow.

    This is a drop-in replacement for ChatOpenAI that routes all requests
    through the TrueFlow gateway, giving you:
    - Policy enforcement (rate limits, spend caps, content filtering)
    - Audit logging (every request logged with cost tracking)
    - Credential injection (no API keys in your code)
    - Guardrails (PII redaction, jailbreak protection)

    Args:
        client:     An initialized TrueFlowClient instance.
        model:      Model name (e.g. "gpt-4o", "gpt-4o-mini").
                    Can also use TrueFlow model aliases.
        temperature: Sampling temperature (0-2).
        max_tokens:  Maximum tokens in the response.
        streaming:   Enable streaming responses.
        default_headers: Extra headers to send with every request.
        **kwargs:    Passed through to ChatOpenAI constructor.

    Returns:
        A ``langchain_openai.ChatOpenAI`` instance.

    Raises:
        ImportError: If langchain-openai is not installed.

    Example::

        from trueflow import TrueFlowClient
        from trueflow.integrations import langchain_chat

        client = TrueFlowClient(api_key="tf_v1_...")
        llm = langchain_chat(client, model="gpt-4o", temperature=0)

        # Use in a chain
        from langchain_core.prompts import ChatPromptTemplate
        prompt = ChatPromptTemplate.from_messages([
            ("system", "You are a helpful assistant."),
            ("user", "{input}"),
        ])
        chain = prompt | llm
        response = chain.invoke({"input": "What is TrueFlow?"})
    """
    try:
        from langchain_openai import ChatOpenAI
    except ImportError:
        raise ImportError(
            "LangChain integration requires the 'langchain-openai' package.\n"
            "Install it with: pip install trueflow[langchain]\n"
            "Or standalone:   pip install langchain-openai"
        ) from None

    headers = {"Authorization": f"Bearer {client.api_key}"}
    if client._agent_name:
        headers["X-TrueFlow-Agent-Name"] = client._agent_name
    if default_headers:
        headers.update(default_headers)

    init_kwargs: dict[str, Any] = {
        "model": model,
        "base_url": client.gateway_url,
        "api_key": client.api_key,
        "default_headers": headers,
        "streaming": streaming,
        **kwargs,
    }
    if temperature is not None:
        init_kwargs["temperature"] = temperature
    if max_tokens is not None:
        init_kwargs["max_tokens"] = max_tokens

    return ChatOpenAI(**init_kwargs)


def langchain_embeddings(
    client: "TrueFlowClient",
    model: str = "text-embedding-3-small",
    **kwargs: Any,
):
    """
    Create a LangChain OpenAIEmbeddings instance routed through TrueFlow.

    Args:
        client:  An initialized TrueFlowClient instance.
        model:   Embedding model name.
        **kwargs: Passed through to OpenAIEmbeddings constructor.

    Returns:
        A ``langchain_openai.OpenAIEmbeddings`` instance.

    Raises:
        ImportError: If langchain-openai is not installed.

    Example::

        from trueflow.integrations import langchain_embeddings
        embeddings = langchain_embeddings(client, model="text-embedding-3-small")

        # Use with a vector store
        vectors = embeddings.embed_documents(["Hello world", "Goodbye world"])
    """
    try:
        from langchain_openai import OpenAIEmbeddings
    except ImportError:
        raise ImportError(
            "LangChain integration requires the 'langchain-openai' package.\n"
            "Install it with: pip install trueflow[langchain]\n"
            "Or standalone:   pip install langchain-openai"
        ) from None

    headers = {"Authorization": f"Bearer {client.api_key}"}
    if client._agent_name:
        headers["X-TrueFlow-Agent-Name"] = client._agent_name

    return OpenAIEmbeddings(
        model=model,
        base_url=client.gateway_url,
        api_key=client.api_key,
        default_headers=headers,
        **kwargs,
    )


class TrueFlowCallbackHandler:
    """
    LangChain callback handler that captures chain, tool, and agent metadata
    and injects it into TrueFlow's X-Properties header for per-step observability.

    This enables:
    - Spend breakdown by chain name (``group_by=tag:chain``)
    - Spend breakdown by tool name (``group_by=tag:tool``)
    - Tracking agent step counts and tool usage in audit logs
    - Filtering audit logs by chain/tool/agent metadata

    Usage::

        from trueflow import TrueFlowClient
        from trueflow.integrations.langchain import langchain_chat, TrueFlowCallbackHandler

        client = TrueFlowClient()
        handler = TrueFlowCallbackHandler(tags={"team": "billing", "env": "prod"})

        llm = langchain_chat(client, model="gpt-4o", callbacks=[handler])

        # Every LLM call now includes chain/tool context in audit logs
        chain = prompt | llm
        chain.invoke({"input": "Hello"})

        # In the TrueFlow dashboard, you can now:
        # - See which chain triggered each LLM call
        # - Track spend per tool or chain name
        # - Filter audit logs by custom tags
    """

    def __init__(self, tags: Optional[dict[str, str]] = None):
        """
        Args:
            tags: Static key-value tags added to every request.
                  Example: {"team": "billing", "env": "prod", "feature": "invoice-gen"}
        """
        self._static_tags = tags or {}
        self._chain_stack: list[str] = []
        self._current_tool: Optional[str] = None
        self._agent_step_count: int = 0
        self._total_tool_calls: int = 0

    # ── LangChain Callback Protocol ──────────────────────────────────────

    def on_chain_start(self, serialized: dict, inputs: Any, **kwargs: Any) -> None:
        """Called when a chain starts running."""
        chain_name = serialized.get("id", ["unknown"])[-1] if serialized.get("id") else \
                     serialized.get("name", "unknown_chain")
        self._chain_stack.append(chain_name)

    def on_chain_end(self, outputs: Any, **kwargs: Any) -> None:
        """Called when a chain finishes."""
        if self._chain_stack:
            self._chain_stack.pop()

    def on_chain_error(self, error: BaseException, **kwargs: Any) -> None:
        """Called when a chain errors."""
        if self._chain_stack:
            self._chain_stack.pop()

    def on_tool_start(self, serialized: dict, input_str: str, **kwargs: Any) -> None:
        """Called when a tool starts running."""
        self._current_tool = serialized.get("name", "unknown_tool")
        self._total_tool_calls += 1

    def on_tool_end(self, output: str, **kwargs: Any) -> None:
        """Called when a tool finishes."""
        self._current_tool = None

    def on_tool_error(self, error: BaseException, **kwargs: Any) -> None:
        """Called when a tool errors."""
        self._current_tool = None

    def on_agent_action(self, action: Any, **kwargs: Any) -> None:
        """Called when an agent takes an action."""
        self._agent_step_count += 1

    def on_llm_start(self, serialized: dict, prompts: list[str], **kwargs: Any) -> None:
        """Called BEFORE each LLM request — injects metadata into headers."""
        # This is where we inject the X-Properties header
        # The invocation_params dict is passed to the LLM client
        invocation_params = kwargs.get("invocation_params", {})
        headers = invocation_params.get("headers", {})

        props = {**self._static_tags}

        # Add chain context
        if self._chain_stack:
            props["chain"] = self._chain_stack[-1]
            props["chain_depth"] = str(len(self._chain_stack))

        # Add tool context (if this LLM call is part of a tool execution)
        if self._current_tool:
            props["tool"] = self._current_tool

        # Add agent step tracking
        if self._agent_step_count > 0:
            props["agent_step"] = str(self._agent_step_count)
            props["total_tool_calls"] = str(self._total_tool_calls)

        if props:
            import json
            headers["X-Properties"] = json.dumps(props)
            invocation_params["headers"] = headers

    def on_llm_end(self, response: Any, **kwargs: Any) -> None:
        """Called when an LLM call finishes."""
        pass

    def on_llm_error(self, error: BaseException, **kwargs: Any) -> None:
        """Called when an LLM call errors."""
        pass

    # ── Properties access ────────────────────────────────────────────────

    def get_properties(self) -> dict[str, str]:
        """Get the current properties dict that would be sent on the next LLM call."""
        props = {**self._static_tags}
        if self._chain_stack:
            props["chain"] = self._chain_stack[-1]
            props["chain_depth"] = str(len(self._chain_stack))
        if self._current_tool:
            props["tool"] = self._current_tool
        if self._agent_step_count > 0:
            props["agent_step"] = str(self._agent_step_count)
            props["total_tool_calls"] = str(self._total_tool_calls)
        return props

    def reset(self) -> None:
        """Reset agent tracking counters (call between agent runs)."""
        self._chain_stack.clear()
        self._current_tool = None
        self._agent_step_count = 0
        self._total_tool_calls = 0

