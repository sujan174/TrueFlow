# Framework Integration Cookbook

TrueFlow integrates natively with popular AI frameworks. All requests are routed through the TrueFlow gateway, giving you policy enforcement, audit logging, spend tracking, and guardrails — with zero code changes to your framework logic.

## Installation

```bash
# Install with specific framework support
pip install trueflow[langchain]
pip install trueflow[crewai]
pip install trueflow[llamaindex]

# Install all frameworks at once
pip install trueflow[frameworks]
```

---

## LangChain

### Basic Chat

```python
from trueflow import TrueFlowClient
from trueflow.integrations import langchain_chat

client = TrueFlowClient(api_key="tf_v1_...")
llm = langchain_chat(client, model="gpt-4o")

# Works with any LangChain chain
from langchain_core.prompts import ChatPromptTemplate

prompt = ChatPromptTemplate.from_messages([
    ("system", "You are a helpful assistant."),
    ("user", "{input}"),
])
chain = prompt | llm
response = chain.invoke({"input": "What is 2 + 2?"})
print(response.content)
```

### Streaming

```python
llm = langchain_chat(client, model="gpt-4o", streaming=True)

for chunk in llm.stream("Tell me a joke"):
    print(chunk.content, end="", flush=True)
```

### With Agents and Tools

```python
from langchain_core.tools import tool
from langchain.agents import create_openai_tools_agent, AgentExecutor
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder

@tool
def get_weather(city: str) -> str:
    """Get current weather for a city."""
    return f"It's 72°F and sunny in {city}."

llm = langchain_chat(client, model="gpt-4o")

prompt = ChatPromptTemplate.from_messages([
    ("system", "You are a helpful assistant with access to tools."),
    ("user", "{input}"),
    MessagesPlaceholder("agent_scratchpad"),
])

agent = create_openai_tools_agent(llm, [get_weather], prompt)
executor = AgentExecutor(agent=agent, tools=[get_weather])
result = executor.invoke({"input": "What's the weather in London?"})
print(result["output"])
```

### Embeddings (for RAG)

```python
from trueflow.integrations import langchain_embeddings

embeddings = langchain_embeddings(client, model="text-embedding-3-small")
vectors = embeddings.embed_documents(["Hello world", "Goodbye world"])
```

### With Session Tracing

```python
# Track all LangChain requests under one session for cost attribution
with client.trace(session_id="langchain-agent-run-42") as traced:
    llm = langchain_chat(client, model="gpt-4o")
    # Every call from this LLM is tracked under session "langchain-agent-run-42"
    response = llm.invoke("Summarize the quarterly report")
```

---

## CrewAI

### Basic Agent Setup

```python
from trueflow import TrueFlowClient
from trueflow.integrations import crewai_llm
from crewai import Agent, Task, Crew

client = TrueFlowClient(api_key="tf_v1_...")

# Create an TrueFlow-routed LLM
llm = crewai_llm(client, model="gpt-4o", temperature=0.7)

# Define agents — all requests go through TrueFlow
researcher = Agent(
    role="Senior Researcher",
    goal="Find the latest developments in AI security",
    backstory="You are an expert security researcher with 10 years of experience.",
    llm=llm,
    verbose=True,
)

writer = Agent(
    role="Technical Writer",
    goal="Write clear, comprehensive reports",
    backstory="You specialize in translating complex technical topics.",
    llm=llm,
    verbose=True,
)

# Define tasks
research_task = Task(
    description="Research the top 5 AI security threats in 2025.",
    expected_output="A detailed report with threat descriptions and mitigations.",
    agent=researcher,
)

writing_task = Task(
    description="Write an executive summary based on the research.",
    expected_output="A 500-word executive summary suitable for a CISO.",
    agent=writer,
)

# Run the crew — all LLM calls enforce TrueFlow policies
crew = Crew(
    agents=[researcher, writer],
    tasks=[research_task, writing_task],
    verbose=True,
)
result = crew.kickoff()
print(result)
```

### Using Different Models per Agent

