"""
CrewAI integration for TrueFlow Gateway.

Provides a factory function that returns a CrewAI-native ``LLM`` object
pre-configured to route through the TrueFlow gateway.

Usage:
    from trueflow import TrueFlowClient
    from trueflow.integrations import crewai_llm

    client = TrueFlowClient(api_key="tf_v1_...")

    # Create a CrewAI-compatible LLM
    llm = crewai_llm(client, model="gpt-4o")

    # Use with CrewAI agents
    from crewai import Agent
    researcher = Agent(
        role="Researcher",
        goal="Find relevant information",
        llm=llm,
    )
"""

from __future__ import annotations
from typing import Optional, Any, TYPE_CHECKING

if TYPE_CHECKING:
    from trueflow.client import TrueFlowClient


def crewai_llm(
    client: "TrueFlowClient",
    model: str = "gpt-4o",
    *,
    temperature: Optional[float] = None,
    max_tokens: Optional[int] = None,
    **kwargs: Any,
):
    """
    Create a CrewAI LLM instance routed through TrueFlow.

    CrewAI uses LiteLLM under the hood, which supports OpenAI-compatible
    endpoints via the ``base_url`` parameter. This function creates a
    CrewAI ``LLM`` object that routes all requests through the TrueFlow
    gateway.

    Args:
        client:      An initialized TrueFlowClient instance.
        model:       Model name (e.g. "gpt-4o", "gpt-4o-mini").
                     Use "openai/<model>" format for explicit provider prefix.
        temperature: Sampling temperature.
        max_tokens:  Maximum tokens in the response.
        **kwargs:    Passed through to CrewAI LLM constructor.

    Returns:
        A ``crewai.LLM`` instance.

    Raises:
        ImportError: If crewai is not installed.

    Example::

        from trueflow import TrueFlowClient
        from trueflow.integrations import crewai_llm
        from crewai import Agent, Task, Crew

        client = TrueFlowClient(api_key="tf_v1_...")

        llm = crewai_llm(client, model="gpt-4o", temperature=0.7)

        researcher = Agent(
            role="Senior Researcher",
            goal="Uncover key findings on the topic",
            backstory="You are an expert researcher.",
            llm=llm,
            verbose=True,
        )

        task = Task(
            description="Research the latest advances in AI security.",
            expected_output="A comprehensive summary report.",
            agent=researcher,
        )

        crew = Crew(agents=[researcher], tasks=[task], verbose=True)
        result = crew.kickoff()
    """
    try:
        from crewai import LLM
    except ImportError:
        raise ImportError(
            "CrewAI integration requires the 'crewai' package.\n"
            "Install it with: pip install trueflow[crewai]\n"
            "Or standalone:   pip install crewai"
        ) from None

    # CrewAI's LLM class uses LiteLLM under the hood.
    # For OpenAI-compatible endpoints, prefix the model with "openai/"
    # to ensure LiteLLM routes through the OpenAI provider path.
    if "/" not in model:
        model = f"openai/{model}"

    init_kwargs: dict[str, Any] = {
        "model": model,
        "base_url": client.gateway_url,
        "api_key": client.api_key,
        **kwargs,
    }
    if temperature is not None:
        init_kwargs["temperature"] = temperature
    if max_tokens is not None:
        init_kwargs["max_tokens"] = max_tokens

    return LLM(**init_kwargs)