```python
# Give the researcher a more capable (expensive) model
researcher_llm = crewai_llm(client, model="gpt-4o", temperature=0.3)

# Give the writer a cheaper model for cost control
writer_llm = crewai_llm(client, model="gpt-4o-mini", temperature=0.7)

researcher = Agent(role="Researcher", goal="...", llm=researcher_llm, ...)
writer = Agent(role="Writer", goal="...", llm=writer_llm, ...)

# TrueFlow spend caps enforce budget regardless of which model each agent uses
```

---

## LlamaIndex

### Basic RAG Pipeline

```python
from trueflow import TrueFlowClient
from trueflow.integrations import llamaindex_llm
from llama_index.core import VectorStoreIndex, SimpleDirectoryReader, Settings

client = TrueFlowClient(api_key="tf_v1_...")

# Set TrueFlow as the global LLM for all LlamaIndex operations
Settings.llm = llamaindex_llm(client, model="gpt-4o", temperature=0)

# Build an index and query it
documents = SimpleDirectoryReader("./data").load_data()
index = VectorStoreIndex.from_documents(documents)

query_engine = index.as_query_engine()
response = query_engine.query("What are the key findings?")
print(response)
```

### Direct Completion

```python
llm = llamaindex_llm(client, model="gpt-4o")

# Simple completion
response = llm.complete("Explain TrueFlow in one sentence.")
print(response.text)

# Chat
from llama_index.core.llms import ChatMessage
messages = [
    ChatMessage(role="system", content="You are a helpful assistant."),
    ChatMessage(role="user", content="What is TrueFlow?"),
]
response = llm.chat(messages)
print(response.message.content)
```

### Streaming

```python
llm = llamaindex_llm(client, model="gpt-4o")

for chunk in llm.stream_complete("Tell me about AI gateways"):
    print(chunk.delta, end="", flush=True)
```

---

## Advanced: Without the Integration Module

If you prefer not to use the integration helpers, you can configure any framework
directly using the TrueFlow gateway URL and your virtual token:

### LangChain (manual)

```python
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    model="gpt-4o",
    base_url="http://localhost:8443",
    api_key="tf_v1_...",
)
```

### CrewAI (manual)

```python
from crewai import LLM

llm = LLM(
    model="openai/gpt-4o",
    base_url="http://localhost:8443",
    api_key="tf_v1_...",
)
```

### LlamaIndex (manual)

```python
from llama_index.llms.openai_like import OpenAILike

llm = OpenAILike(
    model="gpt-4o",
    api_base="http://localhost:8443",
    api_key="tf_v1_...",
    is_chat_model=True,
)
```

### OpenAI SDK (already built-in)

```python
from trueflow import TrueFlowClient

client = TrueFlowClient(api_key="tf_v1_...")
oai = client.openai()  # Returns a configured openai.Client

response = oai.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello!"}],
)
```

---

## Environment Variables

All examples work with environment variables to avoid hardcoding keys:

```bash
export TRUEFLOW_API_KEY="tf_v1_..."
export TRUEFLOW_GATEWAY_URL="http://localhost:8443"
```

```python
# No api_key needed — reads from TRUEFLOW_API_KEY
client = TrueFlowClient()
llm = langchain_chat(client, model="gpt-4o")
```

---

## Troubleshooting

### "AuthenticationError: Missing or invalid token"
Ensure `TRUEFLOW_API_KEY` is set in your environment or passed to the `TrueFlowClient`. The key must start with `tf_v1_`.

### "PolicyDeniedError: Access forbidden"
A TrueFlow policy has blocked the request. Open the TrueFlow Dashboard and check the **Audit Logs** to see exactly which policy rule triggered the block.

### "Model not found" or "Invalid model name"
If you are using manual configuration without the TrueFlow integration helpers, ensure you are passing the model correctly. Some frameworks prefix model names (e.g., CrewAI uses `openai/gpt-4o` internally) but TrueFlow understands native model names (e.g., `gpt-4o`). The `trueflow.integrations` helpers handle this mapping automatically.
